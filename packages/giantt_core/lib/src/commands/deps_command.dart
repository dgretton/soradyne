import 'dart:convert';
import 'dart:io';
import '../query/giantt_query.dart';
import '../storage/dual_file_manager.dart';

/// Programmatic entry point for the `deps` command.
DepsResult runDeps(
  String itemsPath,
  String occludeItemsPath,
  String itemId, {
  int maxDepth = 20,
}) {
  final graph = DualFileManager.loadGraph(itemsPath, occludeItemsPath);
  return GianttQuery(graph).deps(itemId, maxDepth: maxDepth);
}

/// Handles the `giantt deps` CLI subcommand.
void executeDepsCommand({
  required String itemsPath,
  required String occludeItemsPath,
  required String itemId,
  int maxDepth = 20,
  bool upstreamOnly = false,
  bool downstreamOnly = false,
  bool jsonOutput = false,
}) {
  try {
    final result = runDeps(itemsPath, occludeItemsPath, itemId,
        maxDepth: maxDepth);

    if (jsonOutput) {
      // Apply upstream/downstream-only filtering at the output level.
      final data = result.toJson();
      if (upstreamOnly) {
        data['downstream'] = [];
        data['all_ids'] = [result.focalId, ...result.upstream.map((n) => n.id)];
      }
      if (downstreamOnly) {
        data['upstream'] = [];
        data['all_ids'] = [result.focalId, ...result.downstream.map((n) => n.id)];
      }
      print(jsonEncode({'ok': true, 'command': 'deps', 'data': data}));
      return;
    }

    // Human-readable output.
    print('Dependency chain for: ${result.focalTitle} (${result.focalId})');

    if (!downstreamOnly && result.upstream.isNotEmpty) {
      print('');
      print('Upstream (prerequisites):');
      for (final node in result.upstream) {
        final indent = '  ' * node.depth.abs();
        print('$indent${node.status.padRight(12)} ${node.id}  ${node.title}'
            '  [${node.priority}]');
      }
    }

    if (!upstreamOnly && result.downstream.isNotEmpty) {
      print('');
      print('Downstream (blocked by this):');
      for (final node in result.downstream) {
        final indent = '  ' * node.depth;
        print('$indent${node.status.padRight(12)} ${node.id}  ${node.title}'
            '  [${node.priority}]');
      }
    }

    if (result.upstream.isEmpty && result.downstream.isEmpty) {
      print('No dependencies found.');
    }
  } catch (e) {
    if (jsonOutput) {
      print(jsonEncode({'ok': false, 'command': 'deps', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
    exit(1);
  }
}
