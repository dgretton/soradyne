/// One-shot import: reads items.txt and writes all items into the flow.
///
/// Usage: dart run bin/import_items.dart <items_file> [flow_workspace]
import 'dart:convert';
import 'dart:io';
import 'package:giantt_core/src/storage/flow_repository.dart';
import 'package:giantt_core/src/ffi/flow_client.dart';
import 'package:giantt_core/src/operations/giantt_operations.dart';
import 'package:giantt_core/src/parser/giantt_parser.dart';
import 'package:giantt_core/src/graph/giantt_graph.dart';

void main(List<String> args) {
  if (args.isEmpty) {
    stderr.writeln('Usage: dart run bin/import_items.dart <items.txt> [workspace_path]');
    exit(1);
  }

  final itemsFile = args[0];
  final workspace = args.length > 1 ? args[1] : FlowRepository.getDefaultWorkspacePath();

  final deviceId = Platform.localHostname;
  if (!FlowRepository.initializeIfAvailable(deviceId)) {
    stderr.writeln('Error: soradyne native library not available.');
    exit(1);
  }

  final flowId = FlowRepository.getOrCreateFlowId(workspace);
  print('Flow: $flowId');

  // Parse items from file
  final text = File(itemsFile).readAsStringSync();
  final graph = GianttGraph();
  for (final line in text.split('\n')) {
    final trimmed = line.trim();
    if (trimmed.isEmpty || trimmed.startsWith('#')) continue;
    try {
      final item = GianttParser.fromString(trimmed);
      graph.addItem(item);
    } catch (e) {
      stderr.writeln('Skipping: $e');
    }
  }
  print('Parsed ${graph.items.length} items');

  // Write each item's ops, then flush via get/apply-remote round-trip
  final client = FlowClient.open(flowId);
  int opCount = 0;
  try {
    for (final item in graph.items.values) {
      final ops = GianttOp.fromItem(item);
      for (final op in ops) {
        for (final rawOp in op.toOperations()) {
          client.writeOperation(rawOp);
          opCount++;
        }
      }
    }
    print('Wrote $opCount raw ops');

    // Force flush: read all ops back and apply them to trigger a clean persist
    final opsJson = client.getOperationsJson();
    client.applyRemoteOperations(opsJson);

    final drip = client.readDrip();
    final itemLines = drip.split('\n').where((l) {
      final t = l.trim();
      return t.isNotEmpty && !t.startsWith('#');
    }).length;
    print('Materialized items: $itemLines');
  } finally {
    client.close();
  }

  FlowRepository.cleanup();
  print('Done.');
}
