import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

void main() {
  group('Relation System Tests', () {
    late GianttGraph graph;
    late GianttItem itemA;
    late GianttItem itemB;
    late GianttItem itemC;

    setUp(() {
      graph = GianttGraph();
      
      itemA = GianttItem(
        id: 'A',
        title: 'Task A',
        duration: GianttDuration.parse('1d'),
      );
      
      itemB = GianttItem(
        id: 'B', 
        title: 'Task B',
        duration: GianttDuration.parse('1d'),
      );
      
      itemC = GianttItem(
        id: 'C',
        title: 'Task C', 
        duration: GianttDuration.parse('1d'),
      );

      graph.addItem(itemA);
      graph.addItem(itemB);
      graph.addItem(itemC);
    });

    group('Bidirectional Relations', () {
      test('should create BLOCKS when adding REQUIRES', () {
        graph.addRelation('A', RelationType.requires, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['REQUIRES'], contains('B'));
        expect(updatedB.relations['BLOCKS'], contains('A'));
      });

      test('should create REQUIRES when adding BLOCKS', () {
        graph.addRelation('A', RelationType.blocks, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['BLOCKS'], contains('B'));
        expect(updatedB.relations['REQUIRES'], contains('A'));
      });

      test('should create SUFFICIENT when adding ANYOF', () {
        graph.addRelation('A', RelationType.anyof, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['ANYOF'], contains('B'));
        expect(updatedB.relations['SUFFICIENT'], contains('A'));
      });

      test('should create ANYOF when adding SUFFICIENT', () {
        graph.addRelation('A', RelationType.sufficient, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['SUFFICIENT'], contains('B'));
        expect(updatedB.relations['ANYOF'], contains('A'));
      });

      test('should create symmetric relations for CONFLICTS', () {
        graph.addRelation('A', RelationType.conflicts, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['CONFLICTS'], contains('B'));
        expect(updatedB.relations['CONFLICTS'], contains('A'));
      });

      test('should create symmetric relations for TOGETHER', () {
        graph.addRelation('A', RelationType.together, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['TOGETHER'], contains('B'));
        expect(updatedB.relations['TOGETHER'], contains('A'));
      });
    });

    group('Relation Removal', () {
      test('should remove both directions when removing REQUIRES', () {
        graph.addRelation('A', RelationType.requires, 'B');
        graph.removeRelation('A', RelationType.requires, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['REQUIRES'] ?? [], isEmpty);
        expect(updatedB.relations['BLOCKS'] ?? [], isEmpty);
      });

      test('should remove both directions when removing ANYOF', () {
        graph.addRelation('A', RelationType.anyof, 'B');
        graph.removeRelation('A', RelationType.anyof, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['ANYOF'] ?? [], isEmpty);
        expect(updatedB.relations['SUFFICIENT'] ?? [], isEmpty);
      });
    });

    group('Cycle Detection', () {
      test('should detect simple cycle in REQUIRES relations', () {
        graph.addRelation('A', RelationType.requires, 'B');
        
        expect(
          () => graph.addRelation('B', RelationType.requires, 'A'),
          throwsA(isA<CycleDetectedException>()),
        );
      });

      test('should detect complex cycle in REQUIRES relations', () {
        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('B', RelationType.requires, 'C');
        
        expect(
          () => graph.addRelation('C', RelationType.requires, 'A'),
          throwsA(isA<CycleDetectedException>()),
        );
      });

      test('should detect cycle in ANYOF relations', () {
        graph.addRelation('A', RelationType.anyof, 'B');
        
        expect(
          () => graph.addRelation('B', RelationType.anyof, 'A'),
          throwsA(isA<CycleDetectedException>()),
        );
      });

      test('should allow non-strict relations without cycle detection', () {
        // These should not cause cycle detection errors
        expect(() {
          graph.addRelation('A', RelationType.supercharges, 'B');
          graph.addRelation('B', RelationType.supercharges, 'A');
        }, returnsNormally);

        expect(() {
          graph.addRelation('A', RelationType.indicates, 'B');
          graph.addRelation('B', RelationType.indicates, 'A');
        }, returnsNormally);
      });
    });

    group('Error Handling', () {
      test('should throw when adding relation to non-existent item', () {
        expect(
          () => graph.addRelation('A', RelationType.requires, 'NONEXISTENT'),
          throwsArgumentError,
        );
      });

      test('should throw when adding relation from non-existent item', () {
        expect(
          () => graph.addRelation('NONEXISTENT', RelationType.requires, 'B'),
          throwsArgumentError,
        );
      });

      test('should handle removing non-existent relations gracefully', () {
        expect(
          () => graph.removeRelation('A', RelationType.requires, 'B'),
          returnsNormally,
        );
      });
    });

    group('Complex Scenarios', () {
      test('should handle multiple relations between same items', () {
        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('A', RelationType.supercharges, 'B');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        
        expect(updatedA.relations['REQUIRES'], contains('B'));
        expect(updatedA.relations['SUPERCHARGES'], contains('B'));
        expect(updatedB.relations['BLOCKS'], contains('A'));
        expect(updatedB.relations['SUPERCHARGES'], contains('A'));
      });

      test('should prevent duplicate relations', () {
        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('A', RelationType.requires, 'B'); // Duplicate
        
        final updatedA = graph.items['A']!;
        expect(updatedA.relations['REQUIRES']!.where((id) => id == 'B').length, equals(1));
      });

      test('should handle chain of dependencies', () {
        graph.addRelation('A', RelationType.requires, 'B');
        graph.addRelation('B', RelationType.requires, 'C');
        
        final updatedA = graph.items['A']!;
        final updatedB = graph.items['B']!;
        final updatedC = graph.items['C']!;
        
        expect(updatedA.relations['REQUIRES'], contains('B'));
        expect(updatedB.relations['REQUIRES'], contains('C'));
        expect(updatedB.relations['BLOCKS'], contains('A'));
        expect(updatedC.relations['BLOCKS'], contains('B'));
      });
    });

    group('Graph Operations', () {
      test('should find items by substring', () {
        final found = graph.findBySubstring('Task A');
        expect(found.id, equals('A'));
      });

      test('should find items by exact ID', () {
        final found = graph.findBySubstring('B');
        expect(found.id, equals('B'));
      });

      test('should throw when no items match substring', () {
        expect(
          () => graph.findBySubstring('Nonexistent'),
          throwsArgumentError,
        );
      });

      test('should throw when multiple items match substring', () {
        final itemD = GianttItem(
          id: 'D',
          title: 'Task D',
          duration: GianttDuration.parse('1d'),
        );
        graph.addItem(itemD);
        
        expect(
          () => graph.findBySubstring('Task'),
          throwsArgumentError,
        );
      });
    });

    group('Occlusion', () {
      test('should separate included and occluded items', () {
        final occludedItem = itemA.copyWith(occlude: true);
        graph.removeItem('A');
        graph.addItem(occludedItem);
        
        expect(graph.includedItems.keys, containsAll(['B', 'C']));
        expect(graph.includedItems.keys, isNot(contains('A')));
        expect(graph.occludedItems.keys, contains('A'));
      });
    });
  });
}
