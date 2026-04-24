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
/// A schema provides optional structural metadata and validation for a
/// `ConvergentDocument`. The simplest schema is `()`, which accepts any
/// item type and performs no validation.
///
/// App-specific schemas can be defined outside soradyne_core to add
/// validation without coupling app logic to the sync library.
pub trait DocumentSchema: Send + Sync + Clone {
    /// Get the item type spec for a given type name, if the schema knows about it.
    fn item_type_spec(&self, type_name: &str) -> Option<Box<dyn ItemTypeSpec>>;

    /// Get all known item type names (empty means "accept anything").
    fn item_types(&self) -> HashSet<String>;

    /// Validate the materialized document state. Returns an empty vec if valid.
    fn validate(&self, state: &super::document::DocumentState) -> Vec<ValidationIssue>;
}

/// The no-op schema: accepts any item type, performs no validation.
impl DocumentSchema for () {
    fn item_type_spec(&self, _: &str) -> Option<Box<dyn ItemTypeSpec>> {
        None
    }
    fn item_types(&self) -> HashSet<String> {
        HashSet::new()
    }
    fn validate(&self, _: &super::document::DocumentState) -> Vec<ValidationIssue> {
        vec![]
    }
}
