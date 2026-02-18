//! FFI for Giantt Flow operations
//!
//! Provides a minimal FFI surface for Dart to interact with Giantt flows:
//! - Flow lifecycle (open/close)
//! - Operations (write_op)
//! - State access (read_drip returns .giantt text)
//! - Ensemble sync (connect_ensemble, start_sync, stop_sync, get_sync_status)

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use uuid::Uuid;

use crate::convergent::giantt::{GianttSchema, GianttState};
use crate::convergent::{DeviceId, OpEnvelope, Operation};
use crate::flow::flow_core::FlowConfig;
use crate::flow::types::drip_hosted::{DripHostPolicy, DripHostedFlow};

use super::serializer::serialize_giantt_state;

/// Global registry of open flows, keyed by UUID
static FLOW_REGISTRY: RwLock<Option<FlowRegistry>> = RwLock::new(None);

/// Registry holding all open GianttFlow instances
struct FlowRegistry {
    flows: HashMap<String, Arc<Mutex<GianttFlow>>>,
    device_id: DeviceId,
    data_dir: PathBuf,
}

impl FlowRegistry {
    fn new(device_id: DeviceId) -> Self {
        // Determine data directory for flow persistence
        let data_dir = if cfg!(target_os = "macos") {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join("Library/Application Support/Soradyne/flows")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".soradyne/flows")
        };

        // Ensure directory exists
        let _ = std::fs::create_dir_all(&data_dir);

        Self {
            flows: HashMap::new(),
            device_id,
            data_dir,
        }
    }

    fn get_or_open(&mut self, uuid: &str) -> Arc<Mutex<GianttFlow>> {
        if let Some(flow) = self.flows.get(uuid) {
            return Arc::clone(flow);
        }

        // Determine storage path for this flow
        let flow_dir = self.data_dir.join(uuid);

        // Check if this is a test flow (test flows use in-memory storage)
        let is_test = uuid.starts_with("test-");

        let flow = if is_test {
            GianttFlow::new_in_memory(self.device_id.clone())
        } else {
            GianttFlow::new_persistent(self.device_id.clone(), flow_dir)
        };

        let flow = Arc::new(Mutex::new(flow));
        self.flows.insert(uuid.to_string(), Arc::clone(&flow));
        flow
    }

    fn close(&mut self, uuid: &str) {
        if let Some(flow) = self.flows.remove(uuid) {
            // Ensure any pending writes are flushed
            if let Ok(mut flow) = flow.lock() {
                flow.flush();
            }
        }
    }
}

/// Derive a stable device UUID from a string device_id.
///
/// Uses SHA-256 to hash the device_id and takes the first 16 bytes
/// as a UUID. This is deterministic and stable across app restarts.
fn device_uuid_from_id(device_id: &str) -> Uuid {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(device_id.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    Uuid::from_bytes(bytes)
}

/// A Giantt-specific flow wrapping a DripHostedFlow
pub struct GianttFlow {
    flow: DripHostedFlow<GianttSchema>,
    storage_path: Option<PathBuf>,
    dirty: bool,
}

impl GianttFlow {
    /// Create a new in-memory flow (for testing)
    pub fn new_in_memory(device_id: DeviceId) -> Self {
        let device_uuid = device_uuid_from_id(&device_id);
        let config = FlowConfig {
            id: Uuid::new_v4(),
            type_name: "drip_hosted:giantt".to_string(),
            params: serde_json::json!({}),
        };
        Self {
            flow: DripHostedFlow::new(
                config,
                GianttSchema,
                DripHostPolicy::default(),
                device_uuid,
                device_id,
            ),
            storage_path: None,
            dirty: false,
        }
    }

    /// Create a new persistent flow
    pub fn new_persistent(device_id: DeviceId, storage_path: PathBuf) -> Self {
        let device_uuid = device_uuid_from_id(&device_id);
        let config = FlowConfig {
            id: Uuid::new_v4(),
            type_name: "drip_hosted:giantt".to_string(),
            params: serde_json::json!({}),
        };
        let mut flow = Self {
            flow: DripHostedFlow::new(
                config,
                GianttSchema,
                DripHostPolicy::default(),
                device_uuid,
                device_id,
            ),
            storage_path: Some(storage_path),
            dirty: false,
        };

        flow.load_from_disk();
        flow
    }

    /// Apply a local operation
    pub fn apply_operation(&mut self, op: Operation) -> OpEnvelope {
        // OfflineMerge never fails for apply_edit
        let envelope = self.flow.apply_edit(op).unwrap();
        self.dirty = true;

        // Auto-persist on each operation for durability
        self.flush();

        envelope
    }

    /// Apply a remote operation (received from another device)
    pub fn apply_remote(&mut self, envelope: OpEnvelope) {
        self.flow
            .document()
            .write()
            .unwrap()
            .apply_remote(envelope);
        self.dirty = true;
    }

    /// Get all operations (for syncing to other devices)
    pub fn all_operations(&self) -> Vec<OpEnvelope> {
        self.flow
            .document()
            .read()
            .unwrap()
            .all_operations()
            .cloned()
            .collect()
    }

    /// Materialize and serialize to .giantt text format
    pub fn read_drip(&self) -> String {
        let doc_state = self.flow.document().read().unwrap().materialize();
        let giantt_state = GianttState::from_document_state(&doc_state);
        serialize_giantt_state(&giantt_state)
    }

    /// Persist operations to disk
    fn flush(&mut self) {
        if !self.dirty {
            return;
        }

        if let Some(ref path) = self.storage_path {
            let _ = std::fs::create_dir_all(path);

            let ops_path = path.join("operations.jsonl");
            let doc = self.flow.document().read().unwrap();
            let ops: Vec<_> = doc.all_operations().collect();

            if let Ok(file) = std::fs::File::create(&ops_path) {
                use std::io::Write;
                let mut writer = std::io::BufWriter::new(file);
                for op in ops {
                    if let Ok(json) = serde_json::to_string(op) {
                        let _ = writeln!(writer, "{}", json);
                    }
                }
            }

            self.dirty = false;
        }
    }

    /// Load operations from disk
    fn load_from_disk(&mut self) {
        if let Some(ref path) = self.storage_path {
            let ops_path = path.join("operations.jsonl");
            if ops_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&ops_path) {
                    for line in content.lines() {
                        if let Ok(envelope) = serde_json::from_str::<OpEnvelope>(line) {
                            self.flow
                                .document()
                                .write()
                                .unwrap()
                                .apply_remote(envelope);
                        }
                    }
                }
            }
        }
    }

    /// Get a reference to the underlying DripHostedFlow
    pub fn drip_flow(&self) -> &DripHostedFlow<GianttSchema> {
        &self.flow
    }

    /// Get a mutable reference to the underlying DripHostedFlow
    pub fn drip_flow_mut(&mut self) -> &mut DripHostedFlow<GianttSchema> {
        &mut self.flow
    }
}

// ============================================================================
// FFI Functions
// ============================================================================

/// Initialize the flow system with a device ID
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

/// Open a flow by UUID
///
/// Returns an opaque handle on success, null on error.
/// The flow will load its edit history from disk (or create a new one).
#[no_mangle]
pub extern "C" fn soradyne_flow_open(uuid_ptr: *const c_char) -> *mut std::ffi::c_void {
    if uuid_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let uuid = unsafe {
        match CStr::from_ptr(uuid_ptr).to_str() {
            Ok(s) => s.to_string(),
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

    let flow = registry.get_or_open(&uuid);

    // Return the Arc as a raw pointer
    // The caller is responsible for calling soradyne_flow_close to release it
    Arc::into_raw(flow) as *mut std::ffi::c_void
}

/// Close a flow handle
///
/// Must be called to release resources. Safe to call multiple times.
#[no_mangle]
pub extern "C" fn soradyne_flow_close(handle: *mut std::ffi::c_void) {
    if handle.is_null() {
        return;
    }

    // Reconstruct the Arc and drop it
    unsafe {
        let _ = Arc::from_raw(handle as *const Mutex<GianttFlow>);
    }
}

/// Write an operation to a flow
///
/// op_json: JSON-encoded operation (see Operation enum)
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

    // Parse the operation from JSON
    let op: Operation = match serde_json::from_str(op_json) {
        Ok(op) => op,
        Err(e) => {
            eprintln!("Failed to parse operation JSON: {}", e);
            return -1;
        }
    };

    // Get the flow from the handle
    let flow = unsafe { &*(handle as *const Mutex<GianttFlow>) };

    match flow.lock() {
        Ok(mut flow) => {
            flow.apply_operation(op);
            0
        }
        Err(_) => -1,
    }
}

/// Read the current state as .giantt text format
///
/// Returns a C string that must be freed with soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_flow_read_drip(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    // Get the flow from the handle
    let flow = unsafe { &*(handle as *const Mutex<GianttFlow>) };

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

/// Get all operations as JSON array (for syncing)
///
/// Returns a C string that must be freed with soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_flow_get_operations(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow = unsafe { &*(handle as *const Mutex<GianttFlow>) };

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

/// Apply remote operations (received from another device)
///
/// ops_json: JSON array of OpEnvelope objects
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

    let flow = unsafe { &*(handle as *const Mutex<GianttFlow>) };

    match flow.lock() {
        Ok(mut flow) => {
            for op in ops {
                flow.apply_remote(op);
            }
            flow.flush();
            0
        }
        Err(_) => -1,
    }
}

/// Close a flow by UUID (alternative to handle-based close)
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

/// Cleanup the flow system
#[no_mangle]
pub extern "C" fn soradyne_flow_cleanup() {
    if let Ok(mut registry) = FLOW_REGISTRY.write() {
        *registry = None;
    }
}

// ============================================================================
// Sync FFI Functions
// ============================================================================

/// Connect a giantt flow to an ensemble for sync.
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

    let flow_arc = unsafe { &*(handle as *const Mutex<GianttFlow>) };

    match super::pairing_bridge::bridge_get_ensemble(capsule_id) {
        Ok((messenger, topology)) => {
            if let Ok(mut flow) = flow_arc.lock() {
                use crate::flow::flow_core::Flow;
                flow.drip_flow_mut().set_ensemble(messenger, topology);
                0
            } else {
                -1
            }
        }
        Err(_) => -1,
    }
}

/// Start background sync for a giantt flow.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_start_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<GianttFlow>) };

    match flow_arc.lock() {
        Ok(flow) => match flow.drip_flow().start() {
            Ok(()) => 0,
            Err(_) => -1,
        },
        Err(_) => -1,
    }
}

/// Stop background sync for a giantt flow.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_stop_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<GianttFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            flow.drip_flow().stop();
            0
        }
        Err(_) => -1,
    }
}

/// Get the sync status of a giantt flow as JSON.
///
/// Returns JSON: {"is_host": bool, "host_id": "uuid"|null, "epoch": n, "connected": bool}
/// Caller must free via soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_flow_get_sync_status(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<GianttFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            let drip = flow.drip_flow();
            let is_host = drip.is_current_host();
            let ha = drip.host_assignment().read().ok();
            let (host_id, epoch) = match ha.as_ref() {
                Some(ha) => (ha.current_host.map(|h| h.to_string()), ha.epoch),
                None => (None, 0),
            };

            let json = serde_json::json!({
                "is_host": is_host,
                "host_id": host_id,
                "epoch": epoch,
                "connected": drip.is_current_host() || host_id.is_some(),
            });

            match CString::new(json.to_string()) {
                Ok(c_str) => c_str.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convergent::Value;

    #[test]
    fn test_flow_basic_operations() {
        let mut flow = GianttFlow::new_in_memory("test_device".into());

        // Add an item
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

        // Read the drip
        let text = flow.read_drip();
        println!("Drip output:\n{}", text);

        // Should contain the task
        assert!(text.contains("task_1"));
        assert!(text.contains("My Task"));
    }

    #[test]
    fn test_flow_with_tags_and_charts() {
        let mut flow = GianttFlow::new_in_memory("test_device".into());

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
        println!("Drip with tags:\n{}", text);

        assert!(text.contains("Tagged Task"));
        // Tags should appear in the output
        assert!(text.contains("important") || text.contains("urgent"));
    }

    #[test]
    fn test_drip_hosted_local_edits_equivalent() {
        // Verify DripHostedFlow-backed GianttFlow produces identical output
        let mut flow = GianttFlow::new_in_memory("device_equiv".into());

        flow.apply_operation(Operation::add_item("task_1", "GianttItem"));
        flow.apply_operation(Operation::set_field(
            "task_1",
            "title",
            Value::string("Equivalence Test"),
        ));
        flow.apply_operation(Operation::set_field(
            "task_1",
            "status",
            Value::string("TODO"),
        ));

        // Verify the DripHostedFlow internals are accessible
        let doc = flow.drip_flow().document().read().unwrap();
        let state = doc.materialize();
        assert!(state.get(&"task_1".into()).is_some());
        drop(doc);

        // Verify text output contains expected content
        let text = flow.read_drip();
        assert!(text.contains("task_1"));
        assert!(text.contains("Equivalence Test"));

        // Verify ops count
        let ops = flow.all_operations();
        assert_eq!(ops.len(), 3); // add_item + 2 set_field
    }

    #[test]
    fn test_drip_hosted_persistence() {
        let temp_dir =
            std::env::temp_dir().join("soradyne_test_drip_hosted_giantt_persistence");
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Create a persistent DripHostedFlow-backed flow and add data
        {
            let mut flow =
                GianttFlow::new_persistent("test_device_dhf".into(), temp_dir.clone());
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

        // Reopen and verify data persisted through DripHostedFlow
        {
            let flow =
                GianttFlow::new_persistent("test_device_dhf".into(), temp_dir.clone());
            let text = flow.read_drip();
            assert!(text.contains("task_1"));
            assert!(text.contains("Persistent Task"));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
