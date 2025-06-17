import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

void main() {
  group('GianttParser Tests', () {
    group('Basic parsing', () {
      test('should parse simple item', () {
        const line = '○ task1 1d "Simple task" {} tag1,tag2';
        final item = GianttParser.fromString(line);
        
        expect(item.id, equals('task1'));
        expect(item.title, equals('Simple task'));
        expect(item.status, equals(GianttStatus.notStarted));
        expect(item.priority, equals(GianttPriority.neutral));
        expect(item.duration.toString(), equals('1d'));
        expect(item.charts, isEmpty);
        expect(item.tags, equals(['tag1', 'tag2']));
        expect(item.relations, isEmpty);
      });

      test('should parse item with priority', () {
        const line = '◑ task2!! 2w "High priority task" {"Chart1"} urgent';
        final item = GianttParser.fromString(line);
        
        expect(item.id, equals('task2'));
        expect(item.priority, equals(GianttPriority.high));
        expect(item.status, equals(GianttStatus.inProgress));
        expect(item.duration.toString(), equals('2w'));
        expect(item.charts, equals(['Chart1']));
        expect(item.tags, equals(['urgent']));
      });

      test('should parse item with all priority levels', () {
        final testCases = [
          ('task_lowest,,,', GianttPriority.lowest),
          ('task_low...', GianttPriority.low),
          ('task_neutral', GianttPriority.neutral),
          ('task_unsure?', GianttPriority.unsure),
          ('task_medium!', GianttPriority.medium),
          ('task_high!!', GianttPriority.high),
          ('task_critical!!!', GianttPriority.critical),
        ];

        for (final (idPriority, expectedPriority) in testCases) {
          final line = '○ $idPriority 1d "Test task" {}';
          final item = GianttParser.fromString(line);
          expect(item.priority, equals(expectedPriority));
        }
      });
    });

    group('Complex parsing', () {
      test('should parse item with relations', () {
        const line = '● task3 3d "Complex task" {"Chart1","Chart2"} tag1,tag2 >>> ⊢[dep1,dep2] ►[blocked1]';
        final item = GianttParser.fromString(line);
        
        expect(item.id, equals('task3'));
        expect(item.status, equals(GianttStatus.completed));
        expect(item.charts, equals(['Chart1', 'Chart2']));
        expect(item.relations['REQUIRES'], equals(['dep1', 'dep2']));
        expect(item.relations['BLOCKS'], equals(['blocked1']));
      });

      test('should parse item with time constraint', () {
        const line = '⊘ task4 1w "Blocked task" {} tag1 >>> ⊢[dep1] @@@ window(5d:2d,severe)';
        final item = GianttParser.fromString(line);
        
        expect(item.id, equals('task4'));
        expect(item.status, equals(GianttStatus.blocked));
        expect(item.timeConstraint, isNotNull);
        expect(item.timeConstraint!.type, equals(TimeConstraintType.window));
        expect(item.timeConstraint!.duration.toString(), equals('5d'));
        expect(item.timeConstraint!.gracePeriod?.toString(), equals('2d'));
      });

      test('should parse item with comments', () {
        const line = '○ task5 1d "Task with comments" {} tag1 # User comment ### Auto comment';
        final item = GianttParser.fromString(line);
        
        expect(item.id, equals('task5'));
        expect(item.userComment, equals('User comment'));
        expect(item.autoComment, equals('Auto comment'));
      });

      test('should parse item with JSON-escaped title', () {
        const line = r'○ task6 1d "Task with \"quotes\" and \n newlines" {} tag1';
        final item = GianttParser.fromString(line);
        
        expect(item.id, equals('task6'));
        expect(item.title, equals('Task with "quotes" and \n newlines'));
      });

      test('should parse compound duration', () {
        const line = '○ task7 6mo8d3.5s "Long task" {}';
        final item = GianttParser.fromString(line);
        
        expect(item.duration.parts.length, equals(3));
        expect(item.duration.parts[0].amount, equals(6.0));
        expect(item.duration.parts[0].unit, equals('mo'));
        expect(item.duration.parts[1].amount, equals(8.0));
        expect(item.duration.parts[1].unit, equals('d'));
        expect(item.duration.parts[2].amount, equals(3.5));
        expect(item.duration.parts[2].unit, equals('s'));
      });
    });

    group('Round-trip parsing', () {
      test('should maintain data integrity through parse -> toString -> parse', () {
        const originalLine = '◑ complex_task!! 2w3d "Complex \"task\" with everything" {"Chart1","Chart2"} urgent,important >>> ⊢[dep1,dep2] ►[blocked1] ≫[enhanced1] @@@ window(5d:2d,severe) # User note ### Auto note';
        
        final item = GianttParser.fromString(originalLine);
        final regeneratedLine = GianttParser.toString(item);
        final reparsedItem = GianttParser.fromString(regeneratedLine);
        
        expect(reparsedItem.id, equals(item.id));
        expect(reparsedItem.title, equals(item.title));
        expect(reparsedItem.status, equals(item.status));
        expect(reparsedItem.priority, equals(item.priority));
        expect(reparsedItem.duration, equals(item.duration));
        expect(reparsedItem.charts, equals(item.charts));
        expect(reparsedItem.tags, equals(item.tags));
        expect(reparsedItem.relations, equals(item.relations));
        expect(reparsedItem.userComment, equals(item.userComment));
        expect(reparsedItem.autoComment, equals(item.autoComment));
      });

      test('should handle empty charts and tags', () {
        const line = '○ simple_task 1d "Simple task" {}';
        final item = GianttParser.fromString(line);
        final regenerated = GianttParser.toString(item);
        final reparsed = GianttParser.fromString(regenerated);
        
        expect(reparsed.charts, isEmpty);
        expect(reparsed.tags, isEmpty);
        expect(reparsed.relations, isEmpty);
      });
    });

    group('Error handling', () {
      test('should throw on invalid pre-title format', () {
        const line = 'invalid format "Title" {}';
        expect(() => GianttParser.fromString(line), throwsA(isA<GianttParseException>()));
      });

      test('should throw on missing title quotes', () {
        const line = '○ task1 1d Title without quotes {}';
        expect(() => GianttParser.fromString(line), throwsA(isA<GianttParseException>()));
      });

      test('should throw on unbalanced quotes', () {
        const line = '○ task1 1d "Unbalanced quote {}';
        expect(() => GianttParser.fromString(line), throwsA(isA<GianttParseException>()));
      });

      test('should throw on invalid status symbol', () {
        const line = 'X task1 1d "Invalid status" {}';
        expect(() => GianttParser.fromString(line), throwsA(isA<ArgumentError>()));
      });

      test('should throw on invalid duration', () {
        const line = '○ task1 invalid_duration "Title" {}';
        expect(() => GianttParser.fromString(line), throwsA(isA<ArgumentError>()));
      });

      test('should handle empty and comment lines', () {
        expect(() => GianttParser.fromString(''), throwsA(isA<GianttParseException>()));
        expect(() => GianttParser.fromString('# This is a comment'), throwsA(isA<GianttParseException>()));
        expect(() => GianttParser.fromString('   '), throwsA(isA<GianttParseException>()));
      });
    });

    group('All relation types', () {
      test('should parse all relation types correctly', () {
        const line = '○ task1 1d "Task with all relations" {} >>> ⊢[req1] ⋲[any1] ≫[super1] ∴[ind1] ∪[tog1] ⊟[conf1] ►[block1] ≻[suff1]';
        final item = GianttParser.fromString(line);
        
        expect(item.relations['REQUIRES'], equals(['req1']));
        expect(item.relations['ANYOF'], equals(['any1']));
        expect(item.relations['SUPERCHARGES'], equals(['super1']));
        expect(item.relations['INDICATES'], equals(['ind1']));
        expect(item.relations['TOGETHER'], equals(['tog1']));
        expect(item.relations['CONFLICTS'], equals(['conf1']));
        expect(item.relations['BLOCKS'], equals(['block1']));
        expect(item.relations['SUFFICIENT'], equals(['suff1']));
      });
    });

    group('Time constraints', () {
      test('should parse all time constraint types', () {
        final testCases = [
          ('window(5d:2d,severe)', TimeConstraintType.window),
          ('due(2024-12-31:1d,warn)', TimeConstraintType.deadline),
          ('every(7d:1d,escalating,stack)', TimeConstraintType.recurring),
        ];

        for (final (constraintStr, expectedType) in testCases) {
          final line = '○ task1 1d "Test task" {} >>> @@@ $constraintStr';
          final item = GianttParser.fromString(line);
          expect(item.timeConstraint?.type, equals(expectedType));
        }
      });
    });
  });
}
