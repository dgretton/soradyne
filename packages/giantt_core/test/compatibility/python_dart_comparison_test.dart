import 'dart:io';
import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

/// Tests that verify Dart implementation produces identical outputs to Python
void main() {
  group('Python-Dart Compatibility Tests', () {
    late Directory tempDir;
    late String itemsPath;
    late String occludeItemsPath;

    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('giantt_test_');
      final includeDir = Directory('${tempDir.path}/include');
      final occludeDir = Directory('${tempDir.path}/occlude');
      await includeDir.create(recursive: true);
      await occludeDir.create(recursive: true);
      
      itemsPath = '${includeDir.path}/items.txt';
      occludeItemsPath = '${occludeDir.path}/items.txt';
      
      // Create initial empty files with headers
      await File(itemsPath).writeAsString(_getItemsHeader());
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());
    });

    tearDown(() async {
      await tempDir.delete(recursive: true);
    });

    test('Item parsing matches Python exactly', () {
      print('\n=== Item Parsing Compatibility Test ===');
      
      // Test cases from Python implementation
      final testCases = [
        // Basic item
        '○ simple_task 1d "Simple Task" {}',
        
        // Item with priority
        '○ priority_task! 2d "Priority Task" {}',
        
        // Item with high priority
        '○ high_task!! 3d "High Priority Task" {}',
        
        // Item with charts
        '○ chart_task 1d "Chart Task" {"Chart1","Chart2"}',
        
        // Item with tags
        '○ tag_task 1d "Tag Task" {} tag1,tag2,tag3',
        
        // Item with relations
        '○ rel_task 1d "Related Task" {} >>> ⊢[dep1,dep2]',
        
        // Complex item with everything
        '◑ complex!! 5d "Complex Task" {"Chart1"} tag1,tag2 >>> ⊢[dep1] ►[blocked1]',
        
        // Item with JSON-escaped title
        '○ json_task 1d "Task with \\"quotes\\" and \\n newlines" {}',
        
        // Item with time constraint
        '○ time_task 1d "Time Task" {} @@@ window(5d:2d,severe)',
        
        // Item with comments
        '○ comment_task 1d "Comment Task" {} # User comment ### Auto comment',
      ];

      print('Testing ${testCases.length} Python format strings:');
      
      for (int i = 0; i < testCases.length; i++) {
        final testCase = testCases[i];
        print('\nTest ${i + 1}:');
        print('  Python: $testCase');
        
        try {
          // Parse with Dart
          final dartItem = GianttParser.fromString(testCase);
          
          // Convert back to string
          final dartOutput = dartItem.toFileString();
          print('  Dart:   $dartOutput');
          
          // Check if they match
          final normalizedPython = _normalizeItemString(testCase);
          final normalizedDart = _normalizeItemString(dartOutput);
          final match = normalizedDart == normalizedPython;
          
          print('  Result: ${match ? "✓ MATCH" : "✗ DIFFER"}');
          
          if (!match) {
            print('  Normalized Python: "$normalizedPython"');
            print('  Normalized Dart:   "$normalizedDart"');
          }
          
          // Should match original (allowing for minor formatting differences)
          expect(normalizedDart, equals(normalizedPython),
                 reason: 'Failed for test case: $testCase');
        } catch (e) {
          print('  Result: ✗ PARSE ERROR - $e');
          rethrow;
        }
      }
      
      print('\n✓ All ${testCases.length} test cases passed!');
    });

    test('Status symbol parsing matches Python', () {
      final statusTests = {
        '○': GianttStatus.notStarted,
        '◑': GianttStatus.inProgress,
        '⊘': GianttStatus.blocked,
        '●': GianttStatus.completed,
      };

      for (final entry in statusTests.entries) {
        final symbol = entry.key;
        final expectedStatus = entry.value;
        
        final testItem = '$symbol test_id 1d "Test" {}';
        final parsed = GianttParser.fromString(testItem);
        
        expect(parsed.status, equals(expectedStatus),
               reason: 'Status symbol $symbol should parse to ${expectedStatus.name}');
        expect(parsed.status.symbol, equals(symbol),
               reason: 'Status ${expectedStatus.name} should have symbol $symbol');
      }
    });

    test('Priority symbol parsing matches Python', () {
      final priorityTests = {
        ',,,': GianttPriority.lowest,
        '...': GianttPriority.low,
        '': GianttPriority.neutral,
        '?': GianttPriority.unsure,
        '!': GianttPriority.medium,
        '!!': GianttPriority.high,
        '!!!': GianttPriority.critical,
      };

      for (final entry in priorityTests.entries) {
        final symbol = entry.key;
        final expectedPriority = entry.value;
        
        final testItem = '○ test_id$symbol 1d "Test" {}';
        final parsed = GianttParser.fromString(testItem);
        
        expect(parsed.priority, equals(expectedPriority),
               reason: 'Priority symbol "$symbol" should parse to ${expectedPriority.name}');
        expect(parsed.priority.symbol, equals(symbol),
               reason: 'Priority ${expectedPriority.name} should have symbol "$symbol"');
      }
    });

    test('Relation symbol parsing matches Python', () {
      final relationTests = {
        '⊢': 'REQUIRES',
        '⋲': 'ANYOF', 
        '≫': 'SUPERCHARGES',
        '∴': 'INDICATES',
        '∪': 'TOGETHER',
        '⊟': 'CONFLICTS',
        '►': 'BLOCKS',
        '≻': 'SUFFICIENT',
      };

      for (final entry in relationTests.entries) {
        final symbol = entry.key;
        final expectedType = entry.value;
        
        final testItem = '○ test_id 1d "Test" {} >>> $symbol[target1,target2]';
        final parsed = GianttParser.fromString(testItem);
        
        expect(parsed.relations.containsKey(expectedType), isTrue,
               reason: 'Should parse relation type $expectedType from symbol $symbol');
        expect(parsed.relations[expectedType], equals(['target1', 'target2']),
               reason: 'Should parse relation targets correctly');
      }
    });

    test('Duration parsing matches Python', () {
      final durationTests = [
        '1d',
        '2w', 
        '3mo',
        '1.5d',
        '2.5w',
        '0.5mo',
        '1y',
        '6mo2w3d',
        '1y6mo',
        '2w3d4h',
      ];

      for (final durationStr in durationTests) {
        final testItem = '○ test_id $durationStr "Test" {}';
        final parsed = GianttParser.fromString(testItem);
        
        // Should parse without error
        expect(parsed.duration.toString(), equals(durationStr),
               reason: 'Duration $durationStr should round-trip correctly');
      }
    });

    test('Graph operations match Python behavior', () async {
      // Create test items that form a dependency chain
      final items = [
        '○ task_a 1d "Task A" {}',
        '○ task_b 1d "Task B" {} >>> ⊢[task_a]',
        '○ task_c 1d "Task C" {} >>> ⊢[task_b]',
      ];

      // Write items to file
      final content = _getItemsHeader() + '\n' + items.join('\n') + '\n';
      await File(itemsPath).writeAsString(content);
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());

      // Load graph
      final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      
      // Should have 3 items
      expect(graph.items.length, equals(3));
      
      // Should be able to topologically sort
      final sorted = graph.topologicalSort();
      expect(sorted.length, equals(3));
      
      // Order should be: task_a, task_b, task_c
      expect(sorted[0].id, equals('task_a'));
      expect(sorted[1].id, equals('task_b'));
      expect(sorted[2].id, equals('task_c'));
      
      // Save and reload should preserve order
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
      final reloaded = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      final resorted = reloaded.topologicalSort();
      
      expect(resorted.map((i) => i.id).toList(), 
             equals(['task_a', 'task_b', 'task_c']));
    });

    test('Cycle detection matches Python behavior', () async {
      // Create items with circular dependency
      final items = [
        '○ task_a 1d "Task A" {} >>> ⊢[task_b]',
        '○ task_b 1d "Task B" {} >>> ⊢[task_a]',
      ];

      final content = _getItemsHeader() + '\n' + items.join('\n') + '\n';
      await File(itemsPath).writeAsString(content);
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());

      final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      
      // Should detect cycle
      expect(() => graph.topologicalSort(), 
             throwsA(isA<CycleDetectedException>()));
    });

    test('Doctor functionality matches Python behavior', () async {
      // Create items with dangling reference
      final items = [
        '○ task_a 1d "Task A" {} >>> ⊢[nonexistent]',
        '○ task_b 1d "Task B" {}',
      ];

      final content = _getItemsHeader() + '\n' + items.join('\n') + '\n';
      await File(itemsPath).writeAsString(content);
      await File(occludeItemsPath).writeAsString(_getOccludeHeader());

      final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
      final doctor = GraphDoctor(graph);
      
      // Should find dangling reference issue
      final issues = doctor.fullDiagnosis();
      expect(issues.length, equals(1));
      expect(issues.first.type, equals(IssueType.danglingReference));
      expect(issues.first.itemId, equals('task_a'));
      
      // Should be able to fix it
      final fixed = doctor.fixIssues();
      expect(fixed.length, equals(1));
      
      // After fixing, should have no issues
      final afterFix = doctor.fullDiagnosis();
      expect(afterFix.length, equals(0));
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

/// Normalize item strings for comparison by removing extra whitespace
String _normalizeItemString(String item) {
  return item.trim().replaceAll(RegExp(r'\s+'), ' ');
}
