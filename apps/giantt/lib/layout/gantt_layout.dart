import 'dart:collection';
import 'package:giantt_core/giantt_core.dart';

/// A dependency edge between two rows in the Gantt chart.
class GanttDependency {
  const GanttDependency({required this.fromRow, required this.toRow});

  /// Row index of the predecessor (must finish first).
  final int fromRow;

  /// Row index of the dependent item (starts after predecessor).
  final int toRow;
}

/// Layout data for one item row in the Gantt chart.
class GanttRowData {
  const GanttRowData({
    required this.item,
    required this.startSeconds,
    required this.durationSeconds,
    required this.rowIndex,
  });

  final GianttItem item;

  /// Relative start time in seconds from the chart origin.
  final double startSeconds;

  /// Visual duration in seconds (minimum 1 day).
  final double durationSeconds;

  /// Vertical row index (0 = topmost).
  final int rowIndex;

  double get endSeconds => startSeconds + durationSeconds;
}

/// Computed layout for all items in a Gantt chart view.
///
/// Positions are relative (no fixed calendar dates). The timeline is driven
/// by item durations and REQUIRES dependency ordering.
class GanttLayout {
  const GanttLayout({
    required this.rows,
    required this.dependencies,
    required this.totalSpanSeconds,
  });

  final List<GanttRowData> rows;
  final List<GanttDependency> dependencies;

  /// Total timeline length in seconds (padded slightly beyond the last bar).
  final double totalSpanSeconds;

  bool get isEmpty => rows.isEmpty;

  /// Compute a Gantt layout from a flat list of items.
  ///
  /// Uses Kahn's topological sort on REQUIRES relations. Each item's start
  /// time equals the latest end time among its prerequisites. Items with no
  /// prerequisites all start at t=0. Cycle members are appended at the end.
  static GanttLayout compute(List<GianttItem> items) {
    if (items.isEmpty) {
      return const GanttLayout(
        rows: [],
        dependencies: [],
        totalSpanSeconds: 0,
      );
    }

    final itemMap = {for (final item in items) item.id: item};

    // prereqs[id] = set of item IDs that must finish before `id` starts.
    // Only REQUIRES is processed here; BLOCKS is the auto-created inverse so
    // if we processed both we would double-count every edge.
    final prereqs = <String, Set<String>>{
      for (final item in items) item.id: <String>{},
    };
    for (final item in items) {
      for (final reqId in (item.relations['REQUIRES'] ?? [])) {
        if (itemMap.containsKey(reqId)) {
          prereqs[item.id]!.add(reqId);
        }
      }
    }

    // Kahn's algorithm ─ topological sort.
    final inDegree = <String, int>{
      for (final item in items) item.id: prereqs[item.id]!.length,
    };
    final successors = <String, List<String>>{
      for (final item in items) item.id: [],
    };
    for (final entry in prereqs.entries) {
      for (final prereqId in entry.value) {
        successors[prereqId]!.add(entry.key);
      }
    }

    // Seed with items that have no prerequisites, highest priority first.
    final seeds =
        items.where((item) => inDegree[item.id] == 0).toList()
          ..sort(
            (a, b) => b.priority.index.compareTo(a.priority.index),
          );
    final queue = Queue<String>()..addAll(seeds.map((item) => item.id));

    final topoOrder = <String>[];
    while (queue.isNotEmpty) {
      final id = queue.removeFirst();
      topoOrder.add(id);
      for (final successorId in successors[id]!) {
        inDegree[successorId] = inDegree[successorId]! - 1;
        if (inDegree[successorId] == 0) {
          queue.add(successorId);
        }
      }
    }

    // Append any unvisited items (cycle members).
    final visited = topoOrder.toSet();
    for (final item in items) {
      if (!visited.contains(item.id)) {
        topoOrder.add(item.id);
      }
    }

    // Compute start times based on prerequisite end times.
    final endTimes = <String, double>{};
    final rows = <GanttRowData>[];

    for (var i = 0; i < topoOrder.length; i++) {
      final id = topoOrder[i];
      final item = itemMap[id]!;

      double start = 0;
      for (final prereqId in prereqs[id]!) {
        final end = endTimes[prereqId] ?? 0;
        if (end > start) start = end;
      }

      // Give items with no duration a minimum visual size of 1 day.
      final dur =
          item.duration.totalSeconds > 0
              ? item.duration.totalSeconds
              : 86400.0;
      endTimes[id] = start + dur;

      rows.add(
        GanttRowData(
          item: item,
          startSeconds: start,
          durationSeconds: dur,
          rowIndex: i,
        ),
      );
    }

    // Build dependency edges for arrow rendering.
    final rowIndexMap = {for (final row in rows) row.item.id: row.rowIndex};
    final deps = <GanttDependency>[];
    for (final row in rows) {
      for (final prereqId in prereqs[row.item.id]!) {
        final fromIdx = rowIndexMap[prereqId];
        if (fromIdx != null) {
          deps.add(GanttDependency(fromRow: fromIdx, toRow: row.rowIndex));
        }
      }
    }

    final maxEnd =
        rows.fold(0.0, (max, row) => row.endSeconds > max ? row.endSeconds : max);

    return GanttLayout(
      rows: rows,
      dependencies: deps,
      // 5% right-padding so the last bar isn't flush with the edge.
      totalSpanSeconds: maxEnd > 0 ? maxEnd * 1.05 : 86400.0 * 7,
    );
  }
}
