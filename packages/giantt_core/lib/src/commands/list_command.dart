import 'dart:convert';
import 'dart:io';
import '../query/giantt_query.dart';
import '../models/priority.dart';
import '../models/status.dart';
import '../graph/giantt_graph.dart';
import '../storage/dual_file_manager.dart';

/// Programmatic entry point for the `list` command.
ListResult runList(
  String itemsPath,
  String occludeItemsPath, {
  GianttGraph? graph,
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
  List<GianttStatus>? statuses,
  List<String>? tags,
  bool excludeOccluded = false,
}) {
  graph ??= DualFileManager.loadGraph(itemsPath, occludeItemsPath);
  return GianttQuery(graph).list(
    filter: QueryFilter(
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
    ),
    statuses: statuses,
    tags: tags,
    excludeOccluded: excludeOccluded,
  );
}

/// Handles the `giantt list` CLI subcommand.
void executeListCommand({
  required String itemsPath,
  required String occludeItemsPath,
  GianttGraph? graph,
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
  List<GianttStatus>? statuses,
  List<String>? tags,
  bool excludeOccluded = false,
  bool jsonOutput = false,
}) {
  try {
    final result = runList(
      itemsPath,
      occludeItemsPath,
      graph: graph,
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
      statuses: statuses,
      tags: tags,
      excludeOccluded: excludeOccluded,
    );

    if (jsonOutput) {
      print(jsonEncode({'ok': true, 'command': 'list', 'data': result.toJson()}));
      return;
    }

    if (result.totalItems == 0) {
      print('No items match the given filters.');
      return;
    }

    print('${result.totalItems} item(s)');
    print('');
    for (final item in result.items) {
      final occludedMark = item.occluded ? ' [occluded]' : '';
      final charts = item.charts.isNotEmpty ? '  [${item.charts.join(', ')}]' : '';
      print('  ${item.priority.padRight(8)}  ${item.status.padRight(12)}  ${item.id}$occludedMark');
      print('    ${item.title}$charts');
    }
  } catch (e) {
    if (jsonOutput) {
      print(jsonEncode({'ok': false, 'command': 'list', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
    exit(1);
  }
}
