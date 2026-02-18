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
use jni::sys::{jint, jsize};
use jni::JNIEnv;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, Mutex as TokioMutex};

use super::transport::{BleAddress, BleAdvertisement, BleConnection, BlePeripheral};
use super::BleError;
use crate::ble::gatt::{envelope_char_uuid, soradyne_service_uuid};
use crate::android_init::get_jvm;

// ---------------------------------------------------------------------------
// Global GATT state (written at construction, read by JNI callbacks)
// ---------------------------------------------------------------------------

/// Sender used by `nativeOnConnected` to deliver new connections.
static GATT_CONN_TX: OnceLock<mpsc::Sender<AndroidBleConnection>> = OnceLock::new();

/// Map of device address → per-device write channel sender.
/// Protected by a Mutex because JNI callbacks arrive on arbitrary threads.
static GATT_WRITE_TXS: OnceLock<Arc<TokioMutex<HashMap<String, mpsc::Sender<Vec<u8>>>>>> =
    OnceLock::new();

/// The BluetoothGattCharacteristic used for notify-on-send.
/// Set once when the peripheral is started, read by `nativeOnConnected`.
static GATT_CHAR_REF: OnceLock<GlobalRef> = OnceLock::new();

fn write_txs() -> &'static Arc<TokioMutex<HashMap<String, mpsc::Sender<Vec<u8>>>>> {
    GATT_WRITE_TXS.get_or_init(|| Arc::new(TokioMutex::new(HashMap::new())))
}

// ---------------------------------------------------------------------------
// AndroidBleConnection
// ---------------------------------------------------------------------------

/// Represents one connected BLE central device on the Android peripheral side.
pub struct AndroidBleConnection {
    jvm: Arc<jni::JavaVM>,
    /// `BluetoothGattServer` global reference.
    gatt_server: GlobalRef,
    /// `BluetoothDevice` global reference.
    device: GlobalRef,
    /// `BluetoothGattCharacteristic` global reference (for notifications).
    char_ref: GlobalRef,
    /// Receives write payloads forwarded from `nativeOnWrite`.
    /// Wrapped in a Mutex so `recv` can take `&mut Receiver` through a `&self` trait method.
    write_rx: TokioMutex<mpsc::Receiver<Vec<u8>>>,
    address: BleAddress,
}

#[async_trait]
impl BleConnection for AndroidBleConnection {
    /// Send data to the central by calling `notifyCharacteristicChanged` via JNI.
    async fn send(&self, data: &[u8]) -> Result<(), BleError> {
        let jvm = Arc::clone(&self.jvm);
        let gatt_server = self.gatt_server.clone();
        let device = self.device.clone();
        let char_ref = self.char_ref.clone();
        let data = data.to_vec();

        // JNI calls must happen on a thread attached to the JVM.
        tokio::task::spawn_blocking(move || {
            let mut env = jvm
                .attach_current_thread()
                .map_err(|e| BleError::ConnectionError(e.to_string()))?;

            // characteristic.setValue(data)
            let j_data = env
                .byte_array_from_slice(&data)
                .map_err(|e| BleError::ConnectionError(e.to_string()))?;
            env.call_method(
                char_ref.as_obj(),
                "setValue",
                "([B)Z",
                &[JValue::Object(&j_data.into())],
            )
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;

            // gattServer.notifyCharacteristicChanged(device, characteristic, false)
            env.call_method(
                gatt_server.as_obj(),
                "notifyCharacteristicChanged",
                "(Landroid/bluetooth/BluetoothDevice;\
                  Landroid/bluetooth/BluetoothGattCharacteristic;Z)Z",
                &[
                    JValue::Object(device.as_obj()),
                    JValue::Object(char_ref.as_obj()),
                    JValue::Bool(0),
                ],
            )
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;

            Ok(())
        })
        .await
        .map_err(|e| BleError::ConnectionError(e.to_string()))?
    }

    async fn recv(&self) -> Result<Vec<u8>, BleError> {
        self.write_rx.lock().await.recv().await.ok_or(BleError::Disconnected)
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
        GATT_CONN_TX
            .set(conn_tx)
            .map_err(|_| BleError::ConnectionError("GATT_CONN_TX already set".into()))?;
        let _ = GATT_WRITE_TXS.get_or_init(|| Arc::new(TokioMutex::new(HashMap::new())));

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

            // Store char global ref so nativeOnConnected can retrieve it
            let char_global = env
                .new_global_ref(&char_obj)
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            let _ = GATT_CHAR_REF.set(char_global);

            env.call_method(
                &service_obj,
                "addCharacteristic",
                "(Landroid/bluetooth/BluetoothGattCharacteristic;)Z",
                &[JValue::Object(&char_obj)],
            )
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // === Open GattServer ===
            let callback_class = env
                .find_class("com/soradyne/flutter/SoradyneGattCallback")
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // We pass null initially; the callback constructor fills itself in
            let gatt_server = env
                .call_method(
                    &manager,
                    "openGattServer",
                    "(Landroid/content/Context;\
                      Landroid/bluetooth/BluetoothGattServerCallback;)\
                      Landroid/bluetooth/BluetoothGattServer;",
                    &[
                        JValue::Object(context.as_obj()),
                        // Placeholder — Java constructor sets the real server ref
                        JValue::Object(&JObject::null()),
                    ],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?
                .l()
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

            // Instantiate the callback with the server reference
            let callback_obj = env
                .new_object(
                    &callback_class,
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

            // addManufacturerData(0xFFFF, data)
            if !data.is_empty() {
                let j_data = env
                    .byte_array_from_slice(&data)
                    .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
                env.call_method(
                    &adv_builder,
                    "addManufacturerData",
                    "(I[B)Landroid/bluetooth/le/AdvertiseData$Builder;",
                    &[JValue::Int(0xFFFF), JValue::Object(&j_data.into())],
                )
                .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
            }

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

            // AdvertiseCallback stub (anonymous inner class not available via JNI;
            // we use the SoradyneGattCallback which extends AdvertiseCallback too)
            // For simplicity, pass null — failures will surface as logcat errors.
            env.call_method(
                &advertiser,
                "startAdvertising",
                "(Landroid/bluetooth/le/AdvertiseSettings;\
                  Landroid/bluetooth/le/AdvertiseData;\
                  Landroid/bluetooth/le/AdvertiseCallback;)V",
                &[
                    JValue::Object(&settings),
                    JValue::Object(&adv_data),
                    JValue::Object(&callback_obj),
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

    // Retrieve the address string for this device
    let addr_str: String = env
        .call_method(&device, "getAddress", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
        .and_then(|s| env.get_string(&JString::from(s)))
        .map(|s| s.into())
        .unwrap_or_default();

    // Parse the MAC address bytes (format "AA:BB:CC:DD:EE:FF")
    let addr_bytes = parse_mac(&addr_str).unwrap_or([0u8; 6]);
    let ble_addr = BleAddress::Real(addr_bytes);

    // Create a per-connection write channel
    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(64);

    // Register in the global map
    let write_txs = write_txs().clone();
    let addr_key = addr_str.clone();
    tokio::spawn(async move {
        let mut map = write_txs.lock().await;
        map.insert(addr_key, write_tx);
    });

    // Grab the characteristic global ref set during start_advertising
    let char_ref = match GATT_CHAR_REF.get() {
        Some(r) => r.clone(),
        None => return,
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
        write_rx: TokioMutex::new(write_rx),
        address: ble_addr,
    };

    if let Some(tx) = GATT_CONN_TX.get() {
        let _ = tx.try_send(conn);
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

    let write_txs = write_txs().clone();
    tokio::spawn(async move {
        let map = write_txs.lock().await;
        if let Some(tx) = map.get(&key) {
            let _ = tx.send(bytes).await;
        }
    });
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

    let write_txs = write_txs().clone();
    tokio::spawn(async move {
        let mut map = write_txs.lock().await;
        map.remove(&key);
    });
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
