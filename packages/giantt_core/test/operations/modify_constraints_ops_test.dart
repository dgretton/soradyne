import 'package:test/test.dart';
import 'package:giantt_core/src/operations/giantt_operations.dart';
import 'package:giantt_core/src/models/giantt_item.dart';
import 'package:giantt_core/src/models/status.dart';
import 'package:giantt_core/src/models/priority.dart';
import 'package:giantt_core/src/models/duration.dart';
import 'package:giantt_core/src/models/time_constraint.dart';

/// Regression tests for constraint operations.
///
/// Previously, `_buildModifyOps` had no `case 'constraints'` branch,
/// so modifying constraints silently produced zero ops — the change
/// was applied in-memory but never persisted to the CRDT.
void main() {
  group('fromItem includes time constraints', () {
    test('should emit AddToSet ops for timeConstraints', () {
      final constraint = TimeConstraint.parse('due(2026-04-30,severe)')!;
      final item = GianttItem(
        id: 'task_1',
        title: 'Task with deadline',
        duration: GianttDuration.parse('1d'),
        timeConstraints: [constraint],
      );

      final ops = GianttOp.fromItem(item);
      final rawOps = ops.expand((op) => op.toOperations()).toList();

      // Find the timeConstraints AddToSet op
      final constraintOps = rawOps.where((op) =>
          op['AddToSet'] != null &&
          op['AddToSet']['set_name'] == 'timeConstraints');

      expect(constraintOps, hasLength(1));
      expect(constraintOps.first['AddToSet']['element'],
          equals('due(2026-04-30,severe)'));
    });

    test('should emit multiple AddToSet ops for multiple constraints', () {
      final c1 = TimeConstraint.parse('due(2026-04-30,severe)')!;
      final c2 = TimeConstraint.parse('window(5d:2d,warn)')!;
      final item = GianttItem(
        id: 'task_1',
        title: 'Multi-constraint task',
        duration: GianttDuration.parse('1d'),
        timeConstraints: [c1, c2],
      );

      final ops = GianttOp.fromItem(item);
      final rawOps = ops.expand((op) => op.toOperations()).toList();

      final constraintOps = rawOps.where((op) =>
          op['AddToSet'] != null &&
          op['AddToSet']['set_name'] == 'timeConstraints');

      expect(constraintOps, hasLength(2));
    });
  });

  group('addTimeConstraint convenience factory', () {
    test('should create AddToSet on timeConstraints set', () {
      final constraint = TimeConstraint.parse('due(2026-12-31,warn)')!;
      final op = GianttOp.addToSet('task_1', 'timeConstraints', constraint.toString());
      final operations = op.toOperations();

      expect(operations, hasLength(1));
      expect(operations[0]['AddToSet']['set_name'], 'timeConstraints');
      expect(operations[0]['AddToSet']['element'], contains('due('));
    });
  });
}
