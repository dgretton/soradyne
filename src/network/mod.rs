pub mod connection;
pub mod discovery;

pub use connection::NetworkBridge;
pub use discovery::{PeerDiscovery, NoOpDiscovery, MdnsDiscovery, LanDiscovery, DiscoveredPeer};

