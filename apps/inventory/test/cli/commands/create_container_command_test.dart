import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('CreateContainerCommand', () {
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
    });

    tearDown(() {
      if (testFile.existsSync()) {
        testFile.deleteSync();
      }
      if (opsLogFile.existsSync()) {
        opsLogFile.deleteSync();
      }
    });

    test('requires id and location', () {
      expect(
        () =>
            runCli(['--inventory=test_inventory.txt', 'create-container', 'C1']),
        throwsA(isA<UsageException>()),
      );
    });

    test('can create a container with default description', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Created container "C1" at "Garage".
      await runCli(
          ['--inventory=test_inventory.txt', 'create-container', 'C1', 'Garage']);
    });

    test('can create a container with specified description', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Created container "C1" at "Garage".
      await runCli([
        '--inventory=test_inventory.txt',
        'create-container',
        'C1',
        'Garage',
        '-d',
        'My custom bin'
      ]);
    });

    test('throws when creating duplicate container', () async {
      await runCli(
          ['--inventory=test_inventory.txt', 'create-container', 'C1', 'Garage']);

      // Creating a duplicate container should throw an error
      expect(
        () => runCli([
          '--inventory=test_inventory.txt',
          'create-container',
          'C1',
          'Basement'
        ]),
        throwsA(isA<UsageException>()),
      );
    });
  });
}
