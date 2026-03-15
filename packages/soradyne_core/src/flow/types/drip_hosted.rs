//! FullReplicaFlow — a convergent document flow where every party replicates
//! every other party's operation journal locally, and fitting (materialization)
//! happens independently on each device.
//!
//! This is one specific flow type (set of policies). Its characteristics:
//! - **Memorization**: every party stores every other party's ops in per-party
//!   journal files (`journals/{author_device_id}.jsonl`).
//! - **Outbound queue**: ops are queued for each known peer in
//!   `outbound/{peer_device_id}.jsonl` for later delivery.
//! - **Fitting**: local on each device — all journals are loaded and piped
//!   through the convergent document CRDT to materialize the drip stream.
//!
//! A different flow type (e.g. `HostedFitFlow`) might host materialization on
//! a server, store data in SQLite, use erasure coding, etc. — the application
//! using the flow wouldn't change.
//!
//! # Wire Protocol
//!
//! Messages are serialized as CBOR and carried as `RoutedEnvelope` payloads
//! with `MessageType::FlowSync`.

use std::collections::{HashMap, HashSet};
use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ble::gatt::MessageType;
use crate::convergent::giantt::GianttSchema;
use crate::convergent::inventory::InventorySchema;
use crate::convergent::{
    ConvergentDocument, DeviceId, DocumentSchema, Horizon, OpEnvelope, Operation,
};
use crate::flow::error::FlowError;
use crate::flow::flow_core::{Flow, FlowConfig, FlowRegistry, FlowSchema};
use crate::flow::stream::{Stream, StreamSpec};
use crate::topology::capsule::PieceCapabilities;
use crate::topology::ensemble::{EnsembleTopology, PieceReachability};
use crate::topology::messenger::TopologyMessenger;

// ---------------------------------------------------------------------------
// 5.1  Policy types
// ---------------------------------------------------------------------------

/// Policy governing how the drip host is assigned and what happens on failure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DripHostPolicy {
    /// Strategy for selecting which piece hosts the drip.
    pub selection: HostSelectionStrategy,
    /// What to do when the current host disappears.
    pub failover: HostFailoverPolicy,
    /// How long to wait (in millis) before declaring the host dead.
    pub host_timeout_ms: u64,
    /// Can the host voluntarily hand off to another piece?
    pub allow_voluntary_handoff: bool,
}

impl Default for DripHostPolicy {
    fn default() -> Self {
        Self {
            selection: HostSelectionStrategy::FirstEligible,
            failover: HostFailoverPolicy::OfflineMerge,
            host_timeout_ms: 10_000,
            allow_voluntary_handoff: true,
        }
    }
}

impl DripHostPolicy {
    pub fn host_timeout(&self) -> Duration {
        Duration::from_millis(self.host_timeout_ms)
    }
}

/// Strategy for initial host selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HostSelectionStrategy {
    /// First piece with `can_host_drip` that appears in the ensemble.
    FirstEligible,
    /// Piece with the best connectivity to all others.
    BestConnected,
    /// Prefer a specific piece by device_id.
    Preferred { device_id: Uuid },
    /// Score-weighted selection based on capabilities.
    Scored(HostScoreWeights),
}

/// Weights for the scored host selection strategy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostScoreWeights {
    pub connectivity: f64,
    pub storage: f64,
    pub battery_stability: f64,
    pub has_ui: f64,
}

impl Default for HostScoreWeights {
    fn default() -> Self {
        Self {
            connectivity: 1.0,
            storage: 0.5,
            battery_stability: 0.3,
            has_ui: 0.2,
        }
    }
}

/// What to do when the current host drops.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HostFailoverPolicy {
    /// Next eligible piece takes over immediately.
    ImmediateFailover,
    /// Wait for a grace period, queuing edits, then failover.
    GracefulFailover { grace_period_ms: u64 },
    /// All pieces maintain local copies; CRDT converges on reconnect.
    OfflineMerge,
    /// Flow pauses until host returns.
    WaitForHost,
}

// ---------------------------------------------------------------------------
// 5.2  Wire protocol
// ---------------------------------------------------------------------------

/// Messages exchanged between pieces for flow synchronization.
///
/// Serialized with CBOR (`ciborium`) and carried as `RoutedEnvelope` payload
/// with `MessageType::FlowSync`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FlowSyncMessage {
    /// Exchange causal horizons to determine what operations the peer is missing.
    HorizonExchange { flow_id: Uuid, horizon: Horizon },
    /// A batch of operations to apply.
    OperationBatch {
        flow_id: Uuid,
        ops: Vec<OpEnvelope>,
    },
    /// Announce that a piece is (or claims to be) the host for this flow.
    HostAnnouncement {
        flow_id: Uuid,
        host_id: Uuid,
        epoch: u64,
    },
}

impl FlowSyncMessage {
    /// Serialize to CBOR bytes.
    pub fn to_cbor(&self) -> Result<Vec<u8>, FlowError> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| FlowError::SerializationError(e.to_string()))?;
        Ok(buf)
    }

    /// Deserialize from CBOR bytes.
    pub fn from_cbor(data: &[u8]) -> Result<Self, FlowError> {
        ciborium::from_reader(data).map_err(|e| FlowError::SerializationError(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// 5.3  Host assignment tracking
// ---------------------------------------------------------------------------

/// Tracks the current host and uses a monotonically increasing epoch
/// to resolve conflicting announcements (highest epoch wins).
#[derive(Clone, Debug)]
pub struct HostAssignment {
    pub current_host: Option<Uuid>,
    pub epoch: u64,
    pub last_host_seen: Option<Instant>,
}

impl HostAssignment {
    pub fn new() -> Self {
        Self {
            current_host: None,
            epoch: 0,
            last_host_seen: None,
        }
    }

    /// Accept a host announcement if it has a higher epoch.
    /// Returns true if the announcement was accepted.
    pub fn accept_announcement(&mut self, host_id: Uuid, epoch: u64) -> bool {
        if epoch > self.epoch {
            self.current_host = Some(host_id);
            self.epoch = epoch;
            self.last_host_seen = Some(Instant::now());
            true
        } else {
            false
        }
    }

    /// Record that the host was recently seen (heartbeat).
    pub fn touch(&mut self) {
        self.last_host_seen = Some(Instant::now());
    }

    /// Check if the host has timed out.
    pub fn is_host_timed_out(&self, timeout: Duration) -> bool {
        match self.last_host_seen {
            Some(t) => t.elapsed() > timeout,
            None => self.current_host.is_some(), // has host but never seen = timed out
        }
    }
}

impl Default for HostAssignment {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Persistence helpers (module-level, used by FullReplicaFlow)
// ---------------------------------------------------------------------------

/// Sanitize a DeviceId for use as a filename (replace non-alphanumeric chars).
fn device_id_to_filename(device_id: &str) -> String {
    device_id.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// Append a single `OpEnvelope` as a JSON line to a specific journal file.
/// `base_path` is the flow's storage directory; the op goes into
/// `journals/{author_device_id}.jsonl`.
fn append_to_journal(base_path: &std::path::Path, author: &str, envelope: &OpEnvelope) {
    let journals_dir = base_path.join("journals");
    if let Err(e) = std::fs::create_dir_all(&journals_dir) {
        eprintln!("[full_replica] journal: create_dir {:?} failed: {}", journals_dir, e);
        return;
    }
    let filename = format!("{}.jsonl", device_id_to_filename(author));
    let file_path = journals_dir.join(filename);
    match serde_json::to_string(envelope) {
        Ok(json) => {
            match std::fs::OpenOptions::new().create(true).append(true).open(&file_path) {
                Ok(mut file) => {
                    if let Err(e) = writeln!(file, "{}", json) {
                        eprintln!("[full_replica] journal: write failed: {}", e);
                    }
                }
                Err(e) => eprintln!("[full_replica] journal: open {:?} failed: {}", file_path, e),
            }
        }
        Err(e) => eprintln!("[full_replica] journal: serialize failed: {}", e),
    }
}

/// Append a single `OpEnvelope` to the outbound queue for a specific peer.
/// `base_path` is the flow's storage directory; the op goes into
/// `outbound/{peer_device_id}.jsonl`.
fn enqueue_for_peer(base_path: &std::path::Path, peer_device_id: &str, envelope: &OpEnvelope) {
    let outbound_dir = base_path.join("outbound");
    if let Err(e) = std::fs::create_dir_all(&outbound_dir) {
        eprintln!("[full_replica] outbound: create_dir {:?} failed: {}", outbound_dir, e);
        return;
    }
    let filename = format!("{}.jsonl", device_id_to_filename(peer_device_id));
    let file_path = outbound_dir.join(filename);
    match serde_json::to_string(envelope) {
        Ok(json) => {
            match std::fs::OpenOptions::new().create(true).append(true).open(&file_path) {
                Ok(mut file) => {
                    if let Err(e) = writeln!(file, "{}", json) {
                        eprintln!("[full_replica] outbound: write failed: {}", e);
                    }
                }
                Err(e) => eprintln!("[full_replica] outbound: open {:?} failed: {}", file_path, e),
            }
        }
        Err(e) => eprintln!("[full_replica] outbound: serialize failed: {}", e),
    }
}

/// Load the known parties mapping from `parties.json` in the flow's storage directory.
/// Maps DeviceId (String) → device Uuid.
fn load_known_parties(base_path: &std::path::Path) -> HashMap<String, Uuid> {
    let path = base_path.join("parties.json");
    if !path.exists() {
        return HashMap::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// Persist the known parties mapping to `parties.json`.
fn save_known_parties(base_path: &std::path::Path, parties: &HashMap<String, Uuid>) {
    let path = base_path.join("parties.json");
    if let Err(e) = std::fs::create_dir_all(base_path) {
        eprintln!("[full_replica] save_parties: create_dir failed: {}", e);
        return;
    }
    match serde_json::to_string_pretty(parties) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                eprintln!("[full_replica] save_parties: write failed: {}", e);
            }
        }
        Err(e) => eprintln!("[full_replica] save_parties: serialize failed: {}", e),
    }
}

/// Legacy compat: read `operations.jsonl` (the old single-file format) if it exists.
/// Returns the ops and deletes the file after successful read.
fn migrate_legacy_ops(base_path: &std::path::Path) -> Vec<OpEnvelope> {
    let legacy_path = base_path.join("operations.jsonl");
    if !legacy_path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(&legacy_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let ops: Vec<OpEnvelope> = content.lines()
        .filter_map(|line| serde_json::from_str::<OpEnvelope>(line).ok())
        .collect();
    if !ops.is_empty() {
        eprintln!("[full_replica] migrating {} ops from legacy operations.jsonl to per-party journals", ops.len());
        // Don't delete yet — we'll delete after successful migration in load_from_disk
    }
    ops
}

// ---------------------------------------------------------------------------
// 5.4  ConvergentDocumentStream<S>
// ---------------------------------------------------------------------------

/// Wraps an `Arc<std::sync::RwLock<ConvergentDocument<S>>>` as a `Stream`.
///
/// - `read()` materializes the document and serializes to JSON.
/// - `write()` deserializes an `Operation` and calls `apply_local()`.
///
/// Automatically registered as the "state" stream in DripHostedFlow.
pub struct ConvergentDocumentStream<S: DocumentSchema> {
    name: String,
    document: Arc<std::sync::RwLock<ConvergentDocument<S>>>,
    subscribers: Arc<std::sync::RwLock<HashMap<Uuid, Box<dyn Fn(&[u8]) + Send + Sync>>>>,
}

impl<S: DocumentSchema> ConvergentDocumentStream<S> {
    pub fn new(
        name: impl Into<String>,
        document: Arc<std::sync::RwLock<ConvergentDocument<S>>>,
    ) -> Self {
        Self {
            name: name.into(),
            document,
            subscribers: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Notify subscribers with the current materialized state.
    fn notify_subscribers(&self) {
        if let Ok(doc) = self.document.read() {
            let state = doc.materialize();
            if let Ok(bytes) = serde_json::to_vec(&state) {
                if let Ok(subs) = self.subscribers.read() {
                    for cb in subs.values() {
                        cb(&bytes);
                    }
                }
            }
        }
    }
}

impl<S: DocumentSchema + 'static> Stream for ConvergentDocumentStream<S> {
    fn read(&self) -> Result<Option<Vec<u8>>, FlowError> {
        let doc = self
            .document
            .read()
            .map_err(|e| FlowError::SyncError(format!("document lock poisoned: {e}")))?;
        let state = doc.materialize();
        let bytes = serde_json::to_vec(&state)
            .map_err(|e| FlowError::SerializationError(e.to_string()))?;
        Ok(Some(bytes))
    }

    fn write(&self, data: &[u8]) -> Result<(), FlowError> {
        let op: Operation =
            serde_json::from_slice(data).map_err(|e| FlowError::SerializationError(e.to_string()))?;
        {
            let mut doc = self
                .document
                .write()
                .map_err(|e| FlowError::SyncError(format!("document lock poisoned: {e}")))?;
            doc.apply_local(op);
        }
        self.notify_subscribers();
        Ok(())
    }

    fn subscribe(&self, callback: Box<dyn Fn(&[u8]) + Send + Sync>) -> Uuid {
        let id = Uuid::new_v4();
        if let Ok(mut subs) = self.subscribers.write() {
            subs.insert(id, callback);
        }
        id
    }

    fn unsubscribe(&self, subscription_id: Uuid) {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.remove(&subscription_id);
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl<S: DocumentSchema> Clone for ConvergentDocumentStream<S> {
    /// Produces a second handle to the same stream: shared document Arc and
    /// shared subscriber map, so subscribers registered on one are visible to both.
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            document: Arc::clone(&self.document),
            subscribers: Arc::clone(&self.subscribers),
        }
    }
}

// ---------------------------------------------------------------------------
// 5.5  AccessoryMemorizer
// ---------------------------------------------------------------------------

/// Caches operations for a flow so that accessories (or any piece) can serve
/// ops to peers that reconnect. Deduplicates by `OpEnvelope.id`.
pub struct AccessoryMemorizer {
    pub flow_id: Uuid,
    cached_ops: std::sync::RwLock<Vec<OpEnvelope>>,
    horizon: std::sync::RwLock<Horizon>,
    max_ops: usize,
}

impl AccessoryMemorizer {
    pub fn new(flow_id: Uuid, max_ops: usize) -> Self {
        Self {
            flow_id,
            cached_ops: std::sync::RwLock::new(Vec::new()),
            horizon: std::sync::RwLock::new(Horizon::new()),
            max_ops,
        }
    }

    /// Cache an operation. Deduplicates by op id.
    pub fn cache_operation(&self, op: OpEnvelope) {
        if let Ok(mut ops) = self.cached_ops.write() {
            if ops.iter().any(|o| o.id == op.id) {
                return; // already cached
            }
            // Update horizon
            if let Ok(mut h) = self.horizon.write() {
                h.observe(&op.author, op.seq);
            }
            ops.push(op);
            // Evict oldest if over capacity
            if ops.len() > self.max_ops {
                ops.remove(0);
            }
        }
    }

    /// Return operations that the given horizon hasn't seen.
    pub fn operations_since(&self, since: &Horizon) -> Vec<OpEnvelope> {
        if let Ok(ops) = self.cached_ops.read() {
            ops.iter()
                .filter(|op| !since.has_seen(&op.author, op.seq))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the current horizon of cached operations.
    pub fn horizon(&self) -> Horizon {
        self.horizon
            .read()
            .map(|h| h.clone())
            .unwrap_or_default()
    }

    /// Number of cached operations.
    pub fn cached_count(&self) -> usize {
        self.cached_ops.read().map(|o| o.len()).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// 5.6  DripHostedFlow<S>
// ---------------------------------------------------------------------------

/// A flow type where every party replicates every other party's operation
/// journal locally, and fitting (CRDT materialization) happens on each device.
///
/// Streams:
/// - `"state"` (drip, singleton): The convergent document state.
///
/// The host is selected by policy. Under `OfflineMerge` (the default),
/// all pieces accept edits locally and converge via CRDT. Under
/// `WaitForHost`, non-host pieces queue edits until the host is available.
///
/// # Storage layout (when `storage_path` is set)
///
/// ```text
/// {storage_path}/
///   journals/
///     {device_id_A}.jsonl   # ops authored by party A
///     {device_id_B}.jsonl   # ops received from party B
///   outbound/
///     {device_id_B}.jsonl   # ops queued to send to party B
///   parties.json            # known party device IDs
/// ```
pub struct FullReplicaFlow<S: DocumentSchema + 'static> {
    id: Uuid,
    type_name: String,
    schema: FlowSchema,
    document: Arc<std::sync::RwLock<ConvergentDocument<S>>>,
    policy: DripHostPolicy,
    host_assignment: Arc<std::sync::RwLock<HostAssignment>>,
    device_uuid: Uuid,
    device_id: DeviceId,
    messenger: Option<Arc<TopologyMessenger>>,
    topology: Option<Arc<tokio::sync::RwLock<EnsembleTopology>>>,
    pending_edits: Arc<std::sync::RwLock<Vec<Operation>>>,
    streams: HashMap<String, Box<dyn Stream>>,
    shutdown_tx: broadcast::Sender<()>,
    /// Shared clone of the "state" stream; the start() task calls
    /// notify_subscribers() on this after remote ops arrive, so UI pollers
    /// see the updated materialized state without needing a separate callback.
    state_stream: ConvergentDocumentStream<S>,
    /// Filesystem path for per-party journal storage and outbound queues.
    /// `None` for in-memory flows (tests, ephemeral sessions).
    storage_path: Option<PathBuf>,
    /// Known parties to this flow — their ops are stored locally and we
    /// queue outbound ops for them. Maps DeviceId → device Uuid.
    /// Persisted in `parties.json`.
    known_parties: Arc<std::sync::RwLock<HashMap<String, Uuid>>>,
    /// Signal to trigger an outbound queue flush. Fired on: local edit,
    /// inbound message received, new peer connected, and startup.
    /// `Notify` coalesces multiple signals into one wakeup.
    flush_notify: Arc<tokio::sync::Notify>,
}

/// Transitional alias — use `FullReplicaFlow` in new code.
pub type DripHostedFlow<S> = FullReplicaFlow<S>;

impl<S: DocumentSchema + 'static> FullReplicaFlow<S> {
    /// Construct in "disconnected" mode (no messenger/topology).
    /// Under OfflineMerge this is fully functional for local edits.
    pub fn new(
        config: FlowConfig,
        doc_schema: S,
        policy: DripHostPolicy,
        device_uuid: Uuid,
        device_id: DeviceId,
    ) -> Self {
        let document = Arc::new(std::sync::RwLock::new(ConvergentDocument::new(
            doc_schema,
            device_id.clone(),
        )));

        let flow_schema = FlowSchema::new(&config.type_name)
            .with_stream(StreamSpec::drip("state").with_description("Convergent document state"));

        let state_stream = ConvergentDocumentStream::new("state", Arc::clone(&document));
        let state_stream_notify = state_stream.clone();

        let mut streams: HashMap<String, Box<dyn Stream>> = HashMap::new();
        streams.insert("state".to_string(), Box::new(state_stream));

        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            id: config.id,
            type_name: config.type_name,
            schema: flow_schema,
            document,
            policy,
            host_assignment: Arc::new(std::sync::RwLock::new(HostAssignment::new())),
            device_uuid,
            device_id,
            messenger: None,
            topology: None,
            pending_edits: Arc::new(std::sync::RwLock::new(Vec::new())),
            streams,
            shutdown_tx,
            state_stream: state_stream_notify,
            storage_path: None,
            known_parties: Arc::new(std::sync::RwLock::new(HashMap::new())),
            flush_notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Construct with ensemble dependencies already available.
    pub fn new_with_ensemble(
        config: FlowConfig,
        doc_schema: S,
        policy: DripHostPolicy,
        device_uuid: Uuid,
        device_id: DeviceId,
        messenger: Arc<TopologyMessenger>,
        topology: Arc<tokio::sync::RwLock<EnsembleTopology>>,
    ) -> Self {
        let mut flow = Self::new(config, doc_schema, policy, device_uuid, device_id);
        flow.messenger = Some(messenger);
        flow.topology = Some(topology);
        flow
    }

    /// Construct in persistent mode.
    ///
    /// Loads per-party journals from `storage_path/journals/*.jsonl` on startup.
    /// Every subsequent local edit (`apply_edit`) and every remote op applied
    /// by the sync task is appended to the appropriate journal immediately — no explicit flush
    /// needed. Process restart is safe: the log is replayed and deduplicated.
    ///
    /// This is the memorization role from the Self-Data Flow design: persistence
    /// lives inside the flow layer, not in application-specific wrappers.
    pub fn new_persistent(
        config: FlowConfig,
        doc_schema: S,
        policy: DripHostPolicy,
        device_uuid: Uuid,
        device_id: DeviceId,
        storage_path: PathBuf,
    ) -> Self {
        let mut flow = Self::new(config, doc_schema, policy, device_uuid, device_id);
        flow.storage_path = Some(storage_path);
        flow.load_from_disk();
        flow
    }

    /// Load all per-party journals from disk into the in-memory document.
    ///
    /// Reads `journals/*.jsonl` and applies all ops via `apply_remote`
    /// (idempotent — duplicates are harmless). Also handles migration from
    /// the legacy single `operations.jsonl` format.
    fn load_from_disk(&mut self) {
        let Some(ref path) = self.storage_path else {
            return;
        };

        // Load known parties
        let parties = load_known_parties(path);
        if let Ok(mut kp) = self.known_parties.write() {
            *kp = parties;
        }

        // Migrate legacy operations.jsonl if it exists
        let legacy_ops = migrate_legacy_ops(path);
        if !legacy_ops.is_empty() {
            // Write each legacy op into its author's journal
            for env in &legacy_ops {
                append_to_journal(path, &env.author, env);
            }
            // Remove the legacy file now that we've migrated
            let legacy_path = path.join("operations.jsonl");
            let _ = std::fs::remove_file(&legacy_path);
            eprintln!("[full_replica] legacy migration complete, removed operations.jsonl");
        }

        // Read all per-party journal files
        let journals_dir = path.join("journals");
        if !journals_dir.exists() {
            return;
        }
        let entries = match std::fs::read_dir(&journals_dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("[full_replica] load_from_disk: read_dir {:?} failed: {}", journals_dir, e);
                return;
            }
        };

        if let Ok(mut doc) = self.document.write() {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let content = match std::fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[full_replica] load_from_disk: read {:?} failed: {}", file_path, e);
                        continue;
                    }
                };
                for line in content.lines() {
                    if let Ok(env) = serde_json::from_str::<OpEnvelope>(line) {
                        doc.apply_remote(env);
                    }
                }
            }
        }
    }

    /// Register a device as a known party to this flow.
    ///
    /// `party_device_id` is the DeviceId string used as the op author in
    /// the convergent document. `party_uuid` is the Uuid used for routing
    /// messages via the topology messenger.
    ///
    /// When a new party is registered, all existing local ops are queued
    /// for delivery to them. The party set is persisted to `parties.json`.
    pub fn register_party(&self, party_device_id: &str, party_uuid: Uuid) {
        // Don't register ourselves
        if party_device_id == self.device_id.as_str() {
            return;
        }

        let is_new = if let Ok(mut parties) = self.known_parties.write() {
            if parties.contains_key(party_device_id) {
                false
            } else {
                parties.insert(party_device_id.to_string(), party_uuid);
                true
            }
        } else {
            return;
        };

        if !is_new {
            return; // Already known
        }

        // Persist updated party set
        if let Some(ref path) = self.storage_path {
            if let Ok(parties) = self.known_parties.read() {
                save_known_parties(path, &parties);
            }

            // Queue all existing local ops for this new peer
            let my_journal = path.join("journals").join(
                format!("{}.jsonl", device_id_to_filename(&self.device_id))
            );
            if my_journal.exists() {
                if let Ok(content) = std::fs::read_to_string(&my_journal) {
                    for line in content.lines() {
                        if let Ok(env) = serde_json::from_str::<OpEnvelope>(line) {
                            enqueue_for_peer(path, party_device_id, &env);
                        }
                    }
                }
            }
        }
    }

    /// Read the outbound queue for a specific peer without clearing it.
    fn peek_outbound(&self, peer_device_id: &str) -> Vec<OpEnvelope> {
        let Some(ref path) = self.storage_path else {
            return Vec::new();
        };
        let filename = format!("{}.jsonl", device_id_to_filename(peer_device_id));
        let queue_path = path.join("outbound").join(filename);
        if !queue_path.exists() {
            return Vec::new();
        }
        let content = match std::fs::read_to_string(&queue_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        content.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    /// Clear the outbound queue for a specific peer (called after confirmed delivery).
    fn clear_outbound(&self, peer_device_id: &str) {
        let Some(ref path) = self.storage_path else {
            return;
        };
        let filename = format!("{}.jsonl", device_id_to_filename(peer_device_id));
        let queue_path = path.join("outbound").join(filename);
        let _ = std::fs::write(&queue_path, "");
    }

    /// Drain the outbound queue for a specific peer.
    ///
    /// Returns all queued ops and clears the queue file. The caller is
    /// responsible for actually delivering them.
    pub fn drain_outbound(&self, peer_device_id: &str) -> Vec<OpEnvelope> {
        let ops = self.peek_outbound(peer_device_id);
        if !ops.is_empty() {
            self.clear_outbound(peer_device_id);
        }
        ops
    }

    /// Get the known parties mapping (DeviceId → Uuid).
    pub fn known_parties(&self) -> HashMap<String, Uuid> {
        self.known_parties.read()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Attempt to deliver all pending outbound ops to connected peers.
    ///
    /// For each known party with queued ops, reads the queue, sends an
    /// `OperationBatch` via the messenger, and **only clears the queue if
    /// the send succeeds**. If the peer is unreachable, ops stay in the
    /// queue for the next flush attempt.
    pub async fn flush_outbound(&self) -> HashMap<String, usize> {
        let messenger = match &self.messenger {
            Some(m) => Arc::clone(m),
            None => return HashMap::new(),
        };

        let parties = self.known_parties();
        let mut results = HashMap::new();

        for (device_id, peer_uuid) in &parties {
            let ops = self.peek_outbound(device_id);
            if ops.is_empty() {
                continue;
            }
            let count = ops.len();
            let msg = FlowSyncMessage::OperationBatch {
                flow_id: self.id,
                ops,
            };
            if let Ok(payload) = msg.to_cbor() {
                match messenger.send_to(*peer_uuid, MessageType::FlowSync, &payload).await {
                    Ok(()) => {
                        // Send confirmed — now clear the queue.
                        self.clear_outbound(device_id);
                        results.insert(device_id.clone(), count);
                    }
                    Err(_) => {
                        // Send failed — leave queue intact for next attempt.
                    }
                }
            }
        }

        results
    }

    /// Signal that the outbound queues should be flushed.
    /// Safe to call from sync or async contexts. Multiple rapid signals
    /// coalesce into a single flush.
    pub fn signal_flush(&self) {
        self.flush_notify.notify_one();
    }

    /// Start the background sync tasks:
    ///
    /// 1. **Inbound listener**: handles `HorizonExchange`, `OperationBatch`,
    ///    and `HostAnnouncement` messages from peers.
    /// 2. **Flush task**: drains outbound queues whenever signaled (by local
    ///    edits, inbound messages, new peer connections, or startup). Only
    ///    clears a peer's queue after confirmed delivery.
    /// 3. **Topology watcher**: sends `HorizonExchange` to newly-connected
    ///    peers so both sides catch up.
    pub fn start(&self) -> Result<(), FlowError> {
        let messenger = self
            .messenger
            .as_ref()
            .ok_or_else(|| FlowError::SyncError("no messenger configured".into()))?;

        let flush_notify = Arc::clone(&self.flush_notify);

        // --- Task 1: Inbound message listener ---
        {
            let mut rx = messenger.incoming();
            let flow_id = self.id;
            let document = Arc::clone(&self.document);
            let host_assignment = Arc::clone(&self.host_assignment);
            let messenger_task = Arc::clone(messenger);
            let state_stream = self.state_stream.clone();
            let storage_path = self.storage_path.clone();
            let flush_notify = Arc::clone(&flush_notify);
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        result = rx.recv() => {
                            let envelope = match result {
                                Ok(e) => e,
                                Err(_) => continue,
                            };
                            if envelope.message_type != MessageType::FlowSync {
                                continue;
                            }
                            let msg = match FlowSyncMessage::from_cbor(&envelope.payload) {
                                Ok(m) => m,
                                Err(_) => continue,
                            };
                            let msg_flow_id = match &msg {
                                FlowSyncMessage::HorizonExchange { flow_id, .. } => *flow_id,
                                FlowSyncMessage::OperationBatch { flow_id, .. } => *flow_id,
                                FlowSyncMessage::HostAnnouncement { flow_id, .. } => *flow_id,
                            };
                            if msg_flow_id != flow_id {
                                continue;
                            }
                            match msg {
                                FlowSyncMessage::HorizonExchange { flow_id: fid, horizon: peer_horizon } => {
                                    // Compute the ops this peer hasn't seen and send them back.
                                    let ops_to_send: Vec<OpEnvelope> = match document.read() {
                                        Ok(doc) => doc.operations_since(&peer_horizon)
                                            .into_iter().cloned().collect(),
                                        Err(_) => Vec::new(),
                                    };
                                    if !ops_to_send.is_empty() {
                                        let response = FlowSyncMessage::OperationBatch {
                                            flow_id: fid,
                                            ops: ops_to_send,
                                        };
                                        if let Ok(payload) = response.to_cbor() {
                                            let m = Arc::clone(&messenger_task);
                                            let source = envelope.source;
                                            tokio::spawn(async move {
                                                let _ = m.send_to(source, MessageType::FlowSync, &payload).await;
                                            });
                                        }
                                    }
                                    // A peer just connected and exchanged horizons —
                                    // good time to flush our queues too.
                                    flush_notify.notify_one();
                                }
                                FlowSyncMessage::OperationBatch { ops, .. } => {
                                    // Apply ops; collect only the ones that were genuinely new.
                                    let applied: Vec<OpEnvelope> = match document.write() {
                                        Ok(mut doc) => ops.into_iter()
                                            .filter_map(|op| {
                                                if doc.apply_remote(op.clone()) { Some(op) } else { None }
                                            })
                                            .collect(),
                                        Err(_) => Vec::new(),
                                    };
                                    if !applied.is_empty() {
                                        // Persist each op to the author's journal.
                                        if let Some(ref path) = storage_path {
                                            for op in &applied {
                                                append_to_journal(path, &op.author, op);
                                            }
                                        }
                                        // Notify UI subscribers so polling consumers see the new state.
                                        state_stream.notify_subscribers();
                                        // Receiving ops means a peer is online — flush our queues.
                                        flush_notify.notify_one();
                                    }
                                }
                                FlowSyncMessage::HostAnnouncement { host_id, epoch, .. } => {
                                    if let Ok(mut ha) = host_assignment.write() {
                                        ha.accept_announcement(host_id, epoch);
                                    }
                                }
                            }
                        }
                        _ = shutdown_rx.recv() => {
                            break;
                        }
                    }
                }
            });
        }

        // --- Task 2: Event-driven outbound flush ---
        //
        // Waits for flush_notify signals (from local edits, inbound messages,
        // new peer connections) and attempts to deliver queued ops to all peers.
        // Only clears a peer's queue after the send is confirmed.
        {
            let known_parties = Arc::clone(&self.known_parties);
            let storage_path = self.storage_path.clone();
            let messenger_flush = Arc::clone(messenger);
            let flow_id = self.id;
            let flush_notify = Arc::clone(&flush_notify);
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = flush_notify.notified() => {
                            // Small delay to coalesce rapid signals (e.g. multiple
                            // apply_edit calls in quick succession).
                            tokio::time::sleep(Duration::from_millis(50)).await;

                            let parties: HashMap<String, Uuid> = known_parties
                                .read()
                                .map(|p| p.clone())
                                .unwrap_or_default();

                            let Some(ref path) = storage_path else { continue };

                            for (device_id, peer_uuid) in &parties {
                                let filename = format!("{}.jsonl", device_id_to_filename(device_id));
                                let queue_path = path.join("outbound").join(&filename);
                                if !queue_path.exists() {
                                    continue;
                                }
                                let content = match std::fs::read_to_string(&queue_path) {
                                    Ok(c) if !c.is_empty() => c,
                                    _ => continue,
                                };
                                let ops: Vec<OpEnvelope> = content.lines()
                                    .filter_map(|line| serde_json::from_str(line).ok())
                                    .collect();
                                if ops.is_empty() {
                                    continue;
                                }
                                let ops_count = ops.len();
                                let msg = FlowSyncMessage::OperationBatch {
                                    flow_id,
                                    ops,
                                };
                                if let Ok(payload) = msg.to_cbor() {
                                    match messenger_flush.send_to(*peer_uuid, MessageType::FlowSync, &payload).await {
                                        Ok(()) => {
                                            // Confirmed delivery — clear the queue.
                                            let _ = std::fs::write(&queue_path, "");
                                        }
                                        Err(e) => {
                                            // Send failed — leave queue intact for retry.
                                            eprintln!("[full_replica] flush to {} failed: {} — {} ops remain queued", device_id, e, ops_count);
                                        }
                                    }
                                }
                            }
                        }
                        _ = shutdown_rx.recv() => break,
                    }
                }
            });
        }

        // --- Task 3: Topology watcher ---
        //
        // When a new peer becomes directly reachable, send our horizon so they
        // respond with the ops we're missing. Both sides do this, so the
        // exchange is symmetric. Also signals a flush so queued ops get sent.
        if let Some(ref topology) = self.topology {
            let topology = Arc::clone(topology);
            let document = Arc::clone(&self.document);
            let messenger_watch = Arc::clone(messenger);
            let flow_id = self.id;
            let device_uuid = self.device_uuid;
            let flush_notify = Arc::clone(&flush_notify);
            let mut shutdown_rx2 = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                let mut synced_peers: HashSet<Uuid> = HashSet::new();
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {
                            let new_peers: Vec<Uuid> = {
                                let topo = topology.read().await;
                                topo.online_pieces.iter()
                                    .filter(|(id, p)| {
                                        **id != device_uuid
                                            && matches!(p.reachability, PieceReachability::Direct)
                                            && !synced_peers.contains(id)
                                    })
                                    .map(|(id, _)| *id)
                                    .collect()
                            };
                            for peer_id in new_peers {
                                let horizon = match document.read() {
                                    Ok(doc) => doc.horizon().clone(),
                                    Err(_) => continue,
                                };
                                let msg = FlowSyncMessage::HorizonExchange {
                                    flow_id,
                                    horizon,
                                };
                                if let Ok(payload) = msg.to_cbor() {
                                    match messenger_watch.send_to(peer_id, MessageType::FlowSync, &payload).await {
                                        Ok(()) => {
                                            synced_peers.insert(peer_id);
                                            // New peer connected — flush queued ops.
                                            flush_notify.notify_one();
                                        }
                                        Err(e) => {
                                            eprintln!("[full_replica] horizon exchange with {} failed: {}", peer_id, e);
                                        }
                                    }
                                }
                            }
                        }
                        _ = shutdown_rx2.recv() => break,
                    }
                }
            });
        }

        // Signal initial flush so any ops queued before startup get sent.
        flush_notify.notify_one();

        Ok(())
    }

    /// Stop the sync listener.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Apply a local edit. Behavior depends on failover policy:
    /// - `OfflineMerge`: apply locally and broadcast to peers.
    /// - `WaitForHost`: queue if not host; apply + broadcast if host.
    /// - `ImmediateFailover` / `GracefulFailover`: apply locally, broadcast.
    pub fn apply_edit(&self, op: Operation) -> Result<OpEnvelope, FlowError> {
        let is_host = self.is_current_host();

        match &self.policy.failover {
            HostFailoverPolicy::WaitForHost if !is_host => {
                // Queue the edit for when we become host or host reconnects
                if let Ok(mut pending) = self.pending_edits.write() {
                    pending.push(op);
                }
                return Err(FlowError::HostUnavailable(
                    self.host_assignment
                        .read()
                        .ok()
                        .and_then(|h| h.current_host)
                        .unwrap_or(Uuid::nil()),
                ));
            }
            _ => {
                // OfflineMerge, ImmediateFailover, GracefulFailover: apply locally
                let envelope = {
                    let mut doc = self.document.write().map_err(|e| {
                        FlowError::SyncError(format!("document lock poisoned: {e}"))
                    })?;
                    doc.apply_local(op)
                };

                // Persist to this device's journal (our own party's file).
                if let Some(ref path) = self.storage_path {
                    append_to_journal(path, &self.device_id, &envelope);
                    // Queue for all known peers.
                    if let Ok(parties) = self.known_parties.read() {
                        for peer_id in parties.keys() {
                            enqueue_for_peer(path, peer_id, &envelope);
                        }
                    }
                }

                // Best-effort immediate broadcast to all connected peers.
                // This is the fast path — ops arrive instantly when peers are online.
                if let Some(messenger) = &self.messenger {
                    let msg = FlowSyncMessage::OperationBatch {
                        flow_id: self.id,
                        ops: vec![envelope.clone()],
                    };
                    if let Ok(payload) = msg.to_cbor() {
                        let messenger = Arc::clone(messenger);
                        tokio::spawn(async move {
                            let _ = messenger.broadcast(MessageType::FlowSync, &payload).await;
                        });
                    }
                }

                // Also signal the flush task to attempt confirmed delivery
                // from the outbound queue. This is the reliable path — ops
                // stay in the queue until delivery is confirmed, catching
                // anything the broadcast missed (peer offline, routing issues).
                self.flush_notify.notify_one();

                Ok(envelope)
            }
        }
    }

    /// Apply a remote operation from an external caller (manual sync, FFI).
    ///
    /// Applies the op to the in-memory document, persists it if a storage path
    /// is configured, and notifies stream subscribers. Returns `true` if the op
    /// was new (not a duplicate).
    ///
    /// The background `start()` task uses an equivalent path internally for ops
    /// that arrive over the network; this method is for callers that bypass the
    /// task (e.g., the `soradyne_flow_apply_remote` FFI function).
    pub fn apply_remote_op(&self, envelope: OpEnvelope) -> bool {
        let is_new = match self.document.write() {
            Ok(mut doc) => doc.apply_remote(envelope.clone()),
            Err(_) => return false,
        };
        if is_new {
            // Persist to the author's journal (not our own — we didn't write this op).
            if let Some(ref path) = self.storage_path {
                append_to_journal(path, &envelope.author, &envelope);
            }
            self.state_stream.notify_subscribers();
        }
        is_new
    }

    /// Evaluate which piece should host this flow based on the policy and
    /// current topology.
    pub fn evaluate_host_assignment(
        &self,
        piece_capabilities: &HashMap<Uuid, PieceCapabilities>,
    ) -> Option<Uuid> {
        match &self.policy.selection {
            HostSelectionStrategy::FirstEligible => piece_capabilities
                .iter()
                .find(|(_, caps)| caps.can_host_drip)
                .map(|(id, _)| *id),

            HostSelectionStrategy::BestConnected => {
                // Pick the piece with the most topology edges (proxy for connectivity)
                let topo = self.topology.as_ref()?;
                let topo = topo.try_read().ok()?;
                let mut best: Option<(Uuid, usize)> = None;
                for (id, caps) in piece_capabilities {
                    if !caps.can_host_drip {
                        continue;
                    }
                    let edge_count = topo.edges_from(id).len() + topo.edges_to(id).len();
                    if best.is_none() || edge_count > best.unwrap().1 {
                        best = Some((*id, edge_count));
                    }
                }
                best.map(|(id, _)| id)
            }

            HostSelectionStrategy::Preferred { device_id } => {
                // Preferred device if it's eligible; otherwise first eligible
                if piece_capabilities
                    .get(device_id)
                    .map(|c| c.can_host_drip)
                    .unwrap_or(false)
                {
                    Some(*device_id)
                } else {
                    piece_capabilities
                        .iter()
                        .find(|(_, caps)| caps.can_host_drip)
                        .map(|(id, _)| *id)
                }
            }

            HostSelectionStrategy::Scored(weights) => {
                let topo = self.topology.as_ref()?;
                let topo = topo.try_read().ok()?;
                let mut best: Option<(Uuid, f64)> = None;
                for (id, caps) in piece_capabilities {
                    if !caps.can_host_drip {
                        continue;
                    }
                    let connectivity =
                        (topo.edges_from(id).len() + topo.edges_to(id).len()) as f64;
                    let storage = (caps.storage_bytes as f64) / 1_000_000_000.0; // normalize to GB
                    let battery = if caps.battery_aware { 1.0 } else { 0.0 };
                    let ui = if caps.has_ui { 1.0 } else { 0.0 };

                    let score = connectivity * weights.connectivity
                        + storage * weights.storage
                        + battery * weights.battery_stability
                        + ui * weights.has_ui;

                    if best.is_none() || score > best.unwrap().1 {
                        best = Some((*id, score));
                    }
                }
                best.map(|(id, _)| id)
            }
        }
    }

    /// Handle the current host dropping from the ensemble.
    pub fn handle_host_dropout(&self) {
        match &self.policy.failover {
            HostFailoverPolicy::ImmediateFailover => {
                // Clear host; callers should re-evaluate
                if let Ok(mut ha) = self.host_assignment.write() {
                    ha.current_host = None;
                }
            }
            HostFailoverPolicy::GracefulFailover { .. } => {
                // Mark host as potentially timed out; the sync loop checks timeout
                // The grace period is enforced by is_host_timed_out()
            }
            HostFailoverPolicy::OfflineMerge => {
                // Nothing special — local edits keep flowing via CRDT
            }
            HostFailoverPolicy::WaitForHost => {
                // Edits are already queued by apply_edit
            }
        }
    }

    /// Claim host role for this device.
    pub fn become_host(&self) -> Result<(), FlowError> {
        let new_epoch = {
            let mut ha = self.host_assignment.write().map_err(|e| {
                FlowError::SyncError(format!("host assignment lock poisoned: {e}"))
            })?;
            ha.epoch += 1;
            ha.current_host = Some(self.device_uuid);
            ha.last_host_seen = Some(Instant::now());
            ha.epoch
        };

        // Drain and apply pending edits, persist, and broadcast them.
        let mut envelopes = Vec::new();
        if let Ok(mut pending) = self.pending_edits.write() {
            let ops: Vec<Operation> = pending.drain(..).collect();
            if !ops.is_empty() {
                if let Ok(mut doc) = self.document.write() {
                    for op in ops {
                        envelopes.push(doc.apply_local(op));
                    }
                }
            }
        }
        if !envelopes.is_empty() {
            if let Some(ref path) = self.storage_path {
                for env in &envelopes {
                    append_to_journal(path, &self.device_id, env);
                    if let Ok(parties) = self.known_parties.read() {
                        for peer_id in parties.keys() {
                            enqueue_for_peer(path, peer_id, env);
                        }
                    }
                }
            }
        }

        // Broadcast host announcement + any pending edits that were just applied.
        if let Some(messenger) = &self.messenger {
            let msg = FlowSyncMessage::HostAnnouncement {
                flow_id: self.id,
                host_id: self.device_uuid,
                epoch: new_epoch,
            };
            if let Ok(payload) = msg.to_cbor() {
                let messenger = Arc::clone(messenger);
                let ops_msg = if envelopes.is_empty() {
                    None
                } else {
                    FlowSyncMessage::OperationBatch {
                        flow_id: self.id,
                        ops: envelopes,
                    }
                    .to_cbor()
                    .ok()
                };
                tokio::spawn(async move {
                    let _ = messenger.broadcast(MessageType::FlowSync, &payload).await;
                    if let Some(ops_payload) = ops_msg {
                        let _ = messenger
                            .broadcast(MessageType::FlowSync, &ops_payload)
                            .await;
                    }
                });
            }
        }

        Ok(())
    }

    /// Hand off hosting to another piece.
    pub fn handoff_host(&self, new_host: Uuid) -> Result<(), FlowError> {
        if !self.is_current_host() {
            return Err(FlowError::NotHost(self.device_uuid));
        }

        let new_epoch = {
            let mut ha = self.host_assignment.write().map_err(|e| {
                FlowError::SyncError(format!("host assignment lock poisoned: {e}"))
            })?;
            ha.epoch += 1;
            ha.current_host = Some(new_host);
            ha.last_host_seen = Some(Instant::now());
            ha.epoch
        };

        // Broadcast announcement of the new host
        if let Some(messenger) = &self.messenger {
            let msg = FlowSyncMessage::HostAnnouncement {
                flow_id: self.id,
                host_id: new_host,
                epoch: new_epoch,
            };
            if let Ok(payload) = msg.to_cbor() {
                let messenger = Arc::clone(messenger);
                tokio::spawn(async move {
                    let _ = messenger.broadcast(MessageType::FlowSync, &payload).await;
                });
            }
        }

        Ok(())
    }

    /// Initiate sync with a specific peer: send our horizon so they can
    /// compute what ops we're missing.
    pub fn sync_with_peer(&self, peer_id: Uuid) -> Result<(), FlowError> {
        let messenger = self
            .messenger
            .as_ref()
            .ok_or_else(|| FlowError::SyncError("no messenger configured".into()))?;

        let horizon = {
            let doc = self.document.read().map_err(|e| {
                FlowError::SyncError(format!("document lock poisoned: {e}"))
            })?;
            doc.horizon().clone()
        };

        let msg = FlowSyncMessage::HorizonExchange {
            flow_id: self.id,
            horizon,
        };
        let payload = msg.to_cbor()?;
        let messenger = Arc::clone(messenger);
        tokio::spawn(async move {
            let _ = messenger
                .send_to(peer_id, MessageType::FlowSync, &payload)
                .await;
        });

        Ok(())
    }

    /// Process an incoming FlowSyncMessage (direct call, not via the background task).
    pub fn handle_flow_sync(&self, _source: Uuid, msg: FlowSyncMessage) {
        let applied = Self::handle_flow_sync_static(
            &self.document,
            &self.host_assignment,
            msg,
        );
        if !applied.is_empty() {
            if let Some(ref path) = self.storage_path {
                for op in &applied {
                    append_to_journal(path, &op.author, op);
                }
            }
            self.state_stream.notify_subscribers();
        }
    }

    /// Static handler for direct (non-task) calls via `handle_flow_sync`.
    /// Returns the `OpEnvelope`s that were genuinely new so the caller can
    /// persist them and notify subscribers.
    fn handle_flow_sync_static(
        document: &Arc<std::sync::RwLock<ConvergentDocument<S>>>,
        host_assignment: &Arc<std::sync::RwLock<HostAssignment>>,
        msg: FlowSyncMessage,
    ) -> Vec<OpEnvelope> {
        match msg {
            FlowSyncMessage::HorizonExchange { .. } => {
                // HorizonExchange responses require the messenger; callers with
                // &self access (handle_flow_sync) handle this separately.
                Vec::new()
            }
            FlowSyncMessage::OperationBatch { ops, .. } => {
                match document.write() {
                    Ok(mut doc) => ops.into_iter()
                        .filter_map(|op| {
                            if doc.apply_remote(op.clone()) { Some(op) } else { None }
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                }
            }
            FlowSyncMessage::HostAnnouncement { host_id, epoch, .. } => {
                if let Ok(mut ha) = host_assignment.write() {
                    ha.accept_announcement(host_id, epoch);
                }
                Vec::new()
            }
        }
    }

    /// Check if this device is currently the host.
    pub fn is_current_host(&self) -> bool {
        self.host_assignment
            .read()
            .ok()
            .and_then(|ha| ha.current_host)
            .map(|h| h == self.device_uuid)
            .unwrap_or(false)
    }

    /// Get the document (for direct access in tests or FFI).
    pub fn document(&self) -> &Arc<std::sync::RwLock<ConvergentDocument<S>>> {
        &self.document
    }

    /// Get the host assignment state.
    pub fn host_assignment(&self) -> &Arc<std::sync::RwLock<HostAssignment>> {
        &self.host_assignment
    }
}

// ---------------------------------------------------------------------------
// Flow trait implementation
// ---------------------------------------------------------------------------

impl<S: DocumentSchema + 'static> Flow for FullReplicaFlow<S> {
    fn id(&self) -> Uuid {
        self.id
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn schema(&self) -> &FlowSchema {
        &self.schema
    }

    fn stream(&self, name: &str) -> Option<&dyn Stream> {
        self.streams.get(name).map(|s| s.as_ref())
    }

    fn stream_mut(&mut self, name: &str) -> Option<&mut Box<dyn Stream>> {
        self.streams.get_mut(name)
    }

    fn register_stream(&mut self, stream: Box<dyn Stream>) -> Result<(), FlowError> {
        let name = stream.name().to_string();
        if !self.schema.streams.iter().any(|s| s.name == name) {
            return Err(FlowError::InvalidStreamName(name));
        }
        self.streams.insert(name, stream);
        Ok(())
    }

    fn stream_names(&self) -> Vec<String> {
        self.streams.keys().cloned().collect()
    }

    fn set_ensemble(
        &mut self,
        messenger: Arc<TopologyMessenger>,
        topology: Arc<tokio::sync::RwLock<EnsembleTopology>>,
    ) {
        self.messenger = Some(messenger);
        self.topology = Some(topology);
    }
}

// ---------------------------------------------------------------------------
// FlowRegistry constructors
// ---------------------------------------------------------------------------

fn construct_giantt_drip_hosted(config: FlowConfig) -> Result<Box<dyn Flow>, FlowError> {
    let policy: DripHostPolicy = serde_json::from_value(
        config
            .params
            .get("policy")
            .cloned()
            .unwrap_or(serde_json::json!({})),
    )
    .unwrap_or_default();

    let device_uuid: Uuid = serde_json::from_value(
        config
            .params
            .get("device_uuid")
            .cloned()
            .unwrap_or(serde_json::json!(null)),
    )
    .unwrap_or_else(|_| Uuid::new_v4());

    let device_id: String = config
        .params
        .get("device_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let flow = FullReplicaFlow::new(config, GianttSchema, policy, device_uuid, device_id);
    Ok(Box::new(flow))
}

fn construct_inventory_drip_hosted(config: FlowConfig) -> Result<Box<dyn Flow>, FlowError> {
    let policy: DripHostPolicy = serde_json::from_value(
        config
            .params
            .get("policy")
            .cloned()
            .unwrap_or(serde_json::json!({})),
    )
    .unwrap_or_default();

    let device_uuid: Uuid = serde_json::from_value(
        config
            .params
            .get("device_uuid")
            .cloned()
            .unwrap_or(serde_json::json!(null)),
    )
    .unwrap_or_else(|_| Uuid::new_v4());

    let device_id: String = config
        .params
        .get("device_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let flow = FullReplicaFlow::new(config, InventorySchema, policy, device_uuid, device_id);
    Ok(Box::new(flow))
}

/// Register FullReplicaFlow constructors for all known schemas.
pub fn register_drip_hosted_flows(registry: &mut FlowRegistry) {
    registry.register("drip_hosted:giantt", construct_giantt_drip_hosted);
    registry.register("drip_hosted:inventory", construct_inventory_drip_hosted);
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convergent::{DocumentSchema, DocumentState, Horizon, OpEnvelope, Operation, Value};
    use crate::topology::capsule::PieceCapabilities;
    use std::collections::{HashMap, HashSet};

    // Minimal schema for testing
    #[derive(Clone)]
    struct TestSchema;

    impl DocumentSchema for TestSchema {
        type State = DocumentState;

        fn item_type_spec(
            &self,
            _: &str,
        ) -> Option<Box<dyn crate::convergent::ItemTypeSpec>> {
            None
        }

        fn item_types(&self) -> HashSet<String> {
            HashSet::from(["Test".to_string()])
        }

        fn validate(
            &self,
            _: &Self::State,
        ) -> Vec<crate::convergent::ValidationIssue> {
            Vec::new()
        }
    }

    fn make_config(type_name: &str) -> FlowConfig {
        FlowConfig {
            id: Uuid::new_v4(),
            type_name: type_name.to_string(),
            params: serde_json::json!({}),
        }
    }

    fn make_test_flow(policy: DripHostPolicy) -> DripHostedFlow<TestSchema> {
        let device_uuid = Uuid::new_v4();
        DripHostedFlow::new(
            make_config("drip_hosted:test"),
            TestSchema,
            policy,
            device_uuid,
            "device_A".to_string(),
        )
    }

    fn full_capabilities() -> PieceCapabilities {
        PieceCapabilities {
            can_host_drip: true,
            can_memorize: true,
            can_route: true,
            has_ui: true,
            storage_bytes: 100_000_000_000,
            battery_aware: true,
        }
    }

    fn accessory_capabilities() -> PieceCapabilities {
        PieceCapabilities {
            can_host_drip: false,
            can_memorize: true,
            can_route: true,
            has_ui: false,
            storage_bytes: 1_000_000,
            battery_aware: false,
        }
    }

    // -----------------------------------------------------------------------
    // Host selection strategy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_host_selection_first_eligible() {
        let flow = make_test_flow(DripHostPolicy {
            selection: HostSelectionStrategy::FirstEligible,
            ..Default::default()
        });

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let mut caps = HashMap::new();
        caps.insert(id_a, accessory_capabilities()); // can't host
        caps.insert(id_b, full_capabilities()); // can host

        let host = flow.evaluate_host_assignment(&caps);
        assert_eq!(host, Some(id_b));
    }

    #[test]
    fn test_host_selection_first_eligible_none() {
        let flow = make_test_flow(DripHostPolicy {
            selection: HostSelectionStrategy::FirstEligible,
            ..Default::default()
        });

        let mut caps = HashMap::new();
        caps.insert(Uuid::new_v4(), accessory_capabilities());

        assert!(flow.evaluate_host_assignment(&caps).is_none());
    }

    #[test]
    fn test_host_selection_preferred() {
        let preferred = Uuid::new_v4();
        let flow = make_test_flow(DripHostPolicy {
            selection: HostSelectionStrategy::Preferred {
                device_id: preferred,
            },
            ..Default::default()
        });

        let other = Uuid::new_v4();
        let mut caps = HashMap::new();
        caps.insert(preferred, full_capabilities());
        caps.insert(other, full_capabilities());

        assert_eq!(flow.evaluate_host_assignment(&caps), Some(preferred));
    }

    #[test]
    fn test_host_selection_preferred_fallback() {
        let preferred = Uuid::new_v4();
        let flow = make_test_flow(DripHostPolicy {
            selection: HostSelectionStrategy::Preferred {
                device_id: preferred,
            },
            ..Default::default()
        });

        // Preferred not available
        let other = Uuid::new_v4();
        let mut caps = HashMap::new();
        caps.insert(other, full_capabilities());

        // Falls back to first eligible
        assert_eq!(flow.evaluate_host_assignment(&caps), Some(other));
    }

    #[test]
    fn test_host_selection_scored() {
        use crate::topology::ensemble::EnsembleTopology;

        let device_uuid = Uuid::new_v4();
        let mut flow = DripHostedFlow::new(
            make_config("drip_hosted:test"),
            TestSchema,
            DripHostPolicy {
                selection: HostSelectionStrategy::Scored(HostScoreWeights {
                    connectivity: 0.0,
                    storage: 0.0,
                    battery_stability: 0.0,
                    has_ui: 10.0, // only UI matters
                }),
                ..Default::default()
            },
            device_uuid,
            "device_A".to_string(),
        );
        // Scored needs a topology reference (even if connectivity weight is 0)
        flow.topology = Some(Arc::new(tokio::sync::RwLock::new(EnsembleTopology::new())));

        let id_no_ui = Uuid::new_v4();
        let id_ui = Uuid::new_v4();
        let mut caps = HashMap::new();
        caps.insert(
            id_no_ui,
            PieceCapabilities {
                can_host_drip: true,
                has_ui: false,
                ..full_capabilities()
            },
        );
        caps.insert(id_ui, full_capabilities()); // has_ui = true

        assert_eq!(flow.evaluate_host_assignment(&caps), Some(id_ui));
    }

    // -----------------------------------------------------------------------
    // HostAssignment epoch tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_host_assignment_accept_higher_epoch() {
        let mut ha = HostAssignment::new();
        let host_a = Uuid::new_v4();
        let host_b = Uuid::new_v4();

        assert!(ha.accept_announcement(host_a, 1));
        assert_eq!(ha.current_host, Some(host_a));
        assert_eq!(ha.epoch, 1);

        // Higher epoch accepted
        assert!(ha.accept_announcement(host_b, 2));
        assert_eq!(ha.current_host, Some(host_b));
        assert_eq!(ha.epoch, 2);
    }

    #[test]
    fn test_host_assignment_reject_lower_epoch() {
        let mut ha = HostAssignment::new();
        let host_a = Uuid::new_v4();
        let host_b = Uuid::new_v4();

        ha.accept_announcement(host_a, 5);

        // Lower epoch rejected
        assert!(!ha.accept_announcement(host_b, 3));
        assert_eq!(ha.current_host, Some(host_a));
        assert_eq!(ha.epoch, 5);
    }

    #[test]
    fn test_host_assignment_reject_equal_epoch() {
        let mut ha = HostAssignment::new();
        let host_a = Uuid::new_v4();
        let host_b = Uuid::new_v4();

        ha.accept_announcement(host_a, 5);

        // Equal epoch rejected (must be strictly greater)
        assert!(!ha.accept_announcement(host_b, 5));
        assert_eq!(ha.current_host, Some(host_a));
    }

    #[test]
    fn test_host_assignment_timeout() {
        let mut ha = HostAssignment::new();
        ha.accept_announcement(Uuid::new_v4(), 1);

        // Just accepted, shouldn't be timed out
        assert!(!ha.is_host_timed_out(Duration::from_secs(10)));

        // With zero timeout, should be timed out
        assert!(ha.is_host_timed_out(Duration::ZERO));
    }

    // -----------------------------------------------------------------------
    // FlowSyncMessage CBOR round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_flow_sync_message_cbor_horizon_exchange() {
        let mut horizon = Horizon::new();
        horizon.observe(&"dev_A".into(), 5);
        horizon.observe(&"dev_B".into(), 3);

        let msg = FlowSyncMessage::HorizonExchange {
            flow_id: Uuid::new_v4(),
            horizon: horizon.clone(),
        };
        let bytes = msg.to_cbor().unwrap();
        let decoded = FlowSyncMessage::from_cbor(&bytes).unwrap();

        if let FlowSyncMessage::HorizonExchange {
            horizon: h,
            ..
        } = decoded
        {
            assert_eq!(h.get(&"dev_A".into()), 5);
            assert_eq!(h.get(&"dev_B".into()), 3);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_flow_sync_message_cbor_operation_batch() {
        let op = OpEnvelope::new(
            "dev_A".into(),
            1,
            Horizon::new(),
            Operation::add_item("task_1", "Test"),
        );
        let msg = FlowSyncMessage::OperationBatch {
            flow_id: Uuid::new_v4(),
            ops: vec![op.clone()],
        };
        let bytes = msg.to_cbor().unwrap();
        let decoded = FlowSyncMessage::from_cbor(&bytes).unwrap();

        if let FlowSyncMessage::OperationBatch { ops, .. } = decoded {
            assert_eq!(ops.len(), 1);
            assert_eq!(ops[0].author, "dev_A");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_flow_sync_message_cbor_host_announcement() {
        let host_id = Uuid::new_v4();
        let msg = FlowSyncMessage::HostAnnouncement {
            flow_id: Uuid::new_v4(),
            host_id,
            epoch: 42,
        };
        let bytes = msg.to_cbor().unwrap();
        let decoded = FlowSyncMessage::from_cbor(&bytes).unwrap();

        if let FlowSyncMessage::HostAnnouncement {
            host_id: h,
            epoch,
            ..
        } = decoded
        {
            assert_eq!(h, host_id);
            assert_eq!(epoch, 42);
        } else {
            panic!("wrong variant");
        }
    }

    // -----------------------------------------------------------------------
    // ConvergentDocumentStream tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_convergent_document_stream_read_materializes() {
        let doc = Arc::new(std::sync::RwLock::new(ConvergentDocument::new(
            TestSchema,
            "dev_A".into(),
        )));
        {
            let mut d = doc.write().unwrap();
            d.apply_local(Operation::add_item("item_1", "Test"));
            d.apply_local(Operation::set_field(
                "item_1",
                "title",
                Value::string("Hello"),
            ));
        }

        let stream = ConvergentDocumentStream::new("state", Arc::clone(&doc));
        let bytes = stream.read().unwrap().unwrap();
        let state: DocumentState = serde_json::from_slice(&bytes).unwrap();

        let item = state.get(&"item_1".into()).unwrap();
        assert_eq!(item.fields.get("title"), Some(&Value::string("Hello")));
    }

    #[test]
    fn test_convergent_document_stream_write_applies_op() {
        let doc = Arc::new(std::sync::RwLock::new(ConvergentDocument::new(
            TestSchema,
            "dev_A".into(),
        )));

        let stream = ConvergentDocumentStream::new("state", Arc::clone(&doc));

        // Write an AddItem operation
        let op = Operation::add_item("item_1", "Test");
        let op_bytes = serde_json::to_vec(&op).unwrap();
        stream.write(&op_bytes).unwrap();

        // Verify it was applied
        let d = doc.read().unwrap();
        let state = d.materialize();
        assert!(state.get(&"item_1".into()).is_some());
    }

    // -----------------------------------------------------------------------
    // AccessoryMemorizer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_memorizer_cache_and_retrieve() {
        let mem = AccessoryMemorizer::new(Uuid::new_v4(), 100);

        let op = OpEnvelope::new(
            "dev_A".into(),
            1,
            Horizon::new(),
            Operation::add_item("item_1", "Test"),
        );
        mem.cache_operation(op.clone());
        assert_eq!(mem.cached_count(), 1);

        // Operations since empty horizon should return everything
        let ops = mem.operations_since(&Horizon::new());
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].id, op.id);
    }

    #[test]
    fn test_memorizer_dedup() {
        let mem = AccessoryMemorizer::new(Uuid::new_v4(), 100);

        let op = OpEnvelope::new(
            "dev_A".into(),
            1,
            Horizon::new(),
            Operation::add_item("item_1", "Test"),
        );
        mem.cache_operation(op.clone());
        mem.cache_operation(op.clone()); // duplicate
        assert_eq!(mem.cached_count(), 1);
    }

    #[test]
    fn test_memorizer_operations_since() {
        let mem = AccessoryMemorizer::new(Uuid::new_v4(), 100);

        let op1 = OpEnvelope::new(
            "dev_A".into(),
            1,
            Horizon::new(),
            Operation::add_item("item_1", "Test"),
        );
        let op2 = OpEnvelope::new(
            "dev_A".into(),
            2,
            Horizon::at("dev_A".into(), 1),
            Operation::set_field("item_1", "title", Value::string("Hello")),
        );
        mem.cache_operation(op1);
        mem.cache_operation(op2);

        // Horizon that has seen dev_A:1 should only get op2
        let since = Horizon::at("dev_A".into(), 1);
        let ops = mem.operations_since(&since);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].seq, 2);
    }

    #[test]
    fn test_memorizer_max_ops_eviction() {
        let mem = AccessoryMemorizer::new(Uuid::new_v4(), 3);

        for i in 1..=5 {
            let op = OpEnvelope::new(
                "dev_A".into(),
                i,
                Horizon::new(),
                Operation::add_item(format!("item_{i}"), "Test"),
            );
            mem.cache_operation(op);
        }

        // Should only keep 3 (most recent)
        assert_eq!(mem.cached_count(), 3);
    }

    // -----------------------------------------------------------------------
    // FullReplicaFlow per-party journal + outbound queue tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_per_party_journal_storage() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_per_party_journal");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let device_uuid = Uuid::new_v4();
        let flow = FullReplicaFlow::new_persistent(
            make_config("drip_hosted:test"),
            TestSchema,
            DripHostPolicy::default(),
            device_uuid,
            "device_A".to_string(),
            temp_dir.clone(),
        );

        // Apply a local edit
        let _env = flow.apply_edit(Operation::add_item("task_1", "Test")).unwrap();

        // Verify the journal file exists for this device
        let journal_path = temp_dir.join("journals/device_A.jsonl");
        assert!(journal_path.exists(), "journal file should exist");

        let content = std::fs::read_to_string(&journal_path).unwrap();
        assert_eq!(content.lines().count(), 1, "should have 1 op in journal");

        // Apply a remote op from device_B
        let remote_env = OpEnvelope::new(
            "device_B".into(),
            1,
            Horizon::new(),
            Operation::add_item("task_2", "Test"),
        );
        flow.apply_remote_op(remote_env);

        // Verify device_B's journal was created
        let b_journal = temp_dir.join("journals/device_B.jsonl");
        assert!(b_journal.exists(), "remote device journal should exist");
        let b_content = std::fs::read_to_string(&b_journal).unwrap();
        assert_eq!(b_content.lines().count(), 1);

        // Verify device_A's journal is unchanged (still 1 op)
        let a_content = std::fs::read_to_string(&journal_path).unwrap();
        assert_eq!(a_content.lines().count(), 1);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_outbound_queue_for_known_parties() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_outbound_queue");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let device_uuid = Uuid::new_v4();
        let flow = FullReplicaFlow::new_persistent(
            make_config("drip_hosted:test"),
            TestSchema,
            DripHostPolicy::default(),
            device_uuid,
            "device_A".to_string(),
            temp_dir.clone(),
        );

        // Register a known party
        flow.register_party("device_B", Uuid::new_v4());

        // Apply a local edit
        flow.apply_edit(Operation::add_item("task_1", "Test")).unwrap();

        // Verify the outbound queue has the op for device_B
        let queue_path = temp_dir.join("outbound/device_B.jsonl");
        assert!(queue_path.exists(), "outbound queue should exist");
        let content = std::fs::read_to_string(&queue_path).unwrap();
        assert_eq!(content.lines().count(), 1);

        // Drain the queue
        let ops = flow.drain_outbound("device_B");
        assert_eq!(ops.len(), 1);

        // Queue should be empty after drain
        let content_after = std::fs::read_to_string(&queue_path).unwrap();
        assert!(content_after.is_empty(), "queue should be empty after drain");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_register_party_queues_existing_ops() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_register_party_backfill");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let device_uuid = Uuid::new_v4();
        let flow = FullReplicaFlow::new_persistent(
            make_config("drip_hosted:test"),
            TestSchema,
            DripHostPolicy::default(),
            device_uuid,
            "device_A".to_string(),
            temp_dir.clone(),
        );

        // Apply edits BEFORE registering any parties
        flow.apply_edit(Operation::add_item("task_1", "Test")).unwrap();
        flow.apply_edit(Operation::set_field("task_1", "title", Value::string("Hello"))).unwrap();

        // Now register a party — should backfill all existing local ops
        flow.register_party("device_C", Uuid::new_v4());

        let ops = flow.drain_outbound("device_C");
        assert_eq!(ops.len(), 2, "new party should get all existing local ops");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_per_party_reload_from_journals() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_reload_journals");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let flow_uuid = Uuid::new_v4();

        // First session: create ops from two "devices"
        {
            let flow = FullReplicaFlow::new_persistent(
                FlowConfig { id: flow_uuid, type_name: "drip_hosted:test".into(), params: serde_json::json!({}) },
                TestSchema,
                DripHostPolicy::default(),
                Uuid::new_v4(),
                "device_A".to_string(),
                temp_dir.clone(),
            );
            flow.apply_edit(Operation::add_item("task_1", "Test")).unwrap();

            // Simulate receiving a remote op
            let remote = OpEnvelope::new("device_B".into(), 1, Horizon::new(), Operation::add_item("task_2", "Test"));
            flow.apply_remote_op(remote);
        }

        // Second session: reload and verify both items are present
        {
            let flow = FullReplicaFlow::new_persistent(
                FlowConfig { id: flow_uuid, type_name: "drip_hosted:test".into(), params: serde_json::json!({}) },
                TestSchema,
                DripHostPolicy::default(),
                Uuid::new_v4(),
                "device_A".to_string(),
                temp_dir.clone(),
            );

            let doc = flow.document.read().unwrap();
            let state = doc.materialize();
            assert!(state.get(&"task_1".into()).is_some(), "task_1 should persist across sessions");
            assert!(state.get(&"task_2".into()).is_some(), "task_2 should persist across sessions");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_legacy_operations_jsonl_migration() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_legacy_migration");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Write a legacy operations.jsonl
        let env = OpEnvelope::new("device_A".into(), 1, Horizon::new(), Operation::add_item("task_1", "Test"));
        let json = serde_json::to_string(&env).unwrap();
        std::fs::write(temp_dir.join("operations.jsonl"), format!("{}\n", json)).unwrap();

        // Load the flow — should migrate
        let flow = FullReplicaFlow::new_persistent(
            make_config("drip_hosted:test"),
            TestSchema,
            DripHostPolicy::default(),
            Uuid::new_v4(),
            "device_A".to_string(),
            temp_dir.clone(),
        );

        // Legacy file should be gone
        assert!(!temp_dir.join("operations.jsonl").exists(), "legacy file should be deleted");

        // Journal should exist
        assert!(temp_dir.join("journals/device_A.jsonl").exists(), "migrated journal should exist");

        // Document should have the item
        let doc = flow.document.read().unwrap();
        let state = doc.materialize();
        assert!(state.get(&"task_1".into()).is_some());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // -----------------------------------------------------------------------
    // Multi-entity simulation: three devices sharing one flow
    //
    // No network, no simulation harness — just three FullReplicaFlow
    // instances with separate storage dirs, manually delivering queued
    // ops between them.
    // -----------------------------------------------------------------------

    /// Helper: create a persistent FullReplicaFlow with GianttSchema in a temp dir.
    /// Returns (flow, device_uuid) so the UUID can be used for party registration.
    fn make_giantt_flow(
        base_dir: &std::path::Path,
        device_name: &str,
        flow_id: Uuid,
    ) -> (FullReplicaFlow<crate::convergent::giantt::GianttSchema>, Uuid) {
        let storage_path = base_dir.join(device_name);
        let device_uuid = Uuid::new_v4();
        let flow = FullReplicaFlow::new_persistent(
            FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:giantt".into(),
                params: serde_json::json!({}),
            },
            crate::convergent::giantt::GianttSchema,
            DripHostPolicy::default(),
            device_uuid,
            device_name.to_string(),
            storage_path,
        );
        (flow, device_uuid)
    }

    /// Helper: deliver all queued ops from `sender` to `receiver`.
    /// Returns the number of ops delivered.
    fn deliver_queued_ops<S: DocumentSchema + 'static>(
        sender: &FullReplicaFlow<S>,
        receiver: &FullReplicaFlow<S>,
        receiver_device_id: &str,
    ) -> usize {
        let ops = sender.drain_outbound(receiver_device_id);
        let count = ops.len();
        for op in ops {
            receiver.apply_remote_op(op);
        }
        count
    }

    #[test]
    fn test_three_device_simulation() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_3device_sim");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let flow_id = Uuid::new_v4();

        // Create three devices, each with their own storage
        let (mac, mac_uuid) = make_giantt_flow(&temp_dir, "mac", flow_id);
        let (linux, linux_uuid) = make_giantt_flow(&temp_dir, "linux", flow_id);
        let (phone, phone_uuid) = make_giantt_flow(&temp_dir, "phone", flow_id);

        // Register each other as parties
        mac.register_party("linux", linux_uuid);
        mac.register_party("phone", phone_uuid);
        linux.register_party("mac", mac_uuid);
        linux.register_party("phone", phone_uuid);
        phone.register_party("mac", mac_uuid);
        phone.register_party("linux", linux_uuid);

        // === Phase 1: Mac adds items ===
        mac.apply_edit(Operation::add_item("buy_groceries", "GianttItem")).unwrap();
        mac.apply_edit(Operation::set_field("buy_groceries", "title", Value::string("Buy groceries"))).unwrap();
        mac.apply_edit(Operation::set_field("buy_groceries", "duration", Value::string("1h"))).unwrap();

        mac.apply_edit(Operation::add_item("write_report", "GianttItem")).unwrap();
        mac.apply_edit(Operation::set_field("write_report", "title", Value::string("Write quarterly report"))).unwrap();

        // Verify mac's own journal has 5 ops
        let mac_journal = temp_dir.join("mac/journals/mac.jsonl");
        assert_eq!(std::fs::read_to_string(&mac_journal).unwrap().lines().count(), 5);

        // Verify outbound queues have 5 ops each
        assert_eq!(mac.drain_outbound("linux").len(), 5);
        // Note: drain clears, so let's re-apply for the next delivery test.
        // Instead, let's check phone's queue without draining.
        let phone_queue = temp_dir.join("mac/outbound/phone.jsonl");
        assert_eq!(std::fs::read_to_string(&phone_queue).unwrap().lines().count(), 5);

        // Re-queue for linux by re-registering (register_party is idempotent for
        // existing parties — won't re-queue). We need to manually re-apply.
        // Actually, drain already consumed them. Let's just use phone's queue.

        // === Phase 2: Deliver mac→phone, verify phone has the items ===
        let delivered = deliver_queued_ops(&mac, &phone, "phone");
        assert_eq!(delivered, 5);

        let phone_doc = phone.document().read().unwrap();
        let phone_state = phone_doc.materialize();
        assert!(phone_state.get(&"buy_groceries".into()).is_some());
        assert!(phone_state.get(&"write_report".into()).is_some());
        drop(phone_doc);

        // Phone's journal for mac should now have those 5 ops
        let phone_mac_journal = temp_dir.join("phone/journals/mac.jsonl");
        assert!(phone_mac_journal.exists());
        assert_eq!(std::fs::read_to_string(&phone_mac_journal).unwrap().lines().count(), 5);

        // === Phase 3: Linux makes a change ===
        linux.apply_edit(Operation::add_item("fix_server", "GianttItem")).unwrap();
        linux.apply_edit(Operation::set_field("fix_server", "title", Value::string("Fix production server"))).unwrap();
        linux.apply_edit(Operation::set_field("fix_server", "priority", Value::string("CRITICAL"))).unwrap();

        // Linux's own journal has 3 ops
        let linux_journal = temp_dir.join("linux/journals/linux.jsonl");
        assert_eq!(std::fs::read_to_string(&linux_journal).unwrap().lines().count(), 3);

        // === Phase 4: Deliver linux→phone ===
        let delivered = deliver_queued_ops(&linux, &phone, "phone");
        assert_eq!(delivered, 3);

        // Phone should now have all 3 items (2 from mac + 1 from linux)
        let phone_doc = phone.document().read().unwrap();
        let phone_state = phone_doc.materialize();
        assert!(phone_state.get(&"buy_groceries".into()).is_some());
        assert!(phone_state.get(&"write_report".into()).is_some());
        assert!(phone_state.get(&"fix_server".into()).is_some());
        drop(phone_doc);

        // Phone should have 3 separate journal files: mac's, linux's, and its own (empty)
        let phone_journals = temp_dir.join("phone/journals");
        let journal_count = std::fs::read_dir(&phone_journals).unwrap().count();
        assert_eq!(journal_count, 2, "phone should have journals for mac and linux");

        // === Phase 5: Phone makes a change ===
        phone.apply_edit(Operation::add_item("call_mom", "GianttItem")).unwrap();
        phone.apply_edit(Operation::set_field("call_mom", "title", Value::string("Call mom"))).unwrap();

        // Now phone has its own journal too
        let phone_own_journal = temp_dir.join("phone/journals/phone.jsonl");
        assert!(phone_own_journal.exists());
        assert_eq!(std::fs::read_to_string(&phone_own_journal).unwrap().lines().count(), 2);

        // Deliver phone→linux
        let delivered = deliver_queued_ops(&phone, &linux, "linux");
        assert_eq!(delivered, 2);

        // Linux should now have phone's item
        let linux_doc = linux.document().read().unwrap();
        let linux_state = linux_doc.materialize();
        assert!(linux_state.get(&"call_mom".into()).is_some());
        assert!(linux_state.get(&"fix_server".into()).is_some());
        // Linux hasn't received mac's ops yet (we drained mac→linux earlier)
        assert!(linux_state.get(&"buy_groceries".into()).is_none());
        drop(linux_doc);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_two_independent_flows_on_same_device() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_2flows");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let flow_id_1 = Uuid::new_v4();
        let flow_id_2 = Uuid::new_v4();

        // Same device, two different flows (two different giantt graphs)
        let (flow_1, _) = make_giantt_flow(&temp_dir, "mac_flow1", flow_id_1);
        let (flow_2, _) = make_giantt_flow(&temp_dir, "mac_flow2", flow_id_2);

        // Add items to flow 1
        flow_1.apply_edit(Operation::add_item("task_a", "GianttItem")).unwrap();
        flow_1.apply_edit(Operation::set_field("task_a", "title", Value::string("Flow 1 task"))).unwrap();

        // Add items to flow 2
        flow_2.apply_edit(Operation::add_item("task_b", "GianttItem")).unwrap();
        flow_2.apply_edit(Operation::set_field("task_b", "title", Value::string("Flow 2 task"))).unwrap();

        // Each flow should only see its own items
        let state_1 = flow_1.document().read().unwrap().materialize();
        let state_2 = flow_2.document().read().unwrap().materialize();

        assert!(state_1.get(&"task_a".into()).is_some());
        assert!(state_1.get(&"task_b".into()).is_none(), "flow 1 should not see flow 2's items");

        assert!(state_2.get(&"task_b".into()).is_some());
        assert!(state_2.get(&"task_a".into()).is_none(), "flow 2 should not see flow 1's items");

        // Each flow has its own storage directory
        assert!(temp_dir.join("mac_flow1/journals").exists());
        assert!(temp_dir.join("mac_flow2/journals").exists());

        // Register parties independently per flow
        flow_1.register_party("linux", Uuid::new_v4());
        flow_2.register_party("phone", Uuid::new_v4());

        // flow_1 should queue for linux, flow_2 for phone
        assert_eq!(flow_1.drain_outbound("linux").len(), 2);
        assert_eq!(flow_1.drain_outbound("phone").len(), 0, "flow_1 has no phone party");
        assert_eq!(flow_2.drain_outbound("phone").len(), 2);
        assert_eq!(flow_2.drain_outbound("linux").len(), 0, "flow_2 has no linux party");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_giantt_and_inventory_flows_simultaneously() {
        use crate::convergent::inventory::InventorySchema;

        let temp_dir = std::env::temp_dir().join("soradyne_test_giantt_inventory");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let giantt_flow_id = Uuid::new_v4();
        let inventory_flow_id = Uuid::new_v4();

        let device_uuid = Uuid::new_v4();

        // Create a giantt flow
        let giantt = FullReplicaFlow::new_persistent(
            FlowConfig {
                id: giantt_flow_id,
                type_name: "drip_hosted:giantt".into(),
                params: serde_json::json!({}),
            },
            crate::convergent::giantt::GianttSchema,
            DripHostPolicy::default(),
            device_uuid,
            "mac".to_string(),
            temp_dir.join("giantt"),
        );

        // Create an inventory flow
        let inventory = FullReplicaFlow::new_persistent(
            FlowConfig {
                id: inventory_flow_id,
                type_name: "drip_hosted:inventory".into(),
                params: serde_json::json!({}),
            },
            InventorySchema,
            DripHostPolicy::default(),
            device_uuid,
            "mac".to_string(),
            temp_dir.join("inventory"),
        );

        // Both share the same device but have independent storage
        let linux_uuid = Uuid::new_v4();
        giantt.register_party("linux", linux_uuid);
        inventory.register_party("linux", linux_uuid);

        // Add a giantt item
        giantt.apply_edit(Operation::add_item("deploy_v2", "GianttItem")).unwrap();
        giantt.apply_edit(Operation::set_field("deploy_v2", "title", Value::string("Deploy v2"))).unwrap();

        // Add an inventory item
        inventory.apply_edit(Operation::add_item("laptop_1", "InventoryItem")).unwrap();
        inventory.apply_edit(Operation::set_field("laptop_1", "description", Value::string("MacBook Air M2"))).unwrap();
        inventory.apply_edit(Operation::set_field("laptop_1", "category", Value::string("Electronics"))).unwrap();

        // Verify independent state
        let g_state = giantt.document().read().unwrap().materialize();
        let i_state = inventory.document().read().unwrap().materialize();

        assert!(g_state.get(&"deploy_v2".into()).is_some());
        assert!(g_state.get(&"laptop_1".into()).is_none(), "giantt should not see inventory items");

        assert!(i_state.get(&"laptop_1".into()).is_some());
        assert!(i_state.get(&"deploy_v2".into()).is_none(), "inventory should not see giantt items");

        // Independent outbound queues
        let g_queued = giantt.drain_outbound("linux");
        let i_queued = inventory.drain_outbound("linux");

        assert_eq!(g_queued.len(), 2, "giantt should queue 2 ops for linux");
        assert_eq!(i_queued.len(), 3, "inventory should queue 3 ops for linux");

        // Verify the ops are for the right schemas
        assert!(matches!(g_queued[0].op, Operation::AddItem { ref item_type, .. } if item_type == "GianttItem"));
        assert!(matches!(i_queued[0].op, Operation::AddItem { ref item_type, .. } if item_type == "InventoryItem"));

        // Now deliver giantt ops to linux's giantt flow
        let (linux_giantt, _) = make_giantt_flow(&temp_dir, "linux_giantt", giantt_flow_id);
        for op in g_queued {
            linux_giantt.apply_remote_op(op);
        }

        let linux_g_state = linux_giantt.document().read().unwrap().materialize();
        assert!(linux_g_state.get(&"deploy_v2".into()).is_some(), "linux should have giantt item");

        // Deliver inventory ops to linux's inventory flow
        let linux_inventory = FullReplicaFlow::new_persistent(
            FlowConfig {
                id: inventory_flow_id,
                type_name: "drip_hosted:inventory".into(),
                params: serde_json::json!({}),
            },
            InventorySchema,
            DripHostPolicy::default(),
            Uuid::new_v4(),
            "linux".to_string(),
            temp_dir.join("linux_inventory"),
        );
        for op in i_queued {
            linux_inventory.apply_remote_op(op);
        }

        let linux_i_state = linux_inventory.document().read().unwrap().materialize();
        assert!(linux_i_state.get(&"laptop_1".into()).is_some(), "linux should have inventory item");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_persistence_with_parties_and_queues() {
        // Verify that parties, journals, and queues survive across process restarts.
        let temp_dir = std::env::temp_dir().join("soradyne_test_session_persist");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let flow_id = Uuid::new_v4();

        let linux_uuid = Uuid::new_v4();
        let phone_uuid = Uuid::new_v4();

        // Session 1: create flow, register parties, add ops
        {
            let (flow, _) = make_giantt_flow(&temp_dir, "mac", flow_id);
            flow.register_party("linux", linux_uuid);
            flow.register_party("phone", phone_uuid);

            flow.apply_edit(Operation::add_item("task_1", "GianttItem")).unwrap();
            flow.apply_edit(Operation::set_field("task_1", "title", Value::string("Persistent task"))).unwrap();
        }

        // Session 2: reopen flow, verify state and queues are intact
        {
            let (flow, _) = make_giantt_flow(&temp_dir, "mac", flow_id);

            // Known parties should be restored
            let parties = flow.known_parties();
            assert!(parties.contains_key("linux"), "linux should be a known party");
            assert!(parties.contains_key("phone"), "phone should be a known party");

            // Document should have the item
            let doc = flow.document().read().unwrap();
            let state = doc.materialize();
            assert!(state.get(&"task_1".into()).is_some());
            drop(doc);

            // Outbound queues should still have the ops (not yet delivered)
            let linux_ops = flow.drain_outbound("linux");
            assert_eq!(linux_ops.len(), 2, "linux queue should survive restart");

            let phone_ops = flow.drain_outbound("phone");
            assert_eq!(phone_ops.len(), 2, "phone queue should survive restart");

            // Add another op in session 2
            flow.apply_edit(Operation::add_item("task_2", "GianttItem")).unwrap();
        }

        // Session 3: verify the new op is also queued
        {
            let (flow, _) = make_giantt_flow(&temp_dir, "mac", flow_id);

            // linux and phone queues were drained in session 2, so only task_2 should be there
            let linux_ops = flow.drain_outbound("linux");
            assert_eq!(linux_ops.len(), 1, "only the new op should be queued");

            let doc = flow.document().read().unwrap();
            let state = doc.materialize();
            assert!(state.get(&"task_1".into()).is_some());
            assert!(state.get(&"task_2".into()).is_some());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // -----------------------------------------------------------------------
    // FullReplicaFlow failover tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_offline_merge_allows_local_edits() {
        let flow = make_test_flow(DripHostPolicy {
            failover: HostFailoverPolicy::OfflineMerge,
            ..Default::default()
        });

        // No host assigned — OfflineMerge should still allow local edits
        let result = flow.apply_edit(Operation::add_item("task_1", "Test"));
        assert!(result.is_ok());

        let doc = flow.document.read().unwrap();
        let state = doc.materialize();
        assert!(state.get(&"task_1".into()).is_some());
    }

    #[test]
    fn test_wait_for_host_queues_edits() {
        let flow = make_test_flow(DripHostPolicy {
            failover: HostFailoverPolicy::WaitForHost,
            ..Default::default()
        });

        // Not the host, so edit should be queued
        let result = flow.apply_edit(Operation::add_item("task_1", "Test"));
        assert!(matches!(result, Err(FlowError::HostUnavailable(_))));

        // Edit should be in pending queue
        let pending = flow.pending_edits.read().unwrap();
        assert_eq!(pending.len(), 1);

        // Document should be empty (edit wasn't applied)
        let doc = flow.document.read().unwrap();
        let state = doc.materialize();
        assert!(state.get(&"task_1".into()).is_none());
    }

    #[test]
    fn test_immediate_failover_clears_host() {
        let flow = make_test_flow(DripHostPolicy {
            failover: HostFailoverPolicy::ImmediateFailover,
            ..Default::default()
        });

        // Set a host
        {
            let mut ha = flow.host_assignment.write().unwrap();
            ha.accept_announcement(Uuid::new_v4(), 1);
        }

        flow.handle_host_dropout();

        let ha = flow.host_assignment.read().unwrap();
        assert!(ha.current_host.is_none());
    }

    #[test]
    fn test_become_host_drains_pending() {
        let device_uuid = Uuid::new_v4();
        let flow = DripHostedFlow::new(
            make_config("drip_hosted:test"),
            TestSchema,
            DripHostPolicy {
                failover: HostFailoverPolicy::WaitForHost,
                ..Default::default()
            },
            device_uuid,
            "device_A".to_string(),
        );

        // Queue some edits (pretend we weren't host)
        {
            let mut pending = flow.pending_edits.write().unwrap();
            pending.push(Operation::add_item("task_1", "Test"));
            pending.push(Operation::set_field(
                "task_1",
                "title",
                Value::string("Queued edit"),
            ));
        }

        // Become host — should drain and apply pending edits
        flow.become_host().unwrap();

        assert!(flow.is_current_host());
        assert_eq!(flow.pending_edits.read().unwrap().len(), 0);

        let doc = flow.document.read().unwrap();
        let state = doc.materialize();
        let item = state.get(&"task_1".into()).unwrap();
        assert_eq!(
            item.fields.get("title"),
            Some(&Value::string("Queued edit"))
        );
    }

    #[test]
    fn test_handoff_host_requires_being_host() {
        let flow = make_test_flow(DripHostPolicy::default());

        // Not host — handoff should fail
        let result = flow.handoff_host(Uuid::new_v4());
        assert!(matches!(result, Err(FlowError::NotHost(_))));
    }

    // -----------------------------------------------------------------------
    // Flow trait tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_flow_trait_basics() {
        let flow = make_test_flow(DripHostPolicy::default());

        assert_eq!(flow.type_name(), "drip_hosted:test");
        assert_eq!(flow.schema().name, "drip_hosted:test");
        assert!(flow.stream("state").is_some());
        assert!(flow.stream("nonexistent").is_none());
        assert!(flow.stream_names().contains(&"state".to_string()));
    }

    // -----------------------------------------------------------------------
    // FlowRegistry tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_giantt_constructor() {
        let mut registry = FlowRegistry::new();
        register_drip_hosted_flows(&mut registry);

        let config = FlowConfig {
            id: Uuid::new_v4(),
            type_name: "drip_hosted:giantt".to_string(),
            params: serde_json::json!({
                "device_id": "mac_A",
                "device_uuid": Uuid::new_v4().to_string(),
            }),
        };

        let constructor = registry.get("drip_hosted:giantt").unwrap();
        let flow = constructor(config).unwrap();
        assert_eq!(flow.type_name(), "drip_hosted:giantt");
        assert!(flow.stream("state").is_some());
    }

    #[test]
    fn test_registry_inventory_constructor() {
        let mut registry = FlowRegistry::new();
        register_drip_hosted_flows(&mut registry);

        let config = FlowConfig {
            id: Uuid::new_v4(),
            type_name: "drip_hosted:inventory".to_string(),
            params: serde_json::json!({}),
        };

        let constructor = registry.get("drip_hosted:inventory").unwrap();
        let flow = constructor(config).unwrap();
        assert_eq!(flow.type_name(), "drip_hosted:inventory");
    }

    // -----------------------------------------------------------------------
    // Integration: two-device CRDT sync (no network, manual message passing)
    // -----------------------------------------------------------------------

    #[test]
    fn test_two_device_sync_via_operation_batch() {
        let flow_id = Uuid::new_v4();

        let flow_a = DripHostedFlow::new(
            FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:test".into(),
                params: serde_json::json!({}),
            },
            TestSchema,
            DripHostPolicy::default(),
            Uuid::new_v4(),
            "dev_A".into(),
        );

        let flow_b = DripHostedFlow::new(
            FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:test".into(),
                params: serde_json::json!({}),
            },
            TestSchema,
            DripHostPolicy::default(),
            Uuid::new_v4(),
            "dev_B".into(),
        );

        // A adds an item
        let env_a = flow_a
            .apply_edit(Operation::add_item("task_1", "Test"))
            .unwrap();

        // B adds a different item
        let env_b = flow_b
            .apply_edit(Operation::add_item("task_2", "Test"))
            .unwrap();

        // Simulate sync: send A's ops to B and vice versa
        flow_b.handle_flow_sync(
            flow_a.device_uuid,
            FlowSyncMessage::OperationBatch {
                flow_id,
                ops: vec![env_a],
            },
        );
        flow_a.handle_flow_sync(
            flow_b.device_uuid,
            FlowSyncMessage::OperationBatch {
                flow_id,
                ops: vec![env_b],
            },
        );

        // Both should now have both items
        let state_a = flow_a.document.read().unwrap().materialize();
        let state_b = flow_b.document.read().unwrap().materialize();

        assert!(state_a.get(&"task_1".into()).is_some());
        assert!(state_a.get(&"task_2".into()).is_some());
        assert!(state_b.get(&"task_1".into()).is_some());
        assert!(state_b.get(&"task_2".into()).is_some());
    }

    #[test]
    fn test_offline_merge_convergence() {
        let flow_id = Uuid::new_v4();

        let flow_a = DripHostedFlow::new(
            FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:test".into(),
                params: serde_json::json!({}),
            },
            TestSchema,
            DripHostPolicy {
                failover: HostFailoverPolicy::OfflineMerge,
                ..Default::default()
            },
            Uuid::new_v4(),
            "dev_A".into(),
        );

        let flow_b = DripHostedFlow::new(
            FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:test".into(),
                params: serde_json::json!({}),
            },
            TestSchema,
            DripHostPolicy {
                failover: HostFailoverPolicy::OfflineMerge,
                ..Default::default()
            },
            Uuid::new_v4(),
            "dev_B".into(),
        );

        // Both create the same item
        let env_a1 = flow_a
            .apply_edit(Operation::add_item("task_1", "Test"))
            .unwrap();
        let env_b1 = flow_b
            .apply_edit(Operation::add_item("task_1", "Test"))
            .unwrap();

        // Both set the title (concurrently, different values)
        let env_a2 = flow_a
            .apply_edit(Operation::set_field(
                "task_1",
                "title",
                Value::string("Title from A"),
            ))
            .unwrap();
        let env_b2 = flow_b
            .apply_edit(Operation::set_field(
                "task_1",
                "title",
                Value::string("Title from B"),
            ))
            .unwrap();

        // Sync all ops
        flow_b.handle_flow_sync(
            flow_a.device_uuid,
            FlowSyncMessage::OperationBatch {
                flow_id,
                ops: vec![env_a1, env_a2],
            },
        );
        flow_a.handle_flow_sync(
            flow_b.device_uuid,
            FlowSyncMessage::OperationBatch {
                flow_id,
                ops: vec![env_b1, env_b2],
            },
        );

        // Both should converge to the same state (latest-wins)
        let state_a = flow_a.document.read().unwrap().materialize();
        let state_b = flow_b.document.read().unwrap().materialize();

        let title_a = state_a
            .get(&"task_1".into())
            .unwrap()
            .fields
            .get("title")
            .unwrap();
        let title_b = state_b
            .get(&"task_1".into())
            .unwrap()
            .fields
            .get("title")
            .unwrap();

        // They must agree (both converge to same winner)
        assert_eq!(title_a, title_b);
    }

    // -----------------------------------------------------------------------
    // Integration: host announcement propagation
    // -----------------------------------------------------------------------

    #[test]
    fn test_host_announcement_propagation() {
        let flow_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();

        let flow = DripHostedFlow::new(
            FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:test".into(),
                params: serde_json::json!({}),
            },
            TestSchema,
            DripHostPolicy::default(),
            Uuid::new_v4(),
            "dev_A".into(),
        );

        // Receive host announcement
        flow.handle_flow_sync(
            host_id,
            FlowSyncMessage::HostAnnouncement {
                flow_id,
                host_id,
                epoch: 1,
            },
        );

        let ha = flow.host_assignment.read().unwrap();
        assert_eq!(ha.current_host, Some(host_id));
        assert_eq!(ha.epoch, 1);
    }

    // -----------------------------------------------------------------------
    // Async integration tests with SimBleNetwork
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod async_tests {
        use super::*;
        use crate::ble::simulated::SimBleNetwork;
        use crate::ble::transport::{BleCentral, BlePeripheral};
        use crate::topology::ensemble::{
            ConnectionQuality, EnsembleTopology, TopologyEdge, TransportType,
        };
        use crate::topology::messenger::TopologyMessenger;
        use std::sync::Arc;
        use std::time::Duration;
        use tokio::sync::RwLock;

        fn make_sim_edge(from: Uuid, to: Uuid) -> TopologyEdge {
            TopologyEdge {
                from,
                to,
                transport: TransportType::SimulatedBle,
                quality: ConnectionQuality::unknown(),
            }
        }

        async fn make_connection(
            network: &Arc<SimBleNetwork>,
        ) -> (Arc<dyn crate::ble::transport::BleConnection>, Arc<dyn crate::ble::transport::BleConnection>)
        {
            // Use a generous MTU since CBOR-encoded RoutedEnvelopes carrying
            // FlowSyncMessages (especially OperationBatch) can exceed the
            // default 247-byte BLE MTU.
            let mut device_a = network.create_device();
            device_a.set_mtu(4096);
            let device_b = network.create_device();
            let addr_b = device_b.address().clone();

            device_b.start_advertising(vec![0x01]).await.unwrap();
            let accept_handle =
                tokio::spawn(async move { device_b.accept().await.unwrap() });

            let conn_a = device_a.connect(&addr_b).await.unwrap();
            let conn_b = accept_handle.await.unwrap();

            (Arc::from(conn_a), Arc::from(conn_b))
        }

        /// Verify that a broadcast from messenger_a arrives at messenger_b's
        /// incoming channel. This isolates the messenger layer from the flow
        /// listener to find where the message gets lost.
        #[tokio::test]
        async fn test_messenger_broadcast_received() {
            let network = SimBleNetwork::new();
            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4();

            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                Arc::new(RwLock::new(t))
            };

            let messenger_a = TopologyMessenger::new(id_a, make_topo());
            let messenger_b = TopologyMessenger::new(id_b, make_topo());

            let (conn_a, conn_b) = make_connection(&network).await;
            messenger_a.add_connection(id_b, conn_a).await;
            messenger_b.add_connection(id_a, conn_b).await;

            // Subscribe BEFORE sending
            let mut rx = messenger_b.incoming();

            let msg = FlowSyncMessage::OperationBatch {
                flow_id: Uuid::new_v4(),
                ops: vec![],
            };
            let payload = msg.to_cbor().unwrap();
            messenger_a
                .broadcast(MessageType::FlowSync, &payload)
                .await
                .unwrap();

            // Wait for delivery
            let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
            assert!(
                result.is_ok(),
                "Should receive broadcast within timeout"
            );
            let envelope = result.unwrap().unwrap();
            assert_eq!(envelope.source, id_a);
            assert_eq!(envelope.message_type, MessageType::FlowSync);
        }

        /// 3-piece ensemble: A broadcasts an edit, B and C receive it
        /// via their messenger incoming channels and process it through
        /// handle_flow_sync. This tests the full BLE → messenger → flow
        /// pipeline without relying on start()'s spawned task timing.
        #[tokio::test]
        async fn test_3_piece_ensemble_sync() {
            let network = SimBleNetwork::new();
            let flow_id = Uuid::new_v4();

            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4();
            let id_c = Uuid::new_v4();

            // Build topology: A↔B, A↔C
            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                t.add_edge(make_sim_edge(id_a, id_c));
                t.add_edge(make_sim_edge(id_c, id_a));
                Arc::new(RwLock::new(t))
            };

            let topo_a = make_topo();
            let topo_b = make_topo();
            let topo_c = make_topo();

            let messenger_a = TopologyMessenger::new(id_a, topo_a.clone());
            let messenger_b = TopologyMessenger::new(id_b, topo_b.clone());
            let messenger_c = TopologyMessenger::new(id_c, topo_c.clone());

            // Subscribe to incoming BEFORE wiring connections
            let mut rx_b = messenger_b.incoming();
            let mut rx_c = messenger_c.incoming();

            // Wire up connections: A↔B, A↔C
            let (conn_ab_a, conn_ab_b) = make_connection(&network).await;
            let (conn_ac_a, conn_ac_c) = make_connection(&network).await;

            messenger_a.add_connection(id_b, conn_ab_a).await;
            messenger_b.add_connection(id_a, conn_ab_b).await;
            messenger_a.add_connection(id_c, conn_ac_a).await;
            messenger_c.add_connection(id_a, conn_ac_c).await;

            // Create flows (disconnected — we process messages manually)
            let flow_b = DripHostedFlow::<TestSchema>::new(
                FlowConfig {
                    id: flow_id,
                    type_name: "drip_hosted:test".into(),
                    params: serde_json::json!({}),
                },
                TestSchema,
                DripHostPolicy::default(),
                id_b,
                "dev_B".into(),
            );

            let flow_c = DripHostedFlow::<TestSchema>::new(
                FlowConfig {
                    id: flow_id,
                    type_name: "drip_hosted:test".into(),
                    params: serde_json::json!({}),
                },
                TestSchema,
                DripHostPolicy::default(),
                id_c,
                "dev_C".into(),
            );

            // A creates an edit
            let op_env = OpEnvelope::new(
                "dev_A".into(),
                1,
                Horizon::new(),
                Operation::add_item("task_1", "Test"),
            );

            // A broadcasts the operation via messenger
            let msg = FlowSyncMessage::OperationBatch {
                flow_id,
                ops: vec![op_env],
            };
            let payload = msg.to_cbor().unwrap();
            messenger_a
                .broadcast(MessageType::FlowSync, &payload)
                .await
                .unwrap();

            // Receive on B and C
            let env_b = tokio::time::timeout(Duration::from_millis(500), rx_b.recv())
                .await
                .expect("B should receive within timeout")
                .expect("B recv should succeed");
            let env_c = tokio::time::timeout(Duration::from_millis(500), rx_c.recv())
                .await
                .expect("C should receive within timeout")
                .expect("C recv should succeed");

            // Process through flow handlers
            let msg_b = FlowSyncMessage::from_cbor(&env_b.payload).unwrap();
            flow_b.handle_flow_sync(env_b.source, msg_b);

            let msg_c = FlowSyncMessage::from_cbor(&env_c.payload).unwrap();
            flow_c.handle_flow_sync(env_c.source, msg_c);

            // Both should now have the item
            let state_b = flow_b.document.read().unwrap().materialize();
            let state_c = flow_c.document.read().unwrap().materialize();

            assert!(
                state_b.get(&"task_1".into()).is_some(),
                "B should have received task_1"
            );
            assert!(
                state_c.get(&"task_1".into()).is_some(),
                "C should have received task_1"
            );
        }

        #[tokio::test(start_paused = true)]
        async fn test_voluntary_handoff() {
            let flow_id = Uuid::new_v4();
            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4();

            let network = SimBleNetwork::new();
            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                Arc::new(RwLock::new(t))
            };

            let topo_a = make_topo();
            let topo_b = make_topo();

            let messenger_a = TopologyMessenger::new(id_a, topo_a.clone());
            let messenger_b = TopologyMessenger::new(id_b, topo_b.clone());

            let (conn_a, conn_b) = make_connection(&network).await;
            messenger_a.add_connection(id_b, conn_a).await;
            messenger_b.add_connection(id_a, conn_b).await;

            let config = FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:test".into(),
                params: serde_json::json!({}),
            };

            let flow_a = DripHostedFlow::new_with_ensemble(
                config.clone(),
                TestSchema,
                DripHostPolicy {
                    allow_voluntary_handoff: true,
                    ..Default::default()
                },
                id_a,
                "dev_A".into(),
                Arc::clone(&messenger_a),
                topo_a,
            );

            let flow_b = DripHostedFlow::new_with_ensemble(
                config,
                TestSchema,
                DripHostPolicy::default(),
                id_b,
                "dev_B".into(),
                Arc::clone(&messenger_b),
                topo_b,
            );

            flow_b.start().unwrap();

            // A becomes host
            flow_a.become_host().unwrap();
            assert!(flow_a.is_current_host());

            tokio::time::sleep(Duration::from_millis(50)).await;

            // A hands off to B
            flow_a.handoff_host(id_b).unwrap();

            tokio::time::sleep(Duration::from_millis(50)).await;

            // B should see itself as the announced host
            let ha_b = flow_b.host_assignment.read().unwrap();
            assert_eq!(ha_b.current_host, Some(id_b));

            flow_b.stop();
        }

        /// Multi-hop sync: A↔B↔C chain where B is a relay-only party (no flow).
        /// A and C each run a persistent FullReplicaFlow<GianttSchema>.
        /// A edits, flushes → B forwards → C receives. Then C edits, flushes
        /// → B forwards → A receives. Both sides converge.
        #[tokio::test]
        async fn test_multi_hop_giantt_sync_through_relay() {
            let network = SimBleNetwork::new();
            let flow_id = Uuid::new_v4();

            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4(); // relay only
            let id_c = Uuid::new_v4();

            let tmp_a = tempfile::tempdir().unwrap();
            let tmp_c = tempfile::tempdir().unwrap();

            // Chain topology: A↔B↔C (no direct A↔C link)
            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                t.add_edge(make_sim_edge(id_b, id_c));
                t.add_edge(make_sim_edge(id_c, id_b));
                Arc::new(RwLock::new(t))
            };

            // --- Messengers ---
            let messenger_a = Arc::new(TopologyMessenger::new(id_a, make_topo()));
            let messenger_b = Arc::new(TopologyMessenger::new(id_b, make_topo()));
            let messenger_c = Arc::new(TopologyMessenger::new(id_c, make_topo()));

            // --- BLE connections: A↔B and B↔C ---
            let (conn_ab_a, conn_ab_b) = make_connection(&network).await;
            let (conn_bc_b, conn_bc_c) = make_connection(&network).await;

            messenger_a.add_connection(id_b, conn_ab_a).await;
            messenger_b.add_connection(id_a, conn_ab_b).await;
            messenger_b.add_connection(id_c, conn_bc_b).await;
            messenger_c.add_connection(id_b, conn_bc_c).await;

            // --- Flow A: persistent GianttSchema ---
            let config = FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:giantt".into(),
                params: serde_json::json!({}),
            };

            let mut flow_a = FullReplicaFlow::new_persistent(
                config.clone(),
                GianttSchema,
                DripHostPolicy::default(),
                id_a,
                "phone".into(),
                tmp_a.path().to_path_buf(),
            );
            flow_a.messenger = Some(Arc::clone(&messenger_a));
            flow_a.topology = Some(make_topo());
            flow_a.register_party("laptop", id_c);

            // --- Flow C: persistent GianttSchema ---
            let mut flow_c = FullReplicaFlow::new_persistent(
                config.clone(),
                GianttSchema,
                DripHostPolicy::default(),
                id_c,
                "laptop".into(),
                tmp_c.path().to_path_buf(),
            );
            flow_c.messenger = Some(Arc::clone(&messenger_c));
            flow_c.topology = Some(make_topo());
            flow_c.register_party("phone", id_a);

            // B has no flow — it's just a relay.

            // Start C's sync listener so it processes incoming batches.
            flow_c.start().unwrap();

            // --- A edits and flushes → should reach C via B ---
            flow_a
                .apply_edit(Operation::add_item("write_report", "GianttTask"))
                .unwrap();
            flow_a
                .apply_edit(Operation::set_field(
                    "write_report",
                    "title",
                    Value::string("Write quarterly report"),
                ))
                .unwrap();

            let sent_a = flow_a.flush_outbound().await;
            assert_eq!(sent_a.get("laptop"), Some(&2), "A should flush 2 ops toward C");

            // Give time for B to forward and C's start() to process.
            tokio::time::sleep(Duration::from_millis(300)).await;

            // C should have the task.
            {
                let state_c = flow_c.document.read().unwrap().materialize();
                assert!(
                    state_c.get(&"write_report".into()).is_some(),
                    "C should have write_report via multi-hop through B"
                );
            }

            // --- Now C edits and flushes back → should reach A via B ---
            flow_c
                .apply_edit(Operation::add_item("review_draft", "GianttTask"))
                .unwrap();

            // Stop C's listener, start A's, so A can receive.
            flow_c.stop();
            flow_a.start().unwrap();

            let sent_c = flow_c.flush_outbound().await;
            assert_eq!(sent_c.get("phone"), Some(&1), "C should flush 1 op toward A");

            tokio::time::sleep(Duration::from_millis(300)).await;

            // A should have both tasks now.
            {
                let state_a = flow_a.document.read().unwrap().materialize();
                assert!(
                    state_a.get(&"write_report".into()).is_some(),
                    "A should still have its own write_report"
                );
                assert!(
                    state_a.get(&"review_draft".into()).is_some(),
                    "A should have review_draft via multi-hop through B"
                );
            }

            // Both sides have both items — convergence through relay.
            {
                let state_c = flow_c.document.read().unwrap().materialize();
                assert!(
                    state_c.get(&"review_draft".into()).is_some(),
                    "C should have its own review_draft"
                );
                assert!(
                    state_c.get(&"write_report".into()).is_some(),
                    "C should still have write_report"
                );
            }

            flow_a.stop();
        }

        /// Full sender path: op → outbound queue → flush_outbound() → SimBLE →
        /// TopologyMessenger → start() task on receiver → journal + convergence.
        ///
        /// Device A applies ops while offline (no messenger), queuing them.
        /// Then A gets wired to a messenger and flushes. Device B's start()
        /// task receives the batch and applies it.
        #[tokio::test]
        async fn test_flush_outbound_via_sim_ble() {
            let network = SimBleNetwork::new();
            let flow_id = Uuid::new_v4();
            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4();

            let tmp_a = tempfile::tempdir().unwrap();
            let tmp_b = tempfile::tempdir().unwrap();

            let config = FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:test".into(),
                params: serde_json::json!({}),
            };

            // --- Device A: persistent, no messenger yet ---
            let flow_a = FullReplicaFlow::<TestSchema>::new_persistent(
                config.clone(),
                TestSchema,
                DripHostPolicy::default(),
                id_a,
                "dev_A".into(),
                tmp_a.path().to_path_buf(),
            );

            // Register B so ops queue for it
            flow_a.register_party("dev_B", id_b);

            // Apply two ops while offline
            flow_a
                .apply_edit(Operation::add_item("item_1", "Test"))
                .unwrap();
            flow_a
                .apply_edit(Operation::set_field(
                    "item_1",
                    "title",
                    Value::string("Hello from A"),
                ))
                .unwrap();

            // Verify outbound queue has 2 ops
            assert_eq!(flow_a.drain_outbound("dev_B").len(), 2);
            // drain consumed them; re-apply so we can flush
            // Actually: re-applying would create new ops. Instead, let's just
            // do this test without draining first. Re-create flow_a from disk.
            drop(flow_a);

            let flow_a = FullReplicaFlow::<TestSchema>::new_persistent(
                config.clone(),
                TestSchema,
                DripHostPolicy::default(),
                id_a,
                "dev_A".into(),
                tmp_a.path().to_path_buf(),
            );
            flow_a.register_party("dev_B", id_b);

            // Re-apply ops to rebuild outbound queue (reload from disk
            // doesn't re-populate outbound). Apply fresh ops instead.
            flow_a
                .apply_edit(Operation::add_item("item_2", "Test"))
                .unwrap();

            // --- Device B: persistent + messenger + start() ---
            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                Arc::new(RwLock::new(t))
            };

            let topo_b = make_topo();
            let messenger_b = Arc::new(TopologyMessenger::new(id_b, topo_b.clone()));

            let mut flow_b = FullReplicaFlow::<TestSchema>::new_persistent(
                config.clone(),
                TestSchema,
                DripHostPolicy::default(),
                id_b,
                "dev_B".into(),
                tmp_b.path().to_path_buf(),
            );
            flow_b.messenger = Some(Arc::clone(&messenger_b));
            flow_b.topology = Some(topo_b);
            flow_b.start().unwrap();

            // --- Wire A to a messenger and BLE ---
            let topo_a = make_topo();
            let messenger_a = Arc::new(TopologyMessenger::new(id_a, topo_a.clone()));

            let (conn_a, conn_b) = make_connection(&network).await;
            messenger_a.add_connection(id_b, conn_a).await;
            messenger_b.add_connection(id_a, conn_b).await;

            // Attach messenger to flow_a (simulates coming online)
            let flow_a = {
                let mut f = flow_a;
                f.messenger = Some(Arc::clone(&messenger_a));
                f.topology = Some(topo_a);
                f
            };

            // Flush the outbound queue over BLE
            let sent = flow_a.flush_outbound().await;
            assert_eq!(sent.get("dev_B"), Some(&1), "should have flushed 1 op to dev_B");

            // Give B's start() task time to process
            tokio::time::sleep(Duration::from_millis(200)).await;

            // B should have item_2
            let state_b = flow_b.document.read().unwrap().materialize();
            assert!(
                state_b.get(&"item_2".into()).is_some(),
                "B should have received item_2 via flush_outbound"
            );

            // B's journal should have the op persisted
            let journal_dir = tmp_b.path().join("journals");
            assert!(journal_dir.exists(), "B should have journals directory");

            flow_b.stop();
        }

        /// Encrypted multi-hop: A↔B↔C where every link uses Noise IKpsk2.
        /// B is a relay (no flow). A and C sync a GianttSchema FullReplicaFlow
        /// through B with all traffic encrypted per-link.
        #[tokio::test]
        async fn test_encrypted_multi_hop_giantt_sync() {
            use crate::ble::session::{
                establish_initiator, establish_responder, session_psk,
                ResponderIdentity, SessionIdentity,
            };
            use crate::identity::{CapsuleKeyBundle, DeviceIdentity};

            let network = SimBleNetwork::new();
            let flow_id = Uuid::new_v4();

            // Three devices with full cryptographic identities
            let id_a_identity = DeviceIdentity::generate();
            let id_b_identity = DeviceIdentity::generate();
            let id_c_identity = DeviceIdentity::generate();

            let id_a = id_a_identity.device_id();
            let id_b = id_b_identity.device_id();
            let id_c = id_c_identity.device_id();

            // Shared capsule key bundle (all three are members)
            let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
            let psk = session_psk(&capsule_keys);

            let tmp_a = tempfile::tempdir().unwrap();
            let tmp_c = tempfile::tempdir().unwrap();

            // Chain topology: A↔B↔C
            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                t.add_edge(make_sim_edge(id_b, id_c));
                t.add_edge(make_sim_edge(id_c, id_b));
                Arc::new(RwLock::new(t))
            };

            // --- Messengers ---
            let messenger_a = Arc::new(TopologyMessenger::new(id_a, make_topo()));
            let messenger_b = Arc::new(TopologyMessenger::new(id_b, make_topo()));
            let messenger_c = Arc::new(TopologyMessenger::new(id_c, make_topo()));

            // Helper: make raw boxed connections (not Arc) for session establishment
            async fn make_boxed_connection(
                network: &Arc<SimBleNetwork>,
            ) -> (Box<dyn crate::ble::transport::BleConnection>, Box<dyn crate::ble::transport::BleConnection>) {
                let mut device_a = network.create_device();
                device_a.set_mtu(4096);
                let mut device_b = network.create_device();
                device_b.set_mtu(4096);
                let addr_b = device_b.address().clone();

                device_b.start_advertising(vec![0x01]).await.unwrap();
                let accept = tokio::spawn(async move { device_b.accept().await.unwrap() });

                let conn_a = device_a.connect(&addr_b).await.unwrap();
                let conn_b = accept.await.unwrap();

                (conn_a, conn_b)
            }

            // --- Encrypted BLE connections: A↔B ---
            let (raw_ab_a, raw_ab_b) = make_boxed_connection(&network).await;

            let si_a_to_b = SessionIdentity {
                local_private_key: id_a_identity.dh_secret_bytes(),
                local_public_key: id_a_identity.dh_public_bytes(),
                peer_static_public: id_b_identity.dh_public_bytes(),
                psk,
                local_device_id: id_a,
                peer_device_id: id_b,
            };
            let ri_b_from_a = ResponderIdentity {
                local_private_key: id_b_identity.dh_secret_bytes(),
                local_public_key: id_b_identity.dh_public_bytes(),
                psk,
                local_device_id: id_b,
                known_peers: vec![
                    (id_a_identity.dh_public_bytes(), id_a),
                    (id_c_identity.dh_public_bytes(), id_c),
                ],
            };

            // A is initiator (central), B is responder (peripheral)
            let (secure_ab_a, secure_ab_b) = tokio::join!(
                establish_initiator(raw_ab_a, &si_a_to_b),
                establish_responder(raw_ab_b, &ri_b_from_a),
            );
            let secure_ab_a = secure_ab_a.expect("A↔B initiator handshake");
            let secure_ab_b = secure_ab_b.expect("A↔B responder handshake");

            // --- Encrypted BLE connections: B↔C ---
            let (raw_bc_b, raw_bc_c) = make_boxed_connection(&network).await;

            let si_b_to_c = SessionIdentity {
                local_private_key: id_b_identity.dh_secret_bytes(),
                local_public_key: id_b_identity.dh_public_bytes(),
                peer_static_public: id_c_identity.dh_public_bytes(),
                psk,
                local_device_id: id_b,
                peer_device_id: id_c,
            };
            let ri_c_from_b = ResponderIdentity {
                local_private_key: id_c_identity.dh_secret_bytes(),
                local_public_key: id_c_identity.dh_public_bytes(),
                psk,
                local_device_id: id_c,
                known_peers: vec![
                    (id_a_identity.dh_public_bytes(), id_a),
                    (id_b_identity.dh_public_bytes(), id_b),
                ],
            };

            let (secure_bc_b, secure_bc_c) = tokio::join!(
                establish_initiator(raw_bc_b, &si_b_to_c),
                establish_responder(raw_bc_c, &ri_c_from_b),
            );
            let secure_bc_b = secure_bc_b.expect("B↔C initiator handshake");
            let secure_bc_c = secure_bc_c.expect("B↔C responder handshake");

            // Wire encrypted connections to messengers
            messenger_a.add_connection(id_b, Arc::new(secure_ab_a)).await;
            messenger_b.add_connection(id_a, Arc::new(secure_ab_b)).await;
            messenger_b.add_connection(id_c, Arc::new(secure_bc_b)).await;
            messenger_c.add_connection(id_b, Arc::new(secure_bc_c)).await;

            // --- Flows on A and C (B is relay only) ---
            let config = FlowConfig {
                id: flow_id,
                type_name: "drip_hosted:giantt".into(),
                params: serde_json::json!({}),
            };

            let mut flow_a = FullReplicaFlow::new_persistent(
                config.clone(),
                GianttSchema,
                DripHostPolicy::default(),
                id_a,
                "phone".into(),
                tmp_a.path().to_path_buf(),
            );
            flow_a.messenger = Some(Arc::clone(&messenger_a));
            flow_a.topology = Some(make_topo());
            flow_a.register_party("laptop", id_c);

            let mut flow_c = FullReplicaFlow::new_persistent(
                config,
                GianttSchema,
                DripHostPolicy::default(),
                id_c,
                "laptop".into(),
                tmp_c.path().to_path_buf(),
            );
            flow_c.messenger = Some(Arc::clone(&messenger_c));
            flow_c.topology = Some(make_topo());
            flow_c.register_party("phone", id_a);

            // Start C's listener
            flow_c.start().unwrap();

            // A applies ops and flushes through encrypted multi-hop
            flow_a
                .apply_edit(Operation::add_item("encrypted_task", "GianttTask"))
                .unwrap();

            let sent = flow_a.flush_outbound().await;
            assert_eq!(sent.get("laptop"), Some(&1));

            tokio::time::sleep(Duration::from_millis(300)).await;

            // C should have received the task through encrypted relay
            let state_c = flow_c.document.read().unwrap().materialize();
            assert!(
                state_c.get(&"encrypted_task".into()).is_some(),
                "C should have encrypted_task via encrypted multi-hop through B"
            );

            flow_c.stop();
        }

        /// Two independent Giantt flows on the same device, sharing the same
        /// messenger and topology. Verifies that ops on flow_1 don't leak into
        /// flow_2 and vice versa — each flow has its own document, journals,
        /// and outbound queues keyed by flow_id.
        #[tokio::test]
        async fn test_two_giantt_flows_same_device() {
            let network = SimBleNetwork::new();

            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4();
            let flow_id_1 = Uuid::new_v4();
            let flow_id_2 = Uuid::new_v4();

            let tmp_a1 = tempfile::tempdir().unwrap();
            let tmp_a2 = tempfile::tempdir().unwrap();
            let tmp_b1 = tempfile::tempdir().unwrap();
            let tmp_b2 = tempfile::tempdir().unwrap();

            // Shared topology and messenger for each device
            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                Arc::new(RwLock::new(t))
            };

            let topo_a = make_topo();
            let topo_b = make_topo();
            let messenger_a = Arc::new(TopologyMessenger::new(id_a, topo_a.clone()));
            let messenger_b = Arc::new(TopologyMessenger::new(id_b, topo_b.clone()));

            let (conn_a, conn_b) = make_connection(&network).await;
            messenger_a.add_connection(id_b, conn_a).await;
            messenger_b.add_connection(id_a, conn_b).await;

            // --- Flow 1: "work tasks" ---
            let config_1 = FlowConfig {
                id: flow_id_1,
                type_name: "drip_hosted:giantt".into(),
                params: serde_json::json!({}),
            };

            let mut flow_a1 = FullReplicaFlow::new_persistent(
                config_1.clone(),
                GianttSchema,
                DripHostPolicy::default(),
                id_a,
                "phone".into(),
                tmp_a1.path().to_path_buf(),
            );
            flow_a1.messenger = Some(Arc::clone(&messenger_a));
            flow_a1.topology = Some(topo_a.clone());
            flow_a1.register_party("laptop", id_b);

            let mut flow_b1 = FullReplicaFlow::new_persistent(
                config_1,
                GianttSchema,
                DripHostPolicy::default(),
                id_b,
                "laptop".into(),
                tmp_b1.path().to_path_buf(),
            );
            flow_b1.messenger = Some(Arc::clone(&messenger_b));
            flow_b1.topology = Some(topo_b.clone());
            flow_b1.register_party("phone", id_a);
            flow_b1.start().unwrap();

            // --- Flow 2: "personal tasks" ---
            let config_2 = FlowConfig {
                id: flow_id_2,
                type_name: "drip_hosted:giantt".into(),
                params: serde_json::json!({}),
            };

            let mut flow_a2 = FullReplicaFlow::new_persistent(
                config_2.clone(),
                GianttSchema,
                DripHostPolicy::default(),
                id_a,
                "phone".into(),
                tmp_a2.path().to_path_buf(),
            );
            flow_a2.messenger = Some(Arc::clone(&messenger_a));
            flow_a2.topology = Some(topo_a.clone());
            flow_a2.register_party("laptop", id_b);

            let mut flow_b2 = FullReplicaFlow::new_persistent(
                config_2,
                GianttSchema,
                DripHostPolicy::default(),
                id_b,
                "laptop".into(),
                tmp_b2.path().to_path_buf(),
            );
            flow_b2.messenger = Some(Arc::clone(&messenger_b));
            flow_b2.topology = Some(topo_b.clone());
            flow_b2.register_party("phone", id_a);
            flow_b2.start().unwrap();

            // A writes to flow_1 only
            flow_a1
                .apply_edit(Operation::add_item("work_task", "GianttTask"))
                .unwrap();

            // A writes to flow_2 only
            flow_a2
                .apply_edit(Operation::add_item("personal_task", "GianttTask"))
                .unwrap();

            // Flush both flows
            let sent_1 = flow_a1.flush_outbound().await;
            let sent_2 = flow_a2.flush_outbound().await;
            assert_eq!(sent_1.get("laptop"), Some(&1));
            assert_eq!(sent_2.get("laptop"), Some(&1));

            tokio::time::sleep(Duration::from_millis(300)).await;

            // B's flow_1 should have work_task but NOT personal_task
            {
                let state_b1 = flow_b1.document.read().unwrap().materialize();
                assert!(
                    state_b1.get(&"work_task".into()).is_some(),
                    "B flow_1 should have work_task"
                );
                assert!(
                    state_b1.get(&"personal_task".into()).is_none(),
                    "B flow_1 should NOT have personal_task (belongs to flow_2)"
                );
            }

            // B's flow_2 should have personal_task but NOT work_task
            {
                let state_b2 = flow_b2.document.read().unwrap().materialize();
                assert!(
                    state_b2.get(&"personal_task".into()).is_some(),
                    "B flow_2 should have personal_task"
                );
                assert!(
                    state_b2.get(&"work_task".into()).is_none(),
                    "B flow_2 should NOT have work_task (belongs to flow_1)"
                );
            }

            flow_b1.stop();
            flow_b2.stop();
        }

        /// Giantt and Inventory flows running simultaneously on the same device,
        /// sharing the same messenger and topology. Verifies that different schema
        /// types coexist without interference.
        #[tokio::test]
        async fn test_giantt_and_inventory_flows_simultaneously() {
            use crate::convergent::inventory::InventorySchema;

            let network = SimBleNetwork::new();

            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4();
            let giantt_flow_id = Uuid::new_v4();
            let inventory_flow_id = Uuid::new_v4();

            let tmp_a_giantt = tempfile::tempdir().unwrap();
            let tmp_a_inventory = tempfile::tempdir().unwrap();
            let tmp_b_giantt = tempfile::tempdir().unwrap();
            let tmp_b_inventory = tempfile::tempdir().unwrap();

            let make_topo = || {
                let mut t = EnsembleTopology::new();
                t.add_edge(make_sim_edge(id_a, id_b));
                t.add_edge(make_sim_edge(id_b, id_a));
                Arc::new(RwLock::new(t))
            };

            let topo_a = make_topo();
            let topo_b = make_topo();
            let messenger_a = Arc::new(TopologyMessenger::new(id_a, topo_a.clone()));
            let messenger_b = Arc::new(TopologyMessenger::new(id_b, topo_b.clone()));

            let (conn_a, conn_b) = make_connection(&network).await;
            messenger_a.add_connection(id_b, conn_a).await;
            messenger_b.add_connection(id_a, conn_b).await;

            // --- Giantt flow ---
            let giantt_config = FlowConfig {
                id: giantt_flow_id,
                type_name: "drip_hosted:giantt".into(),
                params: serde_json::json!({}),
            };

            let mut flow_a_giantt = FullReplicaFlow::new_persistent(
                giantt_config.clone(),
                GianttSchema,
                DripHostPolicy::default(),
                id_a,
                "phone".into(),
                tmp_a_giantt.path().to_path_buf(),
            );
            flow_a_giantt.messenger = Some(Arc::clone(&messenger_a));
            flow_a_giantt.topology = Some(topo_a.clone());
            flow_a_giantt.register_party("laptop", id_b);

            let mut flow_b_giantt = FullReplicaFlow::new_persistent(
                giantt_config,
                GianttSchema,
                DripHostPolicy::default(),
                id_b,
                "laptop".into(),
                tmp_b_giantt.path().to_path_buf(),
            );
            flow_b_giantt.messenger = Some(Arc::clone(&messenger_b));
            flow_b_giantt.topology = Some(topo_b.clone());
            flow_b_giantt.register_party("phone", id_a);
            flow_b_giantt.start().unwrap();

            // --- Inventory flow ---
            let inventory_config = FlowConfig {
                id: inventory_flow_id,
                type_name: "drip_hosted:inventory".into(),
                params: serde_json::json!({}),
            };

            let mut flow_a_inv = FullReplicaFlow::new_persistent(
                inventory_config.clone(),
                InventorySchema,
                DripHostPolicy::default(),
                id_a,
                "phone".into(),
                tmp_a_inventory.path().to_path_buf(),
            );
            flow_a_inv.messenger = Some(Arc::clone(&messenger_a));
            flow_a_inv.topology = Some(topo_a.clone());
            flow_a_inv.register_party("laptop", id_b);

            let mut flow_b_inv = FullReplicaFlow::new_persistent(
                inventory_config,
                InventorySchema,
                DripHostPolicy::default(),
                id_b,
                "laptop".into(),
                tmp_b_inventory.path().to_path_buf(),
            );
            flow_b_inv.messenger = Some(Arc::clone(&messenger_b));
            flow_b_inv.topology = Some(topo_b.clone());
            flow_b_inv.register_party("phone", id_a);
            flow_b_inv.start().unwrap();

            // A writes a Giantt task
            flow_a_giantt
                .apply_edit(Operation::add_item("write_report", "GianttTask"))
                .unwrap();

            // A writes an inventory item
            flow_a_inv
                .apply_edit(Operation::add_item("hammer", "InventoryItem"))
                .unwrap();
            flow_a_inv
                .apply_edit(Operation::set_field(
                    "hammer",
                    "description",
                    Value::string("Claw hammer"),
                ))
                .unwrap();

            // Flush both flows
            let sent_giantt = flow_a_giantt.flush_outbound().await;
            let sent_inv = flow_a_inv.flush_outbound().await;
            assert_eq!(sent_giantt.get("laptop"), Some(&1));
            assert_eq!(sent_inv.get("laptop"), Some(&2));

            tokio::time::sleep(Duration::from_millis(300)).await;

            // B's Giantt flow should have the task but NOT the inventory item
            {
                let state = flow_b_giantt.document.read().unwrap().materialize();
                assert!(
                    state.get(&"write_report".into()).is_some(),
                    "B giantt flow should have write_report"
                );
                assert!(
                    state.get(&"hammer".into()).is_none(),
                    "B giantt flow should NOT have hammer (belongs to inventory flow)"
                );
            }

            // B's Inventory flow should have the item but NOT the task
            {
                let state = flow_b_inv.document.read().unwrap().materialize();
                assert!(
                    state.get(&"hammer".into()).is_some(),
                    "B inventory flow should have hammer"
                );
                assert!(
                    state.get(&"write_report".into()).is_none(),
                    "B inventory flow should NOT have write_report (belongs to giantt flow)"
                );
            }

            flow_b_giantt.stop();
            flow_b_inv.stop();
        }
    }
}
