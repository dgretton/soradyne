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

use std::collections::HashMap;
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
use crate::topology::ensemble::EnsembleTopology;
use crate::topology::manager::{EnsembleConfig, EnsembleManager};
use crate::topology::static_peers::StaticPeerConfig;
use crate::topology::messenger::TopologyMessenger;
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
    /// Base data directory (e.g. ~/.soradyne).
    data_dir: PathBuf,
    /// EnsembleManagers keyed by capsule ID, shared across all flows in a capsule.
    ensemble_managers: TokioMutex<HashMap<Uuid, Arc<EnsembleManager>>>,
}

static PAIRING_BRIDGE: RwLock<Option<PairingBridge>> = RwLock::new(None);

/// Append-only debug log visible to Dart via `soradyne_ble_debug()`.
/// Used to surface Rust log lines that cannot reach the Flutter terminal
/// on macOS (where native stdout is detached from the `flutter run` console).
pub(crate) static BLE_DEBUG: std::sync::Mutex<String> =
    std::sync::Mutex::new(String::new());

/// Append a line to the in-process debug log.
pub(crate) fn ble_log(msg: &str) {
    if let Ok(mut s) = BLE_DEBUG.lock() {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(msg);
        // Keep last 4 KB so the buffer doesn't grow unboundedly.
        let len = s.len();
        if len > 4096 {
            *s = s[len - 4096..].to_string();
        }
    }
}

/// Android application Context stored independently of `PAIRING_BRIDGE`.
///
/// `onAttachedToEngine` fires ~10 s before Dart calls `soradyne_pairing_init`,
/// so the bridge does not exist yet when the context arrives. Storing it in a
/// separate `OnceLock` avoids the race entirely.
#[cfg(target_os = "android")]
static ANDROID_CONTEXT: std::sync::OnceLock<jni::objects::GlobalRef> =
    std::sync::OnceLock::new();

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a `*const c_char` into a `&str`. Returns `None` if null or invalid UTF-8.
pub(crate) unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

/// Allocate a C string from a Rust `String`. Caller must free via `soradyne_free_string`.
pub(crate) fn to_c_string(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

/// Return a JSON error string.
pub(crate) fn error_json(msg: &str) -> *mut c_char {
    to_c_string(serde_json::json!({"error": msg}).to_string())
}

/// Run a closure inside the pairing bridge's tokio runtime context.
///
/// This is needed so that `tokio::spawn` works when called from FFI
/// functions (which run outside any runtime). The closure runs
/// synchronously but with the runtime entered, so spawned tasks
/// land on the bridge's thread pool.
pub(crate) fn bridge_with_runtime<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce() -> R,
{
    let guard = PAIRING_BRIDGE
        .read()
        .map_err(|_| "bridge lock poisoned".to_string())?;
    let bridge = guard
        .as_ref()
        .ok_or_else(|| "pairing bridge not initialized".to_string())?;
    let _enter = bridge.runtime.enter();
    Ok(f())
}

/// Return the bridge's base data directory, if the bridge is initialized.
///
/// Used by `FlowRegistry` to derive its flow storage path from the same
/// base directory that capsules, identity, and static peers use.
pub(crate) fn bridge_data_dir() -> Option<PathBuf> {
    let guard = PAIRING_BRIDGE.read().ok()?;
    let bridge = guard.as_ref()?;
    Some(bridge.data_dir.clone())
}

/// Return the ID of the first capsule in the local store, if any.
///
/// Used by `soradyne_flow_enable_sync` so the app never needs a capsule ID.
pub(crate) fn bridge_first_capsule_id() -> Result<Uuid, String> {
    let guard = PAIRING_BRIDGE
        .read()
        .map_err(|_| "bridge lock poisoned".to_string())?;
    let bridge = guard
        .as_ref()
        .ok_or_else(|| "pairing bridge not initialized".to_string())?;

    bridge.runtime.block_on(async {
        let store = bridge.capsule_store.lock().await;
        let capsules = store.list_capsules();
        capsules
            .first()
            .map(|c| c.id)
            .ok_or_else(|| "no capsules found".to_string())
    })
}

/// Get or create an EnsembleManager for a capsule, returning its
/// (messenger, topology) for calling `set_ensemble` on a flow.
pub(crate) fn bridge_get_ensemble(
    capsule_id: Uuid,
) -> Result<
    (
        Arc<TopologyMessenger>,
        Arc<tokio::sync::RwLock<EnsembleTopology>>,
    ),
    String,
> {
    let guard = PAIRING_BRIDGE
        .read()
        .map_err(|_| "bridge lock poisoned".to_string())?;
    let bridge = guard
        .as_ref()
        .ok_or_else(|| "pairing bridge not initialized".to_string())?;

    bridge.runtime.block_on(async {
        let store = bridge.capsule_store.lock().await;
        let capsule = store
            .get_capsule(&capsule_id)
            .ok_or_else(|| format!("capsule {} not found", capsule_id))?;

        let mut managers = bridge.ensemble_managers.lock().await;

        if let Some(manager) = managers.get(&capsule_id) {
            let topo = Arc::clone(manager.topology());
            let messenger = Arc::clone(manager.messenger());
            return Ok((messenger, topo));
        }

        // Collect piece device_ids and static keys for the capsule
        let piece_ids: Vec<Uuid> = capsule
            .pieces
            .iter()
            .map(|p| p.device_id)
            .collect();

        let peer_static_keys: std::collections::HashMap<Uuid, [u8; 32]> = capsule
            .pieces
            .iter()
            .filter(|p| p.device_id != bridge.identity.device_id())
            .map(|p| (p.device_id, p.dh_public_key))
            .collect();

        // Get the capsule's key bundle
        let keys = capsule.keys.clone();

        // Load static peer addresses from config
        let static_peers = StaticPeerConfig::load(&bridge.data_dir)
            .map(|cfg| cfg.get(&capsule_id))
            .unwrap_or_default();

        let config = EnsembleConfig {
            static_peers,
            ..EnsembleConfig::default()
        };

        let manager = EnsembleManager::new(
            Arc::clone(&bridge.identity),
            keys,
            piece_ids,
            peer_static_keys,
            config,
        );

        // Start the ensemble manager with simulated BLE transports.
        // Static peer TCP connections run independently of BLE scan/advertise,
        // so sim transports are fine — real BLE will be wired in Phase 7.
        let central = bridge.sim_network.create_device();
        let peripheral = bridge.sim_network.create_device();
        manager.start(
            Arc::new(central),
            Arc::new(peripheral),
        ).await;

        let topo = Arc::clone(manager.topology());
        let messenger = Arc::clone(manager.messenger());

        managers.insert(capsule_id, manager);

        Ok((messenger, topo))
    })
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
        data_dir: base_dir,
        ensemble_managers: TokioMutex::new(HashMap::new()),
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

/// Delete a capsule by ID — removes from memory and from disk. Irreversible.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn soradyne_pairing_delete_capsule(capsule_id: *const c_char) -> i32 {
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
    let capsule_store = Arc::clone(&bridge.capsule_store);
    bridge.runtime.block_on(async move {
        let mut store = capsule_store.lock().await;
        match store.delete_capsule(&uuid) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("soradyne_pairing_delete_capsule: {}", e);
                -1
            }
        }
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

/// Start the inviter flow: advertise via BLE, accept connection, perform
/// ECDH key exchange. Runs asynchronously on the persistent runtime.
///
/// On Android: uses `AndroidBlePeripheral` (real BLE via JNI).
/// On all other platforms: falls back to `SimBleNetwork` (for local testing).
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

    let engine = Arc::clone(&bridge.engine);
    let capsule_store = Arc::clone(&bridge.capsule_store);

    #[cfg(target_os = "android")]
    {
        use crate::ble::android_peripheral::AndroidBlePeripheral;

        // The Android Context arrives via `nativeSetContext` which is called by
        // `onAttachedToEngine` — well before Dart calls `soradyne_pairing_init`.
        // It is stored in a separate OnceLock so that the timing gap between
        // plugin attach and bridge init doesn't cause it to be lost.
        let context_ref = match ANDROID_CONTEXT.get() {
            Some(r) => r.clone(),
            None => {
                eprintln!("[soradyne] startInvite: ANDROID_CONTEXT not set");
                return -1;
            }
        };

        match AndroidBlePeripheral::new(context_ref) {
            Ok(peripheral) => {
                let peripheral = Arc::new(peripheral);
                bridge.runtime.spawn(async move {
                    let store = capsule_store.lock().await;
                    if let Err(e) = engine.invite(uuid, &*store, &*peripheral).await {
                        eprintln!("soradyne_pairing_start_invite: invite failed: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("soradyne_pairing_start_invite: BLE init failed: {}", e);
                return -1;
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        // Non-Android: use the in-process SimBleNetwork (for tests and macOS demo
        // when the Mac is acting as the inviter in a purely simulated scenario).
        let device = bridge.sim_network.create_device();
        bridge.runtime.spawn(async move {
            let store = capsule_store.lock().await;
            if let Err(e) = engine.invite(uuid, &*store, &device).await {
                println!("soradyne_pairing_start_invite: invite failed: {}", e);
            }
        });
    }

    0
}

/// Start the joiner flow: scan for pairing advertisements, connect, perform
/// ECDH key exchange. Runs asynchronously on the persistent runtime.
///
/// On macOS/Linux/Windows with the `ble-central` feature: uses `BtleplugCentral`.
/// Otherwise: falls back to `SimBleNetwork` (for local testing and Android side).
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

    let engine = Arc::clone(&bridge.engine);

    #[cfg(feature = "ble-central")]
    {
        use crate::ble::btleplug_central::BtleplugCentral;

        // On macOS, CBCentralManager must be initialized from the main thread so
        // that CoreBluetooth can trigger the TCC Bluetooth permission dialog.
        // We call block_on here (FFI entry point = main thread on Flutter macOS)
        // to drive the async init synchronously, then hand the ready central
        // off to the tokio task for the actual join work.
        let central = match bridge.runtime.block_on(BtleplugCentral::new()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("soradyne_pairing_start_join: BLE central init failed: {}", e);
                return -1;
            }
        };

        bridge.runtime.spawn(async move {
            if let Err(e) = engine
                .join(name, PieceCapabilities::full(), PieceRole::Full, &central)
                .await
            {
                eprintln!("soradyne_pairing_start_join: join failed: {}", e);
            }
        });
    }

    #[cfg(not(feature = "ble-central"))]
    {
        let device = bridge.sim_network.create_device();
        bridge.runtime.spawn(async move {
            if let Err(e) = engine
                .join(name, PieceCapabilities::full(), PieceRole::Full, &device)
                .await
            {
                println!("soradyne_pairing_start_join: join failed: {}", e);
            }
        });
    }

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

/// Return the current device's UUID as a string.
///
/// The inventory system uses this as the CRDT author ID, ensuring
/// operations are attributed to the right device for cross-device sync.
/// Caller must free via `soradyne_free_string`.
#[no_mangle]
pub extern "C" fn soradyne_pairing_get_device_id() -> *mut c_char {
    let guard = match PAIRING_BRIDGE.read() {
        Ok(g) => g,
        Err(_) => return std::ptr::null_mut(),
    };
    match guard.as_ref() {
        Some(bridge) => to_c_string(bridge.identity.device_id().to_string()),
        None => std::ptr::null_mut(),
    }
}

/// Return and clear the in-process BLE debug log as a plain string.
///
/// Called from Dart's poll loop to surface Rust log lines that cannot reach
/// the `flutter run` terminal on macOS (native stdout is detached there).
/// Caller must free the returned string via `soradyne_free_string`.
#[no_mangle]
pub extern "C" fn soradyne_ble_debug() -> *mut c_char {
    let log = BLE_DEBUG
        .lock()
        .map(|mut g| {
            let content = g.clone();
            g.clear();
            content
        })
        .unwrap_or_default();
    to_c_string(log)
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

/// Store the Android `Context` object so that `soradyne_pairing_start_invite`
/// can pass it to `AndroidBlePeripheral`.
///
/// Called from Kotlin `SoradyneFlutterPlugin.nativeSetContext(context)` via
/// `private external fun nativeSetContext(context: Context)`.
///
/// This is intentionally decoupled from `PAIRING_BRIDGE`: `onAttachedToEngine`
/// fires ~10 s before Dart calls `soradyne_pairing_init`, so the bridge does
/// not exist yet when the context arrives. Storing it in `ANDROID_CONTEXT`
/// (a separate `OnceLock`) makes the call succeed unconditionally.
///
/// Returns 0 on success, -1 if the JNI global ref could not be created.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_soradyne_flutter_SoradyneFlutterPlugin_nativeSetContext(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    context: jni::objects::JObject,
) -> jni::sys::jint {
    let global_ref = match env.new_global_ref(&context) {
        Ok(r) => r,
        Err(_) => return -1,
    };

    // OnceLock::set fails silently if already set — that's fine, the first
    // context is the one we want (application context, set at engine attach).
    let _ = ANDROID_CONTEXT.set(global_ref);

    // Cache JNI class GlobalRefs for the two app-defined callback classes while
    // we are still on the Android main thread where the application class loader
    // is active. find_class on Tokio worker threads uses the bootstrap loader
    // and cannot see app classes, causing ClassNotFoundException.
    if let Err(e) = crate::ble::android_peripheral::cache_classes(&mut env) {
        // Log via logcat — eprintln is invisible on Android.
        let tag = env.new_string("soradyne").unwrap_or_default();
        let msg = env.new_string(format!("cache_classes failed: {}", e)).unwrap_or_default();
        let _ = env.call_static_method(
            "android/util/Log",
            "e",
            "(Ljava/lang/String;Ljava/lang/String;)I",
            &[jni::objects::JValue::Object(&tag.into()),
              jni::objects::JValue::Object(&msg.into())],
        );
        return -1;
    }

    0
}
