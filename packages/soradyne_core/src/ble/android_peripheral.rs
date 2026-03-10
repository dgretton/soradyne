//! Android BLE Peripheral via JNI.
//!
//! Wraps `BluetoothLeAdvertiser` + `BluetoothGattServer` through JNI.
//! The Java side (`SoradyneGattCallback`) calls native methods when:
//!   - a Central connects (`nativeOnConnected`)
//!   - a Central writes to the envelope characteristic (`nativeOnWrite`)
//!   - a Central disconnects (`nativeOnDisconnected`)
//!
//! Compiled only on Android (`cfg(target_os = "android")`).

#![cfg(target_os = "android")]

use async_trait::async_trait;
use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
use jni::sys::jsize;
use jni::JNIEnv;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration as StdDuration;
use tokio::sync::{mpsc, Mutex as TokioMutex};

use super::framing::{build_frame, FrameReassembler, BLE_CHUNK_SIZE};
use super::transport::{BleAddress, BleConnection, BlePeripheral};
use super::BleError;
use crate::ble::gatt::{envelope_char_uuid, soradyne_service_uuid};
use crate::android_init::get_jvm;

// ---------------------------------------------------------------------------
// Global GATT state (written at construction, read by JNI callbacks)
// ---------------------------------------------------------------------------

/// Sender used by `nativeOnConnected` to deliver new connections.
/// Wrapped in Mutex<Option> so it can be replaced on each invite (OnceLock
/// would reject a second set, causing startInvite to return -1 on retry).
static GATT_CONN_TX: OnceLock<StdMutex<Option<mpsc::Sender<AndroidBleConnection>>>> =
    OnceLock::new();

/// Map of device address → per-device write channel sender.
/// Uses std::sync::Mutex (not tokio) because JNI callbacks arrive on Binder
/// threads that are outside the Tokio runtime — tokio::spawn/await would panic.
static GATT_WRITE_TXS: OnceLock<Arc<StdMutex<HashMap<String, mpsc::Sender<Vec<u8>>>>>> =
    OnceLock::new();

/// The BluetoothGattCharacteristic used for notify-on-send.
/// Wrapped in Mutex<Option> so it can be updated on each invite.
static GATT_CHAR_REF: OnceLock<StdMutex<Option<GlobalRef>>> = OnceLock::new();

/// Per-device notification-sent signal channels.
/// Keyed by device address.  `send()` inserts a `SyncSender` before calling
/// `notifyCharacteristicChanged`; `nativeOnNotificationSent` removes and
/// signals it, letting `send()` know the chunk was transmitted and it is safe
/// to send the next one.
static NOTIF_SENT_TXS: OnceLock<StdMutex<HashMap<String, std::sync::mpsc::SyncSender<()>>>> =
    OnceLock::new();

fn notif_sent_txs() -> &'static StdMutex<HashMap<String, std::sync::mpsc::SyncSender<()>>> {
    NOTIF_SENT_TXS.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Global ref to `SoradyneGattCallback` class, cached on the main thread.
/// `find_class` on a Tokio worker thread uses the bootstrap class loader
/// (which can't see app classes), so we cache the ref while we're still on
/// the Android main thread inside `nativeSetContext`.
static GATT_CALLBACK_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Global ref to `SoradyneAdvertiseCallback` class, cached on the main thread.
static ADV_CALLBACK_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Cache JNI class refs for the two app-defined callback classes.
///
/// Must be called from a thread that has the application class loader active
/// (e.g. the Android main thread during `onAttachedToEngine`).
pub fn cache_classes(env: &mut JNIEnv) -> Result<(), BleError> {
    if GATT_CALLBACK_CLASS.get().is_none() {
        let cls = env
            .find_class("com/soradyne/flutter/SoradyneGattCallback")
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
        let global = env
            .new_global_ref(&cls)
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
        let _ = GATT_CALLBACK_CLASS.set(global);
    }
    if ADV_CALLBACK_CLASS.get().is_none() {
        let cls = env
            .find_class("com/soradyne/flutter/SoradyneAdvertiseCallback")
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
        let global = env
            .new_global_ref(&cls)
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
        let _ = ADV_CALLBACK_CLASS.set(global);
    }
    Ok(())
}

fn write_txs() -> &'static Arc<std::sync::Mutex<HashMap<String, mpsc::Sender<Vec<u8>>>>> {
    GATT_WRITE_TXS.get_or_init(|| Arc::new(std::sync::Mutex::new(HashMap::new())))
}

// ---------------------------------------------------------------------------
// AndroidBleConnection
// ---------------------------------------------------------------------------

/// Reassembly state for length-framed BLE messages received from writes.
struct AndroidRecvState {
    reassembler: FrameReassembler,
    rx: mpsc::Receiver<Vec<u8>>,
}

/// Represents one connected BLE central device on the Android peripheral side.
pub struct AndroidBleConnection {
    jvm: Arc<jni::JavaVM>,
    /// `BluetoothGattServer` global reference.
    gatt_server: GlobalRef,
    /// `BluetoothDevice` global reference.
    device: GlobalRef,
    /// `BluetoothGattCharacteristic` global reference (for notifications).
    char_ref: GlobalRef,
    /// Reassembly buffer + write channel, shared under one mutex.
    recv_state: TokioMutex<AndroidRecvState>,
    /// Device MAC address string ("AA:BB:CC:DD:EE:FF") for NOTIF_SENT_TXS lookup.
    addr_str: String,
    address: BleAddress,
}

#[async_trait]
impl BleConnection for AndroidBleConnection {
    /// Send `data` as length-prefixed, BLE-chunked GATT notifications.
    ///
    /// Uses `onNotificationSent` flow control so Android doesn't drop chunks.
    async fn send(&self, data: &[u8]) -> Result<(), BleError> {
        let jvm = Arc::clone(&self.jvm);
        let gatt_server = self.gatt_server.clone();
        let device = self.device.clone();
        let char_ref = self.char_ref.clone();
        let addr = self.addr_str.clone();

        let frame = build_frame(data);

        for chunk in frame.chunks(BLE_CHUNK_SIZE) {
            let chunk = chunk.to_vec();

            // Register flow-control channel BEFORE calling notifyCharacteristicChanged.
            let (notif_tx, notif_rx) = std::sync::mpsc::sync_channel::<()>(1);
            notif_sent_txs().lock().unwrap().insert(addr.clone(), notif_tx);

            let jvm2 = Arc::clone(&jvm);
            let gs = gatt_server.clone();
            let dev = device.clone();
            let chr = char_ref.clone();

            // JNI calls must happen on a thread attached to the JVM.
            tokio::task::spawn_blocking(move || -> Result<(), BleError> {
                let mut env = jvm2
                    .attach_current_thread()
                    .map_err(|e| BleError::ConnectionError(e.to_string()))?;

                let j_data = env
                    .byte_array_from_slice(&chunk)
                    .map_err(|e| BleError::ConnectionError(e.to_string()))?;
                env.call_method(
                    chr.as_obj(),
                    "setValue",
                    "([B)Z",
                    &[JValue::Object(&j_data.into())],
                )
                .map_err(|e| BleError::ConnectionError(e.to_string()))?;

                env.call_method(
                    gs.as_obj(),
                    "notifyCharacteristicChanged",
                    "(Landroid/bluetooth/BluetoothDevice;\
                      Landroid/bluetooth/BluetoothGattCharacteristic;Z)Z",
                    &[
                        JValue::Object(dev.as_obj()),
                        JValue::Object(chr.as_obj()),
                        JValue::Bool(0),
                    ],
                )
                .map_err(|e| BleError::ConnectionError(e.to_string()))?;

                Ok(())
            })
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))??;

            // Wait for onNotificationSent before sending the next chunk.
            tokio::task::spawn_blocking(move || {
                notif_rx
                    .recv_timeout(StdDuration::from_secs(5))
                    .map_err(|_| BleError::ConnectionError("notification send timeout".into()))
            })
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))??;
        }
        Ok(())
    }

    /// Receive one complete length-framed message, reassembling across write chunks.
    async fn recv(&self) -> Result<Vec<u8>, BleError> {
        let mut st = self.recv_state.lock().await;
        loop {
            if let Some(msg) = st.reassembler.try_extract() {
                return Ok(msg);
            }
            let chunk = st.rx.recv().await.ok_or(BleError::Disconnected)?;
            st.reassembler.push(&chunk);
        }
    }

    async fn disconnect(&self) -> Result<(), BleError> {
        let jvm = Arc::clone(&self.jvm);
        let gatt_server = self.gatt_server.clone();
        let device = self.device.clone();

        tokio::task::spawn_blocking(move || {
            let mut env = jvm
                .attach_current_thread()
                .map_err(|e| BleError::ConnectionError(e.to_string()))?;
            env.call_method(
                gatt_server.as_obj(),
                "cancelConnection",
                "(Landroid/bluetooth/BluetoothDevice;)V",
                &[JValue::Object(device.as_obj())],
            )
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| BleError::ConnectionError(e.to_string()))?
    }

    fn rssi(&self) -> Option<i16> {
        None
    }

    fn peer_address(&self) -> &BleAddress {
        &self.address
    }

    fn is_connected(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// AndroidBlePeripheral
// ---------------------------------------------------------------------------

/// BLE Peripheral backed by Android's `BluetoothLeAdvertiser` + `BluetoothGattServer`.
pub struct AndroidBlePeripheral {
    jvm: Arc<jni::JavaVM>,
    /// `android.content.Context` (the Flutter engine's application context).
    context: GlobalRef,
    /// Receives new connections pushed by `nativeOnConnected`.
    /// Wrapped in a Mutex so `accept` can take `&mut Receiver` through a `&self` trait method.
    conn_rx: TokioMutex<mpsc::Receiver<AndroidBleConnection>>,
}

impl AndroidBlePeripheral {
    /// Construct the peripheral.
    ///
    /// `context` must be the Android `Context` object (passed from Flutter/JNI).
    /// Internally uses the JVM captured by `JNI_OnLoad`.
    pub fn new(context_obj: GlobalRef) -> Result<Self, BleError> {
        let jvm = get_jvm().ok_or_else(|| {
            BleError::ConnectionError("JVM not initialized — JNI_OnLoad not called".into())
        })?;

        let (conn_tx, conn_rx) = mpsc::channel(8);
        // Replace sender so a second invite (same process) works cleanly.
        *GATT_CONN_TX
            .get_or_init(|| StdMutex::new(None))
            .lock()
            .unwrap() = Some(conn_tx);
        // Clear stale per-device write channels from any previous invite.
        write_txs().lock().unwrap().clear();

        Ok(Self {
            jvm,
            context: context_obj,
            conn_rx: TokioMutex::new(conn_rx),
        })
    }
}

#[async_trait]
impl BlePeripheral for AndroidBlePeripheral {
    async fn start_advertising(&self, data: Vec<u8>) -> Result<(), BleError> {
        let jvm = Arc::clone(&self.jvm);
        let context = self.context.clone();

        tokio::task::spawn_blocking(move || {
            let mut env = jvm
                .attach_current_thread()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // === Open a BluetoothGattServer ===
            // BluetoothManager manager = (BluetoothManager) context.getSystemService("bluetooth")
            let bt_service = env
                .new_string("bluetooth")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let manager = env
                .call_method(
                    context.as_obj(),
                    "getSystemService",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                    &[JValue::Object(&bt_service.into())],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // BluetoothAdapter adapter = manager.getAdapter()
            let adapter = env
                .call_method(
                    &manager,
                    "getAdapter",
                    "()Landroid/bluetooth/BluetoothAdapter;",
                    &[],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // === Set up the GATT service ===
            // BluetoothGattService service = new BluetoothGattService(serviceUuid, SERVICE_TYPE_PRIMARY)
            let service_uuid_str = env
                .new_string(soradyne_service_uuid().to_string())
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let uuid_class = env
                .find_class("java/util/UUID")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let service_uuid_obj = env
                .call_static_method(
                    &uuid_class,
                    "fromString",
                    "(Ljava/lang/String;)Ljava/util/UUID;",
                    &[JValue::Object(&service_uuid_str.into())],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            let gatt_service_class = env
                .find_class("android/bluetooth/BluetoothGattService")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let service_obj = env
                .new_object(
                    &gatt_service_class,
                    "(Ljava/util/UUID;I)V",
                    &[
                        JValue::Object(&service_uuid_obj),
                        JValue::Int(0), // SERVICE_TYPE_PRIMARY = 0
                    ],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // BluetoothGattCharacteristic characteristic = new BluetoothGattCharacteristic(
            //     charUuid, PROPERTY_WRITE_NO_RESPONSE | PROPERTY_NOTIFY, PERMISSION_WRITE)
            let char_uuid_str = env
                .new_string(envelope_char_uuid().to_string())
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let char_uuid_obj = env
                .call_static_method(
                    &uuid_class,
                    "fromString",
                    "(Ljava/lang/String;)Ljava/util/UUID;",
                    &[JValue::Object(&char_uuid_str.into())],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            let gatt_char_class = env
                .find_class("android/bluetooth/BluetoothGattCharacteristic")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            // PROPERTY_WRITE_NO_RESPONSE = 4, PROPERTY_NOTIFY = 16 → 20
            // PERMISSION_WRITE = 16
            let char_obj = env
                .new_object(
                    &gatt_char_class,
                    "(Ljava/util/UUID;II)V",
                    &[
                        JValue::Object(&char_uuid_obj),
                        JValue::Int(20), // WRITE_NO_RESPONSE | NOTIFY
                        JValue::Int(16), // PERMISSION_WRITE
                    ],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // Add CCCD descriptor (UUID 00002902-...)
            let cccd_uuid_str = env
                .new_string("00002902-0000-1000-8000-00805f9b34fb")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let cccd_uuid_obj = env
                .call_static_method(
                    &uuid_class,
                    "fromString",
                    "(Ljava/lang/String;)Ljava/util/UUID;",
                    &[JValue::Object(&cccd_uuid_str.into())],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            let desc_class = env
                .find_class("android/bluetooth/BluetoothGattDescriptor")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            // PERMISSION_READ | PERMISSION_WRITE = 17
            let desc_obj = env
                .new_object(
                    &desc_class,
                    "(Ljava/util/UUID;I)V",
                    &[JValue::Object(&cccd_uuid_obj), JValue::Int(17)],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            env.call_method(
                &char_obj,
                "addDescriptor",
                "(Landroid/bluetooth/BluetoothGattDescriptor;)Z",
                &[JValue::Object(&desc_obj)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // Store char global ref so nativeOnConnected can retrieve it.
            // Use Mutex<Option> so re-invite (same process) updates the ref.
            let char_global = env
                .new_global_ref(&char_obj)
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            *GATT_CHAR_REF
                .get_or_init(|| StdMutex::new(None))
                .lock()
                .unwrap() = Some(char_global);

            env.call_method(
                &service_obj,
                "addCharacteristic",
                "(Landroid/bluetooth/BluetoothGattCharacteristic;)Z",
                &[JValue::Object(&char_obj)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // === Open GattServer ===
            // The callback must be created BEFORE openGattServer (it needs to be
            // registered as the delegate), but the callback also needs the server
            // handle for sendResponse/notifyCharacteristicChanged calls.
            // We break this cycle by using a no-arg constructor and wiring the
            // server in via setGattServer() immediately after the server is opened.
            //
            // Use the pre-cached GlobalRef rather than find_class — this code
            // runs on a Tokio worker thread where find_class would use the bootstrap
            // class loader (invisible to app classes).
            let gatt_cb_global = GATT_CALLBACK_CLASS
                .get()
                .ok_or_else(|| BleError::AdvertisingError(
                    "SoradyneGattCallback class not cached (call nativeSetContext first)".into(),
                ))?;
            // SAFETY: GlobalRef is valid for any JVM-attached thread.
            let callback_class = unsafe { jni::objects::JClass::from_raw(gatt_cb_global.as_raw()) };

            let callback_obj = env
                .new_object(&callback_class, "()V", &[])
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            let gatt_server = env
                .call_method(
                    &manager,
                    "openGattServer",
                    "(Landroid/content/Context;\
                      Landroid/bluetooth/BluetoothGattServerCallback;)\
                      Landroid/bluetooth/BluetoothGattServer;",
                    &[
                        JValue::Object(context.as_obj()),
                        JValue::Object(&callback_obj),
                    ],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // Now give the callback its server handle.
            env.call_method(
                &callback_obj,
                "setGattServer",
                "(Landroid/bluetooth/BluetoothGattServer;)V",
                &[JValue::Object(&gatt_server)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            env.call_method(
                &gatt_server,
                "addService",
                "(Landroid/bluetooth/BluetoothGattService;)Z",
                &[JValue::Object(&service_obj)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // === Start advertising ===
            let advertiser = env
                .call_method(
                    &adapter,
                    "getBluetoothLeAdvertiser",
                    "()Landroid/bluetooth/le/BluetoothLeAdvertiser;",
                    &[],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // AdvertiseSettings — low latency, connectable
            let settings_builder_class = env
                .find_class("android/bluetooth/le/AdvertiseSettings$Builder")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let settings_builder = env
                .new_object(&settings_builder_class, "()V", &[])
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            // ADVERTISE_MODE_LOW_LATENCY = 2
            env.call_method(
                &settings_builder,
                "setAdvertiseMode",
                "(I)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
                &[JValue::Int(2)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            // setConnectable(true)
            env.call_method(
                &settings_builder,
                "setConnectable",
                "(Z)Landroid/bluetooth/le/AdvertiseSettings$Builder;",
                &[JValue::Bool(1)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let settings = env
                .call_method(
                    &settings_builder,
                    "build",
                    "()Landroid/bluetooth/le/AdvertiseSettings;",
                    &[],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // AdvertiseData — include service UUID + manufacturer bytes
            let adv_data_builder_class = env
                .find_class("android/bluetooth/le/AdvertiseData$Builder")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let adv_builder = env
                .new_object(&adv_data_builder_class, "()V", &[])
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // addServiceUuid(new ParcelUuid(serviceUuid))
            let parcel_uuid_class = env
                .find_class("android/os/ParcelUuid")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let parcel_uuid = env
                .new_object(
                    &parcel_uuid_class,
                    "(Ljava/util/UUID;)V",
                    &[JValue::Object(&service_uuid_obj)],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            env.call_method(
                &adv_builder,
                "addServiceUuid",
                "(Landroid/os/ParcelUuid;)\
                  Landroid/bluetooth/le/AdvertiseData$Builder;",
                &[JValue::Object(&parcel_uuid)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            let adv_data = env
                .call_method(
                    &adv_builder,
                    "build",
                    "()Landroid/bluetooth/le/AdvertiseData;",
                    &[],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // Build a scan response with the protocol marker bytes.
            // A 128-bit service UUID already uses 18 of the 31-byte legacy
            // advertisement payload, leaving no room for manufacturer data
            // (ADVERTISE_FAILED_DATA_TOO_LARGE). The scan response is a
            // separate 31-byte packet sent in reply to SCAN_REQ from an active
            // scanner (CoreBluetooth on macOS always does active scanning).
            // CoreBluetooth merges the scan response manufacturer data into the
            // peripheral's properties, so btleplug sees it in `manufacturer_data`.
            let scan_rsp_builder_class = env
                .find_class("android/bluetooth/le/AdvertiseData$Builder")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let scan_rsp_builder = env
                .new_object(&scan_rsp_builder_class, "()V", &[])
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            if !data.is_empty() {
                let j_data = env
                    .byte_array_from_slice(&data)
                    .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
                env.call_method(
                    &scan_rsp_builder,
                    "addManufacturerData",
                    "(I[B)Landroid/bluetooth/le/AdvertiseData$Builder;",
                    &[JValue::Int(0xFFFF), JValue::Object(&j_data.into())],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            }
            let scan_rsp = env
                .call_method(
                    &scan_rsp_builder,
                    "build",
                    "()Landroid/bluetooth/le/AdvertiseData;",
                    &[],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // Create a proper AdvertiseCallback instance.
            // SoradyneGattCallback extends BluetoothGattServerCallback, not
            // AdvertiseCallback, so we use a dedicated class here.
            // Use the pre-cached GlobalRef for the same thread-classloader reason.
            let adv_cb_global = ADV_CALLBACK_CLASS
                .get()
                .ok_or_else(|| BleError::AdvertisingError(
                    "SoradyneAdvertiseCallback class not cached".into(),
                ))?;
            // SAFETY: GlobalRef is valid for any JVM-attached thread.
            let adv_cb_class = unsafe { jni::objects::JClass::from_raw(adv_cb_global.as_raw()) };
            let adv_cb = env
                .new_object(&adv_cb_class, "()V", &[])
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // Use the 4-argument overload that accepts a scan response packet.
            env.call_method(
                &advertiser,
                "startAdvertising",
                "(Landroid/bluetooth/le/AdvertiseSettings;\
                  Landroid/bluetooth/le/AdvertiseData;\
                  Landroid/bluetooth/le/AdvertiseData;\
                  Landroid/bluetooth/le/AdvertiseCallback;)V",
                &[
                    JValue::Object(&settings),
                    JValue::Object(&adv_data),
                    JValue::Object(&scan_rsp),
                    JValue::Object(&adv_cb),
                ],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            Ok(())
        })
        .await
        .map_err(|e| BleError::AdvertisingError(e.to_string()))?
    }

    async fn stop_advertising(&self) -> Result<(), BleError> {
        // Stopping is handled in cleanup; minimal stub for now.
        Ok(())
    }

    async fn update_advertisement(&self, _data: Vec<u8>) -> Result<(), BleError> {
        // Re-start advertising with new data would require storing the advertiser ref.
        // Not needed for Phase 6.2 (data is static after invite begins).
        Ok(())
    }

    async fn accept(&self) -> Result<Box<dyn BleConnection>, BleError> {
        self.conn_rx
            .lock()
            .await
            .recv()
            .await
            .map(|c| Box::new(c) as Box<dyn BleConnection>)
            .ok_or(BleError::Disconnected)
    }
}

// ---------------------------------------------------------------------------
// JNI callbacks called from SoradyneGattCallback.java
// ---------------------------------------------------------------------------

/// Called by Java when a BLE Central connects to the GATT server.
#[no_mangle]
pub extern "system" fn Java_com_soradyne_flutter_SoradyneGattCallback_nativeOnConnected(
    mut env: JNIEnv,
    _class: JClass,
    device: JObject,
    gatt_server: JObject,
) {
    let jvm = match get_jvm() {
        Some(j) => j,
        None => return,
    };

    // Retrieve the address string for this device.
    // JString::from(jobject) must outlive the get_string borrow, so we bind it first.
    let addr_jobj = env
        .call_method(&device, "getAddress", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
        .unwrap_or_else(|_| jni::objects::JObject::null());
    let addr_jstring = JString::from(addr_jobj);
    let addr_str: String = env
        .get_string(&addr_jstring)
        .map(|s| s.into())
        .unwrap_or_default();

    // Parse the MAC address bytes (format "AA:BB:CC:DD:EE:FF")
    let addr_bytes = parse_mac(&addr_str).unwrap_or([0u8; 6]);
    let ble_addr = BleAddress::Real(addr_bytes);

    // Create a per-connection write channel
    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(64);

    // Register in the global map.
    // Use std::sync::Mutex directly — this runs on an Android Binder thread
    // outside the Tokio runtime, so tokio::spawn/await would panic.
    write_txs().lock().unwrap().insert(addr_str.clone(), write_tx);

    // Grab the characteristic global ref set during start_advertising.
    let char_ref = match GATT_CHAR_REF.get().and_then(|m| m.lock().ok()?.clone()) {
        Some(r) => r,
        None => {
            eprintln!("[soradyne] nativeOnConnected: GATT_CHAR_REF not set, dropping connection");
            return;
        }
    };

    let device_global = match env.new_global_ref(&device) {
        Ok(g) => g,
        Err(_) => return,
    };
    let server_global = match env.new_global_ref(&gatt_server) {
        Ok(g) => g,
        Err(_) => return,
    };

    let conn = AndroidBleConnection {
        jvm,
        gatt_server: server_global,
        device: device_global,
        char_ref,
        recv_state: TokioMutex::new(AndroidRecvState { reassembler: FrameReassembler::new(), rx: write_rx }),
        addr_str: addr_str.clone(),
        address: ble_addr,
    };

    if let Some(guard) = GATT_CONN_TX.get() {
        if let Some(tx) = guard.lock().unwrap().as_ref() {
            let _ = tx.try_send(conn);
        } else {
            eprintln!("[soradyne] nativeOnConnected: GATT_CONN_TX sender is None");
        }
    } else {
        eprintln!("[soradyne] nativeOnConnected: GATT_CONN_TX not initialized");
    }
}

/// Called by Java when a Central writes to the envelope characteristic.
#[no_mangle]
pub extern "system" fn Java_com_soradyne_flutter_SoradyneGattCallback_nativeOnWrite(
    mut env: JNIEnv,
    _class: JClass,
    device_key: JString,
    data: JByteArray,
) {
    let key: String = match env.get_string(&device_key) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    let len = env.get_array_length(&data).unwrap_or(0) as jsize;
    let mut buf = vec![0i8; len as usize];
    if env.get_byte_array_region(&data, 0, &mut buf).is_err() {
        return;
    }
    let bytes: Vec<u8> = buf.iter().map(|&b| b as u8).collect();

    // Use try_send (non-blocking) — Binder thread, no Tokio runtime.
    if let Ok(map) = write_txs().lock() {
        if let Some(tx) = map.get(&key) {
            let _ = tx.try_send(bytes);
        }
    }
}

/// Called by Java when a Central disconnects.
#[no_mangle]
pub extern "system" fn Java_com_soradyne_flutter_SoradyneGattCallback_nativeOnDisconnected(
    mut env: JNIEnv,
    _class: JClass,
    device_key: JString,
) {
    let key: String = match env.get_string(&device_key) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    // Direct remove — Binder thread, no Tokio runtime.
    write_txs().lock().unwrap().remove(&key);
}

/// Called by Java (`onNotificationSent`) after each GATT notification is transmitted.
/// Signals the per-device flow-control channel so `send()` can send the next chunk.
#[no_mangle]
pub extern "system" fn Java_com_soradyne_flutter_SoradyneGattCallback_nativeOnNotificationSent(
    mut env: JNIEnv,
    _class: JClass,
    device_key: JString,
    _status: jni::sys::jint,
) {
    let key: String = match env.get_string(&device_key) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    // Remove-and-signal: the sender is one-shot per chunk.
    if let Ok(mut map) = notif_sent_txs().lock() {
        if let Some(tx) = map.remove(&key) {
            let _ = tx.send(());
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let mut bytes = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(bytes)
}
