//! Unified FFI for convergent-document flows.
//!
//! Schema-agnostic: all flows use `DripHostedFlow<()>`. The `schema` argument to
//! `soradyne_flow_open` is accepted for forwards compatibility but ignored.
//! `soradyne_flow_read_drip` returns generic `DocumentState` JSON:
//!   `{"items": {"<id>": {"item_type": "…", "fields": {…}, "sets": {…}}}}`
//!
//! FFI lifecycle:
//!   soradyne_flow_init(device_id)
//!   handle = soradyne_flow_open(uuid, schema)
//!   soradyne_flow_write_op(handle, op_json)
//!   json = soradyne_flow_read_drip(handle)   // caller must free with soradyne_free_string
//!   soradyne_flow_close(handle)
//!   soradyne_flow_cleanup()

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use uuid::Uuid;

use crate::convergent::{DeviceId, OpEnvelope, Operation};
use crate::flow::flow_core::{Flow, FlowConfig};
use crate::flow::types::drip_hosted::{DripHostPolicy, DripHostedFlow};

// ============================================================================
// ConvergentFlow — thin wrapper over DripHostedFlow<()>
// ============================================================================

/// A convergent flow backed by `DripHostedFlow<()>`.
///
/// All schema knowledge lives in the calling application. This struct handles
/// op pre-processing (auto-fill of `RemoveFromSet` add-IDs) and delegates
/// everything else to the flow.
pub struct ConvergentFlow {
    inner: DripHostedFlow<()>,
}

impl ConvergentFlow {
    pub fn new_in_memory(device_id: DeviceId, flow_uuid: Uuid) -> Self {
        let device_uuid = device_uuid_from_id(&device_id);
        let config = FlowConfig {
            id: flow_uuid,
            type_name: "drip_hosted".to_string(),
            params: serde_json::json!({}),
        };
        Self {
            inner: DripHostedFlow::new(config, (), DripHostPolicy::default(), device_uuid, device_id),
        }
    }

    pub fn new_persistent(device_id: DeviceId, storage_path: PathBuf, flow_uuid: Uuid) -> Self {
        let device_uuid = device_uuid_from_id(&device_id);
        let config = FlowConfig {
            id: flow_uuid,
            type_name: "drip_hosted".to_string(),
            params: serde_json::json!({}),
        };
        Self {
            inner: DripHostedFlow::new_persistent(
                config,
                (),
                DripHostPolicy::default(),
                device_uuid,
                device_id,
                storage_path,
            ),
        }
    }

    /// Apply a local operation, auto-filling `RemoveFromSet` observed add-IDs.
    pub fn apply_operation(&mut self, op: Operation) -> OpEnvelope {
        let op = match op {
            Operation::RemoveFromSet {
                ref item_id,
                ref set_name,
                ref element,
                ref observed_add_ids,
            } if observed_add_ids.is_empty() => {
                let add_ids = self
                    .inner
                    .document()
                    .read()
                    .unwrap()
                    .get_add_ids_for_element(item_id, set_name, element);
                Operation::remove_from_set(
                    item_id.clone(),
                    set_name.clone(),
                    element.clone(),
                    add_ids,
                )
            }
            other => other,
        };
        self.inner.apply_edit(op).unwrap()
    }

    /// Apply a remote operation received from another device.
    pub fn apply_remote(&mut self, envelope: OpEnvelope) {
        self.inner.apply_remote_op(envelope);
    }

    /// Get all operations (for syncing to other devices).
    pub fn all_operations(&self) -> Vec<OpEnvelope> {
        self.inner
            .document()
            .read()
            .unwrap()
            .all_operations()
            .cloned()
            .collect()
    }

    /// Materialize and return as generic DocumentState JSON.
    ///
    /// Format: `{"items": {"<id>": {"item_type": "…", "fields": {…}, "sets": {…}}}}`
    /// Only existing (non-tombstoned) items are included.
    pub fn read_drip(&self) -> String {
        self.inner.document().read().unwrap().materialize().to_json()
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
        self.inner.set_ensemble(messenger, topology);
    }

    /// Start background sync tasks (inbound listener, flush, topology watcher).
    pub fn start(&self) -> Result<(), crate::flow::FlowError> {
        self.inner.start()
    }
}

fn device_uuid_from_id(device_id: &str) -> Uuid {
    Uuid::parse_str(device_id).unwrap_or_else(|_| {
        // Non-UUID device_id: derive a deterministic UUID from its bytes.
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        device_id.hash(&mut h);
        let n = h.finish();
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&n.to_le_bytes());
        bytes[8..].copy_from_slice(&n.wrapping_add(0x9e3779b97f4a7c15).to_le_bytes());
        Uuid::from_bytes(bytes)
    })
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
        let data_dir = super::pairing_bridge::bridge_data_dir()
            .map(|base| base.join("flows"))
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".soradyne/flows")
            });

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

    fn get_or_open(&mut self, uuid: &str, _schema_name: &str) -> Option<Arc<Mutex<ConvergentFlow>>> {
        if let Some(flow) = self.flows.get(uuid) {
            return Some(Arc::clone(flow));
        }

        let flow_dir = self.data_dir.join(uuid);
        let is_test = uuid.starts_with("test-");
        let flow_uuid = Uuid::parse_str(uuid).unwrap_or_else(|_| Uuid::new_v4());

        let flow = if is_test {
            ConvergentFlow::new_in_memory(self.device_id.clone(), flow_uuid)
        } else {
            ConvergentFlow::new_persistent(self.device_id.clone(), flow_dir, flow_uuid)
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

    // If already initialized with the same device ID, leave the running registry
    // intact — replacing it would drop active flows and kill background sync tasks
    // (e.g. on a Dart hot restart where native statics persist but Dart state resets).
    if let Some(existing) = registry.as_ref() {
        if existing.device_id.as_ref() == device_id {
            return 0;
        }
    }

    *registry = Some(FlowRegistry::new(device_id.into()));
    0
}

/// Open a flow by UUID.
///
/// `schema_ptr`: accepted for API compatibility, currently unused.
/// Returns an opaque handle on success, null on error.
#[no_mangle]
pub extern "C" fn soradyne_flow_open(
    uuid_ptr: *const c_char,
    schema_ptr: *const c_char,
) -> *mut std::ffi::c_void {
    if uuid_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let uuid = unsafe {
        match CStr::from_ptr(uuid_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let schema_name = if schema_ptr.is_null() {
        ""
    } else {
        unsafe {
            match CStr::from_ptr(schema_ptr).to_str() {
                Ok(s) => s,
                Err(_) => "",
            }
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

/// Read the current materialized state as generic DocumentState JSON.
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
                flow.inner.set_ensemble(messenger, topology);
                0
            } else {
                -1
            }
        }
        Err(_) => -1,
    }
}

/// Start background sync for a flow.
#[no_mangle]
pub extern "C" fn soradyne_flow_start_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match super::pairing_bridge::bridge_with_runtime(|| {
        match flow_arc.lock() {
            Ok(flow) => match flow.inner.start() {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("soradyne_flow_start_sync: {:?}", e);
                    -1
                }
            },
            Err(_) => -1,
        }
    }) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("soradyne_flow_start_sync: runtime error: {}", e);
            -1
        }
    }
}

/// Enable sync for a flow using the first available capsule.
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
                flow.inner.set_ensemble(messenger, topology);
            } else {
                return -1;
            }
        }
        Err(e) => {
            eprintln!("soradyne_flow_enable_sync: ensemble error: {}", e);
            return -1;
        }
    }

    match super::pairing_bridge::bridge_with_runtime(|| match flow_arc.lock() {
        Ok(flow) => match flow.inner.start() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("soradyne_flow_enable_sync: start error: {:?}", e);
                -1
            }
        },
        Err(_) => -1,
    }) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("soradyne_flow_enable_sync: runtime error: {}", e);
            -1
        }
    }
}

/// Stop background sync for a flow.
#[no_mangle]
pub extern "C" fn soradyne_flow_stop_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            flow.inner.stop();
            0
        }
        Err(_) => -1,
    }
}

/// Get the sync status of a flow as JSON.
#[no_mangle]
pub extern "C" fn soradyne_flow_get_sync_status(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<ConvergentFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            let is_host = flow.inner.is_current_host();
            let ha = flow.inner.host_assignment().read().ok();
            let (host_id, epoch) = match ha.as_ref() {
                Some(ha) => (ha.current_host.map(|h| h.to_string()), ha.epoch),
                None => (None, 0),
            };

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

    #[test]
    fn test_basic_operations() {
        let mut flow = ConvergentFlow::new_in_memory("test_device".into(), Uuid::new_v4());

        flow.apply_operation(Operation::add_item("task_1", "GianttItem"));
        flow.apply_operation(Operation::set_field("task_1", "title", Value::string("My Task")));
        flow.apply_operation(Operation::set_field("task_1", "status", Value::string("TODO")));

        let json: serde_json::Value = serde_json::from_str(&flow.read_drip()).unwrap();
        assert_eq!(json["items"]["task_1"]["fields"]["title"], "My Task");
        assert_eq!(json["items"]["task_1"]["fields"]["status"], "TODO");
    }

    #[test]
    fn test_sets() {
        let mut flow = ConvergentFlow::new_in_memory("test_device".into(), Uuid::new_v4());

        flow.apply_operation(Operation::add_item("task_1", "GianttItem"));
        flow.apply_operation(Operation::set_field("task_1", "title", Value::string("Tagged")));
        flow.apply_operation(Operation::add_to_set("task_1", "tags", Value::string("important")));
        flow.apply_operation(Operation::add_to_set("task_1", "tags", Value::string("urgent")));

        let json: serde_json::Value = serde_json::from_str(&flow.read_drip()).unwrap();
        let tags = json["items"]["task_1"]["sets"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_remove_item_excluded_from_drip() {
        let mut flow = ConvergentFlow::new_in_memory("test_device".into(), Uuid::new_v4());

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::set_field("item_1", "description", Value::string("Hammer")));

        let json: serde_json::Value = serde_json::from_str(&flow.read_drip()).unwrap();
        assert!(json["items"]["item_1"].is_object());

        flow.apply_operation(Operation::remove_item("item_1"));

        let json: serde_json::Value = serde_json::from_str(&flow.read_drip()).unwrap();
        assert!(json["items"]["item_1"].is_null());
    }

    #[test]
    fn test_remote_sync() {
        let flow_uuid = Uuid::new_v4();
        let mut flow_a = ConvergentFlow::new_in_memory("device_A".into(), flow_uuid);
        let mut flow_b = ConvergentFlow::new_in_memory("device_B".into(), flow_uuid);

        let op1 = flow_a.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        let op2 = flow_a.apply_operation(Operation::set_field("item_1", "location", Value::string("Kitchen")));

        flow_b.apply_remote(op1);
        flow_b.apply_remote(op2);

        let op3 = flow_b.apply_operation(Operation::set_field("item_1", "location", Value::string("Garage")));
        flow_a.apply_remote(op3);

        let json_a: serde_json::Value = serde_json::from_str(&flow_a.read_drip()).unwrap();
        let json_b: serde_json::Value = serde_json::from_str(&flow_b.read_drip()).unwrap();
        assert_eq!(
            json_a["items"]["item_1"]["fields"]["location"],
            json_b["items"]["item_1"]["fields"]["location"]
        );
    }

    #[test]
    fn test_remove_from_set_auto_fill() {
        let mut flow = ConvergentFlow::new_in_memory("test_device".into(), Uuid::new_v4());

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::add_to_set("item_1", "tags", Value::string("fragile")));
        flow.apply_operation(Operation::add_to_set("item_1", "tags", Value::string("valuable")));

        flow.apply_operation(Operation::remove_from_set(
            "item_1", "tags", Value::string("fragile"), vec![],
        ));

        let json: serde_json::Value = serde_json::from_str(&flow.read_drip()).unwrap();
        let tags = json["items"]["item_1"]["sets"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&serde_json::json!("valuable")));
    }

    #[test]
    fn test_persistence() {
        let temp_dir = std::env::temp_dir().join("soradyne_test_convergent_flow_persistence");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let flow_uuid = Uuid::new_v4();

        {
            let mut flow = ConvergentFlow::new_persistent("test_device".into(), temp_dir.clone(), flow_uuid);
            flow.apply_operation(Operation::add_item("task_1", "GianttItem"));
            flow.apply_operation(Operation::set_field("task_1", "title", Value::string("Persistent Task")));
        }

        {
            let flow = ConvergentFlow::new_persistent("test_device".into(), temp_dir.clone(), flow_uuid);
            let json: serde_json::Value = serde_json::from_str(&flow.read_drip()).unwrap();
            assert_eq!(json["items"]["task_1"]["fields"]["title"], "Persistent Task");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
