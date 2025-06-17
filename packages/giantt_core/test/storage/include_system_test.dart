import 'dart:io';
import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

void main() {
  group('Include System Tests', () {
    late Directory tempDir;
    
    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('giantt_test_');
    });
    
    tearDown(() async {
      if (tempDir.existsSync()) {
        await tempDir.delete(recursive: true);
      }
    });

    group('Include Directive Parsing', () {
      test('should parse include directives from file header', () async {
        final testFile = File('${tempDir.path}/test.txt');
        await testFile.writeAsString('''
#include shared/common.txt
#include ../other/tasks.txt

○ task1 1d "Main task" {}
○ task2 2d "Another task" {}
''');

        final includes = FileRepository.parseIncludeDirectives(testFile.path);
        expect(includes, equals(['shared/common.txt', '../other/tasks.txt']));
      });

      test('should stop parsing includes when non-directive line is found', () async {
        final testFile = File('${tempDir.path}/test.txt');
        await testFile.writeAsString('''
#include first.txt
#include second.txt

○ task1 1d "Task" {}
#include third.txt
''');

        final includes = FileRepository.parseIncludeDirectives(testFile.path);
        expect(includes, equals(['first.txt', 'second.txt']));
      });

      test('should handle empty files gracefully', () async {
        final testFile = File('${tempDir.path}/empty.txt');
        await testFile.writeAsString('');

        final includes = FileRepository.parseIncludeDirectives(testFile.path);
        expect(includes, isEmpty);
      });

      test('should handle files with no includes', () async {
        final testFile = File('${tempDir.path}/no_includes.txt');
        await testFile.writeAsString('''
○ task1 1d "Task without includes" {}
○ task2 2d "Another task" {}
''');

        final includes = FileRepository.parseIncludeDirectives(testFile.path);
        expect(includes, isEmpty);
      });

      test('should handle non-existent files', () {
        final includes = FileRepository.parseIncludeDirectives('${tempDir.path}/nonexistent.txt');
        expect(includes, isEmpty);
      });
    });

    group('Graph Loading with Includes', () {
      test('should load graph from single file without includes', () async {
        final testFile = File('${tempDir.path}/simple.txt');
        await testFile.writeAsString('''
○ task1 1d "Simple task" {}
◑ task2!! 2w "High priority task" {"Chart1"}
''');

        final graph = FileRepository.loadGraphFromFile(testFile.path);
        expect(graph.items.length, equals(2));
        expect(graph.items['task1'], isNotNull);
        expect(graph.items['task2'], isNotNull);
        expect(graph.items['task2']!.priority, equals(GianttPriority.high));
      });

      test('should load graph with simple includes', () async {
        // Create included file
        final includedFile = File('${tempDir.path}/included.txt');
        await includedFile.writeAsString('''
○ shared_task 1d "Shared task" {}
''');

        // Create main file with include
        final mainFile = File('${tempDir.path}/main.txt');
        await mainFile.writeAsString('''
#include included.txt

○ main_task 2d "Main task" {}
''');

        final graph = FileRepository.loadGraphFromFile(mainFile.path);
        expect(graph.items.length, equals(2));
        expect(graph.items['shared_task'], isNotNull);
        expect(graph.items['main_task'], isNotNull);
      });

      test('should handle relative path includes', () async {
        // Create subdirectory
        final subDir = Directory('${tempDir.path}/shared');
        await subDir.create();

        // Create included file in subdirectory
        final includedFile = File('${subDir.path}/common.txt');
        await includedFile.writeAsString('''
○ common_task 1d "Common task" {}
''');

        // Create main file with relative include
        final mainFile = File('${tempDir.path}/main.txt');
        await mainFile.writeAsString('''
#include shared/common.txt

○ main_task 2d "Main task" {}
''');

        final graph = FileRepository.loadGraphFromFile(mainFile.path);
        expect(graph.items.length, equals(2));
        expect(graph.items['common_task'], isNotNull);
        expect(graph.items['main_task'], isNotNull);
      });

      test('should handle nested includes', () async {
        // Create deeply included file
        final deepFile = File('${tempDir.path}/deep.txt');
        await deepFile.writeAsString('''
○ deep_task 1d "Deep task" {}
''');

        // Create middle file that includes deep file
        final middleFile = File('${tempDir.path}/middle.txt');
        await middleFile.writeAsString('''
#include deep.txt

○ middle_task 2d "Middle task" {}
''');

        // Create main file that includes middle file
        final mainFile = File('${tempDir.path}/main.txt');
        await mainFile.writeAsString('''
#include middle.txt

○ main_task 3d "Main task" {}
''');

        final graph = FileRepository.loadGraphFromFile(mainFile.path);
        expect(graph.items.length, equals(3));
        expect(graph.items['deep_task'], isNotNull);
        expect(graph.items['middle_task'], isNotNull);
        expect(graph.items['main_task'], isNotNull);
      });

      test('should detect circular includes', () async {
        // Create file A that includes B
        final fileA = File('${tempDir.path}/a.txt');
        await fileA.writeAsString('''
#include b.txt

○ task_a 1d "Task A" {}
''');

        // Create file B that includes A (circular)
        final fileB = File('${tempDir.path}/b.txt');
        await fileB.writeAsString('''
#include a.txt

○ task_b 1d "Task B" {}
''');

        expect(
          () => FileRepository.loadGraphFromFile(fileA.path),
          throwsA(isA<GraphException>()),
        );
      });

      test('should handle missing included files gracefully', () async {
        final mainFile = File('${tempDir.path}/main.txt');
        await mainFile.writeAsString('''
#include nonexistent.txt

○ main_task 1d "Main task" {}
''');

        expect(
          () => FileRepository.loadGraphFromFile(mainFile.path),
          throwsA(isA<GraphException>()),
        );
      });

      test('should skip invalid lines with warning', () async {
        final testFile = File('${tempDir.path}/with_invalid.txt');
        await testFile.writeAsString('''
○ valid_task 1d "Valid task" {}
invalid line without proper format
◑ another_valid!! 2d "Another valid task" {}
''');

        final graph = FileRepository.loadGraphFromFile(testFile.path);
        expect(graph.items.length, equals(2));
        expect(graph.items['valid_task'], isNotNull);
        expect(graph.items['another_valid'], isNotNull);
      });
    });

    group('Dual File Loading', () {
      test('should load from both include and occlude files', () async {
        // Create include file
        final includeFile = File('${tempDir.path}/include.txt');
        await includeFile.writeAsString('''
○ active_task 1d "Active task" {}
''');

        // Create occlude file
        final occludeFile = File('${tempDir.path}/occlude.txt');
        await occludeFile.writeAsString('''
● archived_task 1d "Archived task" {}
''');

        final graph = FileRepository.loadGraph(includeFile.path, occludeFile.path);
        expect(graph.items.length, equals(2));
        expect(graph.items['active_task'], isNotNull);
        expect(graph.items['archived_task'], isNotNull);
        expect(graph.items['active_task']!.occlude, isFalse);
        expect(graph.items['archived_task']!.occlude, isTrue);
      });

      test('should handle includes in both files', () async {
        // Create shared include
        final sharedFile = File('${tempDir.path}/shared.txt');
        await sharedFile.writeAsString('''
○ shared_task 1d "Shared task" {}
''');

        // Create include file with include directive
        final includeFile = File('${tempDir.path}/include.txt');
        await includeFile.writeAsString('''
#include shared.txt

○ active_task 1d "Active task" {}
''');

        // Create occlude file
        final occludeFile = File('${tempDir.path}/occlude.txt');
        await occludeFile.writeAsString('''
● archived_task 1d "Archived task" {}
''');

        final graph = FileRepository.loadGraph(includeFile.path, occludeFile.path);
        expect(graph.items.length, equals(3));
        expect(graph.items['shared_task'], isNotNull);
        expect(graph.items['active_task'], isNotNull);
        expect(graph.items['archived_task'], isNotNull);
      });
    });

    group('Include Structure Display', () {
      test('should show simple include structure', () async {
        final includedFile = File('${tempDir.path}/included.txt');
        await includedFile.writeAsString('○ task 1d "Task" {}');

        final mainFile = File('${tempDir.path}/main.txt');
        await mainFile.writeAsString('''
#include included.txt

○ main_task 1d "Main task" {}
''');

        // This should not throw and should print the structure
        expect(
          () => FileRepository.showIncludeStructure(mainFile.path),
          returnsNormally,
        );
      });

      test('should show recursive include structure', () async {
        final deepFile = File('${tempDir.path}/deep.txt');
        await deepFile.writeAsString('○ deep 1d "Deep" {}');

        final middleFile = File('${tempDir.path}/middle.txt');
        await middleFile.writeAsString('''
#include deep.txt

○ middle 1d "Middle" {}
''');

        final mainFile = File('${tempDir.path}/main.txt');
        await mainFile.writeAsString('''
#include middle.txt

○ main 1d "Main" {}
''');

        expect(
          () => FileRepository.showIncludeStructure(mainFile.path, recursive: true),
          returnsNormally,
        );
      });

      test('should handle circular includes in structure display', () async {
        final fileA = File('${tempDir.path}/a.txt');
        await fileA.writeAsString('''
#include b.txt

○ task_a 1d "Task A" {}
''');

        final fileB = File('${tempDir.path}/b.txt');
        await fileB.writeAsString('''
#include a.txt

○ task_b 1d "Task B" {}
''');

        expect(
          () => FileRepository.showIncludeStructure(fileA.path, recursive: true),
          returnsNormally,
        );
      });
    });

    group('Path Handling', () {
      test('should handle absolute paths correctly', () async {
        final absolutePath = '${tempDir.path}/absolute.txt';
        final absoluteFile = File(absolutePath);
        await absoluteFile.writeAsString('○ abs_task 1d "Absolute task" {}');

        final mainFile = File('${tempDir.path}/main.txt');
        await mainFile.writeAsString('''
#include $absolutePath

○ main_task 1d "Main task" {}
''');

        final graph = FileRepository.loadGraphFromFile(mainFile.path);
        expect(graph.items.length, equals(2));
        expect(graph.items['abs_task'], isNotNull);
        expect(graph.items['main_task'], isNotNull);
      });

      test('should handle parent directory references', () async {
        // Create subdirectory
        final subDir = Directory('${tempDir.path}/sub');
        await subDir.create();

        // Create file in parent directory
        final parentFile = File('${tempDir.path}/parent.txt');
        await parentFile.writeAsString('○ parent_task 1d "Parent task" {}');

        // Create file in subdirectory that references parent
        final subFile = File('${subDir.path}/sub.txt');
        await subFile.writeAsString('''
#include ../parent.txt

○ sub_task 1d "Sub task" {}
''');

        final graph = FileRepository.loadGraphFromFile(subFile.path);
        expect(graph.items.length, equals(2));
        expect(graph.items['parent_task'], isNotNull);
        expect(graph.items['sub_task'], isNotNull);
      });
    });
  });
}
