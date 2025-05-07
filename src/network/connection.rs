use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::task;
use tokio::sync::Mutex;
use serde_json;
use crate::types::heartrate::Heartrate;
use crate::flow::SelfDataFlow;

pub struct NetworkBridge {
    peers: Arc<Mutex<Vec<Arc<Mutex<OwnedWriteHalf>>>>>,
}

impl NetworkBridge {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(Vec::new())),
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

