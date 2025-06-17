import '../models/giantt_item.dart';
import '../graph/giantt_graph.dart';

/// Types of issues that can be detected in the graph
enum IssueType {
  danglingReference('dangling_reference'),
  orphanedItem('orphaned_item'),
  incompleteChain('incomplete_chain'),
  chartInconsistency('chart_inconsistency'),
  tagInconsistency('tag_inconsistency');

  const IssueType(this.value);
  final String value;

  static IssueType fromString(String value) {
    for (final type in IssueType.values) {
      if (type.value == value) return type;
    }
    throw ArgumentError('Invalid issue type: $value');
  }
}

/// Represents a specific issue found in the graph
class Issue {
  const Issue({
    required this.type,
    required this.itemId,
    required this.message,
    required this.relatedIds,
    this.suggestedFix,
  });

  final IssueType type;
  final String itemId;
  final String message;
  final List<String> relatedIds;
  final String? suggestedFix;
}

/// Graph health checker with auto-fix capabilities
class GraphDoctor {
  GraphDoctor(this.graph);

  final GianttGraph graph;
  final List<Issue> _issues = [];
  final List<Issue> _fixedIssues = [];

  /// Run a quick check and return number of issues found
  int quickCheck() {
    _issues.clear();
    _checkReferences();
    return _issues.length;
  }

  /// Run all checks and return detailed issues
  List<Issue> fullDiagnosis() {
    _issues.clear();
    _checkReferences();
    _checkChains();
    // Note: Orphan, chart, and tag checks commented out as they may not be actual issues
    return List.unmodifiable(_issues);
  }

  /// Get all issues of a specific type
  List<Issue> getIssuesByType(IssueType issueType) {
    return _issues.where((issue) => issue.type == issueType).toList();
  }

  /// Fix issues of a specific type or for a specific item
  List<Issue> fixIssues({IssueType? issueType, String? itemId}) {
    // Filter issues to fix
    var issuesToFix = _issues.toList();
    if (issueType != null) {
      issuesToFix = issuesToFix.where((issue) => issue.type == issueType).toList();
    }
    if (itemId != null) {
      issuesToFix = issuesToFix.where((issue) => issue.itemId == itemId).toList();
    }

    final fixed = <Issue>[];
    for (final issue in issuesToFix) {
      if (_fixIssue(issue)) {
        fixed.add(issue);
      }
    }

    // Remove fixed issues from the issues list
    for (final issue in fixed) {
      _issues.remove(issue);
    }

    _fixedIssues.addAll(fixed);
    return fixed;
  }

  /// Fix a specific issue. Returns true if fixed, false otherwise
  bool _fixIssue(Issue issue) {
    switch (issue.type) {
      case IssueType.danglingReference:
        return _fixDanglingReference(issue);
      case IssueType.incompleteChain:
        return _fixIncompleteChain(issue);
      case IssueType.orphanedItem:
      case IssueType.chartInconsistency:
      case IssueType.tagInconsistency:
        // These issues typically require manual intervention
        return false;
    }
  }

  /// Fix a dangling reference issue
  bool _fixDanglingReference(Issue issue) {
    final item = graph.items[issue.itemId];
    if (item == null) return false;

    // Find the relation type and target from the message
    String? relType;
    String? target;

    // Extract relation type from message
    for (final type in ['REQUIRES', 'BLOCKS', 'ANYOF', 'SUFFICIENT', 'SUPERCHARGES', 'INDICATES', 'TOGETHER', 'CONFLICTS']) {
      if (issue.message.toLowerCase().contains(type.toLowerCase())) {
        relType = type;
        break;
      }
    }

    if (relType == null) return false;

    // Extract target ID from message
    final match = RegExp(r"non-existent item '([^']+)'").firstMatch(issue.message);
    if (match == null) return false;
    target = match.group(1);

    // Remove the dangling reference
    final relations = Map<String, List<String>>.from(item.relations);
    if (relations.containsKey(relType) && relations[relType]!.contains(target)) {
      relations[relType]!.remove(target);
      if (relations[relType]!.isEmpty) {
        relations.remove(relType);
      }

      // Update the item
      final updatedItem = item.copyWith(relations: relations);
      graph.addItem(updatedItem);
      return true;
    }

    return false;
  }

  /// Fix an incomplete chain issue
  bool _fixIncompleteChain(Issue issue) {
    if (issue.relatedIds.isEmpty || issue.suggestedFix == null) {
      return false;
    }

    final item = graph.items[issue.itemId];
    final relatedItem = graph.items[issue.relatedIds.first];
    if (item == null || relatedItem == null) return false;

    // Parse the suggested fix: "giantt modify <target> --add <relation> <source>"
    final fixParts = issue.suggestedFix!.split(' ');
    if (fixParts.length < 6) return false;

    final targetId = fixParts[2];
    final relType = fixParts[4].toUpperCase();
    final sourceId = fixParts[5];

    final targetItem = graph.items[targetId];
    if (targetItem == null) return false;

    // Add the relation
    final relations = Map<String, List<String>>.from(targetItem.relations);
    relations.putIfAbsent(relType, () => []);
    
    if (!relations[relType]!.contains(sourceId)) {
      relations[relType]!.add(sourceId);
      
      // Update the item
      final updatedItem = targetItem.copyWith(relations: relations);
      graph.addItem(updatedItem);
      return true;
    }

    return false;
  }

  /// Check for dangling references in relations
  void _checkReferences() {
    for (final item in graph.items.values) {
      for (final entry in item.relations.entries) {
        final relType = entry.key;
        final targets = entry.value;
        
        for (final target in targets) {
          if (!graph.items.containsKey(target)) {
            _issues.add(Issue(
              type: IssueType.danglingReference,
              itemId: item.id,
              message: "References non-existent item '$target' in ${relType.toLowerCase()} relation",
              relatedIds: [target],
              suggestedFix: "Remove reference to '$target' from ${relType.toLowerCase()} relation",
            ));
          }
        }
      }
    }
  }

  /// Check for incomplete dependency chains
  void _checkChains() {
    final blocksMap = <String, Set<String>>{};
    final requiresMap = <String, Set<String>>{};
    final sufficientMap = <String, Set<String>>{};
    final anyofMap = <String, Set<String>>{};

    // Build relation maps
    for (final item in graph.items.values) {
      blocksMap[item.id] = Set<String>.from(item.relations['BLOCKS'] ?? []);
      requiresMap[item.id] = Set<String>.from(item.relations['REQUIRES'] ?? []);
      sufficientMap[item.id] = Set<String>.from(item.relations['SUFFICIENT'] ?? []);
      anyofMap[item.id] = Set<String>.from(item.relations['ANYOF'] ?? []);
    }

    // Check for items that block something but aren't required by it
    for (final entry in blocksMap.entries) {
      final itemId = entry.key;
      final blocksItems = entry.value;
      
      for (final blocked in blocksItems) {
        if (graph.items.containsKey(blocked)) {
          final blockedRequires = requiresMap[blocked] ?? <String>{};
          if (!blockedRequires.contains(itemId)) {
            _issues.add(Issue(
              type: IssueType.incompleteChain,
              itemId: itemId,
              message: "Item blocks '$blocked' but isn't required by it",
              relatedIds: [blocked],
              suggestedFix: "giantt modify $blocked --add requires $itemId",
            ));
          }
        }
      }
    }

    // Check for items that require something but aren't blocked by it
    for (final entry in requiresMap.entries) {
      final itemId = entry.key;
      final requiresItems = entry.value;
      
      for (final required in requiresItems) {
        if (graph.items.containsKey(required)) {
          final requiredBlocks = blocksMap[required] ?? <String>{};
          if (!requiredBlocks.contains(itemId)) {
            _issues.add(Issue(
              type: IssueType.incompleteChain,
              itemId: itemId,
              message: "Item requires '$required' but isn't blocked by it",
              relatedIds: [required],
              suggestedFix: "giantt modify $required --add blocks $itemId",
            ));
          }
        }
      }
    }

    // Check for items that are sufficient for something but aren't in an anyof relation
    for (final entry in sufficientMap.entries) {
      final itemId = entry.key;
      final sufficientItems = entry.value;
      
      for (final sufficient in sufficientItems) {
        if (graph.items.containsKey(sufficient)) {
          final sufficientAnyof = anyofMap[sufficient] ?? <String>{};
          if (!sufficientAnyof.contains(itemId)) {
            _issues.add(Issue(
              type: IssueType.incompleteChain,
              itemId: itemId,
              message: "Item is sufficient for '$sufficient' but doesn't have anyof relation with it",
              relatedIds: [sufficient],
              suggestedFix: "giantt modify $sufficient --add anyof $itemId",
            ));
          }
        }
      }
    }

    // Check for items that have anyof relation but aren't sufficient
    for (final entry in anyofMap.entries) {
      final itemId = entry.key;
      final anyofItems = entry.value;
      
      for (final anyofItem in anyofItems) {
        if (graph.items.containsKey(anyofItem)) {
          final anyofSufficient = sufficientMap[anyofItem] ?? <String>{};
          if (!anyofSufficient.contains(itemId)) {
            _issues.add(Issue(
              type: IssueType.incompleteChain,
              itemId: itemId,
              message: "Item has anyof relation with '$anyofItem' but isn't sufficient for it",
              relatedIds: [anyofItem],
              suggestedFix: "giantt modify $anyofItem --add sufficient $itemId",
            ));
          }
        }
      }
    }
  }

  /// Get all fixed issues
  List<Issue> get fixedIssues => List.unmodifiable(_fixedIssues);

  /// Get current issues
  List<Issue> get issues => List.unmodifiable(_issues);
}
