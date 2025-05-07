use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::task;
use tokio::sync::Mutex;
use serde_json;
use crate::types::heartrate::Heartrate;
use crate::flow::SelfDataFlow;
use crate::flow::FlowError;
use crate::network::discovery::{PeerDiscovery, DiscoveredPeer};

pub struct NetworkBridge {
    peers: Arc<Mutex<Vec<Arc<Mutex<OwnedWriteHalf>>>>>,
    discovery: Option<Arc<dyn PeerDiscovery + Send + Sync>>,
}

impl NetworkBridge {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(Vec::new())),
            discovery: None,
        }
    }
    
    /// Set the discovery mechanism for this network bridge
    pub fn with_discovery(mut self, discovery: impl PeerDiscovery + Send + Sync + 'static) -> Self {
        self.discovery = Some(Arc::new(discovery));
        self
    }
    
    /// Start peer discovery
    pub fn start_discovery(&self) -> Result<(), FlowError> {
        if let Some(discovery) = &self.discovery {
            discovery.start_discovery()?;
            
            // Set up callback for newly discovered peers
            let _peers_clone = self.peers.clone();
            discovery.on_peer_discovered(Box::new(move |peer| {
                println!("[NetworkBridge] Discovered peer: {:?}", peer);
                // In a real implementation, we would automatically connect to the peer
                // For now, just log the discovery
            }));
            
            Ok(())
        } else {
            Err(FlowError::PersistenceError("No discovery mechanism configured".to_string()))
        }
    }
    
    /// Stop peer discovery
    pub fn stop_discovery(&self) -> Result<(), FlowError> {
        if let Some(discovery) = &self.discovery {
            discovery.stop_discovery()
        } else {
            Err(FlowError::PersistenceError("No discovery mechanism configured".to_string()))
        }
    }
    
    /// Get discovered peers
    pub fn get_discovered_peers(&self) -> Vec<DiscoveredPeer> {
        if let Some(discovery) = &self.discovery {
            discovery.get_discovered_peers()
        } else {
            Vec::new()
        }
    }

    /// Listen for incoming connections and handle receiving heartrate updates
    pub async fn listen(
        &self,
        addr: &str,
        flow: Arc<SelfDataFlow<Heartrate>>,
    ) -> tokio::io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("[NetworkBridge] Listening on {}", addr);

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            let (read_half, write_half) = socket.into_split();

            let write_half = Arc::new(Mutex::new(write_half));
            println!("[NetworkBridge] New peer connected: {}", peer_addr);

            let flow_clone = flow.clone();
            let peers_clone = self.peers.clone();

            {
                let mut peers = peers_clone.lock().await;
                peers.push(write_half.clone());
            }

            // Handle incoming messages from this peer
            task::spawn(async move {
                let reader = BufReader::new(read_half);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    match serde_json::from_str::<Heartrate>(&line) {
                        Ok(heartrate) => {
                            println!(
                                "[NetworkBridge] Received heartrate: {:.1} bpm",
                                heartrate.bpm
                            );
                            flow_clone.merge(heartrate);
                        }
                        Err(e) => {
                            eprintln!("[NetworkBridge] Failed to parse message: {:?}", e);
                        }
                    }
                }

                println!("[NetworkBridge] Peer {} disconnected", peer_addr);
            });
        }
    }

    /// Connect to a peer and add to broadcast list
    pub async fn connect(&self, addr: &str) -> tokio::io::Result<()> {
        let stream = TcpStream::connect(addr).await?;
        let (_read_half, write_half) = stream.into_split();

        {
            let mut peers = self.peers.lock().await;
            peers.push(Arc::new(Mutex::new(write_half)));
        }
        println!("[NetworkBridge] Connected to peer at {}", addr);
        Ok(())
    }

    /// Broadcast a heartrate to all connected peers
    pub fn broadcast(&self, heartrate: &Heartrate) {
        let json = serde_json::to_string(heartrate).unwrap();
        let peers = self.peers.clone();
        task::spawn(async move {
            let peers = peers.lock().await;
            for peer in peers.iter() {
                let peer = peer.clone();
                let json = json.clone();
                task::spawn(async move {
                    let mut writer = peer.lock().await;
                    if let Err(e) = writer.write_all(json.as_bytes()).await {
                        eprintln!("[NetworkBridge] Failed to send data: {:?}", e);
                    }
                    if let Err(e) = writer.write_all(b"\n").await {
                        eprintln!("[NetworkBridge] Failed to send newline: {:?}", e);
                    }
                });
            }
        });
    }
}

