import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('DeleteCommand', () {
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
        '{"category":"Decor","tags":[]} Vase -> Shelf\n',
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
        () => runCli(['--inventory=test_inventory.txt', 'delete']),
        throwsA(isA<UsageException>()),
      );
    });

    test('can delete an item', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Deleted item matching "Hammer".
      await runCli(['--inventory=test_inventory.txt', 'delete', 'Hammer']);
    });

    test('throws when item not found', () {
      expect(
        () =>
            runCli(['--inventory=test_inventory.txt', 'delete', 'non-existent']),
        throwsA(isA<UsageException>()),
      );
    });
  });
}
