//! Convergent Document System
//!
//! Generic infrastructure for leaderless, peer-to-peer document synchronization.
//! Supports any structured data (Giantt graphs, photo albums, network topology)
//! through schema definitions.

mod horizon;
mod operation;
mod document;
mod schema;
pub mod giantt;
pub mod inventory;

pub use horizon::{Horizon, DeviceId, SeqNum};
pub use operation::{Operation, OpId, OpEnvelope, Value, ItemId};
pub use document::{ConvergentDocument, DocumentState, ItemState};
pub use schema::{DocumentSchema, FieldSpec, SetSpec, ItemTypeSpec, ValidationIssue, IssueSeverity};
