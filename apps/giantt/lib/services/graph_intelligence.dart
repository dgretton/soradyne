import 'package:giantt_core/giantt_core.dart';

/// A scored item for "Available Now".
class ScoredItem {
  final GianttItem item;
  final int blockingCount; // how many items transitively depend on this one
  final bool chainOverdue;  // true if this item sits in a deadline chain that's impossible to meet

  const ScoredItem({
    required this.item,
    required this.blockingCount,
    required this.chainOverdue,
  });
}

class GraphIntelligence {
  /// Returns up to [limit] items that are available to work on right now,
  /// ordered by:
  ///   1. Chain-overdue items first (deadline chain impossible to meet)
  ///   2. Blocking count descending (critical path)
  ///   3. Priority descending
  ///
  /// An item is "available" if its status is NOT_STARTED or BLOCKED
  /// (not completed) and every item in its REQUIRES set is COMPLETED.
  /// Occluded items are treated identically to non-occluded ones.
  static List<ScoredItem> availableNow(GianttGraph graph, {int limit = 7}) {
    final items = graph.items;
    if (items.isEmpty) return [];

    // Precompute blocking counts (number of items that transitively depend on X).
    final blockingCounts = _computeBlockingCounts(items);

    // Precompute which items are on a chain-overdue path.
    final overdueSet = _computeChainOverdue(items);

    final available = <ScoredItem>[];
    for (final item in items.values) {
      if (item.status == GianttStatus.completed) continue;
      if (!_isUnblocked(item, items)) continue;

      available.add(ScoredItem(
        item: item,
        blockingCount: blockingCounts[item.id] ?? 0,
        chainOverdue: overdueSet.contains(item.id),
      ));
    }

    available.sort((a, b) {
      // Chain-overdue items first.
      if (a.chainOverdue != b.chainOverdue) {
        return a.chainOverdue ? -1 : 1;
      }
      // Then by blocking count descending.
      final bc = b.blockingCount.compareTo(a.blockingCount);
      if (bc != 0) return bc;
      // Then by priority descending.
      return b.item.priority.index.compareTo(a.item.priority.index);
    });

    return available.take(limit).toList();
  }

  static List<GianttItem> inProgress(GianttGraph graph) {
    return graph.items.values
        .where((i) => i.status == GianttStatus.inProgress)
        .toList()
      ..sort((a, b) => b.priority.index.compareTo(a.priority.index));
  }

  // ── Blocking count ────────────────────────────────────────────────────────

  /// For each item X, count how many items transitively require X.
  static Map<String, int> _computeBlockingCounts(Map<String, GianttItem> items) {
    // Build reverse map: X → set of items that directly require X.
    final dependents = <String, Set<String>>{};
    for (final item in items.values) {
      final reqs = item.relations['REQUIRES'] ?? [];
      for (final req in reqs) {
        dependents.putIfAbsent(req, () => {}).add(item.id);
      }
    }

    final memo = <String, int>{};
    int count(String id, Set<String> visiting) {
      if (memo.containsKey(id)) return memo[id]!;
      if (visiting.contains(id)) return 0; // cycle guard
      visiting.add(id);
      final direct = dependents[id] ?? {};
      var total = direct.length;
      for (final dep in direct) {
        total += count(dep, visiting);
      }
      visiting.remove(id);
      memo[id] = total;
      return total;
    }

    for (final id in items.keys) {
      count(id, {});
    }
    return memo;
  }

  // ── Chain-overdue detection ───────────────────────────────────────────────

  /// An item is "chain-overdue" if any deadline in its upstream dependency
  /// chain (items it transitively requires) is already past, OR if the total
  /// remaining estimated duration through the chain exceeds the time left
  /// to the nearest deadline.
  static Set<String> _computeChainOverdue(Map<String, GianttItem> items) {
    final overdue = <String>{};
    final now = DateTime.now();

    for (final item in items.values) {
      if (item.status == GianttStatus.completed) continue;

      // Collect all items in this item's transitive require chain + itself.
      final chain = _transitiveDeps(item.id, items, {});

      // Find the tightest deadline in the chain.
      DateTime? tightest;
      for (final id in chain) {
        final dep = items[id];
        if (dep == null) continue;
        for (final tc in dep.timeConstraints) {
          if (tc.dueDate == null) continue;
          try {
            final due = DateTime.parse(tc.dueDate!);
            if (tightest == null || due.isBefore(tightest)) tightest = due;
          } catch (_) {}
        }
      }
      if (tightest == null) continue;

      // Already past the deadline.
      if (tightest.isBefore(now)) {
        overdue.add(item.id);
        continue;
      }

      // Sum estimated durations of incomplete items in the chain.
      var totalHours = 0.0;
      for (final id in chain) {
        final dep = items[id];
        if (dep == null || dep.status == GianttStatus.completed) continue;
        totalHours += _durationHours(dep.duration);
      }

      final hoursLeft = tightest.difference(now).inMinutes / 60.0;
      if (totalHours > hoursLeft) {
        overdue.add(item.id);
      }
    }

    return overdue;
  }

  static Set<String> _transitiveDeps(
      String id, Map<String, GianttItem> items, Set<String> visited) {
    if (visited.contains(id)) return visited;
    visited.add(id);
    final item = items[id];
    if (item == null) return visited;
    for (final req in item.relations['REQUIRES'] ?? <String>[]) {
      _transitiveDeps(req, items, visited);
    }
    return visited;
  }

  static double _durationHours(GianttDuration d) {
    // GianttDuration.toString() gives e.g. "2d", "3h", "1w". Parse roughly.
    final s = d.toString().trim();
    if (s.isEmpty || s == '0s') return 0;
    try {
      final num = double.parse(s.replaceAll(RegExp(r'[^\d.]'), ''));
      if (s.endsWith('mo')) return num * 30 * 8;
      if (s.endsWith('w')) return num * 5 * 8;
      if (s.endsWith('d')) return num * 8;
      if (s.endsWith('h')) return num;
      if (s.endsWith('min')) return num / 60;
    } catch (_) {}
    return 0;
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  static bool _isUnblocked(GianttItem item, Map<String, GianttItem> all) {
    final reqs = item.relations['REQUIRES'] ?? [];
    return reqs.every((req) => all[req]?.status == GianttStatus.completed);
  }

  /// All charts present in the graph, as a set.
  static Set<String> allCharts(GianttGraph graph) {
    final charts = <String>{};
    for (final item in graph.items.values) {
      charts.addAll(item.charts);
    }
    return charts;
  }

  /// Items grouped by chart (an item may appear under multiple charts).
  static Map<String, List<GianttItem>> byChart(GianttGraph graph) {
    final result = <String, List<GianttItem>>{};
    for (final item in graph.items.values) {
      for (final chart in item.charts) {
        result.putIfAbsent(chart, () => []).add(item);
      }
    }
    return result;
  }
}
