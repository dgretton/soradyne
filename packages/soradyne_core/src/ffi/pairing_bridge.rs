//! FFI bridge for pairing protocol and capsule management
//!
//! Exposes the PairingEngine and CapsuleStore to Flutter via FFI.
//!
//! Unlike the flow FFI modules (which create a Runtime per call), pairing
//! uses a **persistent runtime + global state** pattern: `invite()` and `join()`
//! are long-running async operations that block on user input (PIN verification).
//! The persistent runtime lets async tasks run in the background while the UI
//! polls state and submits the PIN via separate FFI calls.
//!
//! Uses `tokio::sync::Mutex` for the CapsuleStore because `confirm_pin()` and
//! `submit_pin()` hold the lock across `.await` points, and `tokio::spawn`
//! requires `Send` futures. `std::sync::MutexGuard` is not `Send`.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

use crate::ble::simulated::SimBleNetwork;
use crate::identity::{CapsuleKeyBundle, DeviceIdentity};
use crate::topology::capsule::{PieceCapabilities, PieceRole};
use crate::topology::capsule_store::CapsuleStore;
use crate::topology::pairing::{pair_simulated_accessory, PairingEngine, PairingState};

// ---------------------------------------------------------------------------
// Global bridge state
// ---------------------------------------------------------------------------

struct PairingBridge {
    runtime: Runtime,
    identity: Arc<DeviceIdentity>,
    capsule_store: Arc<TokioMutex<CapsuleStore>>,
    engine: Arc<PairingEngine>,
    sim_network: Arc<SimBleNetwork>,
}

static PAIRING_BRIDGE: RwLock<Option<PairingBridge>> = RwLock::new(None);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a `*const c_char` into a `&str`. Returns `None` if null or invalid UTF-8.
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

/// Allocate a C string from a Rust `String`. Caller must free via `soradyne_free_string`.
fn to_c_string(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

/// Return a JSON error string.
fn error_json(msg: &str) -> *mut c_char {
    to_c_string(serde_json::json!({"error": msg}).to_string())
}

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Initialize the pairing bridge.
///
/// Creates or loads a DeviceIdentity, creates a CapsuleStore, SimBleNetwork,
/// and PairingEngine. Stores everything in the global PAIRING_BRIDGE.
///
/// `data_dir`: base directory for persistence (identity + capsules).
///             Pass null to use platform defaults.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_pairing_init(data_dir: *const c_char) -> i32 {
    let base_dir = unsafe { cstr_to_str(data_dir) }
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            if cfg!(target_os = "macos") {
                PathBuf::from(home).join("Library/Application Support/Soradyne")
            } else {
                PathBuf::from(home).join(".soradyne")
            }
        });

    let identity_path = base_dir.join("device_identity.json");
    let capsule_dir = base_dir.join("capsules");

    let identity = match DeviceIdentity::load_or_generate(&identity_path) {
        Ok(id) => Arc::new(id),
        Err(e) => {
            println!("soradyne_pairing_init: identity error: {}", e);
            return -1;
        }
    };

    let capsule_store = match CapsuleStore::load(&capsule_dir) {
        Ok(store) => store,
        Err(_) => CapsuleStore::new(capsule_dir),
    };

    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            println!("soradyne_pairing_init: runtime error: {}", e);
            return -1;
        }
    };

    let engine = Arc::new(PairingEngine::new(Arc::clone(&identity)));
    let sim_network = SimBleNetwork::new();

    let bridge = PairingBridge {
        runtime,
        identity,
        capsule_store: Arc::new(TokioMutex::new(capsule_store)),
        engine,
        sim_network,
    };

    match PAIRING_BRIDGE.write() {
        Ok(mut guard) => {
            *guard = Some(bridge);
            0
        }
        Err(e) => {
            println!("soradyne_pairing_init: lock error: {}", e);
            -1
        }
    }
}

/// Tear down the pairing bridge and free resources.
#[no_mangle]
pub extern "C" fn soradyne_pairing_cleanup() {
    if let Ok(mut guard) = PAIRING_BRIDGE.write() {
        *guard = None;
    }
}

/// Create a capsule and add this device as the first piece.
///
/// `name`: human-friendly capsule name (C string).
///
/// Returns JSON `{"capsule_id": "uuid"}` or `{"error": "..."}`.
/// Caller must free the returned string via `soradyne_free_string`.
#[no_mangle]
pub extern "C" fn soradyne_pairing_create_capsule(name: *const c_char) -> *mut c_char {
    let name_str = match unsafe { cstr_to_str(name) } {
        Some(s) => s.to_string(),
        None => return error_json("name is null or invalid UTF-8"),
    };

    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return error_json("bridge lock poisoned"),
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return error_json("pairing bridge not initialized"),
    };

    let identity = Arc::clone(&bridge.identity);
    let capsule_store = Arc::clone(&bridge.capsule_store);

    bridge.runtime.block_on(async move {
        let mut store = capsule_store.lock().await;

        let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
        let capsule_id = match store.create_capsule(&name_str, keys) {
            Ok(id) => id,
            Err(e) => return error_json(&format!("failed to create capsule: {}", e)),
        };

        // Add self as first piece
        let piece = crate::topology::capsule::PieceRecord::from_identity(
            &identity,
            "This device".to_string(),
            PieceCapabilities::full(),
            PieceRole::Full,
        );
        if let Err(e) = store.add_piece(&capsule_id, piece) {
            return error_json(&format!("failed to add self as piece: {}", e));
        }

        to_c_string(
            serde_json::json!({"capsule_id": capsule_id.to_string()}).to_string(),
        )
    })
}

/// List all capsules as a JSON array.
///
/// Returns JSON array of capsule objects. Caller must free via `soradyne_free_string`.
#[no_mangle]
pub extern "C" fn soradyne_pairing_list_capsules() -> *mut c_char {
    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return error_json("bridge lock poisoned"),
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return error_json("pairing bridge not initialized"),
    };

    let capsule_store = Arc::clone(&bridge.capsule_store);

    bridge.runtime.block_on(async move {
        let store = capsule_store.lock().await;

        let capsules: Vec<serde_json::Value> = store
            .list_capsules()
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id.to_string(),
                    "name": c.name,
                    "created_at": c.created_at.to_rfc3339(),
                    "piece_count": c.pieces.len(),
                    "flow_count": c.flows.len(),
                    "is_active": c.is_active(),
                    "pieces": c.pieces.iter().map(|p| {
                        serde_json::json!({
                            "device_id": p.device_id.to_string(),
                            "name": p.name,
                            "role": format!("{:?}", p.role),
                            "added_at": p.added_at.to_rfc3339(),
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();

        to_c_string(serde_json::json!(capsules).to_string())
    })
}

/// Get a single capsule by ID as JSON.
///
/// Returns JSON capsule object or `{"error": "..."}`.
/// Caller must free via `soradyne_free_string`.
#[no_mangle]
pub extern "C" fn soradyne_pairing_get_capsule(capsule_id: *const c_char) -> *mut c_char {
    let id_str = match unsafe { cstr_to_str(capsule_id) } {
        Some(s) => s,
        None => return error_json("capsule_id is null or invalid UTF-8"),
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(_) => return error_json("invalid capsule_id UUID"),
    };

    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return error_json("bridge lock poisoned"),
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return error_json("pairing bridge not initialized"),
    };

    let capsule_store = Arc::clone(&bridge.capsule_store);

    bridge.runtime.block_on(async move {
        let store = capsule_store.lock().await;

        match store.get_capsule(&uuid) {
            Some(c) => {
                let json = serde_json::json!({
                    "id": c.id.to_string(),
                    "name": c.name,
                    "created_at": c.created_at.to_rfc3339(),
                    "is_active": c.is_active(),
                    "pieces": c.pieces.iter().map(|p| {
                        serde_json::json!({
                            "device_id": p.device_id.to_string(),
                            "name": p.name,
                            "role": format!("{:?}", p.role),
                            "added_at": p.added_at.to_rfc3339(),
                            "capabilities": {
                                "can_host_drip": p.capabilities.can_host_drip,
                                "can_memorize": p.capabilities.can_memorize,
                                "can_route": p.capabilities.can_route,
                                "has_ui": p.capabilities.has_ui,
                            },
                        })
                    }).collect::<Vec<serde_json::Value>>(),
                    "flows": c.flows.iter().map(|f| {
                        serde_json::json!({
                            "id": f.id.to_string(),
                            "name": f.name,
                            "schema_type": f.schema_type,
                            "created_at": f.created_at.to_rfc3339(),
                        })
                    }).collect::<Vec<serde_json::Value>>(),
                });
                to_c_string(json.to_string())
            }
            None => error_json("capsule not found"),
        }
    })
}

/// Start the inviter flow: advertise via sim BLE, accept connection, perform
/// ECDH key exchange. Runs asynchronously on the persistent runtime.
///
/// The engine state transitions to `AwaitingVerification` when the PIN is ready.
/// The UI should poll `soradyne_pairing_get_state` to detect this.
///
/// Returns 0 on success (task spawned), -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_pairing_start_invite(capsule_id: *const c_char) -> i32 {
    let id_str = match unsafe { cstr_to_str(capsule_id) } {
        Some(s) => s,
        None => return -1,
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(_) => return -1,
    };

    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return -1,
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return -1,
    };

    let device = bridge.sim_network.create_device();
    let engine = Arc::clone(&bridge.engine);
    let capsule_store = Arc::clone(&bridge.capsule_store);

    bridge.runtime.spawn(async move {
        let store = capsule_store.lock().await;

        if let Err(e) = engine.invite(uuid, &*store, &device).await {
            println!("soradyne_pairing_start_invite: invite failed: {}", e);
        }
    });

    0
}

/// Start the joiner flow: scan for pairing advertisements, connect, perform
/// ECDH key exchange. Runs asynchronously on the persistent runtime.
///
/// `piece_name`: name for this device in the capsule.
///
/// Returns 0 on success (task spawned), -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_pairing_start_join(piece_name: *const c_char) -> i32 {
    let name = match unsafe { cstr_to_str(piece_name) } {
        Some(s) => s.to_string(),
        None => return -1,
    };

    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return -1,
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return -1,
    };

    let device = bridge.sim_network.create_device();
    let engine = Arc::clone(&bridge.engine);

    bridge.runtime.spawn(async move {
        if let Err(e) = engine
            .join(name, PieceCapabilities::full(), PieceRole::Full, &device)
            .await
        {
            println!("soradyne_pairing_start_join: join failed: {}", e);
        }
    });

    0
}

/// Poll the current pairing state as JSON.
///
/// The UI should call this on a timer (e.g. every 500ms) to detect state changes.
///
/// Returns JSON state. Caller must free via `soradyne_free_string`.
#[no_mangle]
pub extern "C" fn soradyne_pairing_get_state() -> *mut c_char {
    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return error_json("bridge lock poisoned"),
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return error_json("pairing bridge not initialized"),
    };

    let state = bridge.runtime.block_on(bridge.engine.state());

    let json = match state {
        PairingState::Idle => serde_json::json!({"state": "idle"}),
        PairingState::AwaitingVerification { pin } => {
            serde_json::json!({
                "state": "awaiting_verification",
                "pin": pin,
            })
        }
        PairingState::Transferring => serde_json::json!({"state": "transferring"}),
        PairingState::Complete {
            capsule_id,
            peer_device_id,
        } => {
            serde_json::json!({
                "state": "complete",
                "capsule_id": capsule_id.to_string(),
                "peer_device_id": peer_device_id.to_string(),
            })
        }
        PairingState::Failed { reason } => {
            serde_json::json!({
                "state": "failed",
                "reason": reason,
            })
        }
    };

    to_c_string(json.to_string())
}

/// Inviter: after displaying the PIN, call this to proceed.
/// Spawns the `confirm_pin` async task on the persistent runtime.
///
/// Returns 0 on success (task spawned), -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_pairing_confirm_pin() -> i32 {
    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return -1,
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return -1,
    };

    let engine = Arc::clone(&bridge.engine);
    let capsule_store = Arc::clone(&bridge.capsule_store);

    bridge.runtime.spawn(async move {
        let mut store = capsule_store.lock().await;

        if let Err(e) = engine.confirm_pin(&mut *store).await {
            println!("soradyne_pairing_confirm_pin: failed: {}", e);
        }
    });

    0
}

/// Joiner: submit the PIN entered by the user.
/// Spawns the `submit_pin` async task on the persistent runtime.
///
/// `pin`: the 6-digit PIN string entered by the user.
///
/// Returns 0 on success (task spawned), -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_pairing_submit_pin(pin: *const c_char) -> i32 {
    let pin_str = match unsafe { cstr_to_str(pin) } {
        Some(s) => s.to_string(),
        None => return -1,
    };

    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return -1,
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return -1,
    };

    let engine = Arc::clone(&bridge.engine);
    let capsule_store = Arc::clone(&bridge.capsule_store);

    bridge.runtime.spawn(async move {
        let mut store = capsule_store.lock().await;

        if let Err(e) = engine.submit_pin(&pin_str, &mut *store).await {
            println!("soradyne_pairing_submit_pin: failed: {}", e);
        }
    });

    0
}

/// Cancel an in-progress pairing from either side.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_pairing_cancel() -> i32 {
    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return -1,
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return -1,
    };

    let engine = Arc::clone(&bridge.engine);

    bridge.runtime.spawn(async move {
        if let Err(e) = engine.cancel().await {
            println!("soradyne_pairing_cancel: failed: {}", e);
        }
    });

    0
}

/// Add a simulated accessory to a capsule (for testing/demo).
///
/// Returns JSON `{"device_id": "uuid"}` or `{"error": "..."}`.
/// Caller must free via `soradyne_free_string`.
#[no_mangle]
pub extern "C" fn soradyne_pairing_add_sim_accessory(
    capsule_id: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    let id_str = match unsafe { cstr_to_str(capsule_id) } {
        Some(s) => s,
        None => return error_json("capsule_id is null or invalid UTF-8"),
    };
    let name_str = match unsafe { cstr_to_str(name) } {
        Some(s) => s,
        None => return error_json("name is null or invalid UTF-8"),
    };

    let uuid = match Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(_) => return error_json("invalid capsule_id UUID"),
    };

    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return error_json("bridge lock poisoned"),
    };
    let bridge = match guard.as_ref() {
        Some(b) => b,
        None => return error_json("pairing bridge not initialized"),
    };

    let capsule_store = Arc::clone(&bridge.capsule_store);

    bridge.runtime.block_on(async move {
        let mut store = capsule_store.lock().await;

        match pair_simulated_accessory(&mut *store, uuid, name_str).await {
            Ok((identity, _piece)) => to_c_string(
                serde_json::json!({"device_id": identity.device_id().to_string()}).to_string(),
            ),
            Err(e) => error_json(&format!("failed to add simulated accessory: {}", e)),
        }
    })
}
