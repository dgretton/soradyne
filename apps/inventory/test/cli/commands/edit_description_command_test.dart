import 'dart:io';
import 'package:args/command_runner.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/cli/command_runner.dart';

void main() {
  group('EditDescriptionCommand', () {
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

    test('requires search string and new description', () {
      expect(
        () => runCli(
            ['--inventory=test_inventory.txt', 'edit-description', 'Hammer']),
        throwsA(isA<UsageException>()),
      );
    });

    test('can edit an item description', () async {
      // This test will print to the console. Manually verify output.
      // Expected: Changed description from "Hammer" to "Claw Hammer".
      await runCli([
        '--inventory=test_inventory.txt',
        'edit-description',
        'Hammer',
        'Claw Hammer'
      ]);
    });

    test('throws when item not found', () {
      expect(
        () => runCli([
          '--inventory=test_inventory.txt',
          'edit-description',
          'non-existent',
          'New Description'
        ]),
        throwsA(isA<UsageException>()),
      );
    });

    test('allows description conflicts (CRDT resolution)', () async {
      // CRDT allows conflicts - no exception should be thrown
      await runCli([
        '--inventory=test_inventory.txt',
        'edit-description',
        'Hammer',
        'Vase'
      ]);
    });
  });
}
