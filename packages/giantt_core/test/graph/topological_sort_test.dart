import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

void main() {
  group('Topological Sort Tests', () {
    late GianttGraph graph;

    setUp(() {
      graph = GianttGraph();
    });

    group('Basic Sorting', () {
      test('should sort items with no dependencies', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);
        graph.addItem(itemC);

        final sorted = graph.topologicalSort();
        expect(sorted.length, equals(3));
        
        // Should be sorted by ID since no dependencies
        expect(sorted[0].id, equals('A'));
        expect(sorted[1].id, equals('B'));
        expect(sorted[2].id, equals('C'));
      });

      test('should sort items with simple dependencies', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);
        graph.addItem(itemC);

        // A requires B, B requires C
        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('B', RelationType.requires, 'C');

        final sorted = graph.topologicalSort();
        expect(sorted.length, equals(3));
        
        // C should come first (no dependencies), then B, then A
        expect(sorted[0].id, equals('C'));
        expect(sorted[1].id, equals('B'));
        expect(sorted[2].id, equals('A'));
      });

      test('should handle complex dependency chains', () {
        final items = ['A', 'B', 'C', 'D', 'E'].map((id) => 
          GianttItem(id: id, title: 'Task $id', duration: GianttDuration.parse('1d'))
        ).toList();

        for (final item in items) {
          graph.addItem(item);
        }

        // Create dependencies: A->B->C, A->D->E, C->E
        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('B', RelationType.requires, 'C');
        graph.addRelation('A', RelationType.requires, 'D');
        graph.addRelation('D', RelationType.requires, 'E');
        graph.addRelation('C', RelationType.requires, 'E');

        final sorted = graph.topologicalSort();
        expect(sorted.length, equals(5));

        // E should come first, then C and D, then B, then A
        expect(sorted[0].id, equals('E'));
        expect(sorted[4].id, equals('A')); // A should be last
        
        // Verify topological order is maintained
        final positions = <String, int>{};
        for (int i = 0; i < sorted.length; i++) {
          positions[sorted[i].id] = i;
        }

        // Check that dependencies come before dependents
        expect(positions['B']! < positions['A']!, isTrue);
        expect(positions['C']! < positions['B']!, isTrue);
        expect(positions['D']! < positions['A']!, isTrue);
        expect(positions['E']! < positions['D']!, isTrue);
        expect(positions['E']! < positions['C']!, isTrue);
      });
    });

    group('Dependency Depth Calculation', () {
      test('should calculate correct dependency depths', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);
        graph.addItem(itemC);

        // A requires B, B requires C
        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('B', RelationType.requires, 'C');

        final sorted = graph.topologicalSort();
        
        // Verify depth-based ordering
        expect(sorted[0].id, equals('C')); // Depth 0
        expect(sorted[1].id, equals('B')); // Depth 1
        expect(sorted[2].id, equals('A')); // Depth 2
      });

      test('should handle items with same depth deterministically', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));
        final itemD = GianttItem(id: 'D', title: 'Task D', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);
        graph.addItem(itemC);
        graph.addItem(itemD);

        // A and B both require C and D
        graph.addRelation('A', RelationType.requires, 'C');
        graph.addRelation('A', RelationType.requires, 'D');
        graph.addRelation('B', RelationType.requires, 'C');
        graph.addRelation('B', RelationType.requires, 'D');

        final sorted = graph.topologicalSort();
        
        // C and D should come first (sorted by ID), then A and B (sorted by ID)
        expect(sorted[0].id, equals('C'));
        expect(sorted[1].id, equals('D'));
        expect(sorted[2].id, equals('A'));
        expect(sorted[3].id, equals('B'));
      });
    });

    group('Cycle Detection', () {
      test('should detect and report simple cycles', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);

        graph.addRelation('A', RelationType.requires, 'B');

        expect(() => graph.addRelation('B', RelationType.requires, 'A'), 
               throwsA(isA<CycleDetectedException>()));
      });

      test('should detect and report complex cycles', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);
        graph.addItem(itemC);

        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('B', RelationType.requires, 'C');

        expect(() => graph.addRelation('C', RelationType.requires, 'A'), 
               throwsA(isA<CycleDetectedException>()));
      });

      test('should provide detailed cycle information', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);
        graph.addItem(itemC);

        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('B', RelationType.requires, 'C');

        try {
          graph.addRelation('C', RelationType.requires, 'A');
          fail('Expected CycleDetectedException');
        } catch (e) {
          expect(e, isA<CycleDetectedException>());
          final cycleException = e as CycleDetectedException;
          expect(cycleException.cycleItems, isNotEmpty);
          expect(cycleException.toString(), contains('Cycle detected'));
        }
      });
    });

    group('Insert Between', () {
      test('should insert item between two connected items', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);

        // A requires B
        graph.addRelation('A', RelationType.requires, 'B');

        // Insert C between A and B
        graph.insertBetween(itemC, 'B', 'A');

        // Verify the new structure: A requires C, C requires B
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        final updatedC = graph.items['C']!;

        expect(updatedA.relations['REQUIRES'], contains('C'));
        expect(updatedA.relations['REQUIRES'], isNot(contains('B')));
        expect(updatedC.relations['REQUIRES'], contains('B'));
        expect(updatedB.relations['BLOCKS'], contains('C'));
        expect(updatedB.relations['BLOCKS'], isNot(contains('A')));
      });

      test('should maintain topological order after insertion', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);

        graph.addRelation('A', RelationType.requires, 'B');
        graph.insertBetween(itemC, 'B', 'A');

        final sorted = graph.topologicalSort();
        final positions = <String, int>{};
        for (int i = 0; i < sorted.length; i++) {
          positions[sorted[i].id] = i;
        }

        // B should come before C, C should come before A
        expect(positions['B']! < positions['C']!, isTrue);
        expect(positions['C']! < positions['A']!, isTrue);
      });

      test('should throw when trying to insert between non-existent items', () {
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        expect(() => graph.insertBetween(itemC, 'A', 'B'), 
               throwsArgumentError);
      });
    });

    group('ANYOF Relations', () {
      test('should handle ANYOF relations in topological sort', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));
        final itemC = GianttItem(id: 'C', title: 'Task C', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);
        graph.addItem(itemC);

        // A requires any of B or C
        graph.addRelation('A', RelationType.anyof, 'B');
        graph.addRelation('A', RelationType.anyof, 'C');

        final sorted = graph.topologicalSort();
        final positions = <String, int>{};
        for (int i = 0; i < sorted.length; i++) {
          positions[sorted[i].id] = i;
        }

        // Both B and C should come before A
        expect(positions['B']! < positions['A']!, isTrue);
        expect(positions['C']! < positions['A']!, isTrue);
      });

      test('should detect cycles in ANYOF relations', () {
        final itemA = GianttItem(id: 'A', title: 'Task A', duration: GianttDuration.parse('1d'));
        final itemB = GianttItem(id: 'B', title: 'Task B', duration: GianttDuration.parse('1d'));

        graph.addItem(itemA);
        graph.addItem(itemB);

        graph.addRelation('A', RelationType.anyof, 'B');

        expect(() => graph.addRelation('B', RelationType.anyof, 'A'), 
               throwsA(isA<CycleDetectedException>()));
      });
    });
  });
}
