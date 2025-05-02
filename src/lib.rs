//! Soradyne: A protocol for secure, peer-to-peer shared self-data objects
//! 
//! This library implements the core functionality of the Soradyne protocol,
//! including identity management, transport, synchronization primitives, and
//! self-data objects.

pub mod core;
pub mod sdo;
pub mod storage;
mod bindings;

// Re-export key types for convenience
pub use crate::core::identity::Identity;
pub use crate::sdo::SelfDataObject;

// This is the entry point for the Node.js binding
#[cfg(feature = "napi")]
#[napi::module_init]
fn init() -> napi::Result<()> {
    // Register any Node.js-specific initialization here
    Ok(())
}

