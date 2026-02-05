import 'package:test/test.dart';
import 'package:giantt_core/src/graph/cycle_detector.dart';
import 'package:giantt_core/src/models/giantt_item.dart';
import 'package:giantt_core/src/models/duration.dart';
import 'package:giantt_core/src/models/graph_exceptions.dart';

void main() {
  group('CycleDetector', () {
    GianttItem createItem(String id, {Map<String, List<String>>? relations}) {
      return GianttItem(
        id: id,
        title: 'Item $id',
        duration: GianttDuration.zero(),
        relations: relations ?? {},
      );
    }

    group('detectCycle', () {
      test('should detect no cycle in empty graph', () {
        final items = <String, GianttItem>{};
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isFalse);
        expect(result.cyclePath, isEmpty);
      });

      test('should detect no cycle in single node graph', () {
        final items = {
          'a': createItem('a'),
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isFalse);
      });

      test('should detect no cycle in linear chain', () {
        final items = {
          'a': createItem('a', relations: {'REQUIRES': ['b']}),
          'b': createItem('b', relations: {'REQUIRES': ['c']}),
          'c': createItem('c'),
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isFalse);
      });

      test('should detect simple two-node cycle', () {
        final items = {
          'a': createItem('a', relations: {'REQUIRES': ['b']}),
          'b': createItem('b', relations: {'REQUIRES': ['a']}),
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isTrue);
        expect(result.cyclePath, containsAll(['a', 'b']));
      });

      test('should detect self-loop cycle', () {
        final items = {
          'a': createItem('a', relations: {'REQUIRES': ['a']}),
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isTrue);
        expect(result.cyclePath, contains('a'));
      });

      test('should detect cycle in larger graph', () {
        final items = {
          'a': createItem('a', relations: {'REQUIRES': ['b']}),
          'b': createItem('b', relations: {'REQUIRES': ['c']}),
          'c': createItem('c', relations: {'REQUIRES': ['d']}),
          'd': createItem('d', relations: {'REQUIRES': ['b']}), // Creates cycle b -> c -> d -> b
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isTrue);
        // The cycle should include b, c, d
        expect(result.cyclePath.toSet(), containsAll(['b', 'c', 'd']));
      });

      test('should detect cycle through ANYOF relation', () {
        final items = {
          'a': createItem('a', relations: {'ANYOF': ['b']}),
          'b': createItem('b', relations: {'ANYOF': ['a']}),
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isTrue);
      });

      test('should not consider non-dependency relations for cycles', () {
        // SUPERCHARGES is not a dependency relation
        final items = {
          'a': createItem('a', relations: {'SUPERCHARGES': ['b']}),
          'b': createItem('b', relations: {'SUPERCHARGES': ['a']}),
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isFalse);
      });

      test('should handle disconnected components', () {
        final items = {
          'a': createItem('a', relations: {'REQUIRES': ['b']}),
          'b': createItem('b'),
          'c': createItem('c', relations: {'REQUIRES': ['d']}),
          'd': createItem('d', relations: {'REQUIRES': ['c']}), // Cycle in second component
        };
        final result = CycleDetector.detectCycle(items);

        expect(result.hasCycle, isTrue);
        expect(result.cyclePath.toSet(), containsAll(['c', 'd']));
      });
    });

    group('wouldCreateCycle', () {
      test('should detect that adding edge would create cycle', () {
        final adjList = {
          'a': <String>{'b'},
          'b': <String>{'c'},
          'c': <String>{},
        };

        // Adding c -> a would create cycle a -> b -> c -> a
        expect(CycleDetector.wouldCreateCycle(adjList, 'c', 'a'), isTrue);
      });

      test('should detect that adding edge would not create cycle', () {
        final adjList = {
          'a': <String>{'b'},
          'b': <String>{},
          'c': <String>{},
        };

        // Adding c -> b would not create a cycle
        expect(CycleDetector.wouldCreateCycle(adjList, 'c', 'b'), isFalse);
      });
    });

    group('hasPath', () {
      test('should find direct path', () {
        final adjList = {
          'a': <String>{'b'},
          'b': <String>{},
        };

        expect(CycleDetector.hasPath(adjList, 'a', 'b'), isTrue);
      });

      test('should find indirect path', () {
        final adjList = {
          'a': <String>{'b'},
          'b': <String>{'c'},
          'c': <String>{},
        };

        expect(CycleDetector.hasPath(adjList, 'a', 'c'), isTrue);
      });

      test('should return false when no path exists', () {
        final adjList = {
          'a': <String>{'b'},
          'b': <String>{},
          'c': <String>{},
        };

        expect(CycleDetector.hasPath(adjList, 'a', 'c'), isFalse);
      });

      test('should return true for same source and target', () {
        final adjList = {
          'a': <String>{},
        };

        expect(CycleDetector.hasPath(adjList, 'a', 'a'), isTrue);
      });
    });

    group('findAllCycles', () {
      test('should find no cycles in acyclic graph', () {
        final adjList = {
          'a': <String>{'b'},
          'b': <String>{'c'},
          'c': <String>{},
        };

        final cycles = CycleDetector.findAllCycles(adjList);
        expect(cycles, isEmpty);
      });

      test('should find single cycle', () {
        final adjList = {
          'a': <String>{'b'},
          'b': <String>{'a'},
        };

        final cycles = CycleDetector.findAllCycles(adjList);
        expect(cycles, isNotEmpty);
      });
    });

    group('formatCycleWithTitles', () {
      test('should format cycle with item titles', () {
        final items = {
          'task1': createItem('task1'),
          'task2': createItem('task2'),
        };

        final formatted = CycleDetector.formatCycleWithTitles(
          ['task1', 'task2', 'task1'],
          items,
        );

        expect(formatted, contains('task1'));
        expect(formatted, contains('task2'));
        expect(formatted, contains('Item task1'));
        expect(formatted, contains('->'));
      });
    });

    group('validateNoCycles', () {
      test('should not throw for acyclic graph', () {
        final items = {
          'a': createItem('a', relations: {'REQUIRES': ['b']}),
          'b': createItem('b'),
        };

        expect(
          () => CycleDetector.validateNoCycles(items),
          returnsNormally,
        );
      });

      test('should throw CycleDetectedException for cyclic graph', () {
        final items = {
          'a': createItem('a', relations: {'REQUIRES': ['b']}),
          'b': createItem('b', relations: {'REQUIRES': ['a']}),
        };

        expect(
          () => CycleDetector.validateNoCycles(items),
          throwsA(isA<CycleDetectedException>()),
        );
      });
    });
  });

  group('CycleDetectionResult', () {
    test('noCycle factory creates correct result', () {
      final result = CycleDetectionResult.noCycle();

      expect(result.hasCycle, isFalse);
      expect(result.cyclePath, isEmpty);
      expect(result.toString(), equals('No cycle detected'));
    });

    test('cycleFound factory creates correct result', () {
      final result = CycleDetectionResult.cycleFound(['a', 'b', 'a']);

      expect(result.hasCycle, isTrue);
      expect(result.cyclePath, equals(['a', 'b', 'a']));
      expect(result.toString(), contains('a -> b -> a'));
    });
  });
}
