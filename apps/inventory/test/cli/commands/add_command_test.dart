import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('AddCommand', () {
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
      // Create an empty file
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

    test('requires category, description, and location', () {
      expect(
        () => runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Hammer']),
        throwsA(isA<UsageException>()),
      );
    });

    test('can add item with basic parameters', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Added: Hammer -> Toolbox
      await runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Hammer', 'Toolbox']);
    });

    test('can add item with tags', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Added: Hammer -> Toolbox
      await runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Hammer', 'Toolbox', '-t', 'heavy,metal']);
    });

    test('can add item as container', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Added: Plastic Bin -> Garage
      await runCli(['--inventory=test_inventory.txt', 'add', 'Containers', 'Plastic Bin', 'Garage', '-c']);
    });

    test('rejects duplicate description (substring match)', () async {
      await runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Hammer', 'Toolbox']);

      expect(
        () => runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Hammer', 'Garage']),
        throwsA(isA<UsageException>()),
      );
    });

    test('rejects substring conflict (new is substring of existing)', () async {
      await runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Claw Hammer', 'Toolbox']);

      expect(
        () => runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Hammer', 'Garage']),
        throwsA(isA<UsageException>()),
      );
    });

    test('rejects substring conflict (existing is substring of new)', () async {
      await runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Hammer', 'Toolbox']);

      expect(
        () => runCli(['--inventory=test_inventory.txt', 'add', 'Tools', 'Claw Hammer', 'Garage']),
        throwsA(isA<UsageException>()),
      );
    });
  });
}
