//! Node.js bindings for Soradyne
//!
//! This module provides bindings for the Soradyne library to be used from Node.js
//! via TypeScript.

mod identity;
mod storage;
mod sdo;

use napi_derive::napi;

// Export all binding modules
pub use identity::*;
pub use storage::*;
pub use sdo::*;

/// Version information for the Soradyne library
#[napi]
pub fn version() -> String {
    format!("{}", env!("CARGO_PKG_VERSION"))
}

/// Initialize the Soradyne library
#[napi]
pub fn initialize() -> bool {
    // Perform any necessary initialization
    true
}
