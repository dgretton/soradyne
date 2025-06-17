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
    try {
      _safeTopologicalSort();
    } catch (e) {
      // Re-throw cycle exceptions, convert others to GraphException
      if (e is CycleDetectedException) rethrow;
      throw GraphException('Graph validation failed: $e');
    }
  }

  /// Performs a safe topological sort that detects cycles and provides detailed error information
  List<GianttItem> _safeTopologicalSort() {
    // Build adjacency list for strict relations
    final adjList = <String, Set<String>>{};
    for (final item in _items.values) {
      adjList[item.id] = <String>{};
    }

    for (final item in _items.values) {
      for (final relType in ['REQUIRES', 'ANYOF']) {
        final targets = item.relations[relType] ?? [];
        for (final target in targets) {
          if (_items.containsKey(target)) {
            adjList[item.id]!.add(target);
          }
        }
      }
    }

    // Calculate in-degrees
    final inDegree = <String, int>{};
    for (final node in adjList.keys) {
      inDegree[node] = 0;
    }
    for (final node in adjList.keys) {
      for (final neighbor in adjList[node]!) {
        inDegree[neighbor] = (inDegree[neighbor] ?? 0) + 1;
      }
    }

    // Find nodes with no dependencies
    final queue = <String>[];
    for (final entry in inDegree.entries) {
      if (entry.value == 0) {
        queue.add(entry.key);
      }
    }

    final sortedItems = <GianttItem>[];
    final visited = <String>{};

    while (queue.isNotEmpty) {
      final node = queue.removeAt(0);
      final item = _items[node];
      if (item != null) {
        sortedItems.add(item);
        visited.add(node);
      }

      for (final neighbor in adjList[node]!) {
        inDegree[neighbor] = inDegree[neighbor]! - 1;
        if (inDegree[neighbor] == 0) {
          queue.add(neighbor);
        }
      }
    }

    // If we haven't visited all nodes, there must be a cycle
    if (sortedItems.length != _items.length) {
      final cycle = _findCycle(adjList, visited);
      throw CycleDetectedException(cycle);
    }

    return sortedItems;
  }

  /// Find a cycle in the graph for detailed error reporting
  List<String> _findCycle(Map<String, Set<String>> adjList, Set<String> visited) {
    final unvisited = _items.keys.toSet().difference(visited);
    final stack = <String>[];
    final path = <String>[];

    bool dfs(String current) {
      if (stack.contains(current)) {
        final cycleStart = stack.indexOf(current);
        final cycle = stack.sublist(cycleStart);
        cycle.add(current); // Add one more occurrence to show complete cycle
        return true;
      }
      if (visited.contains(current)) {
        return false;
      }

      stack.add(current);
      for (final neighbor in adjList[current] ?? <String>{}) {
        if (dfs(neighbor)) {
          return true;
        }
      }
      stack.remove(current);
      return false;
    }

    // Start DFS from any unvisited node
    if (unvisited.isNotEmpty) {
      final startNode = unvisited.first;
      dfs(startNode);
    }

    return stack.isNotEmpty ? stack : ['unknown_cycle'];
  }

  /// Performs a deterministic topological sort of the graph
  List<GianttItem> topologicalSort() {
    // First get basic topological sort
    final sortedItems = _safeTopologicalSort();

    // Sort within each "level" by deterministic criteria
    sortedItems.sort((a, b) {
      // Primary sort by dependency depth
      final depthA = _getDependencyDepth(a);
      final depthB = _getDependencyDepth(b);
      final depthComparison = depthA.compareTo(depthB);
      if (depthComparison != 0) return depthComparison;

      // Secondary sort by ID (deterministic tie-breaker)
      return a.id.compareTo(b.id);
    });

    return sortedItems;
  }

  /// Get the maximum dependency depth of an item
  int _getDependencyDepth(GianttItem item) {
    final visited = <String>{};
    
    int calculateDepth(String itemId) {
      if (visited.contains(itemId)) {
        return 0; // Avoid infinite recursion
      }
      visited.add(itemId);
      
      final currentItem = _items[itemId];
      if (currentItem == null) return 0;
      
      final requires = currentItem.relations['REQUIRES'] ?? [];
      if (requires.isEmpty) return 0;
      
      int maxDepth = 0;
      for (final depId in requires) {
        if (_items.containsKey(depId)) {
          final depDepth = calculateDepth(depId);
          maxDepth = maxDepth > depDepth ? maxDepth : depDepth;
        }
      }
      return maxDepth + 1;
    }
    
    return calculateDepth(item.id);
  }

  /// Insert a new item between two existing items in the dependency chain
  void insertBetween(GianttItem newItem, String beforeId, String afterId) {
    if (!_items.containsKey(beforeId) || !_items.containsKey(afterId)) {
      throw ArgumentError('Both before and after items must exist');
    }

    final beforeItem = _items[beforeId]!;
    final afterItem = _items[afterId]!;

    // Create new item with appropriate relations
    final updatedNewItem = newItem.copyWith(
      relations: {
        'REQUIRES': [beforeId],
        'BLOCKS': [afterId],
      },
    );

    // Update existing items' relations
    final updatedBeforeRelations = Map<String, List<String>>.from(beforeItem.relations);
    if (updatedBeforeRelations.containsKey('BLOCKS')) {
      updatedBeforeRelations['BLOCKS']!.remove(afterId);
      updatedBeforeRelations['BLOCKS']!.add(newItem.id);
    } else {
      updatedBeforeRelations['BLOCKS'] = [newItem.id];
    }

    final updatedAfterRelations = Map<String, List<String>>.from(afterItem.relations);
    if (updatedAfterRelations.containsKey('REQUIRES')) {
      updatedAfterRelations['REQUIRES']!.remove(beforeId);
      updatedAfterRelations['REQUIRES']!.add(newItem.id);
    } else {
      updatedAfterRelations['REQUIRES'] = [newItem.id];
    }

    // Update items in graph
    _items[beforeId] = beforeItem.copyWith(relations: updatedBeforeRelations);
    _items[afterId] = afterItem.copyWith(relations: updatedAfterRelations);
    _items[newItem.id] = updatedNewItem;

    // Validate no cycles were created
    _validateNoCycles();
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
