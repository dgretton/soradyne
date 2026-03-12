import 'dart:convert';
import 'dart:io';
import '../query/giantt_query.dart';
import '../models/priority.dart';
import '../graph/giantt_graph.dart';
import '../storage/dual_file_manager.dart';

/// Programmatic entry point for the `load` command.
///
/// If [graph] is provided, it is used directly; otherwise the graph is loaded
/// from [itemsPath] / [occludeItemsPath].
LoadResult runLoad(
  String itemsPath,
  String occludeItemsPath, {
  GianttGraph? graph,
  required DateTime windowStart,
  required DateTime windowEnd,
  DateTime? today,
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
}) {
  graph ??= DualFileManager.loadGraph(itemsPath, occludeItemsPath);
  return GianttQuery(graph).load(
    windowStart: windowStart,
    windowEnd: windowEnd,
    today: today,
    filter: QueryFilter(
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
    ),
  );
}

/// Handles the `giantt load` CLI subcommand.
void executeLoadCommand({
  required String itemsPath,
  required String occludeItemsPath,
  GianttGraph? graph,
  required DateTime windowStart,
  required DateTime windowEnd,
  DateTime? today,
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
  bool jsonOutput = false,
}) {
  try {
    final result = runLoad(
      itemsPath,
      occludeItemsPath,
      graph: graph,
      windowStart: windowStart,
      windowEnd: windowEnd,
      today: today,
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
    );

    if (jsonOutput) {
      print(jsonEncode({'ok': true, 'command': 'load', 'data': result.toJson()}));
      return;
    }

    // Human-readable output.
    final windowDays = (result.windowSeconds / 86400).toStringAsFixed(0);
    print('Load: ${result.windowStart} → ${result.windowEnd}'
        '  ($windowDays days)');

    if (result.peakConcurrentCount > 0) {
      print('Peak concurrent: ${result.peakConcurrentCount}'
          ' deadline item(s) on ${result.peakConcurrentDate}');
    }
    print('');

    for (final tier in result.tiers) {
      if (tier.items.isEmpty) continue;
      final tierDays = (tier.totalWorkSeconds / 86400).toStringAsFixed(1);
      print('${tier.label.toUpperCase()} (${tier.items.length} items, ${tierDays}d work):');
      for (final item in tier.items) {
        final placement = item.placement == LoadPlacement.deadline
            ? '${item.startDate} → ${item.endDate}'
            : 'recurring every ${_formatSeconds(item.recurrenceIntervalSeconds ?? 0)}';
        print('  ${item.priority.padRight(8)} ${item.id}  ${item.title}');
        print('    $placement');
      }
      print('');
    }

    if (result.windowedUnplaced.isNotEmpty) {
      print('WINDOWED (unplaced, ${result.windowedUnplaced.length} items):');
      for (final item in result.windowedUnplaced) {
        print('  ${item.priority.padRight(8)} ${item.id}  ${item.title}');
      }
      print('');
    }

    if (result.floating.isNotEmpty) {
      print('FLOATING (no constraints, ${result.floating.length} items):');
      for (final item in result.floating) {
        print('  ${item.priority.padRight(8)} ${item.id}  ${item.title}');
      }
    }
  } catch (e) {
    if (jsonOutput) {
      print(jsonEncode({'ok': false, 'command': 'load', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
    exit(1);
  }
}

String _formatSeconds(double seconds) {
  if (seconds >= 86400 * 7) {
    return '${(seconds / (86400 * 7)).toStringAsFixed(1)}w';
  } else if (seconds >= 86400) {
    return '${(seconds / 86400).toStringAsFixed(1)}d';
  } else if (seconds >= 3600) {
    return '${(seconds / 3600).toStringAsFixed(1)}h';
  }
  return '${seconds.toStringAsFixed(0)}s';
}
