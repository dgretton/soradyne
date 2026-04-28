import 'package:meta/meta.dart';
import '../models/giantt_item.dart';
import '../models/time_constraint.dart';
import '../models/duration.dart';
import '../graph/giantt_graph.dart';
import 'filter.dart';
import 'row_order.dart';

/// Configuration for one printed page: which charts to merge and what time
/// window to show.
@immutable
class PageConfig {
  const PageConfig({
    required this.charts,
    required this.from,
    required this.to,
    this.title,
  });

  /// Chart names to merge into this page. An item is included if any of its
  /// `charts` overlaps with this list.
  final List<String> charts;

  /// Inclusive left edge of the time axis.
  final DateTime from;

  /// Inclusive right edge of the time axis.
  final DateTime to;

  /// Optional explicit page title; otherwise derived from chart names.
  final String? title;

  String get derivedTitle => title ?? charts.join(' + ');
}

/// A single placed item ready for rendering.
@immutable
class LaidOutItem {
  const LaidOutItem({
    required this.item,
    required this.row,
    required this.start,
    required this.end,
    required this.timeAnchored,
  });

  final GianttItem item;

  /// 0-based row index from the top of the chart.
  final int row;

  final DateTime start;
  final DateTime end;

  /// True if `start`/`end` were derived from a deadline. False means the bar
  /// position is inferred (predecessor chain or "today" fallback) and should
  /// be rendered with a softer style.
  final bool timeAnchored;
}

/// A dependency edge that fits within a single page.
@immutable
class LaidOutEdge {
  const LaidOutEdge({
    required this.fromId,
    required this.toId,
    required this.relationType,
  });

  final String fromId;
  final String toId;
  final String relationType;
}

/// A predecessor that's on a different page or outside the visible window.
/// Rendered as a margin stub on the destination row.
@immutable
class OffPageStub {
  const OffPageStub({
    required this.toItemId,
    required this.predecessorId,
    required this.predecessorChart,
    required this.relationType,
  });

  final String toItemId;
  final String predecessorId;
  final String? predecessorChart;
  final String relationType;
}

/// Everything render.dart needs to draw one page.
@immutable
class PageLayout {
  const PageLayout({
    required this.config,
    required this.items,
    required this.edges,
    required this.offPageStubs,
    required this.now,
  });

  final PageConfig config;
  final List<LaidOutItem> items;
  final List<LaidOutEdge> edges;
  final List<OffPageStub> offPageStubs;
  final DateTime now;
}

/// Build a single page's layout from the graph.
///
/// Phase 1: filter is "include items in any of the page's charts", row order
/// is the order returned by [orderRows], time placement is inferred per-item.
PageLayout buildPageLayout({
  required PageConfig config,
  required GianttGraph graph,
  required DateTime now,
}) {
  final visibleIds = filterItemsForPage(
    graph: graph,
    chartNames: config.charts,
  );

  // Infer time bars for every visible item. Pass over the graph in topological
  // order so predecessor end-times are known when we compute their successors.
  final times = _inferTimeBars(
    visibleIds: visibleIds,
    graph: graph,
    now: now,
  );

  final orderedIds = orderRows(
    ids: visibleIds.toList(),
    graph: graph,
  );

  final laidOut = <LaidOutItem>[];
  for (var i = 0; i < orderedIds.length; i++) {
    final id = orderedIds[i];
    final item = graph.items[id];
    if (item == null) continue;
    final tb = times[id]!;
    laidOut.add(LaidOutItem(
      item: item,
      row: i,
      start: tb.start,
      end: tb.end,
      timeAnchored: tb.anchored,
    ));
  }

  final edges = <LaidOutEdge>[];
  final stubs = <OffPageStub>[];
  for (final laid in laidOut) {
    final relations = laid.item.relations;
    for (final entry in relations.entries) {
      for (final predId in entry.value) {
        if (visibleIds.contains(predId)) {
          edges.add(LaidOutEdge(
            fromId: predId,
            toId: laid.item.id,
            relationType: entry.key,
          ));
        } else {
          final pred = graph.items[predId];
          stubs.add(OffPageStub(
            toItemId: laid.item.id,
            predecessorId: predId,
            predecessorChart:
                (pred != null && pred.charts.isNotEmpty) ? pred.charts.first : null,
            relationType: entry.key,
          ));
        }
      }
    }
  }

  return PageLayout(
    config: config,
    items: laidOut,
    edges: edges,
    offPageStubs: stubs,
    now: now,
  );
}

class _TimeBar {
  const _TimeBar({required this.start, required this.end, required this.anchored});
  final DateTime start;
  final DateTime end;
  final bool anchored;
}

Map<String, _TimeBar> _inferTimeBars({
  required Set<String> visibleIds,
  required GianttGraph graph,
  required DateTime now,
}) {
  final result = <String, _TimeBar>{};

  // Try a topological pass over the visible items so predecessor ends inform
  // successor starts. If the graph is cyclic for any reason, fall back to
  // arbitrary order — every item still gets *some* placement.
  List<String> order;
  try {
    final topo = graph.topologicalSort();
    order = topo
        .where((it) => visibleIds.contains(it.id))
        .map((it) => it.id)
        .toList();
    for (final id in visibleIds) {
      if (!order.contains(id)) order.add(id);
    }
  } catch (_) {
    order = visibleIds.toList();
  }

  for (final id in order) {
    final item = graph.items[id];
    if (item == null) continue;

    final durationDays = _gianttDurationToDays(item.duration);

    // Anchor on a deadline if one exists.
    final deadline = _firstDeadline(item.timeConstraints);
    if (deadline != null) {
      final end = deadline;
      final start = end.subtract(Duration(days: durationDays));
      result[id] = _TimeBar(start: start, end: end, anchored: true);
      continue;
    }

    // Otherwise place after the latest visible predecessor.
    final preds = item.relations['REQUIRES'] ?? const <String>[];
    DateTime? latestPredEnd;
    for (final p in preds) {
      final pb = result[p];
      if (pb == null) continue;
      if (latestPredEnd == null || pb.end.isAfter(latestPredEnd)) {
        latestPredEnd = pb.end;
      }
    }
    final start = latestPredEnd ?? now;
    final end = start.add(Duration(days: durationDays));
    result[id] = _TimeBar(start: start, end: end, anchored: false);
  }

  return result;
}

int _gianttDurationToDays(GianttDuration d) {
  final secs = d.totalSeconds;
  if (secs <= 0) return 1;
  final days = (secs / 86400.0).ceil();
  return days < 1 ? 1 : days;
}

DateTime? _firstDeadline(List<TimeConstraint> constraints) {
  for (final c in constraints) {
    if (c.type == TimeConstraintType.deadline && c.dueDate != null) {
      try {
        return DateTime.parse(c.dueDate!);
      } catch (_) {
        // skip malformed
      }
    }
  }
  return null;
}
