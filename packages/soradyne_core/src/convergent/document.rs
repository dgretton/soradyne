//! The core ConvergentDocument type
//!
//! A ConvergentDocument stores operations and materializes them into state.
//! It handles informed-remove semantics and latest-wins field resolution.

use super::horizon::{DeviceId, Horizon, SeqNum};
use super::operation::{ItemId, OpEnvelope, OpId, Operation, Value};
use super::schema::DocumentSchema;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Materialized state of a single item
#[derive(Clone, Debug, Default)]
pub struct ItemState {
    pub item_type: String,
    pub fields: HashMap<String, Value>,
    pub sets: HashMap<String, HashSet<Value>>,
    pub exists: bool,
}

/// Materialized state of the entire document
#[derive(Clone, Debug, Default)]
pub struct DocumentState {
    pub items: HashMap<ItemId, ItemState>,
}

impl DocumentState {
    pub fn get(&self, item_id: &ItemId) -> Option<&ItemState> {
        self.items.get(item_id).filter(|i| i.exists)
    }

    pub fn iter_existing(&self) -> impl Iterator<Item = (&ItemId, &ItemState)> {
        self.items.iter().filter(|(_, i)| i.exists)
    }
}

/// A convergent document generic over its schema
pub struct ConvergentDocument<S: DocumentSchema> {
    /// The schema defining item structure
    schema: S,

    /// This device's identity
    device_id: DeviceId,

    /// Next sequence number for local operations
    next_seq: SeqNum,

    /// Our current horizon (what we've seen)
    horizon: Horizon,

    /// All operations, ordered by (author, seq)
    operations: BTreeMap<(DeviceId, SeqNum), OpEnvelope>,

    /// Index: item_id -> operation IDs affecting that item
    item_ops: HashMap<ItemId, Vec<(DeviceId, SeqNum)>>,

    /// Index: AddToSet operations by (item_id, set_name, element)
    add_to_set_ops: HashMap<(ItemId, String, Value), Vec<OpId>>,
}

impl<S: DocumentSchema> ConvergentDocument<S> {
    /// Create a new empty document
    pub fn new(schema: S, device_id: DeviceId) -> Self {
        Self {
            schema,
            device_id,
            next_seq: 1,
            horizon: Horizon::new(),
            operations: BTreeMap::new(),
            item_ops: HashMap::new(),
            add_to_set_ops: HashMap::new(),
        }
    }

    /// Get this document's device ID
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// Get the current horizon
    pub fn horizon(&self) -> &Horizon {
        &self.horizon
    }

    /// Apply a local operation (wraps it in an envelope and stores)
    pub fn apply_local(&mut self, op: Operation) -> OpEnvelope {
        let envelope = OpEnvelope::new(
            self.device_id.clone(),
            self.next_seq,
            self.horizon.clone(),
            op,
        );
        self.next_seq += 1;
        self.store_operation(envelope.clone());
        envelope
    }

    /// Apply a remote operation (already wrapped)
    pub fn apply_remote(&mut self, envelope: OpEnvelope) {
        // Skip if we already have this operation
        let key = (envelope.author.clone(), envelope.seq);
        if self.operations.contains_key(&key) {
            return;
        }
        self.store_operation(envelope);
    }

    /// Store an operation and update indexes
    fn store_operation(&mut self, envelope: OpEnvelope) {
        let key = (envelope.author.clone(), envelope.seq);

        // Update horizon
        self.horizon.observe(&envelope.author, envelope.seq);

        // Update item index
        let item_id = envelope.op.item_id().clone();
        self.item_ops
            .entry(item_id.clone())
            .or_default()
            .push(key.clone());

        // Update AddToSet index
        if let Operation::AddToSet {
            item_id,
            set_name,
            element,
        } = &envelope.op
        {
            self.add_to_set_ops
                .entry((item_id.clone(), set_name.clone(), element.clone()))
                .or_default()
                .push(envelope.id);
        }

        // Store the operation
        self.operations.insert(key, envelope);
    }

    /// Get all operations (for syncing)
    pub fn all_operations(&self) -> impl Iterator<Item = &OpEnvelope> {
        self.operations.values()
    }

    /// Get operations since a horizon (for incremental sync)
    pub fn operations_since(&self, since: &Horizon) -> Vec<&OpEnvelope> {
        self.operations
            .iter()
            .filter(|((author, seq), _)| !since.has_seen(author, *seq))
            .map(|(_, env)| env)
            .collect()
    }

    /// Materialize the current state from all operations
    pub fn materialize(&self) -> DocumentState {
        let mut state = DocumentState::default();

        // Group operations by item
        for (item_id, op_keys) in &self.item_ops {
            let item_state = self.materialize_item(item_id, op_keys);
            state.items.insert(item_id.clone(), item_state);
        }

        state
    }

    /// Materialize a single item's state
    fn materialize_item(&self, _item_id: &ItemId, op_keys: &[(DeviceId, SeqNum)]) -> ItemState {
        let mut item = ItemState::default();

        // Collect relevant operations in order
        let mut ops: Vec<&OpEnvelope> = op_keys
            .iter()
            .filter_map(|k| self.operations.get(k))
            .collect();
        ops.sort_by_key(|e| (e.timestamp, &e.author));

        // Track AddItem and RemoveItem for existence
        let mut add_ops: Vec<&OpEnvelope> = Vec::new();
        let mut remove_ops: Vec<&OpEnvelope> = Vec::new();

        // Track SetField ops per field (for latest-wins)
        let mut field_ops: HashMap<&str, Vec<&OpEnvelope>> = HashMap::new();

        // Track AddToSet and RemoveFromSet per (set_name, element)
        let mut set_add_ops: HashMap<(&str, &Value), Vec<&OpEnvelope>> = HashMap::new();
        let mut set_remove_ops: HashMap<(&str, &Value), Vec<&OpEnvelope>> = HashMap::new();

        for env in ops {
            match &env.op {
                Operation::AddItem { item_type, .. } => {
                    item.item_type = item_type.clone();
                    add_ops.push(env);
                }
                Operation::RemoveItem { .. } => {
                    remove_ops.push(env);
                }
                Operation::SetField { field, .. } => {
                    field_ops.entry(field.as_str()).or_default().push(env);
                }
                Operation::AddToSet {
                    set_name, element, ..
                } => {
                    set_add_ops
                        .entry((set_name.as_str(), element))
                        .or_default()
                        .push(env);
                }
                Operation::RemoveFromSet {
                    set_name, element, ..
                } => {
                    set_remove_ops
                        .entry((set_name.as_str(), element))
                        .or_default()
                        .push(env);
                }
            }
        }

        // Determine existence using informed-remove semantics:
        // The item exists if there's any operation (add or edit) that a remove didn't observe.
        // This means: an item survives if someone made changes the deleter didn't know about.
        if add_ops.is_empty() {
            item.exists = false;
        } else if remove_ops.is_empty() {
            item.exists = true;
        } else {
            // Collect all operations that affect this item's existence (adds and edits)
            let all_affecting_ops: Vec<&OpEnvelope> = add_ops.iter()
                .chain(field_ops.values().flatten())
                .copied()
                .collect();

            // Item exists if any affecting operation wasn't observed by ALL removes
            item.exists = all_affecting_ops.iter().any(|op| {
                // This operation keeps the item alive if any remove didn't see it
                remove_ops.iter().any(|rem| !rem.had_seen(op))
            });
        }

        // Resolve fields: latest-wins
        for (field, ops) in field_ops {
            if let Some(winner) = ops.iter().max_by(|a, b| {
                (a.timestamp, &a.author).cmp(&(b.timestamp, &b.author))
            }) {
                if let Operation::SetField { value, .. } = &winner.op {
                    item.fields.insert(field.to_string(), value.clone());
                }
            }
        }

        // Resolve sets: add wins unless remove had observed that specific add
        for ((set_name, element), adds) in set_add_ops {
            let removes = set_remove_ops.get(&(set_name, element)).cloned().unwrap_or_default();

            // Element is present if any add is not covered by a remove that observed it
            let present = adds.iter().any(|add| {
                !removes.iter().any(|rem| {
                    if let Operation::RemoveFromSet { observed_add_ids, .. } = &rem.op {
                        observed_add_ids.contains(&add.id)
                    } else {
                        false
                    }
                })
            });

            if present {
                item.sets
                    .entry(set_name.to_string())
                    .or_default()
                    .insert(element.clone());
            }
        }

        item
    }

    /// Get the AddToSet operation IDs for an element (needed for RemoveFromSet)
    pub fn get_add_ids_for_element(
        &self,
        item_id: &ItemId,
        set_name: &str,
        element: &Value,
    ) -> Vec<OpId> {
        self.add_to_set_ops
            .get(&(item_id.clone(), set_name.to_string(), element.clone()))
            .cloned()
            .unwrap_or_default()
    }

    /// Validate the current state using the schema
    pub fn validate(&self) -> Vec<super::schema::ValidationIssue> {
        let state = self.materialize();
        // For now, delegate to schema. In the future, could add generic validation.
        // Note: This requires DocumentSchema::State to be DocumentState or compatible.
        // For generic schemas with custom state types, they'd override this.
        self.schema.validate(&self.schema_state(&state))
    }

    /// Convert generic DocumentState to schema-specific state
    /// Override this in schema-specific implementations
    fn schema_state(&self, _state: &DocumentState) -> S::State {
        // This is a placeholder - real implementations would convert
        // For now, panic if called on a schema that doesn't use DocumentState
        panic!("Schema must implement custom state conversion")
    }

    /// Compute a hash of the current state for compaction coordination
    pub fn state_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let state = self.materialize();
        let mut hasher = DefaultHasher::new();

        // Hash all existing items deterministically
        let mut items: Vec<_> = state.iter_existing().collect();
        items.sort_by_key(|(id, _)| *id);

        for (item_id, item) in items {
            item_id.hash(&mut hasher);
            item.item_type.hash(&mut hasher);

            // Hash fields deterministically
            let mut fields: Vec<_> = item.fields.iter().collect();
            fields.sort_by_key(|(k, _)| *k);
            for (k, v) in fields {
                k.hash(&mut hasher);
                format!("{:?}", v).hash(&mut hasher);
            }

            // Hash sets deterministically
            let mut sets: Vec<_> = item.sets.iter().collect();
            sets.sort_by_key(|(k, _)| *k);
            for (k, v) in sets {
                k.hash(&mut hasher);
                let mut elements: Vec<_> = v.iter().map(|e| format!("{:?}", e)).collect();
                elements.sort();
                for e in elements {
                    e.hash(&mut hasher);
                }
            }
        }

        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Minimal test schema
    #[derive(Clone)]
    struct TestSchema;

    impl DocumentSchema for TestSchema {
        type State = DocumentState;

        fn item_type_spec(&self, _: &str) -> Option<Box<dyn super::super::schema::ItemTypeSpec>> {
            None
        }

        fn item_types(&self) -> HashSet<String> {
            HashSet::from(["Test".to_string()])
        }

        fn validate(&self, _: &Self::State) -> Vec<super::super::schema::ValidationIssue> {
            Vec::new()
        }
    }

    #[test]
    fn test_add_and_materialize() {
        let mut doc = ConvergentDocument::new(TestSchema, "device_A".into());

        doc.apply_local(Operation::add_item("task_1", "Test"));
        doc.apply_local(Operation::set_field("task_1", "title", Value::string("Hello")));

        let state = doc.materialize();
        let item = state.get(&"task_1".into()).unwrap();

        assert!(item.exists);
        assert_eq!(item.fields.get("title"), Some(&Value::string("Hello")));
    }

    #[test]
    fn test_informed_remove() {
        let mut doc_a = ConvergentDocument::new(TestSchema, "A".into());
        let mut doc_b = ConvergentDocument::new(TestSchema, "B".into());

        // A creates an item
        let add_op = doc_a.apply_local(Operation::add_item("task_1", "Test"));

        // B receives the add
        doc_b.apply_remote(add_op.clone());

        // B makes an edit
        let edit_op = doc_b.apply_local(Operation::set_field(
            "task_1",
            "title",
            Value::string("B's edit"),
        ));

        // A (not seeing B's edit) deletes the item
        let delete_op = doc_a.apply_local(Operation::remove_item("task_1"));

        // Now sync: A receives B's edit, B receives A's delete
        doc_a.apply_remote(edit_op);
        doc_b.apply_remote(delete_op);

        // Both should converge: item should exist because B's edit wasn't seen by A's delete
        let state_a = doc_a.materialize();
        let state_b = doc_b.materialize();

        // The item should exist on both (B's concurrent edit survives)
        assert!(state_a.get(&"task_1".into()).is_some());
        assert!(state_b.get(&"task_1".into()).is_some());
    }

    #[test]
    fn test_set_operations() {
        let mut doc = ConvergentDocument::new(TestSchema, "A".into());

        doc.apply_local(Operation::add_item("task_1", "Test"));
        doc.apply_local(Operation::add_to_set(
            "task_1",
            "tags",
            Value::string("important"),
        ));
        doc.apply_local(Operation::add_to_set(
            "task_1",
            "tags",
            Value::string("urgent"),
        ));

        let state = doc.materialize();
        let item = state.get(&"task_1".into()).unwrap();
        let tags = item.sets.get("tags").unwrap();

        assert!(tags.contains(&Value::string("important")));
        assert!(tags.contains(&Value::string("urgent")));
    }
}
