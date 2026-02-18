// Soradyne - Collaborative Media Album System

pub mod album;
pub mod ble;
pub mod convergent;
pub mod ffi;
pub mod flow;
pub mod identity;
pub mod network;
pub mod storage;
pub mod topology;
pub mod types;
pub mod video;

// ---------------------------------------------------------------------------
// Android JVM bootstrap
// ---------------------------------------------------------------------------
//
// `JNI_OnLoad` is called automatically by the Android runtime when `libsoradyne.so`
// is loaded. We capture the `JavaVM` pointer here so that the BLE peripheral and
// any other JNI code can attach to the JVM from arbitrary Rust threads without
// needing a `JNIEnv` passed in from Java.

#[cfg(target_os = "android")]
pub mod android_init {
    use jni::JavaVM;
    use std::sync::{Arc, OnceLock};

    static ANDROID_JVM: OnceLock<Arc<JavaVM>> = OnceLock::new();

    /// Called automatically by the Android runtime when the .so is loaded.
    #[no_mangle]
    pub extern "system" fn JNI_OnLoad(
        vm: jni::JavaVM,
        _reserved: *mut std::os::raw::c_void,
    ) -> jni::sys::jint {
        let _ = ANDROID_JVM.set(Arc::new(vm));
        jni::JNIVersion::V6.into()
    }

    /// Retrieve the captured `JavaVM`, or `None` if `JNI_OnLoad` has not fired yet.
    pub fn get_jvm() -> Option<Arc<JavaVM>> {
        ANDROID_JVM.get().cloned()
    }
}

#[cfg(target_os = "android")]
pub use android_init::get_jvm;

use crate::storage::device_identity::discover_soradyne_volumes;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{interval, Duration};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct StorageStatus {
    pub available_devices: usize,
    pub required_threshold: usize,
    pub can_read_data: bool,
    pub missing_devices: usize,
    pub device_paths: Vec<String>,
}

