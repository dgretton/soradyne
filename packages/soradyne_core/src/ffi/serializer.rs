//! Giantt Text Serializer
//!
//! Serializes GianttState to .giantt text format, matching Dart's toFileString() output.
//!
//! Format:
//! ```text
//! ○ item_id!! 2d "Title" {chart1,chart2} tag1,tag2 >>> ⊢[dep1,dep2] @@@ due(2024-01-01) # comment
//! ```

use crate::convergent::giantt::{GianttItem, GianttPriority, GianttState, GianttStatus};

/// Serialize a GianttState to .giantt text format
pub fn serialize_giantt_state(state: &GianttState) -> String {
    let mut lines: Vec<String> = state
        .items
        .values()
        .map(serialize_giantt_item)
        .collect();

    // Sort by item ID for deterministic output
    lines.sort();

    lines.join("\n")
}

/// Serialize a single GianttItem to its text representation
pub fn serialize_giantt_item(item: &GianttItem) -> String {
    let mut parts = Vec::new();

    // 1. Status symbol
    let status_symbol = match item.status {
        GianttStatus::Todo => "○",
        GianttStatus::InProgress => "◑",
        GianttStatus::Blocked => "⊘",
        GianttStatus::Completed => "●",
    };
    parts.push(status_symbol.to_string());

    // 2. ID + Priority suffix
    let priority_suffix = match item.priority {
        GianttPriority::Low => "...",
        GianttPriority::Medium => "",
        GianttPriority::High => "!!",
        GianttPriority::Critical => "!!!",
    };
    parts.push(format!("{}{}", item.id, priority_suffix));

    // 3. Duration (if present, otherwise use default "0s")
    let duration = item.duration.as_deref().unwrap_or("0s");
    parts.push(duration.to_string());

    // 4. JSON-encoded title
    let title_json = serde_json::to_string(&item.title).unwrap_or_else(|_| "\"\"".to_string());
    parts.push(title_json);

    // 5. Charts: {chart1,chart2} or {}
    if item.charts.is_empty() {
        parts.push("{}".to_string());
    } else {
        let mut charts: Vec<_> = item.charts.iter().collect();
        charts.sort();
        let charts_str = charts
            .iter()
            .map(|c| format!("\"{}\"", c))
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("{{{}}}", charts_str));
    }

    // Join the first parts with spaces
    let mut result = parts.join(" ");

    // 6. Tags (optional, space-separated from previous)
    if !item.tags.is_empty() {
        let mut tags: Vec<_> = item.tags.iter().map(|s| s.as_str()).collect();
        tags.sort();
        result.push_str(&format!(" {}", tags.join(",")));
    }

    // 7. Relations (optional)
    let relations = serialize_relations(item);
    if !relations.is_empty() {
        result.push_str(&format!(" >>> {}", relations));
    }

    // 8. Time constraints (optional)
    if !item.time_constraints.is_empty() {
        let constraints: Vec<_> = item.time_constraints.iter().map(|s| s.as_str()).collect();
        result.push_str(&format!(" @@@ {}", constraints.join(" ")));
    }

    // 9. Comment (if present)
    if let Some(ref comment) = item.comment {
        if !comment.is_empty() {
            result.push_str(&format!(" # {}", comment));
        }
    }

    result
}

/// Serialize relations for an item
fn serialize_relations(item: &GianttItem) -> String {
    let mut relation_parts = Vec::new();

    // REQUIRES: ⊢
    if !item.requires.is_empty() {
        let mut deps: Vec<_> = item.requires.iter().map(|s| s.as_str()).collect();
        deps.sort();
        relation_parts.push(format!("⊢[{}]", deps.join(",")));
    }

    // ANYOF: ⋲
    if !item.anyof.is_empty() {
        let mut deps: Vec<_> = item.anyof.iter().map(|s| s.as_str()).collect();
        deps.sort();
        relation_parts.push(format!("⋲[{}]", deps.join(",")));
    }

    // BLOCKS: ►
    if !item.blocks.is_empty() {
        let mut deps: Vec<_> = item.blocks.iter().map(|s| s.as_str()).collect();
        deps.sort();
        relation_parts.push(format!("►[{}]", deps.join(",")));
    }

    relation_parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_serialize_simple_item() {
        let item = GianttItem {
            id: "task_1".to_string(),
            title: "My Task".to_string(),
            status: GianttStatus::Todo,
            priority: GianttPriority::Medium,
            duration: Some("2d".to_string()),
            comment: None,
            tags: HashSet::new(),
            charts: HashSet::new(),
            requires: HashSet::new(),
            anyof: HashSet::new(),
            blocks: HashSet::new(),
            time_constraints: Vec::new(),
        };

        let text = serialize_giantt_item(&item);
        assert!(text.contains("○"));
        assert!(text.contains("task_1"));
        assert!(text.contains("2d"));
        assert!(text.contains("\"My Task\""));
        assert!(text.contains("{}"));
    }

    #[test]
    fn test_serialize_item_with_priority() {
        let item = GianttItem {
            id: "urgent".to_string(),
            title: "Urgent Task".to_string(),
            status: GianttStatus::InProgress,
            priority: GianttPriority::Critical,
            duration: Some("1h".to_string()),
            comment: None,
            tags: HashSet::new(),
            charts: HashSet::new(),
            requires: HashSet::new(),
            anyof: HashSet::new(),
            blocks: HashSet::new(),
            time_constraints: Vec::new(),
        };

        let text = serialize_giantt_item(&item);
        assert!(text.contains("◑")); // InProgress
        assert!(text.contains("urgent!!!")); // Critical priority
    }

    #[test]
    fn test_serialize_item_with_tags_and_charts() {
        let mut tags = HashSet::new();
        tags.insert("important".to_string());
        tags.insert("backend".to_string());

        let mut charts = HashSet::new();
        charts.insert("Sprint1".to_string());
        charts.insert("Q4".to_string());

        let item = GianttItem {
            id: "feature".to_string(),
            title: "New Feature".to_string(),
            status: GianttStatus::Todo,
            priority: GianttPriority::High,
            duration: Some("5d".to_string()),
            comment: None,
            tags,
            charts,
            requires: HashSet::new(),
            anyof: HashSet::new(),
            blocks: HashSet::new(),
            time_constraints: Vec::new(),
        };

        let text = serialize_giantt_item(&item);
        assert!(text.contains("feature!!")); // High priority
        assert!(text.contains("\"Q4\"")); // Chart
        assert!(text.contains("\"Sprint1\"")); // Chart
        assert!(text.contains("backend")); // Tag
        assert!(text.contains("important")); // Tag
    }

    #[test]
    fn test_serialize_item_with_relations() {
        let mut requires = HashSet::new();
        requires.insert("dep1".to_string());
        requires.insert("dep2".to_string());

        let mut blocks = HashSet::new();
        blocks.insert("blocked_task".to_string());

        let item = GianttItem {
            id: "main_task".to_string(),
            title: "Main Task".to_string(),
            status: GianttStatus::Todo,
            priority: GianttPriority::Medium,
            duration: Some("3d".to_string()),
            comment: None,
            tags: HashSet::new(),
            charts: HashSet::new(),
            requires,
            anyof: HashSet::new(),
            blocks,
            time_constraints: Vec::new(),
        };

        let text = serialize_giantt_item(&item);
        assert!(text.contains(">>>")); // Relations marker
        assert!(text.contains("⊢[dep1,dep2]") || text.contains("⊢[dep2,dep1]")); // Requires
        assert!(text.contains("►[blocked_task]")); // Blocks
    }

    #[test]
    fn test_serialize_item_with_time_constraints() {
        let item = GianttItem {
            id: "deadline_task".to_string(),
            title: "Has Deadline".to_string(),
            status: GianttStatus::Todo,
            priority: GianttPriority::Medium,
            duration: Some("1d".to_string()),
            comment: None,
            tags: HashSet::new(),
            charts: HashSet::new(),
            requires: HashSet::new(),
            anyof: HashSet::new(),
            blocks: HashSet::new(),
            time_constraints: vec!["due(2024-12-31,warn)".to_string()],
        };

        let text = serialize_giantt_item(&item);
        assert!(text.contains("@@@")); // Time constraints marker
        assert!(text.contains("due(2024-12-31,warn)"));
    }

    #[test]
    fn test_serialize_item_with_comment() {
        let item = GianttItem {
            id: "commented".to_string(),
            title: "Task with Comment".to_string(),
            status: GianttStatus::Completed,
            priority: GianttPriority::Medium,
            duration: Some("1h".to_string()),
            comment: Some("This is a note".to_string()),
            tags: HashSet::new(),
            charts: HashSet::new(),
            requires: HashSet::new(),
            anyof: HashSet::new(),
            blocks: HashSet::new(),
            time_constraints: Vec::new(),
        };

        let text = serialize_giantt_item(&item);
        assert!(text.contains("●")); // Completed
        assert!(text.contains("# This is a note"));
    }

    #[test]
    fn test_serialize_state_multiple_items() {
        use std::collections::HashMap;

        let mut items = HashMap::new();

        items.insert(
            "task_a".to_string(),
            GianttItem {
                id: "task_a".to_string(),
                title: "Task A".to_string(),
                status: GianttStatus::Todo,
                priority: GianttPriority::Medium,
                duration: Some("1d".to_string()),
                ..Default::default()
            },
        );

        items.insert(
            "task_b".to_string(),
            GianttItem {
                id: "task_b".to_string(),
                title: "Task B".to_string(),
                status: GianttStatus::InProgress,
                priority: GianttPriority::High,
                duration: Some("2d".to_string()),
                ..Default::default()
            },
        );

        let state = GianttState { items };
        let text = serialize_giantt_state(&state);

        // Should have both items
        assert!(text.contains("task_a"));
        assert!(text.contains("task_b"));

        // Should be sorted by ID
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("task_a"));
        assert!(lines[1].contains("task_b"));
    }

    #[test]
    fn test_json_escape_title() {
        let item = GianttItem {
            id: "escaped".to_string(),
            title: "Task with \"quotes\" and\nnewline".to_string(),
            status: GianttStatus::Todo,
            priority: GianttPriority::Medium,
            duration: Some("1d".to_string()),
            ..Default::default()
        };

        let text = serialize_giantt_item(&item);
        // JSON encoding should escape quotes and newlines
        assert!(text.contains("\\\"quotes\\\""));
        assert!(text.contains("\\n"));
    }
}
