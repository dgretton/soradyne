//! Giantt Schema for Convergent Documents
//!
//! Defines the structure of Giantt task items: fields, sets, relations,
//! and domain-specific validation (cycles, dangling references, etc.)

use super::document::DocumentState;
use super::schema::{DocumentSchema, FieldSpec, ItemTypeSpec, SetSpec, ValidationIssue};
use std::collections::{HashMap, HashSet};

/// The Giantt item type specification
#[derive(Clone, Debug)]
pub struct GianttItemSpec;

impl ItemTypeSpec for GianttItemSpec {
    fn type_name(&self) -> &str {
        "GianttItem"
    }

    fn fields(&self) -> Vec<FieldSpec> {
        vec![
            FieldSpec::required("title"),
            FieldSpec::optional("status"),      // TODO, IN_PROGRESS, COMPLETED, BLOCKED
            FieldSpec::optional("priority"),    // LOW, MEDIUM, HIGH, CRITICAL
            FieldSpec::optional("duration"),    // e.g., "2h", "30min", "1d"
            FieldSpec::optional("comment"),     // Freeform notes
        ]
    }

    fn sets(&self) -> Vec<SetSpec> {
        vec![
            SetSpec::new("tags"),           // Arbitrary labels
            SetSpec::new("charts"),         // Gantt chart groupings
            SetSpec::new("requires"),       // Dependency: this task requires those
            SetSpec::new("anyof"),          // Dependency: this task requires any one of those
            SetSpec::new("blocks"),         // Inverse of requires
            SetSpec::new("timeConstraints"), // Time-based constraints
        ]
    }
}

/// The Giantt document schema
#[derive(Clone, Debug)]
pub struct GianttSchema;

impl DocumentSchema for GianttSchema {
    type State = GianttState;

    fn item_type_spec(&self, type_name: &str) -> Option<Box<dyn ItemTypeSpec>> {
        match type_name {
            "GianttItem" => Some(Box::new(GianttItemSpec)),
            _ => None,
        }
    }

    fn item_types(&self) -> HashSet<String> {
        HashSet::from(["GianttItem".to_string()])
    }

    fn validate(&self, state: &Self::State) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Check for cycles in the dependency graph
        if let Some(cycle) = find_cycle(&state.items) {
            issues.push(
                ValidationIssue::error(format!(
                    "Dependency cycle detected: {}",
                    cycle.join(" -> ")
                ))
            );
        }

        // Check for dangling references
        for (item_id, item) in &state.items {
            // Check requires references
            for dep_id in &item.requires {
                if !state.items.contains_key(dep_id) {
                    issues.push(
                        ValidationIssue::warning(format!(
                            "Item '{}' requires non-existent item '{}'",
                            item_id, dep_id
                        ))
                        .for_item(item_id),
                    );
                }
            }

            // Check anyof references
            for dep_id in &item.anyof {
                if !state.items.contains_key(dep_id) {
                    issues.push(
                        ValidationIssue::warning(format!(
                            "Item '{}' has anyof reference to non-existent item '{}'",
                            item_id, dep_id
                        ))
                        .for_item(item_id),
                    );
                }
            }

            // Check blocks references
            for dep_id in &item.blocks {
                if !state.items.contains_key(dep_id) {
                    issues.push(
                        ValidationIssue::warning(format!(
                            "Item '{}' blocks non-existent item '{}'",
                            item_id, dep_id
                        ))
                        .for_item(item_id),
                    );
                }
            }
        }

        issues
    }
}

/// Materialized state specific to Giantt
#[derive(Clone, Debug, Default)]
pub struct GianttState {
    pub items: HashMap<String, GianttItem>,
}

/// A single Giantt item with typed fields
#[derive(Clone, Debug, Default)]
pub struct GianttItem {
    pub id: String,
    pub title: String,
    pub status: GianttStatus,
    pub priority: GianttPriority,
    pub duration: Option<String>,
    pub comment: Option<String>,
    pub tags: HashSet<String>,
    pub charts: HashSet<String>,
    pub requires: HashSet<String>,
    pub anyof: HashSet<String>,
    pub blocks: HashSet<String>,
    pub time_constraints: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GianttStatus {
    #[default]
    Todo,
    InProgress,
    Completed,
    Blocked,
}

impl GianttStatus {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "TODO" | "" => GianttStatus::Todo,
            "IN_PROGRESS" | "INPROGRESS" => GianttStatus::InProgress,
            "COMPLETED" | "DONE" => GianttStatus::Completed,
            "BLOCKED" => GianttStatus::Blocked,
            _ => GianttStatus::Todo,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GianttPriority {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

impl GianttPriority {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "LOW" => GianttPriority::Low,
            "MEDIUM" | "" => GianttPriority::Medium,
            "HIGH" => GianttPriority::High,
            "CRITICAL" => GianttPriority::Critical,
            _ => GianttPriority::Medium,
        }
    }
}

impl GianttState {
    /// Convert from generic DocumentState to Giantt-specific state
    pub fn from_document_state(doc_state: &DocumentState) -> Self {
        let mut state = GianttState::default();

        for (item_id, item_state) in doc_state.iter_existing() {
            if item_state.item_type != "GianttItem" {
                continue;
            }

            let item = GianttItem {
                id: item_id.clone(),
                title: extract_string(&item_state.fields, "title").unwrap_or_default(),
                status: GianttStatus::from_str(
                    &extract_string(&item_state.fields, "status").unwrap_or_default(),
                ),
                priority: GianttPriority::from_str(
                    &extract_string(&item_state.fields, "priority").unwrap_or_default(),
                ),
                duration: extract_string(&item_state.fields, "duration"),
                comment: extract_string(&item_state.fields, "comment"),
                tags: extract_string_set(&item_state.sets, "tags"),
                charts: extract_string_set(&item_state.sets, "charts"),
                requires: extract_string_set(&item_state.sets, "requires"),
                anyof: extract_string_set(&item_state.sets, "anyof"),
                blocks: extract_string_set(&item_state.sets, "blocks"),
                time_constraints: extract_string_set(&item_state.sets, "timeConstraints")
                    .into_iter()
                    .collect(),
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

/// Find a cycle in the dependency graph, if one exists
fn find_cycle(items: &HashMap<String, GianttItem>) -> Option<Vec<String>> {
    let mut visited = HashSet::new();
    let mut path = Vec::new();
    let mut path_set = HashSet::new();

    for item_id in items.keys() {
        if !visited.contains(item_id) {
            if let Some(cycle) = dfs_find_cycle(item_id, items, &mut visited, &mut path, &mut path_set) {
                return Some(cycle);
            }
        }
    }

    None
}

fn dfs_find_cycle(
    item_id: &str,
    items: &HashMap<String, GianttItem>,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
    path_set: &mut HashSet<String>,
) -> Option<Vec<String>> {
    visited.insert(item_id.to_string());
    path.push(item_id.to_string());
    path_set.insert(item_id.to_string());

    if let Some(item) = items.get(item_id) {
        for dep_id in &item.requires {
            if path_set.contains(dep_id) {
                // Found a cycle - extract the cycle portion
                let cycle_start = path.iter().position(|x| x == dep_id).unwrap();
                let mut cycle: Vec<String> = path[cycle_start..].to_vec();
                cycle.push(dep_id.clone());
                return Some(cycle);
            }

            if !visited.contains(dep_id) {
                if let Some(cycle) = dfs_find_cycle(dep_id, items, visited, path, path_set) {
                    return Some(cycle);
                }
            }
        }
    }

    path.pop();
    path_set.remove(item_id);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_giantt_status_parsing() {
        assert_eq!(GianttStatus::from_str("TODO"), GianttStatus::Todo);
        assert_eq!(GianttStatus::from_str("IN_PROGRESS"), GianttStatus::InProgress);
        assert_eq!(GianttStatus::from_str("COMPLETED"), GianttStatus::Completed);
        assert_eq!(GianttStatus::from_str("unknown"), GianttStatus::Todo);
    }

    #[test]
    fn test_cycle_detection() {
        let mut items = HashMap::new();

        items.insert("a".to_string(), GianttItem {
            id: "a".to_string(),
            requires: HashSet::from(["b".to_string()]),
            ..Default::default()
        });

        items.insert("b".to_string(), GianttItem {
            id: "b".to_string(),
            requires: HashSet::from(["c".to_string()]),
            ..Default::default()
        });

        items.insert("c".to_string(), GianttItem {
            id: "c".to_string(),
            requires: HashSet::from(["a".to_string()]),
            ..Default::default()
        });

        let cycle = find_cycle(&items);
        assert!(cycle.is_some());
        let cycle = cycle.unwrap();
        assert!(cycle.len() >= 3);
    }

    #[test]
    fn test_no_cycle() {
        let mut items = HashMap::new();

        items.insert("a".to_string(), GianttItem {
            id: "a".to_string(),
            requires: HashSet::from(["b".to_string()]),
            ..Default::default()
        });

        items.insert("b".to_string(), GianttItem {
            id: "b".to_string(),
            requires: HashSet::from(["c".to_string()]),
            ..Default::default()
        });

        items.insert("c".to_string(), GianttItem {
            id: "c".to_string(),
            requires: HashSet::new(),
            ..Default::default()
        });

        let cycle = find_cycle(&items);
        assert!(cycle.is_none());
    }
}
