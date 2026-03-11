import '../models/giantt_item.dart';
import '../models/priority.dart';
import '../models/status.dart';
import '../models/time_constraint.dart';
import '../graph/giantt_graph.dart';

// ---------------------------------------------------------------------------
// Filter
// ---------------------------------------------------------------------------

/// Filter parameters shared across all query methods.
///
/// Items with no charts belong to the synthetic chart name `"(no chart)"`.
class QueryFilter {
  const QueryFilter({
    this.charts,
    this.excludeCharts,
    this.minPriority,
  });

  /// Include only items belonging to at least one of these charts.
  /// `null` means no chart restriction.
  final List<String>? charts;

  /// Exclude items whose every chart is in this list.
  final List<String>? excludeCharts;

  /// Include only items at this priority or above.
  /// Priority order (ascending severity): lowest, low, neutral, unsure, medium, high, critical.
  final GianttPriority? minPriority;
}

// ---------------------------------------------------------------------------
// Summary result types
// ---------------------------------------------------------------------------

/// Summary statistics for a single chart.
class ChartSummary {
  const ChartSummary({
    required this.chartName,
    required this.totalItems,
    required this.notStarted,
    required this.inProgress,
    required this.blocked,
    required this.completed,
    required this.notFinished,
    required this.totalWorkSeconds,
    required this.remainingWorkSeconds,
    required this.nearestDeadline,
    required this.nearestDeadlineItemId,
    required this.deadlineItemCount,
    required this.deadlineItemsHighOrAbove,
    required this.itemsByPriority,
  });

  final String chartName;
  final int totalItems;
  final int notStarted;
  final int inProgress;
  final int blocked;
  final int completed;

  /// Items that are not completed (notStarted + inProgress + blocked).
  final int notFinished;

  /// Sum of durations of ALL items in this chart (seconds).
  final double totalWorkSeconds;

  /// Sum of durations of not-finished items (seconds).
  final double remainingWorkSeconds;

  /// ISO-8601 date (YYYY-MM-DD) of the nearest deadline across not-finished items.
  final String? nearestDeadline;

  /// ID of the item with the nearest deadline.
  final String? nearestDeadlineItemId;

  /// Count of not-finished items that have at least one deadline constraint.
  final int deadlineItemCount;

  /// Of deadline items, how many are at HIGH or CRITICAL priority.
  final int deadlineItemsHighOrAbove;

  /// Count of not-finished items by priority name (e.g. "HIGH" → 3).
  final Map<String, int> itemsByPriority;

  Map<String, dynamic> toJson() => {
        'chart': chartName,
        'total_items': totalItems,
        'not_started': notStarted,
        'in_progress': inProgress,
        'blocked': blocked,
        'completed': completed,
        'not_finished': notFinished,
        'total_work_seconds': totalWorkSeconds,
        'remaining_work_seconds': remainingWorkSeconds,
        'nearest_deadline': nearestDeadline,
        'nearest_deadline_item_id': nearestDeadlineItemId,
        'deadline_item_count': deadlineItemCount,
        'deadline_items_high_or_above': deadlineItemsHighOrAbove,
        'items_by_priority': itemsByPriority,
      };
}

/// Top-level result from [GianttQuery.summary].
class SummaryResult {
  const SummaryResult({
    required this.asOf,
    required this.totalItems,
    required this.uncategorisedCount,
    required this.charts,
  });

  /// ISO-8601 date used as "today" for this summary.
  final String asOf;

  /// Total non-occluded items passing the filter.
  final int totalItems;

  /// Count of non-occluded items with no chart membership.
  final int uncategorisedCount;

  final List<ChartSummary> charts;

  Map<String, dynamic> toJson() => {
        'as_of': asOf,
        'total_items': totalItems,
        'uncategorised_count': uncategorisedCount,
        'charts': charts.map((c) => c.toJson()).toList(),
      };
}

// ---------------------------------------------------------------------------
// Load result types
// ---------------------------------------------------------------------------

/// How a not-finished item was placed within a load window.
enum LoadPlacement { deadline, recurring, windowed, floating }

/// A single item as it appears in a load analysis window.
class LoadItem {
  const LoadItem({
    required this.id,
    required this.title,
    required this.priority,
    required this.durationSeconds,
    required this.placement,
    this.startDate,
    this.endDate,
    this.recurrenceIntervalSeconds,
    required this.charts,
  });

  final String id;
  final String title;

  /// Priority name (e.g. "HIGH").
  final String priority;

  final double durationSeconds;
  final LoadPlacement placement;

  /// ISO-8601 date — set only for [LoadPlacement.deadline] items.
  final String? startDate;

  /// ISO-8601 date — set only for [LoadPlacement.deadline] items.
  final String? endDate;

  /// Seconds between recurrences — set only for [LoadPlacement.recurring] items.
  final double? recurrenceIntervalSeconds;

  final List<String> charts;

  Map<String, dynamic> toJson() => {
        'id': id,
        'title': title,
        'priority': priority,
        'duration_seconds': durationSeconds,
        'placement': placement.name,
        'start_date': startDate,
        'end_date': endDate,
        'recurrence_interval_seconds': recurrenceIntervalSeconds,
        'charts': charts,
      };
}

/// A priority tier within a load window.
class LoadTier {
  const LoadTier({
    required this.label,
    required this.items,
    required this.totalWorkSeconds,
  });

  /// One of: `"critical+high"`, `"medium"`, `"low+below"`.
  final String label;
  final List<LoadItem> items;
  final double totalWorkSeconds;

  Map<String, dynamic> toJson() => {
        'label': label,
        'total_work_seconds': totalWorkSeconds,
        'items': items.map((i) => i.toJson()).toList(),
      };
}

/// Top-level result from [GianttQuery.load].
class LoadResult {
  const LoadResult({
    required this.windowStart,
    required this.windowEnd,
    required this.windowSeconds,
    required this.tiers,
    required this.windowedUnplaced,
    required this.floating,
    required this.peakConcurrentCount,
    this.peakConcurrentDate,
  });

  final String windowStart;
  final String windowEnd;
  final double windowSeconds;

  /// Three priority tiers containing deadline + recurring items.
  final List<LoadTier> tiers;

  /// Items with `window` constraints but no fixed anchor date.
  final List<LoadItem> windowedUnplaced;

  /// Items with no time constraints at all.
  final List<LoadItem> floating;

  /// Maximum number of deadline items simultaneously active on any single day.
  final int peakConcurrentCount;

  /// ISO-8601 date of peak load (null if no deadline items).
  final String? peakConcurrentDate;

  Map<String, dynamic> toJson() => {
        'window_start': windowStart,
        'window_end': windowEnd,
        'window_seconds': windowSeconds,
        'peak_concurrent_count': peakConcurrentCount,
        'peak_concurrent_date': peakConcurrentDate,
        'tiers': tiers.map((t) => t.toJson()).toList(),
        'windowed_unplaced': windowedUnplaced.map((i) => i.toJson()).toList(),
        'floating': floating.map((i) => i.toJson()).toList(),
      };
}

// ---------------------------------------------------------------------------
// Deps result types
// ---------------------------------------------------------------------------

/// A node in a dependency chain.
class DepNode {
  const DepNode({
    required this.id,
    required this.title,
    required this.status,
    required this.priority,
    required this.durationSeconds,
    required this.depth,
  });

  final String id;
  final String title;

  /// Status name (e.g. "IN_PROGRESS").
  final String status;

  /// Priority name (e.g. "HIGH").
  final String priority;

  final double durationSeconds;

  /// Negative = upstream (prerequisites), positive = downstream (blocked items).
  /// Direct neighbours are ±1.
  final int depth;

  Map<String, dynamic> toJson() => {
        'id': id,
        'title': title,
        'status': status,
        'priority': priority,
        'duration_seconds': durationSeconds,
        'depth': depth,
      };
}

/// Top-level result from [GianttQuery.deps].
class DepsResult {
  const DepsResult({
    required this.focalId,
    required this.focalTitle,
    required this.upstream,
    required this.downstream,
    required this.allIds,
  });

  final String focalId;
  final String focalTitle;

  /// Items this item requires, transitively (depth < 0).
  final List<DepNode> upstream;

  /// Items blocked by this item, transitively (depth > 0).
  final List<DepNode> downstream;

  /// All IDs in the chain including the focal item.
  final List<String> allIds;

  Map<String, dynamic> toJson() => {
        'focal_id': focalId,
        'focal_title': focalTitle,
        'upstream': upstream.map((n) => n.toJson()).toList(),
        'downstream': downstream.map((n) => n.toJson()).toList(),
        'all_ids': allIds,
      };
}

// ---------------------------------------------------------------------------
// Blocked result types
// ---------------------------------------------------------------------------

/// A single item that is blocked (either explicitly or via unfinished requires).
class BlockedItem {
  const BlockedItem({
    required this.id,
    required this.title,
    required this.status,
    required this.priority,
    required this.durationSeconds,
    required this.charts,
    required this.blockedReason,
    required this.unfinishedRequires,
  });

  final String id;
  final String title;
  final String status;
  final String priority;
  final double durationSeconds;
  final List<String> charts;

  /// `"explicit"` if status==BLOCKED; `"requires_unfinished"` otherwise.
  final String blockedReason;

  /// IDs of not-finished items that this item REQUIRES.
  final List<String> unfinishedRequires;

  Map<String, dynamic> toJson() => {
        'id': id,
        'title': title,
        'status': status,
        'priority': priority,
        'duration_seconds': durationSeconds,
        'charts': charts,
        'blocked_reason': blockedReason,
        'unfinished_requires': unfinishedRequires,
      };
}

/// Top-level result from [GianttQuery.blocked].
class BlockedResult {
  const BlockedResult({
    required this.totalBlocked,
    required this.explicitCount,
    required this.impliedCount,
    required this.items,
  });

  final int totalBlocked;

  /// Items with status == BLOCKED.
  final int explicitCount;

  /// Items blocked because at least one REQUIRES target is not-finished.
  final int impliedCount;

  final List<BlockedItem> items;

  Map<String, dynamic> toJson() => {
        'total_blocked': totalBlocked,
        'explicit_count': explicitCount,
        'implied_count': impliedCount,
        'items': items.map((i) => i.toJson()).toList(),
      };
}

// ---------------------------------------------------------------------------
// GianttQuery
// ---------------------------------------------------------------------------

/// Pure query layer over a [GianttGraph].
///
/// All methods operate on [GianttGraph.includedItems] (non-occluded items only)
/// unless otherwise noted.
class GianttQuery {
  const GianttQuery(this.graph);

  final GianttGraph graph;

  static const String _noChart = '(no chart)';

  // ---- filter helpers ----

  List<String> _itemCharts(GianttItem item) =>
      item.charts.isEmpty ? [_noChart] : List<String>.from(item.charts);

  bool _passesChartFilter(GianttItem item, QueryFilter filter) {
    final charts = _itemCharts(item);
    if (filter.excludeCharts != null) {
      // Exclude if ALL of the item's charts are in the exclusion list.
      if (charts.every((c) => filter.excludeCharts!.contains(c))) return false;
    }
    if (filter.charts != null) {
      // Include only if at least one chart matches.
      if (!charts.any((c) => filter.charts!.contains(c))) return false;
    }
    return true;
  }

  bool _passesPriorityFilter(GianttItem item, QueryFilter filter) {
    if (filter.minPriority == null) return true;
    return GianttPriority.values.indexOf(item.priority) >=
        GianttPriority.values.indexOf(filter.minPriority!);
  }

  bool _isNotFinished(GianttItem item) =>
      item.status != GianttStatus.completed;

  static String _dateToIso(DateTime dt) =>
      '${dt.year.toString().padLeft(4, '0')}-'
      '${dt.month.toString().padLeft(2, '0')}-'
      '${dt.day.toString().padLeft(2, '0')}';

  // ---- summary ----

  /// Returns per-chart statistics for the loaded graph.
  ///
  /// Items belonging to multiple charts appear in each chart's counts.
  /// Items with no charts are counted in [SummaryResult.uncategorisedCount]
  /// and appear under the synthetic chart `"(no chart)"`.
  SummaryResult summary({
    DateTime? today,
    QueryFilter filter = const QueryFilter(),
  }) {
    final now = today ?? DateTime.now();

    final allItems = graph.includedItems.values
        .where((item) =>
            _passesChartFilter(item, filter) &&
            _passesPriorityFilter(item, filter))
        .toList();

    // Collect all chart names (items may be in multiple charts).
    final chartNames = <String>{};
    int uncategorisedCount = 0;
    for (final item in allItems) {
      if (item.charts.isEmpty) {
        uncategorisedCount++;
        chartNames.add(_noChart);
      } else {
        chartNames.addAll(item.charts);
      }
    }

    final chartSummaries = <ChartSummary>[];
    for (final chartName in chartNames.toList()..sort()) {
      final chartItems =
          allItems.where((item) => _itemCharts(item).contains(chartName)).toList();
      chartSummaries.add(_buildChartSummary(chartName, chartItems, now));
    }

    return SummaryResult(
      asOf: _dateToIso(now),
      totalItems: allItems.length,
      uncategorisedCount: uncategorisedCount,
      charts: chartSummaries,
    );
  }

  ChartSummary _buildChartSummary(
      String chartName, List<GianttItem> items, DateTime today) {
    int notStarted = 0, inProgress = 0, blocked = 0, completed = 0;
    double totalWork = 0, remainingWork = 0;

    final priorityCounts = <String, int>{
      for (final p in GianttPriority.values) p.name: 0,
    };

    DateTime? nearestDeadline;
    String? nearestDeadlineItemId;
    int deadlineItemCount = 0;
    int deadlineHighOrAbove = 0;

    final highIdx = GianttPriority.values.indexOf(GianttPriority.high);

    for (final item in items) {
      totalWork += item.duration.totalSeconds;
      switch (item.status) {
        case GianttStatus.notStarted:
          notStarted++;
          break;
        case GianttStatus.inProgress:
          inProgress++;
          break;
        case GianttStatus.blocked:
          blocked++;
          break;
        case GianttStatus.completed:
          completed++;
          break;
      }

      if (_isNotFinished(item)) {
        remainingWork += item.duration.totalSeconds;
        priorityCounts[item.priority.name] =
            (priorityCounts[item.priority.name] ?? 0) + 1;

        final deadlines = item.timeConstraints
            .where((tc) =>
                tc.type == TimeConstraintType.deadline && tc.dueDate != null)
            .toList();

        if (deadlines.isNotEmpty) {
          deadlineItemCount++;
          if (GianttPriority.values.indexOf(item.priority) >= highIdx) {
            deadlineHighOrAbove++;
          }
          for (final tc in deadlines) {
            final due = DateTime.parse(tc.dueDate!);
            if (nearestDeadline == null || due.isBefore(nearestDeadline)) {
              nearestDeadline = due;
              nearestDeadlineItemId = item.id;
            }
          }
        }
      }
    }

    return ChartSummary(
      chartName: chartName,
      totalItems: items.length,
      notStarted: notStarted,
      inProgress: inProgress,
      blocked: blocked,
      completed: completed,
      notFinished: notStarted + inProgress + blocked,
      totalWorkSeconds: totalWork,
      remainingWorkSeconds: remainingWork,
      nearestDeadline:
          nearestDeadline != null ? _dateToIso(nearestDeadline) : null,
      nearestDeadlineItemId: nearestDeadlineItemId,
      deadlineItemCount: deadlineItemCount,
      deadlineItemsHighOrAbove: deadlineHighOrAbove,
      itemsByPriority: priorityCounts,
    );
  }

  // ---- load ----

  /// Analyses temporal load within [windowStart]..[windowEnd].
  ///
  /// Classification (checked in order):
  /// 1. **deadline** — item has a deadline constraint; placed at [dueDate-duration, dueDate].
  ///    Uses `item.duration.totalSeconds`, not `constraint.duration` (which is hardcoded to 1d).
  /// 2. **recurring** — item has a recurring constraint; included if any occurrence overlaps window.
  /// 3. **windowed** — item has a window constraint only; unplaced, no fixed anchor.
  /// 4. **floating** — no time constraints at all.
  ///
  /// Deadline + recurring items are grouped into priority tiers.
  /// Windowed items go into [LoadResult.windowedUnplaced].
  /// Floating items go into [LoadResult.floating].
  LoadResult load({
    required DateTime windowStart,
    required DateTime windowEnd,
    DateTime? today,
    QueryFilter filter = const QueryFilter(),
  }) {
    final now = today ?? DateTime.now();
    final windowSeconds = windowEnd.difference(windowStart).inSeconds.toDouble();

    final items = graph.includedItems.values
        .where((item) =>
            _isNotFinished(item) &&
            _passesChartFilter(item, filter) &&
            _passesPriorityFilter(item, filter))
        .toList();

    final deadlineItems = <LoadItem>[];
    final recurringItems = <LoadItem>[];
    final windowedItems = <LoadItem>[];
    final floatingItems = <LoadItem>[];

    for (final item in items) {
      final classified = _classifyItem(item, windowStart, windowEnd, now);
      if (classified == null) continue; // has deadline but outside window
      switch (classified.placement) {
        case LoadPlacement.deadline:
          deadlineItems.add(classified);
          break;
        case LoadPlacement.recurring:
          recurringItems.add(classified);
          break;
        case LoadPlacement.windowed:
          windowedItems.add(classified);
          break;
        case LoadPlacement.floating:
          floatingItems.add(classified);
          break;
      }
    }

    final tiers = _buildTiers([...deadlineItems, ...recurringItems]);
    final (peak, peakDate) = _computePeak(deadlineItems);

    return LoadResult(
      windowStart: _dateToIso(windowStart),
      windowEnd: _dateToIso(windowEnd),
      windowSeconds: windowSeconds,
      tiers: tiers,
      windowedUnplaced: windowedItems,
      floating: floatingItems,
      peakConcurrentCount: peak,
      peakConcurrentDate: peakDate != null ? _dateToIso(peakDate) : null,
    );
  }

  /// Returns null if the item has a deadline but it falls outside [windowStart]..[windowEnd].
  LoadItem? _classifyItem(
      GianttItem item, DateTime windowStart, DateTime windowEnd, DateTime today) {
    final durationSeconds = item.duration.totalSeconds;
    final charts = List<String>.from(item.charts);

    // 1. Deadline?
    final deadlines = item.timeConstraints
        .where((tc) =>
            tc.type == TimeConstraintType.deadline && tc.dueDate != null)
        .toList();
    if (deadlines.isNotEmpty) {
      // Use the earliest deadline to be conservative.
      DateTime? earliest;
      for (final tc in deadlines) {
        final d = DateTime.parse(tc.dueDate!);
        if (earliest == null || d.isBefore(earliest)) earliest = d;
      }
      final endDate = earliest!;
      final startDate =
          endDate.subtract(Duration(seconds: durationSeconds.round()));
      // Overlap check: [startDate, endDate) ∩ [windowStart, windowEnd)
      if (startDate.isBefore(windowEnd) && endDate.isAfter(windowStart)) {
        return LoadItem(
          id: item.id,
          title: item.title,
          priority: item.priority.name,
          durationSeconds: durationSeconds,
          placement: LoadPlacement.deadline,
          startDate: _dateToIso(startDate),
          endDate: _dateToIso(endDate),
          charts: charts,
        );
      }
      // Has a deadline but outside the window — skip entirely.
      return null;
    }

    // 2. Recurring?
    final recurring = item.timeConstraints
        .where((tc) =>
            tc.type == TimeConstraintType.recurring && tc.interval != null)
        .toList();
    if (recurring.isNotEmpty) {
      final tc = recurring.first;
      final intervalSeconds = tc.interval!.totalSeconds;
      if (intervalSeconds > 0) {
        if (_recurringOverlaps(today, intervalSeconds, durationSeconds,
            windowStart, windowEnd)) {
          return LoadItem(
            id: item.id,
            title: item.title,
            priority: item.priority.name,
            durationSeconds: durationSeconds,
            placement: LoadPlacement.recurring,
            recurrenceIntervalSeconds: intervalSeconds,
            charts: charts,
          );
        }
      }
      // Recurring but no occurrence in window — skip.
      return null;
    }

    // 3. Window constraint?
    final hasWindow =
        item.timeConstraints.any((tc) => tc.type == TimeConstraintType.window);
    if (hasWindow) {
      return LoadItem(
        id: item.id,
        title: item.title,
        priority: item.priority.name,
        durationSeconds: durationSeconds,
        placement: LoadPlacement.windowed,
        charts: charts,
      );
    }

    // 4. Floating.
    return LoadItem(
      id: item.id,
      title: item.title,
      priority: item.priority.name,
      durationSeconds: durationSeconds,
      placement: LoadPlacement.floating,
      charts: charts,
    );
  }

  /// Returns true if any occurrence of a recurring task (anchored at [today]
  /// with [intervalSeconds] period) overlaps [windowStart]..[windowEnd].
  bool _recurringOverlaps(
    DateTime today,
    double intervalSeconds,
    double durationSeconds,
    DateTime windowStart,
    DateTime windowEnd,
  ) {
    final intervalDur = Duration(seconds: intervalSeconds.round());
    final itemDur = Duration(seconds: durationSeconds.round());

    bool overlaps(DateTime occStart) {
      final occEnd = occStart.add(itemDur);
      return occStart.isBefore(windowEnd) && occEnd.isAfter(windowStart);
    }

    // Walk forward from today.
    var occ = today;
    // Limit iterations to avoid infinite loops (window can't be wider than a year of seconds).
    final maxIter = (windowEnd.difference(windowStart).inSeconds / intervalSeconds).ceil() + 2;
    for (int i = 0; i <= maxIter; i++) {
      if (occ.isAfter(windowEnd)) break;
      if (overlaps(occ)) return true;
      occ = occ.add(intervalDur);
    }

    // Walk backward from today (for windows in the past).
    occ = today.subtract(intervalDur);
    for (int i = 0; i <= maxIter; i++) {
      final occEnd = occ.add(itemDur);
      if (occEnd.isBefore(windowStart)) break;
      if (overlaps(occ)) return true;
      occ = occ.subtract(intervalDur);
    }

    return false;
  }

  List<LoadTier> _buildTiers(List<LoadItem> items) {
    final highIdx = GianttPriority.values.indexOf(GianttPriority.high);
    final mediumIdx = GianttPriority.values.indexOf(GianttPriority.medium);

    final criticalHigh = <LoadItem>[];
    final medium = <LoadItem>[];
    final lowBelow = <LoadItem>[];

    for (final item in items) {
      final priorityEnum = GianttPriority.values.firstWhere(
        (p) => p.name == item.priority,
        orElse: () => GianttPriority.neutral,
      );
      final idx = GianttPriority.values.indexOf(priorityEnum);
      if (idx >= highIdx) {
        criticalHigh.add(item);
      } else if (idx >= mediumIdx) {
        medium.add(item);
      } else {
        lowBelow.add(item);
      }
    }

    double workSum(List<LoadItem> list) =>
        list.fold(0.0, (s, i) => s + i.durationSeconds);

    return [
      LoadTier(
          label: 'critical+high',
          items: criticalHigh,
          totalWorkSeconds: workSum(criticalHigh)),
      LoadTier(
          label: 'medium',
          items: medium,
          totalWorkSeconds: workSum(medium)),
      LoadTier(
          label: 'low+below',
          items: lowBelow,
          totalWorkSeconds: workSum(lowBelow)),
    ];
  }

  /// Interval sweep to find the day with the most simultaneously active deadline items.
  (int peak, DateTime? peakDate) _computePeak(List<LoadItem> deadlineItems) {
    if (deadlineItems.isEmpty) return (0, null);

    // Events: (date, delta) — end events (-1) sort before start events (+1) on same day.
    final events = <(DateTime, int)>[];
    for (final item in deadlineItems) {
      if (item.startDate == null || item.endDate == null) continue;
      events.add((DateTime.parse(item.startDate!), 1));
      events.add((DateTime.parse(item.endDate!), -1));
    }

    events.sort((a, b) {
      final cmp = a.$1.compareTo(b.$1);
      if (cmp != 0) return cmp;
      return a.$2.compareTo(b.$2); // -1 before +1 on same day
    });

    int current = 0;
    int peak = 0;
    DateTime? peakDate;

    for (final (date, delta) in events) {
      current += delta;
      if (current > peak) {
        peak = current;
        peakDate = date;
      }
    }

    return (peak, peakDate);
  }

  // ---- deps ----

  /// Returns the dependency chain for an item identified by [itemId].
  ///
  /// [itemId] may be an exact ID or a title substring (same matching as `show`).
  /// Traverses REQUIRES upstream and BLOCKS downstream transitively up to [maxDepth].
  ///
  /// Uses [GianttGraph.items] (all items, including occluded) for traversal so
  /// chains are not silently severed by occlusion.
  ///
  /// Throws [ArgumentError] if the item is not found.
  DepsResult deps(String itemId, {int maxDepth = 20}) {
    final allItems = graph.items;

    // Resolve focal item: exact ID first, then substring.
    GianttItem focal;
    if (allItems.containsKey(itemId)) {
      focal = allItems[itemId]!;
    } else {
      focal = graph.findBySubstring(itemId);
    }

    final upstream = <DepNode>[];
    final downstream = <DepNode>[];

    _bfsChain(focal.id, 'REQUIRES', allItems, <String>{focal.id}, upstream, -1,
        maxDepth);
    _bfsChain(focal.id, 'BLOCKS', allItems, <String>{focal.id}, downstream, 1,
        maxDepth);

    final allIds = <String>{focal.id}
      ..addAll(upstream.map((n) => n.id))
      ..addAll(downstream.map((n) => n.id));

    return DepsResult(
      focalId: focal.id,
      focalTitle: focal.title,
      upstream: upstream,
      downstream: downstream,
      allIds: allIds.toList(),
    );
  }

  void _bfsChain(
    String startId,
    String relationKey,
    Map<String, GianttItem> allItems,
    Set<String> visited,
    List<DepNode> results,
    int depthSign,
    int maxDepth,
  ) {
    // Queue: (itemId, depth)
    final queue = <(String, int)>[];
    for (final t in allItems[startId]?.relations[relationKey] ?? <String>[]) {
      if (!visited.contains(t)) queue.add((t, depthSign));
    }

    while (queue.isNotEmpty) {
      final (currentId, depth) = queue.removeAt(0);
      if (visited.contains(currentId)) continue;
      visited.add(currentId);

      final item = allItems[currentId];
      if (item == null) continue;

      results.add(DepNode(
        id: item.id,
        title: item.title,
        status: item.status.name,
        priority: item.priority.name,
        durationSeconds: item.duration.totalSeconds,
        depth: depth,
      ));

      if (depth.abs() < maxDepth) {
        for (final t in item.relations[relationKey] ?? <String>[]) {
          if (!visited.contains(t)) {
            queue.add((t, depth + depthSign));
          }
        }
      }
    }
  }

  // ---- blocked ----

  /// Returns all not-finished items that are blocked.
  ///
  /// An item is blocked if:
  /// - Its status is [GianttStatus.blocked] (`"explicit"`), OR
  /// - At least one of its REQUIRES targets is also not-finished (`"requires_unfinished"`).
  ///
  /// Explicit status takes precedence in [BlockedItem.blockedReason].
  /// [BlockedItem.unfinishedRequires] is always populated regardless of reason.
  ///
  /// Results are sorted by priority descending (critical first).
  BlockedResult blocked({QueryFilter filter = const QueryFilter()}) {
    final allIncluded = graph.includedItems;
    final items = allIncluded.values
        .where((item) =>
            _isNotFinished(item) &&
            _passesChartFilter(item, filter) &&
            _passesPriorityFilter(item, filter))
        .toList();

    final blockedItems = <BlockedItem>[];

    for (final item in items) {
      final requires = item.relations['REQUIRES'] ?? <String>[];
      final unfinishedRequires = requires
          .where((id) {
            final req = allIncluded[id];
            return req != null && _isNotFinished(req);
          })
          .toList();

      final isExplicit = item.status == GianttStatus.blocked;
      final isImplied = unfinishedRequires.isNotEmpty;

      if (!isExplicit && !isImplied) continue;

      blockedItems.add(BlockedItem(
        id: item.id,
        title: item.title,
        status: item.status.name,
        priority: item.priority.name,
        durationSeconds: item.duration.totalSeconds,
        charts: List<String>.from(item.charts),
        blockedReason: isExplicit ? 'explicit' : 'requires_unfinished',
        unfinishedRequires: unfinishedRequires,
      ));
    }

    // Sort by priority descending (critical first).
    blockedItems.sort((a, b) {
      final pa = GianttPriority.values
          .firstWhere((p) => p.name == a.priority, orElse: () => GianttPriority.neutral);
      final pb = GianttPriority.values
          .firstWhere((p) => p.name == b.priority, orElse: () => GianttPriority.neutral);
      return GianttPriority.values.indexOf(pb)
          .compareTo(GianttPriority.values.indexOf(pa));
    });

    final explicitCount =
        blockedItems.where((i) => i.blockedReason == 'explicit').length;

    return BlockedResult(
      totalBlocked: blockedItems.length,
      explicitCount: explicitCount,
      impliedCount: blockedItems.length - explicitCount,
      items: blockedItems,
    );
  }
}
