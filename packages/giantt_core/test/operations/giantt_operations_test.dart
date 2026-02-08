import 'package:test/test.dart';
import 'package:giantt_core/src/operations/giantt_operations.dart';
import 'package:giantt_core/src/models/giantt_item.dart';
import 'package:giantt_core/src/models/status.dart';
import 'package:giantt_core/src/models/priority.dart';
import 'package:giantt_core/src/models/duration.dart';

void main() {
  group('GianttOp', () {
    group('AddItemOp', () {
      test('creates AddItem operation', () {
        final op = GianttOp.addItem('task_1');
        final operations = op.toOperations();

        expect(operations.length, 1);
        expect(operations[0], {
          'AddItem': {
            'item_id': 'task_1',
            'item_type': 'GianttItem',
          },
        });
      });
    });

    group('SetFieldOp', () {
      test('creates SetField operation with string value', () {
        final op = GianttOp.setField('task_1', 'title', 'My Task');
        final operations = op.toOperations();

        expect(operations.length, 1);
        expect(operations[0], {
          'SetField': {
            'item_id': 'task_1',
            'field': 'title',
            'value': {'String': 'My Task'},
          },
        });
      });

      test('creates SetField operation with int value', () {
        final op = GianttOp.setField('task_1', 'count', 42);
        final operations = op.toOperations();

        expect(operations[0]['SetField']['value'], {'Int': 42});
      });

      test('creates SetField operation with bool value', () {
        final op = GianttOp.setField('task_1', 'active', true);
        final operations = op.toOperations();

        expect(operations[0]['SetField']['value'], {'Bool': true});
      });
    });

    group('AddToSetOp', () {
      test('creates AddToSet operation', () {
        final op = GianttOp.addToSet('task_1', 'tags', 'important');
        final operations = op.toOperations();

        expect(operations.length, 1);
        expect(operations[0], {
          'AddToSet': {
            'item_id': 'task_1',
            'set_name': 'tags',
            'element': {'String': 'important'},
          },
        });
      });
    });

    group('RemoveFromSetOp', () {
      test('creates RemoveFromSet operation with observed IDs', () {
        final op = GianttOp.removeFromSet(
          'task_1',
          'tags',
          'old-tag',
          ['uuid-1', 'uuid-2'],
        );
        final operations = op.toOperations();

        expect(operations.length, 1);
        expect(operations[0], {
          'RemoveFromSet': {
            'item_id': 'task_1',
            'set_name': 'tags',
            'element': {'String': 'old-tag'},
            'observed_add_ids': ['uuid-1', 'uuid-2'],
          },
        });
      });
    });

    group('RemoveItemOp', () {
      test('creates RemoveItem operation', () {
        final op = GianttOp.removeItem('task_1');
        final operations = op.toOperations();

        expect(operations.length, 1);
        expect(operations[0], {
          'RemoveItem': {
            'item_id': 'task_1',
          },
        });
      });
    });

    group('convenience factories', () {
      test('setTitle creates SetField for title', () {
        final op = GianttOp.setTitle('task_1', 'New Title');
        final operations = op.toOperations();

        expect(operations[0]['SetField']['field'], 'title');
        expect(operations[0]['SetField']['value'], {'String': 'New Title'});
      });

      test('setStatus creates SetField for status', () {
        final op = GianttOp.setStatus('task_1', GianttStatus.completed);
        final operations = op.toOperations();

        expect(operations[0]['SetField']['field'], 'status');
        expect(operations[0]['SetField']['value'], {'String': 'COMPLETED'});
      });

      test('setPriority creates SetField for priority', () {
        final op = GianttOp.setPriority('task_1', GianttPriority.high);
        final operations = op.toOperations();

        expect(operations[0]['SetField']['field'], 'priority');
        expect(operations[0]['SetField']['value'], {'String': 'HIGH'});
      });

      test('setDuration creates SetField for duration', () {
        final op = GianttOp.setDuration('task_1', GianttDuration.parse('2d'));
        final operations = op.toOperations();

        expect(operations[0]['SetField']['field'], 'duration');
        expect(operations[0]['SetField']['value'], {'String': '2d'});
      });

      test('addTag creates AddToSet for tags', () {
        final op = GianttOp.addTag('task_1', 'urgent');
        final operations = op.toOperations();

        expect(operations[0]['AddToSet']['set_name'], 'tags');
        expect(operations[0]['AddToSet']['element'], {'String': 'urgent'});
      });

      test('addChart creates AddToSet for charts', () {
        final op = GianttOp.addChart('task_1', 'Sprint1');
        final operations = op.toOperations();

        expect(operations[0]['AddToSet']['set_name'], 'charts');
        expect(operations[0]['AddToSet']['element'], {'String': 'Sprint1'});
      });

      test('addRequires creates AddToSet for requires', () {
        final op = GianttOp.addRequires('task_1', 'task_2');
        final operations = op.toOperations();

        expect(operations[0]['AddToSet']['set_name'], 'requires');
        expect(operations[0]['AddToSet']['element'], {'String': 'task_2'});
      });
    });

    group('fromItem', () {
      test('converts GianttItem to operations', () {
        final item = GianttItem(
          id: 'task_1',
          title: 'My Task',
          status: GianttStatus.inProgress,
          priority: GianttPriority.high,
          duration: GianttDuration.parse('2d'),
          tags: ['important', 'backend'],
          charts: ['Sprint1'],
          relations: {
            'REQUIRES': ['dep1', 'dep2'],
          },
          userComment: 'This is a note',
        );

        final ops = GianttOp.fromItem(item);

        // Should have: AddItem + 5 SetFields + 2 tags + 1 chart + 2 requires = 11 ops
        expect(ops.length, greaterThanOrEqualTo(11));

        // First op should be AddItem
        final firstOp = ops[0].toOperations()[0];
        expect(firstOp['AddItem'], isNotNull);
        expect(firstOp['AddItem']['item_id'], 'task_1');
      });

      test('handles empty item with defaults', () {
        final item = GianttItem(
          id: 'minimal',
          title: 'Minimal',
          duration: GianttDuration.parse('1d'),
        );

        final ops = GianttOp.fromItem(item);

        // Should have: AddItem + 4 SetFields (title, status, priority, duration)
        expect(ops.length, greaterThanOrEqualTo(5));
      });
    });
  });
}
