//! FFI for Inventory Flow operations
//!
//! Provides a minimal FFI surface for Dart to interact with Inventory flows:
//! - Flow lifecycle (init/open/close/cleanup)
//! - Operations (write_op)
//! - State access (read_drip returns JSON)
//! - Sync (get_operations, apply_remote)
//! - Ensemble sync (connect_ensemble, start_sync, stop_sync, get_sync_status)

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use uuid::Uuid;

use crate::convergent::inventory::{InventorySchema, InventoryState};
use crate::convergent::{DeviceId, OpEnvelope, Operation};
use crate::flow::flow_core::FlowConfig;
use crate::flow::types::drip_hosted::{DripHostPolicy, DripHostedFlow};

/// Global registry of open inventory flows
static INVENTORY_REGISTRY: RwLock<Option<InventoryRegistry>> = RwLock::new(None);

/// Registry holding all open InventoryFlow instances
struct InventoryRegistry {
    flows: HashMap<String, Arc<Mutex<InventoryFlow>>>,
    device_id: DeviceId,
    data_dir: PathBuf,
}

impl InventoryRegistry {
    fn new(device_id: DeviceId) -> Self {
        let data_dir = if cfg!(target_os = "macos") {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join("Library/Application Support/Soradyne/inventory_flows")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".soradyne/inventory_flows")
        };

        let _ = std::fs::create_dir_all(&data_dir);

        Self {
            flows: HashMap::new(),
            device_id,
            data_dir,
        }
    }

    fn get_or_open(&mut self, uuid: &str) -> Arc<Mutex<InventoryFlow>> {
        if let Some(flow) = self.flows.get(uuid) {
            return Arc::clone(flow);
        }

        let flow_dir = self.data_dir.join(uuid);
        let is_test = uuid.starts_with("test-");

        let flow = if is_test {
            InventoryFlow::new_in_memory(self.device_id.clone())
        } else {
            InventoryFlow::new_persistent(self.device_id.clone(), flow_dir)
        };

        let flow = Arc::new(Mutex::new(flow));
        self.flows.insert(uuid.to_string(), Arc::clone(&flow));
        flow
    }

    fn close(&mut self, uuid: &str) {
        if let Some(flow) = self.flows.remove(uuid) {
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

/// An Inventory-specific flow wrapping a DripHostedFlow
pub struct InventoryFlow {
    flow: DripHostedFlow<InventorySchema>,
    storage_path: Option<PathBuf>,
    dirty: bool,
}

impl InventoryFlow {
    /// Create a new in-memory flow (for testing)
    pub fn new_in_memory(device_id: DeviceId) -> Self {
        let device_uuid = device_uuid_from_id(&device_id);
        let config = FlowConfig {
            id: Uuid::new_v4(),
            type_name: "drip_hosted:inventory".to_string(),
            params: serde_json::json!({}),
        };
        Self {
            flow: DripHostedFlow::new(
                config,
                InventorySchema,
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
            type_name: "drip_hosted:inventory".to_string(),
            params: serde_json::json!({}),
        };
        let mut flow = Self {
            flow: DripHostedFlow::new(
                config,
                InventorySchema,
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
    ///
    /// For RemoveFromSet with empty observed_add_ids, automatically fills in
    /// the add IDs from the document. This lets the Dart side send a simple
    /// RemoveFromSet without needing to track add IDs.
    pub fn apply_operation(&mut self, op: Operation) -> OpEnvelope {
        let op = match op {
            Operation::RemoveFromSet {
                ref item_id,
                ref set_name,
                ref element,
                ref observed_add_ids,
            } if observed_add_ids.is_empty() => {
                let add_ids = self
                    .flow
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

        // OfflineMerge never fails for apply_edit
        let envelope = self.flow.apply_edit(op).unwrap();
        self.dirty = true;
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

    /// Materialize and serialize to JSON format
    pub fn read_drip(&self) -> String {
        let doc_state = self.flow.document().read().unwrap().materialize();
        let inventory_state = InventoryState::from_document_state(&doc_state);
        serialize_inventory_state(&inventory_state)
    }

    /// Persist operations to disk
    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }

        let Some(ref path) = self.storage_path else {
            return;
        };

        if let Err(e) = std::fs::create_dir_all(path) {
            eprintln!(
                "[inventory_flow] flush: failed to create dir {:?}: {}",
                path, e
            );
            return;
        }

        let ops_path = path.join("operations.jsonl");
        let doc = self.flow.document().read().unwrap();
        let ops: Vec<_> = doc.all_operations().collect();
        let op_count = ops.len();

        // Write to a temp file first, then rename for atomicity
        let tmp_path = path.join("operations.jsonl.tmp");

        let result = (|| -> std::io::Result<()> {
            use std::io::Write;
            let file = std::fs::File::create(&tmp_path)?;
            let mut writer = std::io::BufWriter::new(file);
            let mut written = 0usize;
            for op in &ops {
                let json = serde_json::to_string(op)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                writeln!(writer, "{}", json)?;
                written += 1;
            }
            writer.flush()?;
            eprintln!(
                "[inventory_flow] flush: wrote {}/{} ops to {:?}",
                written, op_count, tmp_path
            );
            Ok(())
        })();

        match result {
            Ok(()) => {
                if let Err(e) = std::fs::rename(&tmp_path, &ops_path) {
                    eprintln!(
                        "[inventory_flow] flush: rename {:?} -> {:?} failed: {}",
                        tmp_path, ops_path, e
                    );
                    return;
                }
                self.dirty = false;
            }
            Err(e) => {
                eprintln!("[inventory_flow] flush: write failed: {}", e);
                // Clean up partial temp file
                let _ = std::fs::remove_file(&tmp_path);
                // dirty remains true so next flush retries
            }
        }
    }

    /// Load operations from disk
    fn load_from_disk(&mut self) {
        let Some(ref path) = self.storage_path else {
            return;
        };

        let ops_path = path.join("operations.jsonl");
        if !ops_path.exists() {
            eprintln!(
                "[inventory_flow] load: no operations file at {:?}",
                ops_path
            );
            return;
        }

        let content = match std::fs::read_to_string(&ops_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "[inventory_flow] load: failed to read {:?}: {}",
                    ops_path, e
                );
                return;
            }
        };

        let mut loaded = 0usize;
        let mut skipped = 0usize;
        for (i, line) in content.lines().enumerate() {
            match serde_json::from_str::<OpEnvelope>(line) {
                Ok(envelope) => {
                    self.flow
                        .document()
                        .write()
                        .unwrap()
                        .apply_remote(envelope);
                    loaded += 1;
                }
                Err(e) => {
                    eprintln!(
                        "[inventory_flow] load: skipping line {} in {:?}: {}",
                        i + 1,
                        ops_path,
                        e
                    );
                    skipped += 1;
                }
            }
        }

        eprintln!(
            "[inventory_flow] load: {} ops loaded, {} skipped from {:?}",
            loaded, skipped, ops_path
        );
    }

    /// Get a reference to the underlying DripHostedFlow
    pub fn drip_flow(&self) -> &DripHostedFlow<InventorySchema> {
        &self.flow
    }

    /// Get a mutable reference to the underlying DripHostedFlow
    pub fn drip_flow_mut(&mut self) -> &mut DripHostedFlow<InventorySchema> {
        &mut self.flow
    }
}

/// Serialize InventoryState to JSON matching Dart's InventoryEntry format
fn serialize_inventory_state(state: &InventoryState) -> String {
    let items: HashMap<&str, serde_json::Value> = state
        .items
        .iter()
        .map(|(id, item)| {
            let mut tags: Vec<&str> = item.tags.iter().map(|s| s.as_str()).collect();
            tags.sort();

            (
                id.as_str(),
                serde_json::json!({
                    "id": item.id,
                    "category": item.category,
                    "description": item.description,
                    "location": item.location,
                    "tags": tags,
                }),
            )
        })
        .collect();

    serde_json::json!({ "items": items }).to_string()
}

// ============================================================================
// FFI Functions
// ============================================================================

/// Initialize the inventory flow system with a device ID
///
/// Must be called before any other inventory FFI functions.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_inventory_init(device_id_ptr: *const c_char) -> i32 {
    if device_id_ptr.is_null() {
        return -1;
    }

    let device_id = unsafe {
        match CStr::from_ptr(device_id_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return -1,
        }
    };

    let mut registry = match INVENTORY_REGISTRY.write() {
        Ok(r) => r,
        Err(_) => return -1,
    };

    *registry = Some(InventoryRegistry::new(device_id.into()));
    0
}

/// Open an inventory flow by UUID
///
/// Returns an opaque handle on success, null on error.
#[no_mangle]
pub extern "C" fn soradyne_inventory_open(uuid_ptr: *const c_char) -> *mut std::ffi::c_void {
    if uuid_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let uuid = unsafe {
        match CStr::from_ptr(uuid_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return std::ptr::null_mut(),
        }
    };

    let mut registry = match INVENTORY_REGISTRY.write() {
        Ok(r) => r,
        Err(_) => return std::ptr::null_mut(),
    };

    let registry = match registry.as_mut() {
        Some(r) => r,
        None => return std::ptr::null_mut(),
    };

    let flow = registry.get_or_open(&uuid);
    Arc::into_raw(flow) as *mut std::ffi::c_void
}

/// Close an inventory flow handle
#[no_mangle]
pub extern "C" fn soradyne_inventory_close(handle: *mut std::ffi::c_void) {
    if handle.is_null() {
        return;
    }

    unsafe {
        let arc = Arc::from_raw(handle as *const Mutex<InventoryFlow>);
        if let Ok(mut flow) = arc.lock() {
            flow.flush();
        };
    }
}

/// Write an operation to an inventory flow
///
/// op_json: JSON-encoded Operation (see convergent::Operation enum)
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_inventory_write_op(
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
            eprintln!("Failed to parse inventory operation JSON: {}", e);
            return -1;
        }
    };

    let flow = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

    match flow.lock() {
        Ok(mut flow) => {
            flow.apply_operation(op);
            0
        }
        Err(_) => -1,
    }
}

/// Read the current inventory state as JSON
///
/// Returns a C string that must be freed with soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_inventory_read_drip(handle: *mut std::ffi::c_void) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

    match flow.lock() {
        Ok(flow) => {
            let json = flow.read_drip();
            match CString::new(json) {
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
pub extern "C" fn soradyne_inventory_get_operations(
    handle: *mut std::ffi::c_void,
) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

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
pub extern "C" fn soradyne_inventory_apply_remote(
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
            eprintln!("Failed to parse remote inventory operations JSON: {}", e);
            return -1;
        }
    };

    let flow = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

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

/// Cleanup the inventory flow system
#[no_mangle]
pub extern "C" fn soradyne_inventory_cleanup() {
    if let Ok(mut registry) = INVENTORY_REGISTRY.write() {
        *registry = None;
    }
}

// ============================================================================
// Sync FFI Functions
// ============================================================================

/// Connect an inventory flow to an ensemble for sync.
///
/// capsule_id_ptr: UUID string of the capsule to sync within.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_inventory_connect_ensemble(
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

    let flow_arc = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

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

/// Start background sync for an inventory flow.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_inventory_start_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

    match flow_arc.lock() {
        Ok(flow) => match flow.drip_flow().start() {
            Ok(()) => 0,
            Err(_) => -1,
        },
        Err(_) => -1,
    }
}

/// Stop background sync for an inventory flow.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_inventory_stop_sync(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            flow.drip_flow().stop();
            0
        }
        Err(_) => -1,
    }
}

/// Get the sync status of an inventory flow as JSON.
///
/// Returns JSON: {"is_host": bool, "host_id": "uuid"|null, "epoch": n, "connected": bool}
/// Caller must free via soradyne_free_string.
#[no_mangle]
pub extern "C" fn soradyne_inventory_get_sync_status(
    handle: *mut std::ffi::c_void,
) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }

    let flow_arc = unsafe { &*(handle as *const Mutex<InventoryFlow>) };

    match flow_arc.lock() {
        Ok(flow) => {
            let drip = flow.drip_flow();
            let is_host = drip.is_current_host();
            let ha = drip.host_assignment().read().ok();
            let (host_id, epoch) = match ha.as_ref() {
                Some(ha) => (
                    ha.current_host.map(|h| h.to_string()),
                    ha.epoch,
                ),
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
    fn test_inventory_basic_operations() {
        let mut flow = InventoryFlow::new_in_memory("test_device".into());

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
        let mut flow = InventoryFlow::new_in_memory("test_device".into());

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));

        // Verify it exists
        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["items"].as_object().unwrap().len(), 1);

        // Delete it
        flow.apply_operation(Operation::remove_item("item_1"));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["items"].as_object().unwrap().is_empty());
    }

    #[test]
    fn test_inventory_tags() {
        let mut flow = InventoryFlow::new_in_memory("test_device".into());

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
        let mut flow_a = InventoryFlow::new_in_memory("device_A".into());
        let mut flow_b = InventoryFlow::new_in_memory("device_B".into());

        // A creates an item
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

        // B receives A's ops
        flow_b.apply_remote(op1);
        flow_b.apply_remote(op2);
        flow_b.apply_remote(op3);

        // B updates location
        let op4 = flow_b.apply_operation(Operation::set_field(
            "item_1",
            "location",
            Value::string("Garage"),
        ));

        // A receives B's update
        flow_a.apply_remote(op4);

        // Both should converge
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
        let temp_dir = std::env::temp_dir().join("soradyne_test_inventory_persistence");
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Create a persistent flow and add data
        {
            let mut flow =
                InventoryFlow::new_persistent("test_device".into(), temp_dir.clone());
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

        // Reopen and verify data persisted
        {
            let flow = InventoryFlow::new_persistent("test_device".into(), temp_dir.clone());
            let json = flow.read_drip();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

            let item = &parsed["items"]["item_1"];
            assert_eq!(item["description"], "Hammer");
            assert_eq!(item["location"], "Toolbox");
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_inventory_remove_tag_auto_fill() {
        let mut flow = InventoryFlow::new_in_memory("test_device".into());

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

        // Verify both tags exist
        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = parsed["items"]["item_1"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);

        // Remove "fragile" with empty observed_add_ids — auto-fill should handle it
        flow.apply_operation(Operation::remove_from_set(
            "item_1",
            "tags",
            Value::string("fragile"),
            vec![], // empty — will be auto-filled
        ));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = parsed["items"]["item_1"]["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&serde_json::json!("valuable")));
        assert!(!tags.contains(&serde_json::json!("fragile")));
    }

    #[test]
    fn test_inventory_json_format() {
        let mut flow = InventoryFlow::new_in_memory("test_device".into());

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
        flow.apply_operation(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("workshop"),
        ));

        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Verify JSON structure matches what Dart expects
        assert!(parsed["items"].is_object());
        let item = &parsed["items"]["item_1"];
        assert_eq!(item["id"], "item_1");
        assert_eq!(item["category"], "Tools");
        assert_eq!(item["description"], "Hammer");
        assert_eq!(item["location"], "Toolbox");
        assert!(item["tags"].is_array());
        assert!(item["tags"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("workshop")));
    }

    #[test]
    fn test_drip_hosted_local_edits_equivalent() {
        // Verify DripHostedFlow-backed InventoryFlow produces identical output
        // to what the old ConvergentDocument-backed version would produce.
        let mut flow = InventoryFlow::new_in_memory("device_equiv".into());

        flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "category",
            Value::string("Electronics"),
        ));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "description",
            Value::string("Multimeter"),
        ));
        flow.apply_operation(Operation::set_field(
            "item_1",
            "location",
            Value::string("Workbench"),
        ));
        flow.apply_operation(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("precision"),
        ));

        // Verify the DripHostedFlow internals are accessible
        let doc = flow.drip_flow().document().read().unwrap();
        let state = doc.materialize();
        assert!(state.get(&"item_1".into()).is_some());
        drop(doc);

        // Verify JSON output is correct
        let json = flow.read_drip();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["items"]["item_1"]["category"], "Electronics");
        assert_eq!(parsed["items"]["item_1"]["description"], "Multimeter");
        assert_eq!(parsed["items"]["item_1"]["location"], "Workbench");

        // Verify ops count via all_operations
        let ops = flow.all_operations();
        assert_eq!(ops.len(), 5); // add_item + 3 set_field + 1 add_to_set
    }

    #[test]
    fn test_drip_hosted_persistence() {
        let temp_dir =
            std::env::temp_dir().join("soradyne_test_drip_hosted_inventory_persistence");
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Create a persistent DripHostedFlow-backed flow and add data
        {
            let mut flow =
                InventoryFlow::new_persistent("test_device_dhf".into(), temp_dir.clone());
            flow.apply_operation(Operation::add_item("item_1", "InventoryItem"));
            flow.apply_operation(Operation::set_field(
                "item_1",
                "description",
                Value::string("Oscilloscope"),
            ));
            flow.apply_operation(Operation::set_field(
                "item_1",
                "location",
                Value::string("Lab"),
            ));
        }

        // Reopen and verify data persisted through DripHostedFlow
        {
            let flow =
                InventoryFlow::new_persistent("test_device_dhf".into(), temp_dir.clone());
            let json = flow.read_drip();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

            let item = &parsed["items"]["item_1"];
            assert_eq!(item["description"], "Oscilloscope");
            assert_eq!(item["location"], "Lab");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
