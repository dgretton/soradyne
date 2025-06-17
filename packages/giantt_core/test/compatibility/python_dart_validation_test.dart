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
      print('\n=== File Format Validation ===');
      
      // Expected Python outputs for comparison
      final expectedPythonOutputs = [
        '○ task_a 1d "First Task" {}',
        '◑ task_b!! 2d "Second Task" {"Chart1"} urgent >>> ⊢[task_a]',
        '● task_c... 0.5d "Task with \\"quotes\\" and special chars" {"Chart1","Chart2"} done,tested >>> ⊢[task_b] ►[task_d]',
      ];
      
      print('Expected Python CLI outputs:');
      for (int i = 0; i < expectedPythonOutputs.length; i++) {
        print('  Python[$i]: ${expectedPythonOutputs[i]}');
      }
      
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

      print('\nActual Dart CLI outputs:');
      for (int i = 0; i < lines.length; i++) {
        print('  Dart[$i]:   ${lines[i]}');
      }
      
      print('\nComparison Results:');
      for (int i = 0; i < expectedPythonOutputs.length && i < lines.length; i++) {
        final match = lines[i] == expectedPythonOutputs[i];
        print('  Line $i: ${match ? "✓ MATCH" : "✗ DIFFER"}');
        if (!match) {
          print('    Expected: ${expectedPythonOutputs[i]}');
          print('    Actual:   ${lines[i]}');
        }
      }

      // Validate exact format matches Python output
      expect(lines.length, equals(3), reason: 'Should have exactly 3 item lines');
      
      // Validate each line matches Python exactly
      for (int i = 0; i < expectedPythonOutputs.length; i++) {
        expect(lines[i], equals(expectedPythonOutputs[i]),
               reason: 'Line $i should match Python output exactly');
      }
    });

    test('Topological sort order matches Python', () async {
      print('\n=== Topological Sort Validation ===');
      
      // Expected Python topological sort order for this dependency chain
      final expectedPythonOrder = ['a_first', 'b_middle', 'z_last'];
      print('Expected Python topological order: $expectedPythonOrder');
      
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

      print('\nAdding items in random order:');
      // Add in random order
      graph.addItem(items[0]); // z_last
      print('  Added: z_last (requires b_middle)');
      graph.addItem(items[1]); // a_first  
      print('  Added: a_first (no dependencies)');
      graph.addItem(items[2]); // b_middle
      print('  Added: b_middle (requires a_first)');

      // Sort should produce: a_first, b_middle, z_last
      final sorted = graph.topologicalSort();
      final sortedIds = sorted.map((item) => item.id).toList();
      
      print('\nDart topological sort result: $sortedIds');
      print('Comparison: ${sortedIds.toString() == expectedPythonOrder.toString() ? "✓ MATCH" : "✗ DIFFER"}');
      
      expect(sortedIds, equals(expectedPythonOrder),
             reason: 'Topological sort should match Python dependency order');

      // Save and reload to verify file order
      await File(itemsPath).writeAsString(_getItemsHeader());
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
      
      print('\nTesting file save/reload persistence:');
      final reloaded = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      final reloadedSorted = reloaded.topologicalSort();
      final reloadedIds = reloadedSorted.map((item) => item.id).toList();
      
      print('After save/reload: $reloadedIds');
      print('Persistence check: ${reloadedIds.toString() == expectedPythonOrder.toString() ? "✓ PRESERVED" : "✗ LOST"}');
      
      expect(reloadedIds, equals(expectedPythonOrder),
             reason: 'File save/load should preserve topological order');
    });

    test('Symbol mappings are identical to Python', () {
      print('\n=== Symbol Mapping Validation ===');
      
      // Expected Python symbol mappings
      final expectedStatusSymbols = {
        'NOT_STARTED': '○',
        'IN_PROGRESS': '◑', 
        'BLOCKED': '⊘',
        'COMPLETED': '●',
      };
      
      final expectedPrioritySymbols = {
        'LOWEST': ',,,',
        'LOW': '...',
        'NEUTRAL': '',
        'UNSURE': '?',
        'MEDIUM': '!',
        'HIGH': '!!',
        'CRITICAL': '!!!',
      };
      
      final expectedRelationSymbols = {
        'REQUIRES': '⊢',
        'BLOCKS': '►',
        'ANYOF': '⋲',
        'SUFFICIENT': '≻',
        'SUPERCHARGES': '≫',
        'INDICATES': '∴',
        'TOGETHER': '∪',
        'CONFLICTS': '⊟',
      };
      
      print('Python Status Symbols:');
      expectedStatusSymbols.forEach((name, symbol) => 
        print('  $name: "$symbol"'));
      
      print('\nPython Priority Symbols:');
      expectedPrioritySymbols.forEach((name, symbol) => 
        print('  $name: "$symbol"'));
      
      print('\nPython Relation Symbols:');
      expectedRelationSymbols.forEach((name, symbol) => 
        print('  $name: "$symbol"'));

      // Test all status symbols
      final statusMappings = {
        GianttStatus.notStarted: '○',
        GianttStatus.inProgress: '◑', 
        GianttStatus.blocked: '⊘',
        GianttStatus.completed: '●',
      };

      print('\nDart Status Symbol Validation:');
      for (final entry in statusMappings.entries) {
        final match = entry.key.symbol == entry.value;
        print('  ${entry.key.name}: "${entry.key.symbol}" ${match ? "✓" : "✗"}');
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

      print('\nDart Priority Symbol Validation:');
      for (final entry in priorityMappings.entries) {
        final match = entry.key.symbol == entry.value;
        print('  ${entry.key.name}: "${entry.key.symbol}" ${match ? "✓" : "✗"}');
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
      print('\nDart Relation Symbol Validation:');
      print('  Generated output: $output');
      
      final relationTests = {
        'REQUIRES': '⊢[dep1]',
        'BLOCKS': '►[blocked1]',
        'ANYOF': '⋲[any1]',
        'SUFFICIENT': '≻[suff1]',
      };
      
      for (final test in relationTests.entries) {
        final containsSymbol = output.contains(test.value);
        print('  ${test.key}: ${test.value} ${containsSymbol ? "✓" : "✗"}');
        expect(output, contains(test.value), 
               reason: '${test.key} should use ${test.value}');
      }
    });

    test('Round-trip parsing preserves data exactly', () {
      print('\n=== Round-trip Parsing Validation ===');
      
      // Test complex items that exercise all parsing features
      final testCases = [
        '○ simple 1d "Simple Task" {}',
        '◑ complex!! 2.5d "Complex Task" {"Chart1","Chart2"} tag1,tag2 >>> ⊢[dep1,dep2] ►[block1]',
        '● escaped 1d "Task with \\"quotes\\" and \\n newlines" {}',
        '⊘ priority,,, 3d "Lowest Priority" {} urgent',
        '○ relations 1d "All Relations" {} >>> ⊢[r1] ⋲[a1] ≫[s1] ∴[i1] ∪[t1] ⊟[c1] ►[b1] ≻[sf1]',
      ];

      print('Testing round-trip parsing (Python format → Dart parse → Dart format):');
      
      for (int i = 0; i < testCases.length; i++) {
        final testCase = testCases[i];
        print('\nTest case ${i + 1}:');
        print('  Input:  $testCase');
        
        // Parse the string
        final parsed = GianttParser.fromString(testCase);
        
        // Convert back to string
        final output = parsed.toFileString();
        print('  Output: $output');
        
        // Check if they match
        final normalizedInput = _normalizeString(testCase);
        final normalizedOutput = _normalizeString(output);
        final match = normalizedOutput == normalizedInput;
        
        print('  Result: ${match ? "✓ PRESERVED" : "✗ CHANGED"}');
        
        if (!match) {
          print('  Normalized Input:  "$normalizedInput"');
          print('  Normalized Output: "$normalizedOutput"');
        }
        
        // Should be identical (normalized for whitespace)
        expect(normalizedOutput, equals(normalizedInput),
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
