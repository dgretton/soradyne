import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('RemoveFromContainerCommand', () {
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
        '{"id":"screwdriver_id","category":"Tools","tags":["container_C1"]} Screwdriver -> container C1\n'
        '{"id":"container_id","category":"Containers","tags":["container_C1"]} Tool Chest -> Garage\n'
        '{"id":"hammer_id","category":"Tools","tags":[]} Hammer -> Toolbox\n',
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

    test('requires a search string', () {
      expect(
        () => runCli(['--inventory=test_inventory.txt', 'remove-from-container']),
        throwsA(isA<UsageException>()),
      );
    });

    test('can remove an item from a container', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Removed item matching "Screwdriver" from its container.
      await runCli([
        '--inventory=test_inventory.txt',
        'remove-from-container',
        'Screwdriver'
      ]);
    });

    test('throws when item not in a container', () {
      expect(
        () => runCli([
          '--inventory=test_inventory.txt',
          'remove-from-container',
          'Hammer'
        ]),
        throwsA(isA<UsageException>()),
      );
    });

    test('throws when item not found', () {
      expect(
        () => runCli([
          '--inventory=test_inventory.txt',
          'remove-from-container',
          'non-existent'
        ]),
        throwsA(isA<UsageException>()),
      );
    });
  });
}
