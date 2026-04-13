//! Unified FFI for convergent-document flows.
//!
//! A single FFI surface for any schema backed by `DripHostedFlow`.
//! The schema is selected at open time via a name string ("giantt", "inventory").
//!
//! FFI lifecycle:
//!   soradyne_flow_init(device_id)
//!   handle = soradyne_flow_open(uuid, schema)
//!   soradyne_flow_write_op(handle, op_json)
//!   soradyne_flow_read_drip(handle)          // returns schema-specific text
//!   soradyne_flow_close(handle)
//!   soradyne_flow_cleanup()

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use uuid::Uuid;

use crate::convergent::giantt::{GianttSchema, GianttState};
use crate::convergent::inventory::{InventorySchema, InventoryState};
use crate::convergent::{DeviceId, OpEnvelope, Operation};
use crate::flow::flow_core::{Flow, FlowConfig};
use crate::flow::types::drip_hosted::{DripHostPolicy, DripHostedFlow};

use super::serializer::{serialize_giantt_state, serialize_inventory_state};

// ============================================================================
// Schema-erased flow wrapper
// ============================================================================

/// Dispatch macro: runs the same expression against whichever `DripHostedFlow<S>`
/// variant is inside a `SchemaFlow`. Works because all variants expose identical
/// method signatures via the `Flow` trait and `DripHostedFlow` API.
macro_rules! with_drip_flow {
    ($self:expr, $flow:ident => $body:expr) => {
        match &$self.inner {
            SchemaFlow::Giantt($flow) => $body,
            SchemaFlow::Inventory($flow) => $body,
        }
    };
}

macro_rules! with_drip_flow_mut {
    ($self:expr, $flow:ident => $body:expr) => {
        match &mut $self.inner {
            SchemaFlow::Giantt($flow) => $body,
            SchemaFlow::Inventory($flow) => $body,
        }
    };
}

enum SchemaFlow {
    Giantt(DripHostedFlow<GianttSchema>),
    Inventory(DripHostedFlow<InventorySchema>),
}

/// A schema-erased convergent flow.
///
/// Wraps a `DripHostedFlow<S>` for any supported schema. The wrapper is thin:
/// all persistence, sync, and CRDT convergence live inside `DripHostedFlow`.
/// This struct handles schema-specific concerns: op pre-processing (auto-fill
/// of RemoveFromSet add-IDs) and state serialization.
pub struct ConvergentFlow {
    inner: SchemaFlow,
}

impl ConvergentFlow {
    pub fn new_in_memory(schema_name: &str, device_id: DeviceId, flow_uuid: Uuid) -> Option<Self> {
        let device_uuid = device_uuid_from_id(&device_id);
        let type_name = format!("drip_hosted:{}", schema_name);
        let config = FlowConfig {
            id: flow_uuid,
            type_name,
            params: serde_json::json!({}),
        };
        let inner = match schema_name {
            "giantt" => SchemaFlow::Giantt(DripHostedFlow::new(
                config,
                GianttSchema,
                DripHostPolicy::default(),
                device_uuid,
                device_id,
            )),
            "inventory" => SchemaFlow::Inventory(DripHostedFlow::new(
                config,
                InventorySchema,
                DripHostPolicy::default(),
                device_uuid,
                device_id,
            )),
            _ => return None,
        };
        Some(Self { inner })
    }

    pub fn new_persistent(
        schema_name: &str,
        device_id: DeviceId,
        storage_path: PathBuf,
        flow_uuid: Uuid,
    ) -> Option<Self> {
        let device_uuid = device_uuid_from_id(&device_id);
        let type_name = format!("drip_hosted:{}", schema_name);
        let config = FlowConfig {
            id: flow_uuid,
            type_name,
            params: serde_json::json!({}),
        };
        let inner = match schema_name {
            "giantt" => SchemaFlow::Giantt(DripHostedFlow::new_persistent(
                config,
                GianttSchema,
                DripHostPolicy::default(),
                device_uuid,
                device_id,
                storage_path,
            )),
            "inventory" => SchemaFlow::Inventory(DripHostedFlow::new_persistent(
                config,
                InventorySchema,
                DripHostPolicy::default(),
                device_uuid,
                device_id,
                storage_path,
            )),
            _ => return None,
        };
        Some(Self { inner })
    }

    /// Apply a local operation.
    ///
    /// For RemoveFromSet with empty observed_add_ids, automatically fills in
    /// the add IDs from the document so the caller doesn't need to track them.
    pub fn apply_operation(&mut self, op: Operation) -> OpEnvelope {
        let op = match op {
            Operation::RemoveFromSet {
                ref item_id,
                ref set_name,
                ref element,
                ref observed_add_ids,
            } if observed_add_ids.is_empty() => {
                let add_ids = with_drip_flow!(self, flow => {
                    flow.document()
                        .read()
                        .unwrap()
                        .get_add_ids_for_element(item_id, set_name, element)
                });
                Operation::remove_from_set(
                    item_id.clone(),
                    set_name.clone(),
                    element.clone(),
                    add_ids,
                )
            }
            other => other,
        };

        with_drip_flow_mut!(self, flow => flow.apply_edit(op).unwrap())
    }

    /// Apply a remote operation (received from another device).
    pub fn apply_remote(&mut self, envelope: OpEnvelope) {
        with_drip_flow_mut!(self, flow => flow.apply_remote_op(envelope));
    }

    /// Get all operations (for syncing to other devices).
    pub fn all_operations(&self) -> Vec<OpEnvelope> {
        with_drip_flow!(self, flow => {
            flow.document()
                .read()
                .unwrap()
                .all_operations()
                .cloned()
                .collect()
        })
    }

    /// Materialize and serialize to the schema's native format.
    ///
    /// - Giantt → `.giantt` text notation
    /// - Inventory → JSON
    pub fn read_drip(&self) -> String {
        match &self.inner {
            SchemaFlow::Giantt(flow) => {
                let doc_state = flow.document().read().unwrap().materialize();
                let state = GianttState::from_document_state(&doc_state);
                serialize_giantt_state(&state)
            }
            SchemaFlow::Inventory(flow) => {
                let doc_state = flow.document().read().unwrap().materialize();
                let state = InventoryState::from_document_state(&doc_state);
                serialize_inventory_state(&state)
            }
        }
    }

    /// Write the materialized state to a file on demand.
    pub fn write_snapshot(&self, path: &std::path::Path) -> std::io::Result<()> {
        let text = self.read_drip();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)
    }

    /// Wire this flow to an ensemble's messenger and topology for sync.
    pub fn set_ensemble(
        &mut self,
        messenger: Arc<crate::topology::messenger::TopologyMessenger>,
        topology: Arc<tokio::sync::RwLock<crate::topology::ensemble::EnsembleTopology>>,
    ) {
        eprintln!("[ConvergentFlow] set_ensemble called");
        with_drip_flow_mut!(self, f => f.set_ensemble(messenger, topology));
    }

    /// Start background sync tasks (inbound listener, flush, topology watcher).
    ///
    /// The flow must have an ensemble wired via [`set_ensemble`] first.
    pub fn start(&self) -> Result<(), crate::flow::FlowError> {
        eprintln!("[ConvergentFlow] start called");
        with_drip_flow!(self, f => f.start())
    }
}

/// Parse a device UUID from a string device_id.
///
/// Panics if the string is not a valid UUID. The device_id always comes from
/// `DeviceIdentity::device_id().to_string()`, so this should never fail in
/// practice.
fn device_uuid_from_id(device_id: &str) -> Uuid {
    Uuid::parse_str(device_id)
        .unwrap_or_else(|e| panic!("device_id is not a valid UUID: \"{}\": {}", device_id, e))
}

// ============================================================================
// Registry
// ============================================================================

static FLOW_REGISTRY: RwLock<Option<FlowRegistry>> = RwLock::new(None);

struct FlowRegistry {
    flows: HashMap<String, Arc<Mutex<ConvergentFlow>>>,
    device_id: DeviceId,
    data_dir: PathBuf,
    runtime: tokio::runtime::Runtime,
}

impl FlowRegistry {
    fn new(device_id: DeviceId) -> Self {
        let data_dir = if cfg!(target_os = "macos") {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join("Library/Application Support/Soradyne/flows")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".soradyne/flows")
        };

        let _ = std::fs::create_dir_all(&data_dir);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("convergent-flow")
            .build()
            .expect("failed to build Tokio runtime for ConvergentFlow");

        Self {
            flows: HashMap::new(),
            device_id,
            data_dir,
            runtime,
        }
    }

    fn get_or_open(&mut self, uuid: &str, schema_name: &str) -> Option<Arc<Mutex<ConvergentFlow>>> {
        if let Some(flow) = self.flows.get(uuid) {
            return Some(Arc::clone(flow));
        }

        let flow_dir = self.data_dir.join(uuid);
        let is_test = uuid.starts_with("test-");
        let flow_uuid = Uuid::parse_str(uuid).unwrap_or_else(|_| Uuid::new_v4());

        let flow = if is_test {
            ConvergentFlow::new_in_memory(schema_name, self.device_id.clone(), flow_uuid)?
        } else {
            ConvergentFlow::new_persistent(schema_name, self.device_id.clone(), flow_dir, flow_uuid)?
        };

        let flow = Arc::new(Mutex::new(flow));
        self.flows.insert(uuid.to_string(), Arc::clone(&flow));
        Some(flow)
    }

    fn close(&mut self, uuid: &str) {
        self.flows.remove(uuid);
    }
}

// ============================================================================
// FFI Functions
// ============================================================================

/// Initialize the flow system with a device ID.
///
/// Must be called before any other flow FFI functions.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_init(device_id_ptr: *const c_char) -> i32 {
    if device_id_ptr.is_null() {
        return -1;
    }

    let device_id = unsafe {
        match CStr::from_ptr(device_id_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return -1,
        }
    };

    let mut registry = match FLOW_REGISTRY.write() {
        Ok(r) => r,
        Err(_) => return -1,
    };

    *registry = Some(FlowRegistry::new(device_id.into()));
    0
}

/// Open a flow by UUID and schema name.
///
/// `schema_ptr`: `"giantt"` or `"inventory"`.
/// Returns an opaque handle on success, null on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_open(
    uuid_ptr: *const c_char,
    schema_ptr: *const c_char,
) -> *mut std::ffi::c_void {
    if uuid_ptr.is_null() || schema_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let uuid = unsafe {
        match CStr::from_ptr(uuid_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let schema_name = unsafe {
        match CStr::from_ptr(schema_ptr).to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let mut registry = match FLOW_REGISTRY.write() {
        Ok(r) => r,
        Err(_) => return std::ptr::null_mut(),
    };

    let registry = match registry.as_mut() {
        Some(r) => r,
        None => return std::ptr::null_mut(),
    };

    match registry.get_or_open(&uuid, schema_name) {
        Some(flow) => Arc::into_raw(flow) as *mut std::ffi::c_void,
        None => std::ptr::null_mut(),
    }
}

/// Close a flow handle.
#[no_mangle]
pub extern "C" fn soradyne_flow_close(handle: *mut std::ffi::c_void) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let _ = Arc::from_raw(handle as *const Mutex<ConvergentFlow>);
    }
}

/// Write an operation to a flow.
///
/// op_json: JSON-encoded Operation (see convergent::Operation enum).
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_write_op(
    handle: *mut std::ffi::c_void,
    op_json_ptr: *const c_char,
) -> i32 {
    if handle.is_null() || op_json_ptr.is_null() {
        return -1;
    }

    let op_json = unsafe {
        match CStr::from_ptr(op_json_ptr).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };

    let op: Operation = match serde_json::from_str(op_json) {
        Ok(op) => op,
        Err(e) => {
            eprintln!("Failed to parse operation JSON: {}", e);
            return -1;
        }
    };

    let flow = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow.lock() {
        Ok(mut flow) => {
            flow.apply_operation(op);
            0
        }
        Err(_) => -1,
    }
}

/// Read the current state in the schema's native format.
///
/// Returns a C string that must be freed with soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_flow_read_drip(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow.lock() {
        Ok(flow) => {
            let text = flow.read_drip();
            match CString::new(text) {
                Ok(c_str) => c_str.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Write the materialized state to a file at the given path.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_write_snapshot(
    handle: *mut std::ffi::c_void,
    path_ptr: *const c_char,
) -> i32 {
    if handle.is_null() || path_ptr.is_null() {
        return -1;
    }

    let path_str = unsafe {
        match CStr::from_ptr(path_ptr).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };

    let flow = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };
    match flow.lock() {
        Ok(flow) => match flow.write_snapshot(std::path::Path::new(path_str)) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("[convergent_flow] write_snapshot failed: {}", e);
                -1
            }
        },
        Err(_) => -1,
    }
}

/// Get all operations as JSON array (for syncing).
///
/// Returns a C string that must be freed with soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_flow_get_operations(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow.lock() {
        Ok(flow) => {
            let ops = flow.all_operations();
            match serde_json::to_string(&ops) {
                Ok(json) => match CString::new(json) {
                    Ok(c_str) => c_str.into_raw(),
                    Err(_) => std::ptr::null_mut(),
                },
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Apply remote operations (received from another device).
///
/// ops_json: JSON array of OpEnvelope objects.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_apply_remote(
    handle: *mut std::ffi::c_void,
    ops_json_ptr: *const c_char,
) -> i32 {
    if handle.is_null() || ops_json_ptr.is_null() {
        return -1;
    }

    let ops_json = unsafe {
        match CStr::from_ptr(ops_json_ptr).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };

    let ops: Vec<OpEnvelope> = match serde_json::from_str(ops_json) {
        Ok(ops) => ops,
        Err(e) => {
            eprintln!("Failed to parse remote operations JSON: {}", e);
            return -1;
        }
    };

    let flow = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow.lock() {
        Ok(mut flow) => {
            for op in ops {
                flow.apply_remote(op);
            }
            0
        }
        Err(_) => -1,
    }
}

/// Close a flow by UUID (alternative to handle-based close).
#[no_mangle]
pub extern "C" fn soradyne_flow_close_by_uuid(uuid_ptr: *const c_char) {
    if uuid_ptr.is_null() {
        return;
    }

    let uuid = unsafe {
        match CStr::from_ptr(uuid_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return,
        }
    };

    if let Ok(mut registry) = FLOW_REGISTRY.write() {
        if let Some(ref mut registry) = *registry {
            registry.close(&uuid);
        }
    }
}

/// Cleanup the flow system.
#[no_mangle]
pub extern "C" fn soradyne_flow_cleanup() {
    if let Ok(mut registry) = FLOW_REGISTRY.write() {
        *registry = None;
    }
}

// ============================================================================
// Sync FFI Functions
// ============================================================================

/// Connect a flow to an ensemble for sync.
///
/// capsule_id_ptr: UUID string of the capsule to sync within.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_connect_ensemble(
    handle: *mut std::ffi::c_void,
    capsule_id_ptr: *const c_char,
) -> i32 {
    if handle.is_null() || capsule_id_ptr.is_null() {
        return -1;
    }

    let capsule_id_str = unsafe {
        match CStr::from_ptr(capsule_id_ptr).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };

    let capsule_id = match Uuid::parse_str(capsule_id_str) {
        Ok(u) => u,
        Err(_) => return -1,
    };

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match super::pairing_bridge::bridge_get_ensemble(capsule_id) {
        Ok((messenger, topology)) => {
            if let Ok(mut flow) = flow_arc.lock() {
                use crate::flow::flow_core::Flow;
                with_drip_flow_mut!(flow, f => f.set_ensemble(messenger, topology));
                0
            } else {
                -1
            }
        }
        Err(_) => -1,
    }
}

/// Start background sync for a flow.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_start_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            let result = with_drip_flow!(flow, f => f.start());
            match result {
                Ok(()) => 0,
                Err(_) => -1,
            }
        }
        Err(_) => -1,
    }
}

/// Enable sync for a flow without requiring a capsule ID.
///
/// Looks up the first capsule in the local store, wires the flow to its
/// ensemble, and starts background sync. The caller never needs to know
/// which capsule is involved.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_enable_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    let capsule_id = match super::pairing_bridge::bridge_first_capsule_id() {
        Ok(id) => id,
        Err(e) => {
            eprintln!("soradyne_flow_enable_sync: {}", e);
            return -1;
        }
    };

    match super::pairing_bridge::bridge_get_ensemble(capsule_id) {
        Ok((messenger, topology)) => {
            if let Ok(mut flow) = flow_arc.lock() {
                use crate::flow::flow_core::Flow;
                with_drip_flow_mut!(flow, f => f.set_ensemble(messenger, topology));
            } else {
                return -1;
            }
        }
        Err(e) => {
            eprintln!("soradyne_flow_enable_sync: ensemble error: {}", e);
            return -1;
        }
    }

    // Start sync
    match flow_arc.lock() {
        Ok(flow) => {
            let result = with_drip_flow!(flow, f => f.start());
            match result {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("soradyne_flow_enable_sync: start error: {:?}", e);
                    -1
                }
            }
        }
        Err(_) => -1,
    }
}

/// Stop background sync for a flow.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_stop_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            with_drip_flow!(flow, f => f.stop());
            0
        }
        Err(_) => -1,
    }
}

/// Get the sync status of a flow as JSON.
///
/// Returns JSON: {"is_host": bool, "host_id": "uuid"|null, "epoch": n, "connected": bool}
/// Caller must free via soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_flow_get_sync_status(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            let (is_host, host_id, epoch) = with_drip_flow!(flow, f => {
                let is_host = f.is_current_host();
                let ha = f.host_assignment().read().ok();
                let (host_id, epoch) = match ha.as_ref() {
                    Some(ha) => (ha.current_host.map(|h| h.to_string()), ha.epoch),
                    None => (None, 0),
                };
                (is_host, host_id, epoch)
            });

            let json = serde_json::json!({
                "is_host": is_host,
                "host_id": host_id,
                "epoch": epoch,
                "connected": is_host || host_id.is_some(),
            });

            match CString::new(json.to_string()) {
                Ok(c_str) => c_str.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convergent::Value;

    // -- Giantt tests ---------------------------------------------------------

    #[test]
    fn test_giantt_basic_operations() {
        let mut flow =
            ConvergentFlow::new_in_memory("giantt", "test_device".into(), Uuid::new_v4()).unwrap();

        flow.apply_operation(Operation::add_item("task_1", "GianttItem"));
        flow.apply_operation(Operation::set_field(
            "task_1",
            "title",
            Value::string("My Task"),
        ));
        flow.apply_operation(Operation::set_field(
            "task_1",
            "status",
            Value::string("TODO"),
        ));
        flow.apply_operation(Operation::set_field(
            "task_1",
            "priority",
            Value::string("HIGH"),
        ));
        flow.apply_operation(Operation::set_field(
            "task_1",
            "duration",
            Value::string("2d"),
        ));

        let text = flow.read_drip();
        assert!(text.contains("task_1"));
        assert!(text.contains("My Task"));
    }

    #[test]
    fn test_giantt_with_tags_and_charts() {
        let mut flow =
            ConvergentFlow::new_in_memory("giantt", "test_device".into(), Uuid::new_v4()).unwrap();

        flow.apply_operation(Operation::add_item("task_1", "GianttItem"));
        flow.apply_operation(Operation::set_field(
            "task_1",
            "title",
            Value::string("Tagged Task"),
        ));
        flow.apply_operation(Operation::add_to_set(
            "task_1",
            "tags",
            Value::string("important"),
        ));
        flow.apply_operation(Operation::add_to_set(
            "task_1",
            "tags",
            Value::string("urgent"),
        ));
        flow.apply_operation(Operation::add_to_set(
            "task_1",
            "charts",
            Value::string("Sprint1"),
        ));

        let text = flow.read_drip();
        assert!(text.contains("Tagged Task"));
        assert!(text.contains("important") || text.contains("urgent"));
    }

    #[test]
    fn test_giantt_persistence() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_convergent_giantt_persistence");
        let _ = std::fs::remove_dir_all(&temp_dir);

        {
            let mut flow = ConvergentFlow::new_persistent(
                "giantt",
                "test_device_dhf".into(),
                temp_dir.clone(),
                Uuid::new_v4(),
            )
            .unwrap();
            flow.apply_operation(Operation::add_item("task_1", "GianttItem"));
            flow.apply_operation(Operation::set_field(
                "task_1",
                "title",
                Value::string("Persistent Task"),
            ));
            flow.apply_operation(Operation::set_field(
                "task_1",
                "status",
                Value::string("DONE"),
            ));
        }

        {
            let flow = ConvergentFlow::new_persistent(
                "giantt",
                "test_device_dhf".into(),
                temp_dir.clone(),
                Uuid::new_v4(),
            )
            .unwrap();
            let text = flow.read_drip();
            assert!(text.contains("task_1"));
            assert!(text.contains("Persistent Task"));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // -- Inventory tests ------------------------------------------------------

    #[test]
    fn test_inventory_basic_operations() {
        let mut flow =
            ConvergentFlow::new_in_memory("inventory", "test_device".into(), Uuid::new_v4())
                .unwrap();

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "category",
            Value::string("Tools"),
        ));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "location",
            Value::string("Toolbox"),
        ));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let items = parsed["items"].as_object().unwrap();
        assert_eq!(items.len(), 1);

        let item = &items["item_1"];
        assert_eq!(item["category"], "Tools");
        assert_eq!(item["description"], "Hammer");
        assert_eq!(item["location"], "Toolbox");
    }

    #[test]
    fn test_inventory_delete() {
        let mut flow =
            ConvergentFlow::new_in_memory("inventory", "test_device".into(), Uuid::new_v4())
                .unwrap();

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["items"].as_object().unwrap().len(), 1);

        flow.apply_operation(Operation::remove_item("item_1"));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["items"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_inventory_tags() {
        let mut flow =
            ConvergentFlow::new_in_memory("inventory", "test_device".into(), Uuid::new_v4())
                .unwrap();

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "description",
            Value::string("Vase"),
        ));
        flow.apply_operation(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("fragile"),
        ));
        flow.apply_operation(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("valuable"),
        ));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = parsed["items"]["item_1"]["tags"].as_array().unwrap();

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&serde_json::json!("fragile")));
        assert!(tags.contains(&serde_json::json!("valuable")));
    }

    #[test]
    fn test_inventory_remote_sync() {
        let mut flow_a =
            ConvergentFlow::new_in_memory("inventory", "device_A".into(), Uuid::new_v4()).unwrap();
        let mut flow_b =
            ConvergentFlow::new_in_memory("inventory", "device_B".into(), Uuid::new_v4()).unwrap();

        let op1 = flow_a.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        let op2 = flow_a.apply_operation(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));
        let op3 = flow_a.apply_operation(Operation::set_field(
            "item_1",
            "location",
            Value::string("Kitchen"),
        ));

        flow_b.apply_remote(op1);
        flow_b.apply_remote(op2);
        flow_b.apply_remote(op3);

        let op4 = flow_b.apply_operation(Operation::set_field(
            "item_1",
            "location",
            Value::string("Garage"),
        ));

        flow_a.apply_remote(op4);

        let json_a = flow_a.read_drip();
        let json_b = flow_b.read_drip();
        let parsed_a: serde_json::Value = serde_json::from_str(&json_a).unwrap();
        let parsed_b: serde_json::Value = serde_json::from_str(&json_b).unwrap();

        assert_eq!(
            parsed_a["items"]["item_1"]["location"],
            parsed_b["items"]["item_1"]["location"]
        );
    }

    #[test]
    fn test_inventory_persistence() {
        let temp_dir =
            std::env::temp_dir().join("soradyne_test_convergent_inventory_persistence");
        let _ = std::fs::remove_dir_all(&temp_dir);

        {
            let mut flow = ConvergentFlow::new_persistent(
                "inventory",
                "test_device".into(),
                temp_dir.clone(),
                Uuid::new_v4(),
            )
            .unwrap();
            flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
            flow.apply_operation(Operation::set_field(
                "item_1",
                "category",
                Value::string("Tools"),
            ));
            flow.apply_operation(Operation::set_field(
                "item_1",
                "description",
                Value::string("Hammer"),
            ));
            flow.apply_operation(Operation::set_field(
                "item_1",
                "location",
                Value::string("Toolbox"),
            ));
        }

        {
            let flow = ConvergentFlow::new_persistent(
                "inventory",
                "test_device".into(),
                temp_dir.clone(),
                Uuid::new_v4(),
            )
            .unwrap();
            let json = flow.read_drip();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

            let item = &parsed["items"]["item_1"];
            assert_eq!(item["description"], "Hammer");
            assert_eq!(item["location"], "Toolbox");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_inventory_remove_tag_auto_fill() {
        let mut flow =
            ConvergentFlow::new_in_memory("inventory", "test_device".into(), Uuid::new_v4())
                .unwrap();

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("fragile"),
        ));
        flow.apply_operation(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("valuable"),
        ));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = parsed["items"]["item_1"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);

        flow.apply_operation(Operation::remove_from_set(
            "item_1",
            "tags",
            Value::string("fragile"),
            vec![],
        ));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = parsed["items"]["item_1"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&serde_json::json!("valuable")));
        assert!(!tags.contains(&serde_json::json!("fragile")));
    }

    #[test]
    fn test_unknown_schema_returns_none() {
        let result =
            ConvergentFlow::new_in_memory("unknown_schema", "test_device".into(), Uuid::new_v4());
        assert!(result.is_none());
    }
}
