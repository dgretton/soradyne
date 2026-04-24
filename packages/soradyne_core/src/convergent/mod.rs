//! Convergent Document System
//!
//! Generic infrastructure for leaderless, peer-to-peer document synchronization.
//! App-specific schemas (field definitions, validation) live outside soradyne_core.
//! The default schema is `()`, which accepts any item type and performs no validation.

mod horizon;
mod operation;
mod document;
mod schema;

pub use horizon::{Horizon, DeviceId, SeqNum};
pub use operation::{Operation, OpId, OpEnvelope, Value, ItemId};
pub use document::{ConvergentDocument, DocumentState, ItemState};
pub use schema::{DocumentSchema, FieldSpec, SetSpec, ItemTypeSpec, ValidationIssue, IssueSeverity};
