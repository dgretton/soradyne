import 'dart:io';
import 'package:test/test.dart';

/// Integration tests for CLI commands based on real-world usage patterns
void main() {
  group('CLI Integration Tests', () {
    late Directory tempDir;
    late String itemsPath;
    late String occludeItemsPath;
    late String logsPath;
    late String occludeLogsPath;

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('giantt_cli_test_');
      final includeDir = Directory('${tempDir.path}/include');
      final occludeDir = Directory('${tempDir.path}/occlude');
      await includeDir.create(recursive: true);
      await occludeDir.create(recursive: true);

      itemsPath = '${includeDir.path}/items.txt';
      occludeItemsPath = '${occludeDir.path}/items.txt';
      logsPath = '${includeDir.path}/logs.jsonl';
      occludeLogsPath = '${occludeDir.path}/logs.jsonl';

      // Initialize files
      await File(itemsPath).writeAsString(_itemsHeader);
      await File(occludeItemsPath).writeAsString(_occludeHeader);
      await File(logsPath).writeAsString('');
      await File(occludeLogsPath).writeAsString('');
    });

    tearDown(() async {
      await tempDir.delete(recursive: true);
    });

    Future<ProcessResult> runGiantt(List<String> args) async {
      return Process.run(
        'dart',
        ['run', 'bin/giantt.dart', ...args],
        workingDirectory: '.',
      );
    }

    group('Real-world usage patterns', () {
      test('log command with session and tags', () async {
        final result = await runGiantt([
          'log',
          '--file', logsPath,
          '--occlude-file', occludeLogsPath,
          'sentinel0',
          'Setting up Sentinel Bio consulting task structure',
          '--tags', 'sentinel,consulting,planning',
        ]);

        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');
        expect(result.stdout, contains('Log entry created'));

        // Verify log was written
        final logContent = await File(logsPath).readAsString();
        expect(logContent, contains('sentinel0'));
        expect(logContent, contains('Setting up Sentinel Bio'));
      });

      test('add command with charts, tags, duration, and requires', () async {
        // First add a prerequisite item
        var result = await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'sentinel_ben_access',
          'Get Ben access',
          '--duration', '1h',
        ]);
        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');

        // Now add the real item with requires
        result = await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'sentinel_read_jolien',
          'Read Jolien long doc spreadsheet and LLM transcripts on dolphins',
          '--charts', 'Sentinel Bio',
          '--tags', 'sentinel,sentinel0,research,dolphin,reading',
          '--duration', '3h',
          '--priority', 'MEDIUM',
          '--requires', 'sentinel_ben_access',
        ]);

        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');
        expect(result.stdout, contains('Successfully added'));

        // Verify item was written correctly
        final content = await File(itemsPath).readAsString();
        expect(content, contains('sentinel_read_jolien'));
        expect(content, contains('Sentinel Bio'));
        expect(content, contains('3h'));
      });

      test('add command with 30min duration', () async {
        final result = await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'quick_task',
          'Quick task',
          '--duration', '30min',
        ]);

        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');
      });

      test('set-status command', () async {
        // First add an item
        await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'test_task',
          'Test task',
        ]);

        // Set status to COMPLETED
        final result = await runGiantt([
          'set-status',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'test_task',
          'COMPLETED',
        ]);

        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');
        expect(result.stdout, contains('COMPLETED'));

        // Verify status changed
        final content = await File(itemsPath).readAsString();
        expect(content, contains('●')); // COMPLETED symbol
      });

      test('remove command', () async {
        // Add an item
        await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'to_remove',
          'Item to remove',
        ]);

        // Remove it
        final result = await runGiantt([
          'remove',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'to_remove',
        ]);

        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');
        expect(result.stdout, contains('Successfully removed'));

        // Verify item was removed
        final content = await File(itemsPath).readAsString();
        expect(content, isNot(contains('to_remove')));
      });

      test('modify command with --remove flag', () async {
        // Add two items with a dependency
        await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'item_a',
          'Item A',
        ]);
        await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'item_b',
          'Item B',
          '--requires', 'item_a',
        ]);

        // Remove the dependency using modify --remove
        final result = await runGiantt([
          'modify',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          '--remove',
          'item_b',
          'requires',
          'item_a',
        ]);

        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');

        // Verify dependency was removed
        final content = await File(itemsPath).readAsString();
        // item_b should exist but not have a REQUIRES relation to item_a
        expect(content, contains('item_b'));
        // The REQUIRES symbol should not appear for item_b's line
        final lines = content.split('\n');
        final itemBLine = lines.firstWhere((l) => l.contains('item_b'), orElse: () => '');
        expect(itemBLine, isNot(contains('⊢'))); // REQUIRES symbol
      });

      test('doctor check command', () async {
        // Add some items
        await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'healthy_item',
          'Healthy item',
        ]);

        final result = await runGiantt([
          'doctor',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'check',
        ]);

        // Should succeed (exit code 0) for healthy graph
        expect(result.exitCode, equals(0), reason: 'stderr: ${result.stderr}');
      });

      test('remove command refuses without --force when item has dependents', () async {
        // Add items with dependency
        await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'base_item',
          'Base item',
        ]);
        await runGiantt([
          'add',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'dependent_item',
          'Dependent item',
          '--requires', 'base_item',
        ]);

        // Try to remove without force - should fail
        final result = await runGiantt([
          'remove',
          '--file', itemsPath,
          '--occlude-file', occludeItemsPath,
          'base_item',
        ]);

        expect(result.exitCode, equals(1));
        expect(result.stderr, contains('required by'));
      });

      test('remove chain of items', () async {
        // Add a chain of items
        await runGiantt([
          'add', '--file', itemsPath, '--occlude-file', occludeItemsPath,
          'gen_paper_format', 'Format paper',
        ]);
        await runGiantt([
          'add', '--file', itemsPath, '--occlude-file', occludeItemsPath,
          'gen_paper_outline', 'Outline paper',
        ]);
        await runGiantt([
          'add', '--file', itemsPath, '--occlude-file', occludeItemsPath,
          'gen_paper_draft', 'Draft paper',
        ]);

        // Remove them all (no dependencies between them)
        for (final id in ['gen_paper_format', 'gen_paper_outline', 'gen_paper_draft']) {
          final result = await runGiantt([
            'remove', '--file', itemsPath, '--occlude-file', occludeItemsPath, id,
          ]);
          expect(result.exitCode, equals(0), reason: 'Failed to remove $id: ${result.stderr}');
        }

        // Verify all removed
        final content = await File(itemsPath).readAsString();
        expect(content, isNot(contains('gen_paper_format')));
        expect(content, isNot(contains('gen_paper_outline')));
        expect(content, isNot(contains('gen_paper_draft')));
      });
    });
  });
}

const _itemsHeader = '''
########################################
#                                      #
#            Giantt Items              #
#                                      #
########################################

''';

const _occludeHeader = '''
########################################
#                                      #
#        Occluded Giantt Items         #
#                                      #
########################################

''';
