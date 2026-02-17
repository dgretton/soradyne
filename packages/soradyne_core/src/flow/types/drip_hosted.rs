//! DripHostedFlow — a flow where one piece hosts an authoritative CRDT document.
//!
//! Other pieces send edits, the host merges them, and topology changes trigger
//! host re-evaluation with policy-driven failover. Under OfflineMerge failover,
//! all pieces accept edits locally and converge via CRDT when they reconnect.
//!
//! # Wire Protocol
//!
//! Messages are serialized as CBOR and carried as `RoutedEnvelope` payloads
//! with `MessageType::FlowSync`.
//!
//! # Phase 5 of the capsule-ensemble implementation plan.
//! Phase 5.5 will wire this to Giantt/Inventory via FFI.

use std::collections::HashMap;
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
use crate::topology::ensemble::EnsembleTopology;
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

/// A flow type where one piece hosts an authoritative CRDT document.
///
/// Streams:
/// - `"state"` (drip, singleton): The convergent document state.
///
/// The host is selected by policy. Under `OfflineMerge` (the default),
/// all pieces accept edits locally and converge via CRDT. Under
/// `WaitForHost`, non-host pieces queue edits until the host is available.
pub struct DripHostedFlow<S: DocumentSchema + 'static> {
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
}

impl<S: DocumentSchema + 'static> DripHostedFlow<S> {
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

    /// Start the sync listener task. Subscribes to incoming messages from
    /// the messenger and dispatches FlowSync messages.
    pub fn start(&self) -> Result<(), FlowError> {
        let messenger = self
            .messenger
            .as_ref()
            .ok_or_else(|| FlowError::SyncError("no messenger configured".into()))?;

        let mut rx = messenger.incoming();
        let flow_id = self.id;
        let document = Arc::clone(&self.document);
        let host_assignment = Arc::clone(&self.host_assignment);
        let pending = Arc::clone(&self.pending_edits);
        let policy = self.policy.clone();
        let device_uuid = self.device_uuid;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Ok(envelope) = rx.recv() => {
                        if envelope.message_type != MessageType::FlowSync {
                            continue;
                        }
                        let msg = match FlowSyncMessage::from_cbor(&envelope.payload) {
                            Ok(m) => m,
                            Err(_) => continue,
                        };
                        // Only handle messages for our flow
                        let msg_flow_id = match &msg {
                            FlowSyncMessage::HorizonExchange { flow_id, .. } => *flow_id,
                            FlowSyncMessage::OperationBatch { flow_id, .. } => *flow_id,
                            FlowSyncMessage::HostAnnouncement { flow_id, .. } => *flow_id,
                        };
                        if msg_flow_id != flow_id {
                            continue;
                        }
                        Self::handle_flow_sync_static(
                            &document,
                            &host_assignment,
                            &pending,
                            &policy,
                            device_uuid,
                            envelope.source,
                            msg,
                        );
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

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

                // Broadcast to peers if we have a messenger
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

                Ok(envelope)
            }
        }
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

        // Drain and apply pending edits
        if let Ok(mut pending) = self.pending_edits.write() {
            let ops: Vec<Operation> = pending.drain(..).collect();
            if let Ok(mut doc) = self.document.write() {
                for op in ops {
                    doc.apply_local(op);
                }
            }
        }

        // Broadcast host announcement
        if let Some(messenger) = &self.messenger {
            let msg = FlowSyncMessage::HostAnnouncement {
                flow_id: self.id,
                host_id: self.device_uuid,
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

    /// Process an incoming FlowSyncMessage.
    pub fn handle_flow_sync(&self, source: Uuid, msg: FlowSyncMessage) {
        Self::handle_flow_sync_static(
            &self.document,
            &self.host_assignment,
            &self.pending_edits,
            &self.policy,
            self.device_uuid,
            source,
            msg,
        );
    }

    /// Static handler (used by both the sync task and direct calls).
    fn handle_flow_sync_static(
        document: &Arc<std::sync::RwLock<ConvergentDocument<S>>>,
        host_assignment: &Arc<std::sync::RwLock<HostAssignment>>,
        _pending: &Arc<std::sync::RwLock<Vec<Operation>>>,
        _policy: &DripHostPolicy,
        _device_uuid: Uuid,
        _source: Uuid,
        msg: FlowSyncMessage,
    ) {
        match msg {
            FlowSyncMessage::HorizonExchange { .. } => {
                // Peer is requesting sync — we'd respond with our ops since
                // their horizon. For now, handled at the orchestration layer
                // (the caller pairs HorizonExchange with OperationBatch).
            }
            FlowSyncMessage::OperationBatch { ops, .. } => {
                if let Ok(mut doc) = document.write() {
                    for op in ops {
                        doc.apply_remote(op);
                    }
                }
            }
            FlowSyncMessage::HostAnnouncement {
                host_id, epoch, ..
            } => {
                if let Ok(mut ha) = host_assignment.write() {
                    ha.accept_announcement(host_id, epoch);
                }
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

impl<S: DocumentSchema + 'static> Flow for DripHostedFlow<S> {
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

    let flow = DripHostedFlow::new(config, GianttSchema, policy, device_uuid, device_id);
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

    let flow = DripHostedFlow::new(config, InventorySchema, policy, device_uuid, device_id);
    Ok(Box::new(flow))
}

/// Register DripHostedFlow constructors for all known schemas.
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
    // DripHostedFlow failover tests
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
    }
}
