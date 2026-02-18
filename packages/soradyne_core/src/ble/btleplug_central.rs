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

use super::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection};
use super::BleError;
use crate::ble::gatt::{envelope_char_uuid, soradyne_service_uuid};

// ---------------------------------------------------------------------------
// BtleplugGattConnection
// ---------------------------------------------------------------------------

/// An active GATT connection to a rim peripheral.
pub struct BtleplugGattConnection {
    peripheral: Peripheral,
    char: btleplug::api::Characteristic,
    /// Receives notification payloads forwarded by the background pump task.
    /// Wrapped in a Mutex so `recv` can take `&mut Receiver` through a `&self` trait method.
    notif_rx: TokioMutex<mpsc::Receiver<Vec<u8>>>,
    address: BleAddress,
}

#[async_trait]
impl BleConnection for BtleplugGattConnection {
    async fn send(&self, data: &[u8]) -> Result<(), BleError> {
        self.peripheral
            .write(&self.char, data, WriteType::WithoutResponse)
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))
    }

    async fn recv(&self) -> Result<Vec<u8>, BleError> {
        self.notif_rx.lock().await.recv().await.ok_or(BleError::Disconnected)
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
        let filter = ScanFilter {
            services: vec![soradyne_service_uuid()],
        };
        self.adapter
            .start_scan(filter)
            .await
            .map_err(|e| BleError::ScanError(e.to_string()))?;

        // Pump newly discovered peripherals into the advertisement broadcast channel.
        let adapter = self.adapter.clone();
        let adv_tx = self.adv_tx.clone();
        let service_uuid = soradyne_service_uuid();

        tokio::spawn(async move {
            use btleplug::api::Central as _;
            use futures_util::StreamExt;

            // btleplug emits events via a stream on the adapter.
            let mut events = match adapter.events().await {
                Ok(s) => s,
                Err(_) => return,
            };

            while let Some(event) = events.next().await {
                if let btleplug::api::CentralEvent::DeviceDiscovered(id) = event {
                    if let Ok(p) = adapter.peripheral(&id).await {
                        if let Ok(Some(props)) = p.properties().await {
                            if props.services.contains(&service_uuid) {
                                let addr_bytes = props
                                    .address
                                    .into_inner();
                                let adv = BleAdvertisement {
                                    data: props.manufacturer_data
                                        .values()
                                        .flat_map(|v| v.iter().copied())
                                        .collect(),
                                    rssi: props.rssi.map(|r| r as i16),
                                    source_address: BleAddress::Real(addr_bytes),
                                };
                                let _ = adv_tx.send(adv);
                            }
                        }
                    }
                }
            }
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

        peripheral
            .connect()
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;

        peripheral
            .discover_services()
            .await
            .map_err(|e| BleError::GattError(e.to_string()))?;

        let char_uuid = envelope_char_uuid();
        let characteristics = peripheral.characteristics();
        let gatt_char = characteristics
            .into_iter()
            .find(|c| c.uuid == char_uuid)
            .ok_or_else(|| {
                BleError::GattError(format!("envelope characteristic {} not found", char_uuid))
            })?;

        peripheral
            .subscribe(&gatt_char)
            .await
            .map_err(|e| BleError::GattError(e.to_string()))?;

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
        });

        Ok(Box::new(BtleplugGattConnection {
            peripheral,
            char: gatt_char,
            notif_rx: TokioMutex::new(notif_rx),
            address: ble_addr,
        }))
    }
}
