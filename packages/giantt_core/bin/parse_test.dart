import 'dart:io';
import 'package:giantt_core/src/parser/giantt_parser.dart';
import 'package:giantt_core/src/graph/giantt_graph.dart';

void main(List<String> args) {
  final file = File(args[0]);
  final text = file.readAsStringSync();
  final graph = GianttGraph();
  int parsed = 0, failed = 0;

  for (final line in text.split('\n')) {
    final trimmed = line.trim();
    if (trimmed.isEmpty || trimmed.startsWith('#')) continue;
    try {
      final item = GianttParser.fromString(trimmed);
      graph.addItem(item);
      parsed++;
    } catch (e) {
      failed++;
      if (failed <= 5) stderr.writeln('FAIL: $e\n  line: $trimmed');
    }
  }

  print('Parsed: $parsed items, Failed: $failed lines');
  print('Graph items: ${graph.items.length}');
}
