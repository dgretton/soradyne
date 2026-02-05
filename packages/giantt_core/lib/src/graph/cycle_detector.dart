import '../models/giantt_item.dart';
import '../models/graph_exceptions.dart';

/// Result of cycle detection analysis
class CycleDetectionResult {
  const CycleDetectionResult({
    required this.hasCycle,
    this.cyclePath = const [],
    this.cycleDescription,
  });

  /// Whether a cycle was detected
  final bool hasCycle;

  /// The path of item IDs forming the cycle (empty if no cycle)
  final List<String> cyclePath;

  /// Human-readable description of the cycle
  final String? cycleDescription;

  /// Create a result indicating no cycle was found
  factory CycleDetectionResult.noCycle() {
    return const CycleDetectionResult(hasCycle: false);
  }

  /// Create a result indicating a cycle was found
  factory CycleDetectionResult.cycleFound(List<String> path) {
    final pathStr = path.join(' -> ');
    return CycleDetectionResult(
      hasCycle: true,
      cyclePath: path,
      cycleDescription: 'Cycle detected: $pathStr',
    );
  }

  @override
  String toString() {
    if (!hasCycle) return 'No cycle detected';
    return cycleDescription ?? 'Cycle detected: ${cyclePath.join(' -> ')}';
  }
}

/// Detects cycles in dependency graphs
///
/// This class provides methods to detect cycles in directed graphs,
/// particularly useful for dependency tracking in Giantt items.
class CycleDetector {
  /// Detect cycles in a graph represented by items
  ///
  /// [items] - Map of item ID to GianttItem
  /// [relationTypes] - Which relation types to consider as dependencies
  ///                   (defaults to REQUIRES and ANYOF)
  ///
  /// Returns a [CycleDetectionResult] with details about any cycle found
  static CycleDetectionResult detectCycle(
    Map<String, GianttItem> items, {
    List<String> relationTypes = const ['REQUIRES', 'ANYOF'],
  }) {
    // Build adjacency list from items
    final adjList = buildAdjacencyList(items, relationTypes: relationTypes);
    return detectCycleInGraph(adjList);
  }

  /// Build an adjacency list from items and their relations
  static Map<String, Set<String>> buildAdjacencyList(
    Map<String, GianttItem> items, {
    List<String> relationTypes = const ['REQUIRES', 'ANYOF'],
  }) {
    final adjList = <String, Set<String>>{};

    // Initialize all nodes
    for (final item in items.values) {
      adjList[item.id] = <String>{};
    }

    // Add edges based on specified relation types
    for (final item in items.values) {
      for (final relType in relationTypes) {
        final targets = item.relations[relType] ?? [];
        for (final target in targets) {
          // Only add edge if target exists in items
          if (items.containsKey(target)) {
            adjList[item.id]!.add(target);
          }
        }
      }
    }

    return adjList;
  }

  /// Detect cycles in a graph represented as an adjacency list
  ///
  /// Uses depth-first search with three-color marking:
  /// - White (unvisited): not yet processed
  /// - Gray (in stack): currently being processed
  /// - Black (done): fully processed
  static CycleDetectionResult detectCycleInGraph(Map<String, Set<String>> adjList) {
    final white = adjList.keys.toSet(); // Unvisited
    final gray = <String>{}; // In current DFS path
    final black = <String>{}; // Fully processed

    // Track the current path for cycle reconstruction
    final path = <String>[];

    /// DFS that returns the cycle path if found, null otherwise
    List<String>? dfs(String node) {
      // Move from white to gray
      white.remove(node);
      gray.add(node);
      path.add(node);

      for (final neighbor in adjList[node] ?? <String>{}) {
        if (gray.contains(neighbor)) {
          // Found a back edge - cycle detected!
          // Build the cycle path from where we first saw this node
          final cycleStart = path.indexOf(neighbor);
          final cyclePath = path.sublist(cycleStart);
          cyclePath.add(neighbor); // Complete the cycle
          return cyclePath;
        }

        if (white.contains(neighbor)) {
          final result = dfs(neighbor);
          if (result != null) return result;
        }
        // If black, skip - already fully processed
      }

      // Move from gray to black
      gray.remove(node);
      black.add(node);
      path.removeLast();

      return null;
    }

    // Run DFS from each unvisited node
    while (white.isNotEmpty) {
      final startNode = white.first;
      final cyclePath = dfs(startNode);
      if (cyclePath != null) {
        return CycleDetectionResult.cycleFound(cyclePath);
      }
    }

    return CycleDetectionResult.noCycle();
  }

  /// Check if adding a specific edge would create a cycle
  ///
  /// [adjList] - Current adjacency list
  /// [from] - Source node of the new edge
  /// [to] - Target node of the new edge
  ///
  /// Returns true if adding the edge would create a cycle
  static bool wouldCreateCycle(
    Map<String, Set<String>> adjList,
    String from,
    String to,
  ) {
    // Adding from -> to creates a cycle if there's already a path from to -> from
    return hasPath(adjList, to, from);
  }

  /// Check if there's a path from source to target in the graph
  static bool hasPath(
    Map<String, Set<String>> adjList,
    String source,
    String target,
  ) {
    if (source == target) return true;

    final visited = <String>{};
    final queue = <String>[source];

    while (queue.isNotEmpty) {
      final current = queue.removeAt(0);
      if (current == target) return true;

      if (visited.contains(current)) continue;
      visited.add(current);

      for (final neighbor in adjList[current] ?? <String>{}) {
        if (!visited.contains(neighbor)) {
          queue.add(neighbor);
        }
      }
    }

    return false;
  }

  /// Find all cycles in the graph (not just the first one)
  ///
  /// Note: This can be expensive for large graphs with many cycles.
  /// Use [detectCycle] for simple cycle existence checking.
  static List<List<String>> findAllCycles(Map<String, Set<String>> adjList) {
    final cycles = <List<String>>[];
    final visited = <String>{};

    void dfs(String node, List<String> path, Set<String> pathSet) {
      if (pathSet.contains(node)) {
        // Found a cycle
        final cycleStart = path.indexOf(node);
        final cycle = path.sublist(cycleStart);
        cycle.add(node);
        cycles.add(cycle);
        return;
      }

      if (visited.contains(node)) return;

      path.add(node);
      pathSet.add(node);

      for (final neighbor in adjList[node] ?? <String>{}) {
        dfs(neighbor, path, pathSet);
      }

      path.removeLast();
      pathSet.remove(node);
      visited.add(node);
    }

    for (final node in adjList.keys) {
      if (!visited.contains(node)) {
        dfs(node, [], {});
      }
    }

    return cycles;
  }

  /// Get detailed information about a cycle including item titles
  static String formatCycleWithTitles(
    List<String> cyclePath,
    Map<String, GianttItem> items,
  ) {
    final parts = cyclePath.map((id) {
      final item = items[id];
      if (item != null) {
        return '$id ("${item.title}")';
      }
      return id;
    }).toList();

    return parts.join(' -> ');
  }

  /// Validate that a graph has no cycles, throwing if one is found
  ///
  /// This is a convenience method that throws [CycleDetectedException]
  /// if a cycle is detected.
  static void validateNoCycles(
    Map<String, GianttItem> items, {
    List<String> relationTypes = const ['REQUIRES', 'ANYOF'],
  }) {
    final result = detectCycle(items, relationTypes: relationTypes);
    if (result.hasCycle) {
      throw CycleDetectedException(result.cyclePath);
    }
  }
}
