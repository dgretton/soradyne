import '../models/giantt_item.dart';
import '../models/relation.dart';
import '../models/graph_exceptions.dart';

/// Main graph class for managing Giantt items and their relations
class GianttGraph {
  final Map<String, GianttItem> _items = {};

  /// Get all items in the graph
  Map<String, GianttItem> get items => Map.unmodifiable(_items);

  /// Add an item to the graph
  void addItem(GianttItem item) {
    _items[item.id] = item;
  }

  /// Remove an item from the graph
  void removeItem(String itemId) {
    _items.remove(itemId);
  }

  /// Find an item by substring in ID or title
  GianttItem findBySubstring(String substring) {
    final matches = _items.values.where((item) => 
        item.id == substring || 
        item.title.toLowerCase().contains(substring.toLowerCase())
    ).toList();
    
    if (matches.isEmpty) {
      throw ArgumentError('No items with ID "$substring" or title containing "$substring" found');
    }
    if (matches.length > 1) {
      final ids = matches.map((item) => item.id).join(', ');
      throw ArgumentError('Multiple matches found: $ids');
    }
    return matches.first;
  }

  /// Add a relation between two items with automatic bidirectional creation
  void addRelation(String fromId, RelationType relationType, String toId) {
    if (!_items.containsKey(fromId)) {
      throw ArgumentError('Item "$fromId" not found');
    }
    if (!_items.containsKey(toId)) {
      throw ArgumentError('Item "$toId" not found');
    }

    final fromItem = _items[fromId]!;
    final toItem = _items[toId]!;

    // Add the primary relation
    final updatedFromRelations = Map<String, List<String>>.from(fromItem.relations);
    updatedFromRelations.putIfAbsent(relationType.name, () => []);
    if (!updatedFromRelations[relationType.name]!.contains(toId)) {
      updatedFromRelations[relationType.name]!.add(toId);
    }

    // Create the automatic bidirectional relation
    final bidirectionalType = _getBidirectionalRelation(relationType);
    final updatedToRelations = Map<String, List<String>>.from(toItem.relations);
    if (bidirectionalType != null) {
      updatedToRelations.putIfAbsent(bidirectionalType.name, () => []);
      if (!updatedToRelations[bidirectionalType.name]!.contains(fromId)) {
        updatedToRelations[bidirectionalType.name]!.add(fromId);
      }
    }

    // Update items with new relations
    _items[fromId] = fromItem.copyWith(relations: updatedFromRelations);
    _items[toId] = toItem.copyWith(relations: updatedToRelations);

    // Check for cycles after adding the relation
    _validateNoCycles();
  }

  /// Remove a relation between two items and its bidirectional counterpart
  void removeRelation(String fromId, RelationType relationType, String toId) {
    if (!_items.containsKey(fromId) || !_items.containsKey(toId)) {
      return; // Items don't exist, nothing to remove
    }

    final fromItem = _items[fromId]!;
    final toItem = _items[toId]!;

    // Remove the primary relation
    final updatedFromRelations = Map<String, List<String>>.from(fromItem.relations);
    if (updatedFromRelations.containsKey(relationType.name)) {
      updatedFromRelations[relationType.name]!.remove(toId);
      if (updatedFromRelations[relationType.name]!.isEmpty) {
        updatedFromRelations.remove(relationType.name);
      }
    }

    // Remove the automatic bidirectional relation
    final bidirectionalType = _getBidirectionalRelation(relationType);
    final updatedToRelations = Map<String, List<String>>.from(toItem.relations);
    if (bidirectionalType != null && updatedToRelations.containsKey(bidirectionalType.name)) {
      updatedToRelations[bidirectionalType.name]!.remove(fromId);
      if (updatedToRelations[bidirectionalType.name]!.isEmpty) {
        updatedToRelations.remove(bidirectionalType.name);
      }
    }

    // Update items with modified relations
    _items[fromId] = fromItem.copyWith(relations: updatedFromRelations);
    _items[toId] = toItem.copyWith(relations: updatedToRelations);
  }

  /// Get the bidirectional relation type for automatic creation
  RelationType? _getBidirectionalRelation(RelationType relationType) {
    switch (relationType) {
      case RelationType.requires:
        return RelationType.blocks;
      case RelationType.blocks:
        return RelationType.requires;
      case RelationType.anyof:
        return RelationType.sufficient;
      case RelationType.sufficient:
        return RelationType.anyof;
      case RelationType.supercharges:
      case RelationType.indicates:
      case RelationType.together:
      case RelationType.conflicts:
        return relationType; // These are symmetric relations
    }
  }

  /// Validate that the graph has no cycles in strict dependencies
  void _validateNoCycles() {
    final visited = <String>{};
    final recursionStack = <String>{};

    bool hasCycleDfs(String itemId, List<String> path) {
      if (recursionStack.contains(itemId)) {
        // Found a cycle, create the cycle path
        final cycleStart = path.indexOf(itemId);
        final cyclePath = path.sublist(cycleStart)..add(itemId);
        throw CycleDetectedException(cyclePath);
      }
      if (visited.contains(itemId)) {
        return false;
      }

      visited.add(itemId);
      recursionStack.add(itemId);
      path.add(itemId);

      final item = _items[itemId];
      if (item != null) {
        // Only check strict dependencies (REQUIRES and ANYOF)
        for (final relationType in ['REQUIRES', 'ANYOF']) {
          final targets = item.relations[relationType] ?? [];
          for (final target in targets) {
            if (_items.containsKey(target)) {
              if (hasCycleDfs(target, List.from(path))) {
                return true;
              }
            }
          }
        }
      }

      recursionStack.remove(itemId);
      path.removeLast();
      return false;
    }

    for (final itemId in _items.keys) {
      if (!visited.contains(itemId)) {
        hasCycleDfs(itemId, []);
      }
    }
  }

  /// Get items that are not occluded
  Map<String, GianttItem> get includedItems {
    return Map.fromEntries(
      _items.entries.where((entry) => !entry.value.occlude)
    );
  }

  /// Get items that are occluded
  Map<String, GianttItem> get occludedItems {
    return Map.fromEntries(
      _items.entries.where((entry) => entry.value.occlude)
    );
  }

  /// Create a copy of this graph
  GianttGraph copy() {
    final newGraph = GianttGraph();
    for (final item in _items.values) {
      newGraph.addItem(item.copyWith());
    }
    return newGraph;
  }

  /// Merge another graph into this one
  GianttGraph operator +(GianttGraph other) {
    final newGraph = copy();
    for (final item in other._items.values) {
      newGraph.addItem(item.copyWith());
    }
    return newGraph;
  }
}
