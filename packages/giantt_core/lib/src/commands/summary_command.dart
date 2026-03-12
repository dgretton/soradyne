import 'dart:convert';
import 'dart:io';
import '../query/giantt_query.dart';
import '../models/priority.dart';
import '../graph/giantt_graph.dart';
import '../storage/dual_file_manager.dart';

/// Programmatic entry point for the `summary` command.
///
/// Returns a [SummaryResult] or throws on error.
/// Intended for use by integration layers (MCP servers, Flutter, etc.).
/// If [graph] is provided, it is used directly; otherwise the graph is loaded
/// from [itemsPath] / [occludeItemsPath].
SummaryResult runSummary(
  String itemsPath,
  String occludeItemsPath, {
  GianttGraph? graph,
  DateTime? today,
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
}) {
  graph ??= DualFileManager.loadGraph(itemsPath, occludeItemsPath);
  return GianttQuery(graph).summary(
    today: today,
    filter: QueryFilter(
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
    ),
  );
}

/// Handles the `giantt summary` CLI subcommand.
///
/// Called from `bin/giantt.dart` with the parsed [ArgResults]-equivalent values
/// already extracted. Prints either human-readable text or JSON to stdout.
void executeSummaryCommand({
  required String itemsPath,
  required String occludeItemsPath,
  GianttGraph? graph,
  DateTime? today,
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
  bool jsonOutput = false,
}) {
  try {
    final result = runSummary(
      itemsPath,
      occludeItemsPath,
      graph: graph,
      today: today,
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
    );

    if (jsonOutput) {
      print(jsonEncode({'ok': true, 'command': 'summary', 'data': result.toJson()}));
      return;
    }

    // Human-readable output.
    print('Summary as of ${result.asOf}  (${result.totalItems} items'
        '${result.uncategorisedCount > 0 ? ', ${result.uncategorisedCount} uncategorised' : ''})');
    print('');

    if (result.charts.isEmpty) {
      print('No items found.');
      return;
    }

    for (final chart in result.charts) {
      print('── ${chart.chartName} ──');
      print('  Items: ${chart.totalItems}'
          '  (not-finished: ${chart.notFinished}'
          ', completed: ${chart.completed})');
      print('  Status breakdown:'
          '  ○ ${chart.notStarted}'
          '  ◑ ${chart.inProgress}'
          '  ⊘ ${chart.blocked}'
          '  ● ${chart.completed}');

      final remainingDays =
          (chart.remainingWorkSeconds / 86400).toStringAsFixed(1);
      print('  Remaining work: ${remainingDays}d');

      if (chart.nearestDeadline != null) {
        print('  Nearest deadline: ${chart.nearestDeadline}'
            ' (${chart.nearestDeadlineItemId})'
            '  [${chart.deadlineItemCount} deadline item(s)'
            ', ${chart.deadlineItemsHighOrAbove} high+]');
      } else {
        print('  No deadlines set.');
      }
      print('');
    }
  } catch (e) {
    if (jsonOutput) {
      print(jsonEncode({'ok': false, 'command': 'summary', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
    exit(1);
  }
}
