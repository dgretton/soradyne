import 'dart:io';
import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

/// Tests that validate Dart implementation against known Python outputs
void main() {
  group('Python-Dart Output Validation', () {
    late Directory tempDir;
    late String itemsPath;
    late String occludeItemsPath;

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('giantt_validation_');
      final includeDir = Directory('${tempDir.path}/include');
      final occludeDir = Directory('${tempDir.path}/occlude');
      await includeDir.create(recursive: true);
      await occludeDir.create(recursive: true);
      
      itemsPath = '${includeDir.path}/items.txt';
      occludeItemsPath = '${occludeDir.path}/items.txt';
    });

    tearDown(() async {
      await tempDir.delete(recursive: true);
    });

    test('File format output matches Python exactly', () async {
      // Create items using Dart CLI equivalent operations
      final graph = GianttGraph();
      
      // Add items that would be created by Python CLI
      final items = [
        GianttItem(
          id: 'task_a',
          title: 'First Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.parse('1d'),
          charts: [],
          tags: [],
          relations: {},
        ),
        GianttItem(
          id: 'task_b',
          title: 'Second Task',
          status: GianttStatus.inProgress,
          priority: GianttPriority.high,
          duration: GianttDuration.parse('2d'),
          charts: ['Chart1'],
          tags: ['urgent'],
          relations: {'REQUIRES': ['task_a']},
        ),
        GianttItem(
          id: 'task_c',
          title: 'Task with "quotes" and special chars',
          status: GianttStatus.completed,
          priority: GianttPriority.low,
          duration: GianttDuration.parse('0.5d'),
          charts: ['Chart1', 'Chart2'],
          tags: ['done', 'tested'],
          relations: {'REQUIRES': ['task_b'], 'BLOCKS': ['task_d']},
        ),
      ];

      for (final item in items) {
        graph.addItem(item);
      }

      // Save to files
      await File(itemsPath).writeAsString(_getItemsHeader());
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);

      // Read the generated file content
      final content = await File(itemsPath).readAsString();
      final lines = content.split('\n').where((line) => 
        line.trim().isNotEmpty && !line.trim().startsWith('#')).toList();

      // Validate exact format matches Python output
      expect(lines.length, equals(3), reason: 'Should have exactly 3 item lines');
      
      // Validate task_a format
      expect(lines[0], equals('○ task_a 1d "First Task" {}'),
             reason: 'Basic item format should match Python');
      
      // Validate task_b format  
      expect(lines[1], equals('◑ task_b!! 2d "Second Task" {"Chart1"} urgent >>> ⊢[task_a]'),
             reason: 'Complex item format should match Python');
      
      // Validate task_c format with JSON escaping
      expect(lines[2], contains('"Task with \\"quotes\\" and special chars"'),
             reason: 'JSON escaping should match Python');
      expect(lines[2], contains('{"Chart1","Chart2"}'),
             reason: 'Multiple charts should be formatted correctly');
      expect(lines[2], contains('done,tested'),
             reason: 'Multiple tags should be comma-separated');
    });

    test('Topological sort order matches Python', () async {
      // Create a dependency chain that Python would sort in a specific order
      final graph = GianttGraph();
      
      // Add items in random order to test sorting
      final items = [
        GianttItem(
          id: 'z_last',
          title: 'Last Task',
          duration: GianttDuration.parse('1d'),
          relations: {'REQUIRES': ['b_middle']},
        ),
        GianttItem(
          id: 'a_first', 
          title: 'First Task',
          duration: GianttDuration.parse('1d'),
          relations: {},
        ),
        GianttItem(
          id: 'b_middle',
          title: 'Middle Task', 
          duration: GianttDuration.parse('1d'),
          relations: {'REQUIRES': ['a_first']},
        ),
      ];

      // Add in random order
      graph.addItem(items[0]); // z_last
      graph.addItem(items[1]); // a_first  
      graph.addItem(items[2]); // b_middle

      // Sort should produce: a_first, b_middle, z_last
      final sorted = graph.topologicalSort();
      final sortedIds = sorted.map((item) => item.id).toList();
      
      expect(sortedIds, equals(['a_first', 'b_middle', 'z_last']),
             reason: 'Topological sort should match Python dependency order');

      // Save and reload to verify file order
      await File(itemsPath).writeAsString(_getItemsHeader());
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
      
      final reloaded = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      final reloadedSorted = reloaded.topologicalSort();
      final reloadedIds = reloadedSorted.map((item) => item.id).toList();
      
      expect(reloadedIds, equals(['a_first', 'b_middle', 'z_last']),
             reason: 'File save/load should preserve topological order');
    });

    test('Symbol mappings are identical to Python', () {
      // Test all status symbols
      final statusMappings = {
        GianttStatus.notStarted: '○',
        GianttStatus.inProgress: '◑', 
        GianttStatus.blocked: '⊘',
        GianttStatus.completed: '●',
      };

      for (final entry in statusMappings.entries) {
        expect(entry.key.symbol, equals(entry.value),
               reason: 'Status ${entry.key.name} should have symbol ${entry.value}');
      }

      // Test all priority symbols
      final priorityMappings = {
        GianttPriority.lowest: ',,,',
        GianttPriority.low: '...',
        GianttPriority.neutral: '',
        GianttPriority.unsure: '?',
        GianttPriority.medium: '!',
        GianttPriority.high: '!!',
        GianttPriority.critical: '!!!',
      };

      for (final entry in priorityMappings.entries) {
        expect(entry.key.symbol, equals(entry.value),
               reason: 'Priority ${entry.key.name} should have symbol "${entry.value}"');
      }

      // Test relation symbols in item output
      final item = GianttItem(
        id: 'test',
        title: 'Test',
        duration: GianttDuration.parse('1d'),
        relations: {
          'REQUIRES': ['dep1'],
          'BLOCKS': ['blocked1'],
          'ANYOF': ['any1'],
          'SUFFICIENT': ['suff1'],
        },
      );

      final output = item.toFileString();
      expect(output, contains('⊢[dep1]'), reason: 'REQUIRES should use ⊢ symbol');
      expect(output, contains('►[blocked1]'), reason: 'BLOCKS should use ► symbol');
      expect(output, contains('⋲[any1]'), reason: 'ANYOF should use ⋲ symbol');
      expect(output, contains('≻[suff1]'), reason: 'SUFFICIENT should use ≻ symbol');
    });

    test('Round-trip parsing preserves data exactly', () {
      // Test complex items that exercise all parsing features
      final testCases = [
        '○ simple 1d "Simple Task" {}',
        '◑ complex!! 2.5d "Complex Task" {"Chart1","Chart2"} tag1,tag2 >>> ⊢[dep1,dep2] ►[block1]',
        '● escaped 1d "Task with \\"quotes\\" and \\n newlines" {}',
        '⊘ priority,,, 3d "Lowest Priority" {} urgent',
        '○ relations 1d "All Relations" {} >>> ⊢[r1] ⋲[a1] ≫[s1] ∴[i1] ∪[t1] ⊟[c1] ►[b1] ≻[sf1]',
      ];

      for (final testCase in testCases) {
        // Parse the string
        final parsed = GianttParser.fromString(testCase);
        
        // Convert back to string
        final output = parsed.toFileString();
        
        // Should be identical (normalized for whitespace)
        expect(_normalizeString(output), equals(_normalizeString(testCase)),
               reason: 'Round-trip should preserve: $testCase');
      }
    });

    test('Error handling matches Python behavior', () {
      // Test cycle detection
      final graph = GianttGraph();
      graph.addItem(GianttItem(
        id: 'a',
        title: 'Task A',
        duration: GianttDuration.parse('1d'),
        relations: {'REQUIRES': ['b']},
      ));
      graph.addItem(GianttItem(
        id: 'b', 
        title: 'Task B',
        duration: GianttDuration.parse('1d'),
        relations: {'REQUIRES': ['a']},
      ));

      expect(() => graph.topologicalSort(), 
             throwsA(isA<CycleDetectedException>()),
             reason: 'Should detect cycles like Python');

      // Test invalid parsing
      expect(() => GianttParser.fromString('invalid format'),
             throwsA(isA<GianttParseException>()),
             reason: 'Should throw parse exceptions like Python');
    });

    test('Doctor issues match Python detection', () async {
      // Create graph with known issues that Python would detect
      final graph = GianttGraph();
      
      // Add item with dangling reference
      graph.addItem(GianttItem(
        id: 'broken',
        title: 'Broken Task',
        duration: GianttDuration.parse('1d'),
        relations: {'REQUIRES': ['nonexistent']},
      ));
      
      // Add incomplete chain
      graph.addItem(GianttItem(
        id: 'blocker',
        title: 'Blocker Task', 
        duration: GianttDuration.parse('1d'),
        relations: {'BLOCKS': ['blocked']},
      ));
      graph.addItem(GianttItem(
        id: 'blocked',
        title: 'Blocked Task',
        duration: GianttDuration.parse('1d'),
        relations: {}, // Missing REQUIRES relation
      ));

      final doctor = GraphDoctor(graph);
      final issues = doctor.fullDiagnosis();
      
      // Should find exactly the issues Python would find
      expect(issues.length, equals(2), reason: 'Should find 2 issues');
      
      final issueTypes = issues.map((i) => i.type).toSet();
      expect(issueTypes, contains(IssueType.danglingReference),
             reason: 'Should detect dangling reference');
      expect(issueTypes, contains(IssueType.incompleteChain),
             reason: 'Should detect incomplete chain');

      // Test auto-fix
      final fixed = doctor.fixIssues();
      expect(fixed.length, equals(2), reason: 'Should fix both issues');
      
      final afterFix = doctor.fullDiagnosis();
      expect(afterFix.length, equals(0), reason: 'Should have no issues after fix');
    });

    test('CLI operations produce expected file changes', () async {
      // Initialize files
      await File(itemsPath).writeAsString(_getItemsHeader());
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());

      // Simulate CLI add operation
      final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      
      final newItem = GianttItem(
        id: 'cli_test',
        title: 'CLI Test Item',
        status: GianttStatus.notStarted,
        priority: GianttPriority.high,
        duration: GianttDuration.parse('2d'),
        charts: ['TestChart'],
        tags: ['cli', 'test'],
        relations: {},
      );
      
      graph.addItem(newItem);
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);

      // Verify file content
      final content = await File(itemsPath).readAsString();
      expect(content, contains('○ cli_test!! 2d "CLI Test Item" {"TestChart"} cli,test'),
             reason: 'CLI add should produce correct file format');

      // Simulate CLI show operation
      final reloaded = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      final found = reloaded.items['cli_test'];
      expect(found, isNotNull, reason: 'CLI show should find added item');
      expect(found!.title, equals('CLI Test Item'));
      expect(found.priority, equals(GianttPriority.high));
    });
  });
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

String _normalizeString(String str) {
  return str.trim().replaceAll(RegExp(r'\s+'), ' ');
}
