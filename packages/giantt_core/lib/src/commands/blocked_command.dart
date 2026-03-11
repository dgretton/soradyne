import 'dart:convert';
import 'dart:io';
import '../query/giantt_query.dart';
import '../models/priority.dart';
import '../storage/dual_file_manager.dart';

/// Programmatic entry point for the `blocked` command.
BlockedResult runBlocked(
  String itemsPath,
  String occludeItemsPath, {
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
}) {
  final graph = DualFileManager.loadGraph(itemsPath, occludeItemsPath);
  return GianttQuery(graph).blocked(
    filter: QueryFilter(
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
    ),
  );
}

/// Handles the `giantt blocked` CLI subcommand.
void executeBlockedCommand({
  required String itemsPath,
  required String occludeItemsPath,
  List<String>? charts,
  List<String>? excludeCharts,
  GianttPriority? minPriority,
  bool jsonOutput = false,
}) {
  try {
    final result = runBlocked(
      itemsPath,
      occludeItemsPath,
      charts: charts,
      excludeCharts: excludeCharts,
      minPriority: minPriority,
    );

    if (jsonOutput) {
      print(jsonEncode({'ok': true, 'command': 'blocked', 'data': result.toJson()}));
      return;
    }

    // Human-readable output.
    if (result.totalBlocked == 0) {
      print('No blocked items.');
      return;
    }

    print('${result.totalBlocked} blocked item(s)'
        '  (${result.explicitCount} explicit, ${result.impliedCount} implied)');
    print('');

    for (final item in result.items) {
      final reasonLabel = item.blockedReason == 'explicit'
          ? '⊘ explicit'
          : '⊢ requires unfinished';
      print('  ${item.priority.padRight(8)}  ${item.id}  ${item.title}');
      print('    $reasonLabel');
      if (item.unfinishedRequires.isNotEmpty) {
        print('    waiting on: ${item.unfinishedRequires.join(', ')}');
      }
    }
  } catch (e) {
    if (jsonOutput) {
      print(jsonEncode({'ok': false, 'command': 'blocked', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
    exit(1);
  }
}
