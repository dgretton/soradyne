//! Schema definition traits for convergent documents
//!
//! A schema describes the structure of items: what fields they have,
//! what sets they contain, and how to validate the materialized state.

use std::collections::HashSet;

/// Specification for a scalar field
#[derive(Clone, Debug)]
pub struct FieldSpec {
    pub name: String,
    pub required: bool,
}

impl FieldSpec {
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
        }
    }

    pub fn optional(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: false,
        }
    }
}

/// Specification for a set/collection field
#[derive(Clone, Debug)]
pub struct SetSpec {
    pub name: String,
}

impl SetSpec {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Specification for an item type within a schema
pub trait ItemTypeSpec: Send + Sync {
    /// The type name (e.g., "GianttItem", "Photo", "NetworkNode")
    fn type_name(&self) -> &str;

    /// Scalar fields this item type has
    fn fields(&self) -> Vec<FieldSpec>;

    /// Set/collection fields this item type has
    fn sets(&self) -> Vec<SetSpec>;

    /// Validate field names
    fn has_field(&self, name: &str) -> bool {
        self.fields().iter().any(|f| f.name == name)
    }

    /// Validate set names
    fn has_set(&self, name: &str) -> bool {
        self.sets().iter().any(|s| s.name == name)
    }
}

/// A validation issue found in the document state
#[derive(Clone, Debug)]
pub struct ValidationIssue {
    pub item_id: Option<String>,
    pub severity: IssueSeverity,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueSeverity {
    Warning,
    Error,
}

impl ValidationIssue {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            item_id: None,
            severity: IssueSeverity::Error,
            message: message.into(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            item_id: None,
            severity: IssueSeverity::Warning,
            message: message.into(),
        }
    }

    pub fn for_item(mut self, item_id: impl Into<String>) -> Self {
        self.item_id = Some(item_id.into());
        self
    }
}

/// Trait for document schemas
///
/// A schema defines:
/// - What item types exist
/// - How to materialize state from operations
/// - How to validate the materialized state
pub trait DocumentSchema: Send + Sync + Clone {
    /// The materialized state type
    type State: Send + Sync;

    /// Get the item type spec for a given type name
    fn item_type_spec(&self, type_name: &str) -> Option<Box<dyn ItemTypeSpec>>;

    /// Get all known item type names
    fn item_types(&self) -> HashSet<String>;

    /// Schema-specific validation of materialized state
    fn validate(&self, state: &Self::State) -> Vec<ValidationIssue>;
}
