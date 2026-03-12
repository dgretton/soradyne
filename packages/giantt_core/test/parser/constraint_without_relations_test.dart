import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

/// Regression tests for parsing time constraints without relations.
///
/// Previously, the parser only looked for @@@ after >>>, so items
/// with constraints but no relations had the constraint string
/// incorrectly absorbed into tags.
void main() {
  group('Parser: constraints without relations (@@@ without >>>)', () {
    test('should parse deadline constraint when no relations present', () {
      const line =
          '○ task1 1d "Task with deadline only" {MyChart} tag1,tag2 @@@ due(2026-04-30,severe)';
      final item = GianttParser.fromString(line);

      expect(item.tags, equals(['tag1', 'tag2']));
      expect(item.timeConstraints, hasLength(1));
      expect(item.timeConstraints.first.type, equals(TimeConstraintType.deadline));
      expect(item.timeConstraints.first.dueDate, equals('2026-04-30'));
      expect(item.timeConstraints.first.consequenceType, equals(ConsequenceType.severe));
      expect(item.relations, isEmpty);
    });

    test('should not leak constraint text into tags', () {
      const line =
          '○ task1 1d "Deadline task" {} decision,lily,relationship @@@ due(2026-04-30,severe)';
      final item = GianttParser.fromString(line);

      expect(item.tags, equals(['decision', 'lily', 'relationship']));
      expect(item.tags, isNot(contains(contains('@@@'))));
      expect(item.tags, isNot(contains(contains('severe'))));
      expect(item.timeConstraints, hasLength(1));
    });

    test('should parse window constraint when no relations present', () {
      const line = '○ task1 2w "Windowed task" {} planning @@@ window(5d:2d,warn)';
      final item = GianttParser.fromString(line);

      expect(item.tags, equals(['planning']));
      expect(item.timeConstraints, hasLength(1));
      expect(item.timeConstraints.first.type, equals(TimeConstraintType.window));
    });

    test('should parse recurring constraint when no relations present', () {
      const line = '○ task1 1d "Recurring task" {} @@@ every(7d:1d,escalating,stack)';
      final item = GianttParser.fromString(line);

      expect(item.tags, isEmpty);
      expect(item.timeConstraints, hasLength(1));
      expect(item.timeConstraints.first.type, equals(TimeConstraintType.recurring));
    });

    test('should still parse constraint after relations normally', () {
      const line =
          '○ task1 1d "Both" {} tag1 >>> ⊢[dep1] @@@ due(2026-12-31,warn)';
      final item = GianttParser.fromString(line);

      expect(item.tags, equals(['tag1']));
      expect(item.relations['REQUIRES'], equals(['dep1']));
      expect(item.timeConstraints, hasLength(1));
      expect(item.timeConstraints.first.type, equals(TimeConstraintType.deadline));
    });

    test('round-trip preserves constraint without relations', () {
      const line =
          '○ task1 1d "Deadline only" {Chart} tag1 @@@ due(2026-06-30,severe)';
      final item = GianttParser.fromString(line);
      final regenerated = GianttParser.itemToString(item);
      final reparsed = GianttParser.fromString(regenerated);

      expect(reparsed.tags, equals(item.tags));
      expect(reparsed.timeConstraints.length, equals(item.timeConstraints.length));
      expect(reparsed.timeConstraints.first.type,
          equals(item.timeConstraints.first.type));
      expect(reparsed.timeConstraints.first.dueDate,
          equals(item.timeConstraints.first.dueDate));
      expect(reparsed.relations, isEmpty);
    });
  });
}
