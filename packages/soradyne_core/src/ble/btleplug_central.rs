//! btleplug-based BLE Central for Mac/Linux/Windows.
//!
//! Scans for rim GATT service advertisements and connects via CoreBluetooth
//! (macOS), BlueZ (Linux), or WinRT (Windows).
//!
//! Compiled only when the `ble-central` feature is enabled.

#![cfg(feature = "ble-central")]

use async_trait::async_trait;
use btleplug::api::{
    Central as _, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use tokio::sync::{broadcast, mpsc, Mutex as TokioMutex};
use tokio::time::{sleep, Duration};

use super::framing::{build_frame, FrameReassembler, BLE_CHUNK_SIZE};
use super::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection};
use super::BleError;
use crate::ble::gatt::{envelope_char_uuid, soradyne_service_uuid};

// ---------------------------------------------------------------------------
// BtleplugGattConnection
// ---------------------------------------------------------------------------

/// Reassembly state for length-framed BLE messages.
struct RecvState {
    reassembler: FrameReassembler,
    rx: mpsc::Receiver<Vec<u8>>,
}

/// An active GATT connection to a rim peripheral.
pub struct BtleplugGattConnection {
    peripheral: Peripheral,
    char: btleplug::api::Characteristic,
    /// Reassembly buffer + notification channel, shared under one mutex.
    recv_state: TokioMutex<RecvState>,
    address: BleAddress,
}

#[async_trait]
impl BleConnection for BtleplugGattConnection {
    /// Send `data` as a length-prefixed, BLE-chunked sequence of GATT writes.
    async fn send(&self, data: &[u8]) -> Result<(), BleError> {
        let frame = build_frame(data);
        for chunk in frame.chunks(BLE_CHUNK_SIZE) {
            self.peripheral
                .write(&self.char, chunk, WriteType::WithoutResponse)
                .await
                .map_err(|e| BleError::ConnectionError(e.to_string()))?;
        }
        Ok(())
    }

    /// Receive one complete length-framed message, reassembling across chunks.
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
        self.peripheral
            .disconnect()
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))
    }

    fn rssi(&self) -> Option<i16> {
        None // btleplug 0.11 does not expose live RSSI after connect
    }

    fn peer_address(&self) -> &BleAddress {
        &self.address
    }

    fn is_connected(&self) -> bool {
        // btleplug peripherals remain connected until explicitly disconnected
        // or an error occurs on the notification stream.
        true
    }
}

// ---------------------------------------------------------------------------
// BtleplugCentral
// ---------------------------------------------------------------------------

/// BLE Central backed by btleplug (CoreBluetooth on macOS).
pub struct BtleplugCentral {
    adapter: Adapter,
    /// Broadcast channel for discovered advertisements.
    adv_tx: broadcast::Sender<BleAdvertisement>,
}

impl BtleplugCentral {
    /// Create a new `BtleplugCentral` using the first available BLE adapter.
    pub async fn new() -> Result<Self, BleError> {
        let manager = Manager::new()
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;
        let adapters = manager
            .adapters()
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or_else(|| BleError::ConnectionError("no BLE adapter found".into()))?;

        let (adv_tx, _) = broadcast::channel(64);
        Ok(Self { adapter, adv_tx })
    }

    /// Find the first discovered peripheral advertising our service UUID,
    /// waiting up to `timeout` for one to appear.
    async fn find_peripheral(
        &self,
        timeout: Duration,
    ) -> Result<Peripheral, BleError> {
        use btleplug::api::Central as _;
        let service_uuid = soradyne_service_uuid();
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let peripherals = self
                .adapter
                .peripherals()
                .await
                .map_err(|e| BleError::ScanError(e.to_string()))?;

            for p in peripherals {
                if let Ok(Some(props)) = p.properties().await {
                    if props.services.contains(&service_uuid) {
                        return Ok(p);
                    }
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(BleError::Timeout);
            }
            sleep(Duration::from_millis(200)).await;
        }
    }
}

#[async_trait]
impl BleCentral for BtleplugCentral {
    async fn start_scan(&self) -> Result<(), BleError> {
        let filter = ScanFilter::default();
        self.adapter
            .start_scan(filter)
            .await
            .map_err(|e| BleError::ScanError(e.to_string()))?;
        crate::ffi::pairing_bridge::ble_log("[btleplug] scan started");

        // Pump BLE events into the advertisement broadcast channel.
        // We handle three event types because CoreBluetooth behaves differently
        // depending on whether the peripheral is newly seen or cached:
        //   • DeviceDiscovered  — first sighting (peripheral not in CoreBluetooth cache)
        //   • DeviceUpdated     — scan-response packet on an active scan (may carry services)
        //   • ServicesAdvertisement — CoreBluetooth reports services for a *cached* peripheral;
        //                            DeviceDiscovered never re-fires for these between runs.
        let adapter = self.adapter.clone();
        let adv_tx = self.adv_tx.clone();
        let service_uuid = soradyne_service_uuid();

        tokio::spawn(async move {
            use btleplug::api::Central as _;
            use btleplug::api::CentralEvent::{
                DeviceDiscovered, DeviceUpdated, ServicesAdvertisement,
            };
            use futures_util::StreamExt;

            let mut events = match adapter.events().await {
                Ok(s) => s,
                Err(e) => {
                    crate::ffi::pairing_bridge::ble_log(&format!(
                        "[btleplug] pump: events() failed: {}", e
                    ));
                    return;
                }
            };

            while let Some(event) = events.next().await {
                // Extract the peripheral ID from the event types we care about.
                // For ServicesAdvertisement we can check the service list directly
                // before doing an extra peripheral lookup.
                let id = match &event {
                    DeviceDiscovered(id) | DeviceUpdated(id) => id.clone(),
                    ServicesAdvertisement { id, services } => {
                        if services.contains(&service_uuid) {
                            id.clone()
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };

                if let Ok(p) = adapter.peripheral(&id).await {
                    if let Ok(Some(props)) = p.properties().await {
                        if props.services.contains(&service_uuid) {
                            let addr_bytes = props.address.into_inner();
                            crate::ffi::pairing_bridge::ble_log(&format!(
                                "[btleplug] found inviter at {:?}", props.address
                            ));
                            let adv = BleAdvertisement {
                                data: crate::topology::pairing::PAIRING_ADV_MARKER
                                    .to_vec(),
                                rssi: props.rssi.map(|r| r as i16),
                                source_address: BleAddress::Real(addr_bytes),
                            };
                            let _ = adv_tx.send(adv);
                        }
                    }
                }
            }
            crate::ffi::pairing_bridge::ble_log("[btleplug] pump: stream ended");
        });

        Ok(())
    }

    async fn stop_scan(&self) -> Result<(), BleError> {
        self.adapter
            .stop_scan()
            .await
            .map_err(|e| BleError::ScanError(e.to_string()))
    }

    fn advertisements(&self) -> broadcast::Receiver<BleAdvertisement> {
        self.adv_tx.subscribe()
    }

    async fn connect(&self, _address: &BleAddress) -> Result<Box<dyn BleConnection>, BleError> {
        // Find the peripheral (scan must already be running).
        let peripheral = self
            .find_peripheral(Duration::from_secs(20))
            .await?;

        // Record its address before we move `peripheral`.
        let addr_bytes = peripheral
            .properties()
            .await
            .ok()
            .flatten()
            .map(|p| p.address.into_inner())
            .unwrap_or([0u8; 6]);
        let ble_addr = BleAddress::Real(addr_bytes);

        crate::ffi::pairing_bridge::ble_log("[btleplug] connecting to peripheral...");
        peripheral
            .connect()
            .await
            .map_err(|e| {
                crate::ffi::pairing_bridge::ble_log(&format!(
                    "[btleplug] connect failed: {}", e
                ));
                BleError::ConnectionError(e.to_string())
            })?;

        peripheral
            .discover_services()
            .await
            .map_err(|e| {
                crate::ffi::pairing_bridge::ble_log(&format!(
                    "[btleplug] discover_services failed: {}", e
                ));
                BleError::GattError(e.to_string())
            })?;

        let char_uuid = envelope_char_uuid();
        let gatt_char = peripheral
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == char_uuid)
            .ok_or_else(|| {
                let msg = format!("envelope characteristic {} not found", char_uuid);
                crate::ffi::pairing_bridge::ble_log(&format!("[btleplug] {}", msg));
                BleError::GattError(msg)
            })?;

        peripheral
            .subscribe(&gatt_char)
            .await
            .map_err(|e| {
                crate::ffi::pairing_bridge::ble_log(&format!(
                    "[btleplug] subscribe failed: {}", e
                ));
                BleError::GattError(e.to_string())
            })?;

        crate::ffi::pairing_bridge::ble_log("[btleplug] connected and subscribed");

        // Pump notification stream into an mpsc channel.
        let (notif_tx, notif_rx) = mpsc::channel::<Vec<u8>>(64);
        let peripheral_clone = peripheral.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            if let Ok(mut stream) = peripheral_clone.notifications().await {
                while let Some(data) = stream.next().await {
                    if notif_tx.send(data.value).await.is_err() {
                        break;
                    }
                }
            }
            crate::ffi::pairing_bridge::ble_log("[btleplug] notification stream ended");
        });

        Ok(Box::new(BtleplugGattConnection {
            peripheral,
            char: gatt_char,
            recv_state: TokioMutex::new(RecvState { reassembler: FrameReassembler::new(), rx: notif_rx }),
            address: ble_addr,
        }))
    }
}
