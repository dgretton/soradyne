import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('MoveCommand', () {
    late File testFile;
    late File opsLogFile;

    setUp(() {
      testFile = File('test_inventory.txt');
      opsLogFile = File('test_inventory_ops.jsonl');
      if (testFile.existsSync()) {
        testFile.deleteSync();
      }
      if (opsLogFile.existsSync()) {
        opsLogFile.deleteSync();
      }
      testFile.createSync();
      testFile.writeAsStringSync(
        '{"category":"Tools","tags":[]} Hammer -> Toolbox\n',
      );
    });

    tearDown(() {
      if (testFile.existsSync()) {
        testFile.deleteSync();
      }
      if (opsLogFile.existsSync()) {
        opsLogFile.deleteSync();
      }
    });

    test('requires search string and new location', () {
      expect(
        () => runCli(['--inventory=test_inventory.txt', 'move', 'Hammer']),
        throwsA(isA<UsageException>()),
      );
    });

    test('can move an item', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Moved item matching "Hammer" to "Garage".
      await runCli(
          ['--inventory=test_inventory.txt', 'move', 'Hammer', 'Garage']);
    });

    test('throws when item not found', () {
      expect(
        () => runCli([
          '--inventory=test_inventory.txt',
          'move',
          'non-existent',
          'Garage'
        ]),
        throwsA(isA<UsageException>()),
      );
    });
  });
}
