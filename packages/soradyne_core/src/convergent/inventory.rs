//! Inventory Schema for Convergent Documents
//!
//! Defines the structure of inventory items: category, description, location,
//! and tags. Supports containers as a special item type with container_* tags.

use super::document::DocumentState;
use super::schema::{DocumentSchema, FieldSpec, ItemTypeSpec, SetSpec, ValidationIssue};
use std::collections::{HashMap, HashSet};

/// The Inventory item type specification
#[derive(Clone, Debug)]
pub struct InventoryItemSpec;

impl ItemTypeSpec for InventoryItemSpec {
    fn type_name(&self) -> &str {
        "InventoryItem"
    }

    fn fields(&self) -> Vec<FieldSpec> {
        vec![
            FieldSpec::required("category"),
            FieldSpec::required("description"),
            FieldSpec::required("location"),
        ]
    }

    fn sets(&self) -> Vec<SetSpec> {
        vec![SetSpec::new("tags")]
    }
}

/// The Inventory document schema
#[derive(Clone, Debug)]
pub struct InventorySchema;

impl DocumentSchema for InventorySchema {
    type State = InventoryState;

    fn item_type_spec(&self, type_name: &str) -> Option<Box<dyn ItemTypeSpec>> {
        match type_name {
            "InventoryItem" => Some(Box::new(InventoryItemSpec)),
            _ => None,
        }
    }

    fn item_types(&self) -> HashSet<String> {
        HashSet::from(["InventoryItem".to_string()])
    }

    fn validate(&self, state: &Self::State) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Check for items referencing non-existent containers
        for (item_id, item) in &state.items {
            for tag in &item.tags {
                if let Some(container_id) = tag.strip_prefix("container_") {
                    // Check that a container with this ID exists
                    let container_exists = state.items.values().any(|other| {
                        other.category == "Containers"
                            && other.tags.contains(&format!("container_{}", container_id))
                    });
                    if !container_exists {
                        issues.push(
                            ValidationIssue::warning(format!(
                                "Item '{}' references non-existent container '{}'",
                                item.description, container_id
                            ))
                            .for_item(item_id),
                        );
                    }
                }
            }
        }

        issues
    }
}

/// Materialized state specific to Inventory
#[derive(Clone, Debug, Default)]
pub struct InventoryState {
    pub items: HashMap<String, InventoryItem>,
}

/// A single inventory item with typed fields
#[derive(Clone, Debug, Default)]
pub struct InventoryItem {
    pub id: String,
    pub category: String,
    pub description: String,
    pub location: String,
    pub tags: HashSet<String>,
}

impl InventoryState {
    /// Convert from generic DocumentState to Inventory-specific state
    pub fn from_document_state(doc_state: &DocumentState) -> Self {
        let mut state = InventoryState::default();

        for (item_id, item_state) in doc_state.iter_existing() {
            if item_state.item_type != "InventoryItem" {
                continue;
            }

            let item = InventoryItem {
                id: item_id.clone(),
                category: extract_string(&item_state.fields, "category").unwrap_or_default(),
                description: extract_string(&item_state.fields, "description").unwrap_or_default(),
                location: extract_string(&item_state.fields, "location").unwrap_or_default(),
                tags: extract_string_set(&item_state.sets, "tags"),
            };

            state.items.insert(item_id.clone(), item);
        }

        state
    }
}

// Helper functions for extracting typed values from generic state

fn extract_string(
    fields: &HashMap<String, super::operation::Value>,
    key: &str,
) -> Option<String> {
    fields.get(key).and_then(|v| match v {
        super::operation::Value::String(s) => Some(s.clone()),
        _ => None,
    })
}

fn extract_string_set(
    sets: &HashMap<String, HashSet<super::operation::Value>>,
    key: &str,
) -> HashSet<String> {
    sets.get(key)
        .map(|s| {
            s.iter()
                .filter_map(|v| match v {
                    super::operation::Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convergent::{ConvergentDocument, Operation, Value};

    #[test]
    fn test_add_and_materialize() {
        let mut doc = ConvergentDocument::new(InventorySchema, "device_A".into());

        doc.apply_local(Operation::add_item("item_1", "InventoryItem"));
        doc.apply_local(Operation::set_field("item_1", "category", Value::string("Tools")));
        doc.apply_local(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));
        doc.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("Toolbox"),
        ));

        let doc_state = doc.materialize();
        let state = InventoryState::from_document_state(&doc_state);

        assert_eq!(state.items.len(), 1);
        let item = state.items.get("item_1").unwrap();
        assert_eq!(item.category, "Tools");
        assert_eq!(item.description, "Hammer");
        assert_eq!(item.location, "Toolbox");
        assert!(item.tags.is_empty());
    }

    #[test]
    fn test_item_with_tags() {
        let mut doc = ConvergentDocument::new(InventorySchema, "device_A".into());

        doc.apply_local(Operation::add_item("item_1", "InventoryItem"));
        doc.apply_local(Operation::set_field(
            "item_1",
            "category",
            Value::string("Decor"),
        ));
        doc.apply_local(Operation::set_field(
            "item_1",
            "description",
            Value::string("Blue Vase"),
        ));
        doc.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("Shelf"),
        ));
        doc.apply_local(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("fragile"),
        ));
        doc.apply_local(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("valuable"),
        ));

        let doc_state = doc.materialize();
        let state = InventoryState::from_document_state(&doc_state);

        let item = state.items.get("item_1").unwrap();
        assert!(item.tags.contains("fragile"));
        assert!(item.tags.contains("valuable"));
        assert_eq!(item.tags.len(), 2);
    }

    #[test]
    fn test_delete_item() {
        let mut doc = ConvergentDocument::new(InventorySchema, "device_A".into());

        doc.apply_local(Operation::add_item("item_1", "InventoryItem"));
        doc.apply_local(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));
        doc.apply_local(Operation::remove_item("item_1"));

        let doc_state = doc.materialize();
        let state = InventoryState::from_document_state(&doc_state);

        assert!(state.items.is_empty());
    }

    #[test]
    fn test_update_field() {
        let mut doc = ConvergentDocument::new(InventorySchema, "device_A".into());

        doc.apply_local(Operation::add_item("item_1", "InventoryItem"));
        doc.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("Kitchen"),
        ));
        doc.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("Garage"),
        ));

        let doc_state = doc.materialize();
        let state = InventoryState::from_document_state(&doc_state);

        let item = state.items.get("item_1").unwrap();
        assert_eq!(item.location, "Garage");
    }

    #[test]
    fn test_container_validation() {
        let mut doc = ConvergentDocument::new(InventorySchema, "device_A".into());

        // Item referencing a non-existent container
        doc.apply_local(Operation::add_item("item_1", "InventoryItem"));
        doc.apply_local(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));
        doc.apply_local(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("container_nonexistent"),
        ));

        let doc_state = doc.materialize();
        let state = InventoryState::from_document_state(&doc_state);
        let issues = InventorySchema.validate(&state);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("non-existent container"));
    }

    #[test]
    fn test_container_system() {
        let mut doc = ConvergentDocument::new(InventorySchema, "device_A".into());

        // Create a container
        doc.apply_local(Operation::add_item("container_1", "InventoryItem"));
        doc.apply_local(Operation::set_field(
            "container_1",
            "category",
            Value::string("Containers"),
        ));
        doc.apply_local(Operation::set_field(
            "container_1",
            "description",
            Value::string("Tool Chest"),
        ));
        doc.apply_local(Operation::set_field(
            "container_1",
            "location",
            Value::string("Garage"),
        ));
        doc.apply_local(Operation::add_to_set(
            "container_1",
            "tags",
            Value::string("container_Tool Chest"),
        ));

        // Create an item in the container
        doc.apply_local(Operation::add_item("item_1", "InventoryItem"));
        doc.apply_local(Operation::set_field(
            "item_1",
            "category",
            Value::string("Tools"),
        ));
        doc.apply_local(Operation::set_field(
            "item_1",
            "description",
            Value::string("Screwdriver"),
        ));
        doc.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("container Tool Chest"),
        ));
        doc.apply_local(Operation::add_to_set(
            "item_1",
            "tags",
            Value::string("container_Tool Chest"),
        ));

        let doc_state = doc.materialize();
        let state = InventoryState::from_document_state(&doc_state);
        let issues = InventorySchema.validate(&state);

        // No validation issues - container exists
        assert!(issues.is_empty());
        assert_eq!(state.items.len(), 2);
    }

    #[test]
    fn test_multi_device_convergence() {
        let mut doc_a = ConvergentDocument::new(InventorySchema, "A".into());
        let mut doc_b = ConvergentDocument::new(InventorySchema, "B".into());

        // A creates an item
        let op1 = doc_a.apply_local(Operation::add_item("item_1", "InventoryItem"));
        let op2 = doc_a.apply_local(Operation::set_field(
            "item_1",
            "description",
            Value::string("Hammer"),
        ));
        let op3 = doc_a.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("Kitchen"),
        ));

        // B receives A's ops
        doc_b.apply_remote(op1);
        doc_b.apply_remote(op2);
        doc_b.apply_remote(op3);

        // B updates location concurrently
        let op4 = doc_b.apply_local(Operation::set_field(
            "item_1",
            "location",
            Value::string("Garage"),
        ));

        // A receives B's update
        doc_a.apply_remote(op4);

        // Both should converge
        let state_a =
            InventoryState::from_document_state(&doc_a.materialize());
        let state_b =
            InventoryState::from_document_state(&doc_b.materialize());

        assert_eq!(
            state_a.items.get("item_1").unwrap().location,
            state_b.items.get("item_1").unwrap().location
        );
    }
}
