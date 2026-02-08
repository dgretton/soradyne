import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('PutInCommand', () {
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
        '{"category":"Tools","tags":[]} Screwdriver -> Toolbox\n'
        '{"category":"Containers","tags":["container_Tool Chest"]} Tool Chest -> Garage\n',
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

    test('requires container ID and search string', () {
      expect(
        () => runCli(['--inventory=test_inventory.txt', 'put-in', 'Tool Chest']),
        throwsA(isA<UsageException>()),
      );
    });

    test('can put an item in a container', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Put "Screwdriver" in container "Tool Chest".
      await runCli([
        '--inventory=test_inventory.txt',
        'put-in',
        'Tool Chest',
        'Screwdriver'
      ]);
    });

    test('throws when item not found', () {
      expect(
        () => runCli([
          '--inventory=test_inventory.txt',
          'put-in',
          'Tool Chest',
          'non-existent'
        ]),
        throwsA(isA<UsageException>()),
      );
    });

    test('throws when container not found', () {
      expect(
        () => runCli([
          '--inventory=test_inventory.txt',
          'put-in',
          'non-existent-chest',
          'Screwdriver'
        ]),
        throwsA(isA<UsageException>()),
      );
    });
  });
}
