//! TCP transport — LAN-based implementation of the transport traits.
//!
//! Provides `TcpConnection`, `TcpCentral`, and `TcpPeripheral` for
//! CRDT sync over TCP without BLE hardware. Uses the same length-prefix
//! framing as the BLE transports (without BLE chunking).
//!
//! Compiled only when the `tcp-transport` feature is enabled.

#![cfg(feature = "tcp-transport")]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex as TokioMutex};
use tokio::task::JoinHandle;

use super::framing::{build_frame, FrameReassembler};
use super::transport::{BleAddress, BleAdvertisement, BleCentral, BleConnection, BlePeripheral};
use super::BleError;

// ---------------------------------------------------------------------------
// TcpConnection
// ---------------------------------------------------------------------------

/// An active TCP connection implementing the `BleConnection` trait.
///
/// Wraps a `TcpStream` split into reader and writer halves. The reader
/// task feeds a channel via `FrameReassembler`; `send()` writes
/// length-prefixed frames via a writer task.
pub struct TcpConnection {
    /// Sender for outgoing data (serialized frames).
    /// Wrapped in Option so `disconnect()` can drop it.
    write_tx: TokioMutex<Option<mpsc::Sender<Vec<u8>>>>,
    /// Receiver for complete reassembled messages.
    msg_rx: TokioMutex<mpsc::Receiver<Vec<u8>>>,
    /// Whether the connection is still alive.
    connected: Arc<AtomicBool>,
    /// The peer's address.
    peer_addr: BleAddress,
    /// Reader task handle — aborted on disconnect/drop to close the socket.
    /// Uses StdMutex so it can be aborted from the synchronous Drop impl.
    reader_handle: StdMutex<Option<JoinHandle<()>>>,
    /// Writer task handle — aborted on disconnect/drop.
    writer_handle: StdMutex<Option<JoinHandle<()>>>,
}

impl TcpConnection {
    /// Create a `TcpConnection` from an established `TcpStream`.
    ///
    /// Spawns a reader task and a writer task. The reader reassembles
    /// length-prefixed frames and feeds them to `msg_rx`. The writer
    /// drains `write_tx` and flushes to the socket.
    pub fn from_stream(stream: TcpStream, peer_socket: SocketAddr) -> Self {
        // Set TCP_NODELAY for low-latency CRDT sync.
        let _ = stream.set_nodelay(true);

        let (reader, writer) = tokio::io::split(stream);
        let connected = Arc::new(AtomicBool::new(true));

        // Message channel: reader task -> recv()
        let (msg_tx, msg_rx) = mpsc::channel::<Vec<u8>>(64);

        // Write channel: send() -> writer task
        let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(64);

        // Reader task
        let reader_handle = {
            let connected = Arc::clone(&connected);
            tokio::spawn(async move {
                let mut reader = reader;
                let mut reassembler = FrameReassembler::new();
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            reassembler.push(&buf[..n]);
                            while let Some(msg) = reassembler.try_extract() {
                                if msg_tx.send(msg).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                connected.store(false, Ordering::SeqCst);
            })
        };

        // Writer task
        let writer_handle = {
            let connected = Arc::clone(&connected);
            tokio::spawn(async move {
                let mut writer = writer;
                while let Some(data) = write_rx.recv().await {
                    if writer.write_all(&data).await.is_err() {
                        break;
                    }
                    if writer.flush().await.is_err() {
                        break;
                    }
                }
                connected.store(false, Ordering::SeqCst);
            })
        };

        Self {
            write_tx: TokioMutex::new(Some(write_tx)),
            msg_rx: TokioMutex::new(msg_rx),
            connected,
            peer_addr: BleAddress::Tcp(peer_socket),
            reader_handle: StdMutex::new(Some(reader_handle)),
            writer_handle: StdMutex::new(Some(writer_handle)),
        }
    }

    fn abort_tasks(&self) {
        if let Ok(mut h) = self.reader_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
        if let Ok(mut h) = self.writer_handle.lock() {
            if let Some(handle) = h.take() {
                handle.abort();
            }
        }
    }
}

impl Drop for TcpConnection {
    fn drop(&mut self) {
        // Abort tasks so both halves of the TcpStream are dropped, causing
        // the peer's reader to see EOF. This handles the case where the
        // connection is dropped without an explicit disconnect() call.
        self.abort_tasks();
    }
}

#[async_trait]
impl BleConnection for TcpConnection {
    async fn send(&self, data: &[u8]) -> Result<(), BleError> {
        if !self.is_connected() {
            return Err(BleError::Disconnected);
        }
        let frame = build_frame(data);
        let guard = self.write_tx.lock().await;
        match guard.as_ref() {
            Some(tx) => tx.send(frame).await.map_err(|_| BleError::Disconnected),
            None => Err(BleError::Disconnected),
        }
    }

    async fn recv(&self) -> Result<Vec<u8>, BleError> {
        let mut rx = self.msg_rx.lock().await;
        rx.recv().await.ok_or(BleError::Disconnected)
    }

    async fn disconnect(&self) -> Result<(), BleError> {
        self.connected.store(false, Ordering::SeqCst);
        // Drop the write sender so no more data can be queued.
        *self.write_tx.lock().await = None;
        // Abort both I/O tasks, dropping both halves of the TcpStream.
        // The peer's reader will see EOF and clean up.
        self.abort_tasks();
        Ok(())
    }

    fn rssi(&self) -> Option<i16> {
        None // Not applicable to TCP
    }

    fn peer_address(&self) -> &BleAddress {
        &self.peer_addr
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// TcpCentral
// ---------------------------------------------------------------------------

/// TCP-based "central" that connects to known socket addresses.
///
/// `start_scan()` and `stop_scan()` are no-ops — TCP has no broadcast
/// discovery. Peers are connected directly via `connect()`.
pub struct TcpCentral {
    adv_tx: broadcast::Sender<BleAdvertisement>,
}

impl TcpCentral {
    pub fn new() -> Self {
        let (adv_tx, _) = broadcast::channel(16);
        Self { adv_tx }
    }
}

#[async_trait]
impl BleCentral for TcpCentral {
    async fn start_scan(&self) -> Result<(), BleError> {
        Ok(()) // No-op: TCP has no broadcast discovery.
    }

    async fn stop_scan(&self) -> Result<(), BleError> {
        Ok(())
    }

    fn advertisements(&self) -> broadcast::Receiver<BleAdvertisement> {
        self.adv_tx.subscribe()
    }

    async fn connect(&self, address: &BleAddress) -> Result<Box<dyn BleConnection>, BleError> {
        let addr = match address {
            BleAddress::Tcp(addr) => *addr,
            other => {
                return Err(BleError::ConnectionError(format!(
                    "TcpCentral cannot connect to non-TCP address: {:?}",
                    other
                )))
            }
        };

        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| BleError::ConnectionError(e.to_string()))?;

        Ok(Box::new(TcpConnection::from_stream(stream, addr)))
    }
}

// ---------------------------------------------------------------------------
// TcpPeripheral
// ---------------------------------------------------------------------------

/// TCP-based "peripheral" that listens for incoming connections.
///
/// `start_advertising()` binds a `TcpListener` and spawns an accept loop
/// that feeds connections into a channel. `accept()` receives from the
/// channel, matching the `BlePeripheral` trait pattern.
pub struct TcpPeripheral {
    /// Receives accepted connections from the listener task.
    conn_rx: TokioMutex<mpsc::Receiver<Box<dyn BleConnection>>>,
    /// Sender side, stored so we can start the listener.
    conn_tx: mpsc::Sender<Box<dyn BleConnection>>,
    /// The actual bound address (resolved after bind, useful for port 0).
    local_addr: TokioMutex<Option<SocketAddr>>,
    /// The address to bind to.
    bind_addr: SocketAddr,
}

impl TcpPeripheral {
    /// Create a new `TcpPeripheral` that will bind to the given address.
    ///
    /// Use `"0.0.0.0:0"` or `"127.0.0.1:0"` for OS-assigned port.
    /// Call `local_addr()` after `start_advertising()` to get the actual port.
    pub fn new(bind_addr: SocketAddr) -> Self {
        let (conn_tx, conn_rx) = mpsc::channel(16);
        Self {
            conn_rx: TokioMutex::new(conn_rx),
            conn_tx,
            local_addr: TokioMutex::new(None),
            bind_addr,
        }
    }

    /// Get the actual bound address after `start_advertising()`.
    pub async fn local_addr(&self) -> Option<SocketAddr> {
        *self.local_addr.lock().await
    }
}

#[async_trait]
impl BlePeripheral for TcpPeripheral {
    async fn start_advertising(&self, _data: Vec<u8>) -> Result<(), BleError> {
        let listener = TcpListener::bind(self.bind_addr)
            .await
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;

        let actual_addr = listener
            .local_addr()
            .map_err(|e| BleError::AdvertisingError(e.to_string()))?;
        *self.local_addr.lock().await = Some(actual_addr);

        let conn_tx = self.conn_tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let conn = TcpConnection::from_stream(stream, peer_addr);
                        if conn_tx
                            .send(Box::new(conn) as Box<dyn BleConnection>)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(())
    }

    async fn stop_advertising(&self) -> Result<(), BleError> {
        // Dropping the listener (via task cancellation) stops accepting.
        // For now, minimal stub — the task will exit when conn_tx is dropped.
        Ok(())
    }

    async fn update_advertisement(&self, _data: Vec<u8>) -> Result<(), BleError> {
        Ok(()) // No-op: TCP has no advertisement payload.
    }

    async fn accept(&self) -> Result<Box<dyn BleConnection>, BleError> {
        self.conn_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(BleError::Disconnected)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn localhost_any_port() -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
    }

    #[tokio::test]
    async fn test_send_recv() {
        let peripheral = TcpPeripheral::new(localhost_any_port());
        peripheral.start_advertising(vec![]).await.unwrap();
        let addr = peripheral.local_addr().await.unwrap();

        let central = TcpCentral::new();
        let tcp_addr = BleAddress::Tcp(addr);

        let accept_handle = tokio::spawn(async move { peripheral.accept().await.unwrap() });

        let conn_c = central.connect(&tcp_addr).await.unwrap();
        let conn_p = accept_handle.await.unwrap();

        // Central -> Peripheral
        conn_c.send(b"hello from central").await.unwrap();
        let received = conn_p.recv().await.unwrap();
        assert_eq!(received, b"hello from central");

        // Peripheral -> Central
        conn_p.send(b"hello from peripheral").await.unwrap();
        let received = conn_c.recv().await.unwrap();
        assert_eq!(received, b"hello from peripheral");
    }

    #[tokio::test]
    async fn test_disconnect_detection() {
        let peripheral = TcpPeripheral::new(localhost_any_port());
        peripheral.start_advertising(vec![]).await.unwrap();
        let addr = peripheral.local_addr().await.unwrap();

        let central = TcpCentral::new();
        let tcp_addr = BleAddress::Tcp(addr);

        let accept_handle = tokio::spawn(async move { peripheral.accept().await.unwrap() });

        let conn_c = central.connect(&tcp_addr).await.unwrap();
        let conn_p = accept_handle.await.unwrap();

        // Disconnect central side
        conn_c.disconnect().await.unwrap();

        // Give the writer task time to notice
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Peripheral recv should eventually return Disconnected
        let result = conn_p.recv().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_large_message() {
        let peripheral = TcpPeripheral::new(localhost_any_port());
        peripheral.start_advertising(vec![]).await.unwrap();
        let addr = peripheral.local_addr().await.unwrap();

        let central = TcpCentral::new();
        let tcp_addr = BleAddress::Tcp(addr);

        let accept_handle = tokio::spawn(async move { peripheral.accept().await.unwrap() });

        let conn_c = central.connect(&tcp_addr).await.unwrap();
        let conn_p = accept_handle.await.unwrap();

        // 100KB message — no BLE MTU limit over TCP
        let large_data = vec![0xABu8; 100_000];
        conn_c.send(&large_data).await.unwrap();
        let received = conn_p.recv().await.unwrap();
        assert_eq!(received.len(), 100_000);
        assert_eq!(received, large_data);
    }

    #[tokio::test]
    async fn test_rapid_sequential_messages() {
        let peripheral = TcpPeripheral::new(localhost_any_port());
        peripheral.start_advertising(vec![]).await.unwrap();
        let addr = peripheral.local_addr().await.unwrap();

        let central = TcpCentral::new();
        let tcp_addr = BleAddress::Tcp(addr);

        let accept_handle = tokio::spawn(async move { peripheral.accept().await.unwrap() });

        let conn_c = central.connect(&tcp_addr).await.unwrap();
        let conn_p = accept_handle.await.unwrap();

        // Send 100 messages rapidly to stress-test framing correctness
        for i in 0u32..100 {
            let msg = format!("msg-{}", i);
            conn_c.send(msg.as_bytes()).await.unwrap();
        }

        for i in 0u32..100 {
            let expected = format!("msg-{}", i);
            let received = conn_p.recv().await.unwrap();
            assert_eq!(received, expected.as_bytes(), "mismatch at message {}", i);
        }
    }

    #[tokio::test]
    async fn test_central_connect() {
        let peripheral = TcpPeripheral::new(localhost_any_port());
        peripheral.start_advertising(vec![]).await.unwrap();
        let addr = peripheral.local_addr().await.unwrap();

        let central = TcpCentral::new();
        let tcp_addr = BleAddress::Tcp(addr);

        let accept_handle = tokio::spawn(async move { peripheral.accept().await.unwrap() });

        let conn = central.connect(&tcp_addr).await.unwrap();
        let _ = accept_handle.await.unwrap();

        assert!(conn.is_connected());
        assert_eq!(conn.peer_address(), &tcp_addr);
    }

    #[tokio::test]
    async fn test_peripheral_accept() {
        let peripheral = TcpPeripheral::new(localhost_any_port());
        peripheral.start_advertising(vec![]).await.unwrap();
        let addr = peripheral.local_addr().await.unwrap();

        let central = TcpCentral::new();
        let tcp_addr = BleAddress::Tcp(addr);

        let accept_handle = tokio::spawn(async move { peripheral.accept().await.unwrap() });

        let _conn_c = central.connect(&tcp_addr).await.unwrap();
        let conn_p = accept_handle.await.unwrap();

        assert!(conn_p.is_connected());
    }

    #[tokio::test]
    async fn test_connect_to_non_tcp_address_fails() {
        let central = TcpCentral::new();
        let result = central.connect(&BleAddress::Simulated(uuid::Uuid::new_v4())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_scan_noop() {
        let central = TcpCentral::new();
        central.start_scan().await.unwrap();
        central.stop_scan().await.unwrap();
    }
}
