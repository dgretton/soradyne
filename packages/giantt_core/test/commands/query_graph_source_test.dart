import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';
import 'package:giantt_core/src/commands/summary_command.dart';
import 'package:giantt_core/src/commands/load_command.dart';
import 'package:giantt_core/src/commands/deps_command.dart';
import 'package:giantt_core/src/commands/blocked_command.dart';
import 'package:giantt_core/src/graph/giantt_graph.dart';
import 'package:giantt_core/src/models/duration.dart';

/// Regression tests for query commands using a provided graph.
///
/// Previously, runSummary / runLoad / runDeps / runBlocked always loaded
/// from file via DualFileManager.loadGraph, ignoring any external graph.
/// This meant the CRDT flow graph was never used even when available.
void main() {
  late GianttGraph graph;

  setUp(() {
    graph = GianttGraph();
    graph.addItem(GianttItem(
      id: 'crdt_item_1',
      title: 'Item only in CRDT',
      status: GianttStatus.notStarted,
      priority: GianttPriority.high,
      duration: GianttDuration.parse('2d'),
      charts: ['TestChart'],
    ));
    graph.addItem(GianttItem(
      id: 'crdt_item_2',
      title: 'Second CRDT item',
      status: GianttStatus.notStarted,
      priority: GianttPriority.neutral,
      duration: GianttDuration.parse('1d'),
      charts: ['TestChart'],
      relations: {'REQUIRES': ['crdt_item_1']},
    ));
    // Add reverse relation for graph consistency
    final item1 = graph.items['crdt_item_1']!;
    graph.addItem(item1.copyWith(
      relations: {'BLOCKS': ['crdt_item_2']},
    ));
  });

  group('runSummary uses provided graph', () {
    test('should use injected graph instead of loading from file', () {
      final result = runSummary(
        '/nonexistent/include/items.txt',
        '/nonexistent/occlude/items.txt',
        graph: graph,
      );

      expect(result.totalItems, equals(2));
      expect(result.charts.any((c) => c.chartName == 'TestChart'), isTrue);
      final chart = result.charts.firstWhere((c) => c.chartName == 'TestChart');
      expect(chart.totalItems, equals(2));
    });
  });

  group('runLoad uses provided graph', () {
    test('should use injected graph instead of loading from file', () {
      final now = DateTime(2026, 3, 12);
      final result = runLoad(
        '/nonexistent/include/items.txt',
        '/nonexistent/occlude/items.txt',
        graph: graph,
        windowStart: now,
        windowEnd: now.add(const Duration(days: 30)),
        today: now,
      );

      // Items should be present as floating (no time constraints)
      expect(result.floating.length, equals(2));
    });
  });

  group('runDeps uses provided graph', () {
    test('should use injected graph instead of loading from file', () {
      final result = runDeps(
        '/nonexistent/include/items.txt',
        '/nonexistent/occlude/items.txt',
        'crdt_item_2',
        graph: graph,
      );

      expect(result.focalId, equals('crdt_item_2'));
      expect(result.upstream, isNotEmpty);
      expect(result.upstream.first.id, equals('crdt_item_1'));
    });
  });

  group('runBlocked uses provided graph', () {
    test('should use injected graph instead of loading from file', () {
      final result = runBlocked(
        '/nonexistent/include/items.txt',
        '/nonexistent/occlude/items.txt',
        graph: graph,
      );

      // crdt_item_2 requires crdt_item_1 (not finished) → implied blocked
      expect(result.totalBlocked, equals(1));
      expect(result.items.first.id, equals('crdt_item_2'));
    });
  });
}
