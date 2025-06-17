import 'dart:io';
import 'dart:convert';
import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

/// Tests that execute actual Python CLI and compare with Dart implementation
void main() {
  group('Python-Dart Execution Comparison', () {
    late Directory tempDir;
    late Directory pythonTempDir;
    late String dartItemsPath;
    late String dartOccludeItemsPath;
    late String pythonItemsPath;
    late String pythonOccludeItemsPath;

    setUp(() async {
      // Create temp directories for both implementations
      tempDir = await Directory.systemTemp.createTemp('giantt_dart_test_');
      pythonTempDir = await Directory.systemTemp.createTemp('giantt_python_test_');
      
      // Set up Dart paths
      final dartIncludeDir = Directory('${tempDir.path}/include');
      final dartOccludeDir = Directory('${tempDir.path}/occlude');
      await dartIncludeDir.create(recursive: true);
      await dartOccludeDir.create(recursive: true);
      
      dartItemsPath = '${dartIncludeDir.path}/items.txt';
      dartOccludeItemsPath = '${dartOccludeDir.path}/items.txt';
      
      // Set up Python paths
      final pythonIncludeDir = Directory('${pythonTempDir.path}/include');
      final pythonOccludeDir = Directory('${pythonTempDir.path}/occlude');
      await pythonIncludeDir.create(recursive: true);
      await pythonOccludeDir.create(recursive: true);
      
      pythonItemsPath = '${pythonIncludeDir.path}/items.txt';
      pythonOccludeItemsPath = '${pythonOccludeDir.path}/items.txt';
      
      // Initialize both with empty files
      await File(dartItemsPath).writeAsString(_getItemsHeader());
      await File(dartOccludeItemsPath).writeAsString(_getOccludeHeader());
      await File(pythonItemsPath).writeAsString(_getItemsHeader());
      await File(pythonOccludeItemsPath).writeAsString(_getOccludeHeader());
    });

    tearDown(() async {
      await tempDir.delete(recursive: true);
      await pythonTempDir.delete(recursive: true);
    });

    test('Python and Dart init commands produce identical directory structure', () async {
      print('\n=== Testing init command ===');
      
      // Clean up existing directories
      await tempDir.delete(recursive: true);
      await pythonTempDir.delete(recursive: true);
      
      // Run Python init
      print('Running Python init...');
      final pythonResult = await Process.run(
        'python3', 
        ['../../docs/port_reference/giantt_cli.py', 'init', '--data-dir', pythonTempDir.path],
        workingDirectory: '.',
      );
      
      print('Python init exit code: ${pythonResult.exitCode}');
      print('Python init stdout: ${pythonResult.stdout}');
      if (pythonResult.stderr.isNotEmpty) {
        print('Python init stderr: ${pythonResult.stderr}');
      }
      
      // Run Dart init
      print('Running Dart init...');
      final dartResult = await Process.run(
        'dart', 
        ['run', 'bin/giantt.dart', 'init', '--data-dir', tempDir.path],
        workingDirectory: '.',
      );
      
      print('Dart init exit code: ${dartResult.exitCode}');
      print('Dart init stdout: ${dartResult.stdout}');
      if (dartResult.stderr.isNotEmpty) {
        print('Dart init stderr: ${dartResult.stderr}');
      }
      
      // Compare directory structures
      final pythonFiles = await _getDirectoryStructure(pythonTempDir.path);
      final dartFiles = await _getDirectoryStructure(tempDir.path);
      
      print('\nPython created files:');
      for (final file in pythonFiles) {
        print('  $file');
      }
      
      print('\nDart created files:');
      for (final file in dartFiles) {
        print('  $file');
      }
      
      expect(dartFiles.length, equals(pythonFiles.length), 
             reason: 'Should create same number of files');
      
      // Check that both created the same relative file structure
      final pythonRelative = pythonFiles.map((f) => f.replaceFirst(pythonTempDir.path, '')).toSet();
      final dartRelative = dartFiles.map((f) => f.replaceFirst(tempDir.path, '')).toSet();
      
      expect(dartRelative, equals(pythonRelative),
             reason: 'Should create identical directory structure');
    });

    test('Python and Dart add commands produce identical file content', () async {
      print('\n=== Testing add command ===');
      
      // Test cases to add
      final testCases = [
        {
          'id': 'simple_task',
          'title': 'Simple Task',
          'args': [],
        },
        {
          'id': 'priority_task',
          'title': 'Priority Task',
          'args': ['--priority', 'HIGH'],
        },
        {
          'id': 'complex_task',
          'title': 'Complex Task with "quotes"',
          'args': ['--duration', '2.5d', '--priority', 'MEDIUM', '--charts', 'Chart1,Chart2', '--tags', 'urgent,test'],
        },
      ];
      
      for (final testCase in testCases) {
        print('\nTesting add: ${testCase['id']}');
        
        // Run Python add
        final pythonArgs = [
          '../../docs/port_reference/giantt_cli.py', 'add',
          '--file', pythonItemsPath,
          '--occlude-file', pythonOccludeItemsPath,
          testCase['id'] as String,
          testCase['title'] as String,
          ...(testCase['args'] as List<dynamic>).cast<String>(),
        ];
        
        print('Python command: python3 ${pythonArgs.join(' ')}');
        final pythonResult = await Process.run('python3', pythonArgs, workingDirectory: '.');
        
        print('Python add exit code: ${pythonResult.exitCode}');
        if (pythonResult.exitCode != 0) {
          print('Python add stderr: ${pythonResult.stderr}');
        }
        
        // Run Dart add
        final dartArgs = [
          'run', 'bin/giantt.dart', 'add',
          '--file', dartItemsPath,
          '--occlude-file', dartOccludeItemsPath,
          testCase['id'] as String,
          testCase['title'] as String,
          ...(testCase['args'] as List<dynamic>).cast<String>(),
        ];
        
        print('Dart command: dart ${dartArgs.join(' ')}');
        final dartResult = await Process.run('dart', dartArgs, workingDirectory: '.');
        
        print('Dart add exit code: ${dartResult.exitCode}');
        if (dartResult.exitCode != 0) {
          print('Dart add stderr: ${dartResult.stderr}');
        }
        
        expect(dartResult.exitCode, equals(pythonResult.exitCode),
               reason: 'Exit codes should match for ${testCase['id']}');
      }
      
      // Compare final file contents
      final pythonContent = await File(pythonItemsPath).readAsString();
      final dartContent = await File(dartItemsPath).readAsString();
      
      print('\nPython file content:');
      print(pythonContent);
      
      print('\nDart file content:');
      print(dartContent);
      
      // Extract just the item lines (skip headers)
      final pythonItems = _extractItemLines(pythonContent);
      final dartItems = _extractItemLines(dartContent);
      
      print('\nPython items:');
      for (int i = 0; i < pythonItems.length; i++) {
        print('  [$i]: ${pythonItems[i]}');
      }
      
      print('\nDart items:');
      for (int i = 0; i < dartItems.length; i++) {
        print('  [$i]: ${dartItems[i]}');
      }
      
      expect(dartItems.length, equals(pythonItems.length),
             reason: 'Should have same number of items');
      
      for (int i = 0; i < pythonItems.length; i++) {
        expect(dartItems[i], equals(pythonItems[i]),
               reason: 'Item $i should match exactly');
      }
    });

    test('Python and Dart show commands produce identical output', () async {
      print('\n=== Testing show command ===');
      
      // First add an item to both
      await _addTestItem('test_item', 'Test Item', pythonItemsPath, pythonOccludeItemsPath, dartItemsPath, dartOccludeItemsPath);
      
      // Run Python show
      final pythonResult = await Process.run(
        'python3', 
        ['../../docs/port_reference/giantt_cli.py', 'show', '--file', pythonItemsPath, '--occlude-file', pythonOccludeItemsPath, 'test_item'],
        workingDirectory: '.',
      );
      
      // Run Dart show
      final dartResult = await Process.run(
        'dart', 
        ['run', 'bin/giantt.dart', 'show', '--file', dartItemsPath, '--occlude-file', dartOccludeItemsPath, 'test_item'],
        workingDirectory: '.',
      );
      
      print('Python show output:');
      print(pythonResult.stdout);
      
      print('Dart show output:');
      print(dartResult.stdout);
      
      expect(dartResult.exitCode, equals(pythonResult.exitCode),
             reason: 'Exit codes should match');
      
      // Normalize whitespace for comparison
      final pythonOutput = _normalizeOutput(pythonResult.stdout);
      final dartOutput = _normalizeOutput(dartResult.stdout);
      
      expect(dartOutput, equals(pythonOutput),
             reason: 'Show output should be identical');
    });

    test('Python and Dart sort commands produce identical ordering', () async {
      print('\n=== Testing sort command ===');
      
      // Add items with dependencies in both systems
      await _addTestItem('task_a', 'Task A', pythonItemsPath, pythonOccludeItemsPath, dartItemsPath, dartOccludeItemsPath);
      await _addTestItem('task_b', 'Task B', pythonItemsPath, pythonOccludeItemsPath, dartItemsPath, dartOccludeItemsPath, requires: 'task_a');
      await _addTestItem('task_c', 'Task C', pythonItemsPath, pythonOccludeItemsPath, dartItemsPath, dartOccludeItemsPath, requires: 'task_b');
      
      // Run Python sort
      final pythonResult = await Process.run(
        'python3', 
        ['../../docs/port_reference/giantt_cli.py', 'sort', '--file', pythonItemsPath, '--occlude-file', pythonOccludeItemsPath],
        workingDirectory: '.',
      );
      
      // Run Dart sort
      final dartResult = await Process.run(
        'dart', 
        ['run', 'bin/giantt.dart', 'sort', '--file', dartItemsPath, '--occlude-file', dartOccludeItemsPath],
        workingDirectory: '.',
      );
      
      print('Python sort output:');
      print(pythonResult.stdout);
      
      print('Dart sort output:');
      print(dartResult.stdout);
      
      expect(dartResult.exitCode, equals(pythonResult.exitCode),
             reason: 'Sort exit codes should match');
      
      // Compare final file ordering
      final pythonContent = await File(pythonItemsPath).readAsString();
      final dartContent = await File(dartItemsPath).readAsString();
      
      final pythonItems = _extractItemLines(pythonContent);
      final dartItems = _extractItemLines(dartContent);
      
      print('\nPython sorted order:');
      for (int i = 0; i < pythonItems.length; i++) {
        print('  [$i]: ${pythonItems[i]}');
      }
      
      print('\nDart sorted order:');
      for (int i = 0; i < dartItems.length; i++) {
        print('  [$i]: ${dartItems[i]}');
      }
      
      expect(dartItems, equals(pythonItems),
             reason: 'Sorted order should be identical');
    });

    test('Python and Dart doctor commands produce identical issue detection', () async {
      print('\n=== Testing doctor command ===');
      
      // Add items with issues to both systems
      await _addTestItem('broken_task', 'Broken Task', pythonItemsPath, pythonOccludeItemsPath, dartItemsPath, dartOccludeItemsPath, requires: 'nonexistent');
      
      // Run Python doctor
      final pythonResult = await Process.run(
        'python3', 
        ['../../docs/port_reference/giantt_cli.py', 'doctor', '--file', pythonItemsPath, '--occlude-file', pythonOccludeItemsPath],
        workingDirectory: '.',
      );
      
      // Run Dart doctor
      final dartResult = await Process.run(
        'dart', 
        ['run', 'bin/giantt.dart', 'doctor', '--file', dartItemsPath, '--occlude-file', dartOccludeItemsPath],
        workingDirectory: '.',
      );
      
      print('Python doctor output:');
      print(pythonResult.stdout);
      
      print('Dart doctor output:');
      print(dartResult.stdout);
      
      expect(dartResult.exitCode, equals(pythonResult.exitCode),
             reason: 'Doctor exit codes should match');
      
      // Both should detect issues
      expect(pythonResult.stdout, contains('issue'),
             reason: 'Python should detect issues');
      expect(dartResult.stdout, contains('issue'),
             reason: 'Dart should detect issues');
    });
  });
}

/// Helper to add a test item to both Python and Dart systems
Future<void> _addTestItem(String id, String title, String pythonItemsPath, String pythonOccludeItemsPath, 
                         String dartItemsPath, String dartOccludeItemsPath, {String? requires}) async {
  final extraArgs = requires != null ? ['--requires', requires] : <String>[];
  
  // Add to Python
  await Process.run(
    'python3', 
    ['../../docs/port_reference/giantt_cli.py', 'add', '--file', pythonItemsPath, '--occlude-file', pythonOccludeItemsPath, id, title, ...extraArgs],
    workingDirectory: '.',
  );
  
  // Add to Dart
  await Process.run(
    'dart', 
    ['run', 'bin/giantt.dart', 'add', '--file', dartItemsPath, '--occlude-file', dartOccludeItemsPath, id, title, ...extraArgs],
    workingDirectory: '.',
  );
}

/// Get all files in a directory recursively
Future<List<String>> _getDirectoryStructure(String dirPath) async {
  final files = <String>[];
  final dir = Directory(dirPath);
  
  if (!await dir.exists()) return files;
  
  await for (final entity in dir.list(recursive: true)) {
    if (entity is File) {
      files.add(entity.path);
    }
  }
  
  files.sort();
  return files;
}

/// Extract item lines from file content (skip headers and empty lines)
List<String> _extractItemLines(String content) {
  return content
      .split('\n')
      .where((line) => line.trim().isNotEmpty && !line.trim().startsWith('#'))
      .toList();
}

/// Normalize output for comparison (remove extra whitespace, sort lines if needed)
String _normalizeOutput(String output) {
  return output.trim().replaceAll(RegExp(r'\s+'), ' ');
}

String _getItemsHeader() {
  return '''
##############################################
#                                            #
#                Giantt Items                #
#                                            #
#   This file contains all include Giantt   #
#   items in topological order according    #
#   to the REQUIRES (⊢) relation.           #
#   You can use #include directives at the  #
#   top of this file to include other       #
#   Giantt item files.                      #
#   Edit this file manually at your own     #
#   risk.                                    #
#                                            #
##############################################

''';
}

String _getOccludeHeader() {
  return '''
##############################################
#                                            #
#            Giantt Occluded Items           #
#                                            #
#   This file contains all occluded Giantt  #
#   items in topological order according    #
#   to the REQUIRES (⊢) relation.           #
#   Edit this file manually at your own     #
#   risk.                                    #
#                                            #
##############################################

''';
}
