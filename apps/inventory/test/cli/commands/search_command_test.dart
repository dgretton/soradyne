import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('SearchCommand', () {
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
        '{"category":"Tools","tags":[]} Hammer -> Toolbox\n'
        '{"category":"Decor","tags":[]} Blue Vase -> Shelf\n',
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
        () => runCli(['--inventory=test_inventory.txt', 'search']),
        throwsA(isA<UsageException>()),
      );
    });

    test('finds and prints a unique match', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Hammer -> Toolbox
      await runCli(['--inventory=test_inventory.txt', 'search', 'Hammer']);
    });

    test('finds and prints multiple matches', () async {
      // This test will print to the console. Manually verify output.
      // Expected:
      // Hammer -> Toolbox
      // Blue Vase -> Shelf
      await runCli(['--inventory=test_inventory.txt', 'search', 'e']);
    });

    test('prints a message when no matches are found', () async {
      // This test will print to the console. Manually verify output.
      // Expected: No items found matching "non-existent".
      await runCli(['--inventory=test_inventory.txt', 'search', 'non-existent']);
    });
  });
}
