import 'package:test/test.dart';
import '../../lib/src/validation/graph_doctor.dart';
import '../../lib/src/graph/giantt_graph.dart';
import '../../lib/src/models/giantt_item.dart';
import '../../lib/src/models/status.dart';
import '../../lib/src/models/priority.dart';
import '../../lib/src/models/duration.dart';

void main() {
  group('GraphDoctor Tests', () {
    late GianttGraph graph;
    late GraphDoctor doctor;

    setUp(() {
      graph = GianttGraph();
      doctor = GraphDoctor(graph);
    });

    group('Basic Health Checks', () {
      test('should find no issues in empty graph', () {
        final issues = doctor.fullDiagnosis();
        expect(issues, isEmpty);
      });

      test('should find no issues in healthy graph', () {
        // Create a simple healthy graph
        final item1 = GianttItem(
          id: 'task1',
          title: 'First Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final item2 = GianttItem(
          id: 'task2',
          title: 'Second Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'REQUIRES': ['task1']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final item3 = GianttItem(
          id: 'task1',
          title: 'First Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'BLOCKS': ['task2']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item3); // Updated item1 with BLOCKS relation
        graph.addItem(item2);

        final issues = doctor.fullDiagnosis();
        expect(issues, isEmpty);
      });
    });

    group('Dangling Reference Detection', () {
      test('should detect dangling REQUIRES reference', () {
        final item = GianttItem(
          id: 'task1',
          title: 'Task with bad dependency',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'REQUIRES': ['nonexistent']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item);
        final issues = doctor.fullDiagnosis();

        expect(issues, hasLength(1));
        expect(issues.first.type, equals(IssueType.danglingReference));
        expect(issues.first.itemId, equals('task1'));
        expect(issues.first.message, contains('nonexistent'));
        expect(issues.first.message, contains('requires'));
      });

      test('should detect dangling BLOCKS reference', () {
        final item = GianttItem(
          id: 'task1',
          title: 'Task with bad block',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'BLOCKS': ['missing_task']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item);
        final issues = doctor.fullDiagnosis();

        expect(issues, hasLength(1));
        expect(issues.first.type, equals(IssueType.danglingReference));
        expect(issues.first.relatedIds, contains('missing_task'));
      });

      test('should detect multiple dangling references', () {
        final item = GianttItem(
          id: 'task1',
          title: 'Task with multiple bad refs',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {
            'REQUIRES': ['missing1', 'missing2'],
            'BLOCKS': ['missing3']
          },
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item);
        final issues = doctor.fullDiagnosis();

        expect(issues, hasLength(3));
        expect(issues.every((issue) => issue.type == IssueType.danglingReference), isTrue);
      });
    });

    group('Incomplete Chain Detection', () {
      test('should detect item that blocks but is not required', () {
        final item1 = GianttItem(
          id: 'task1',
          title: 'Blocking Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'BLOCKS': ['task2']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final item2 = GianttItem(
          id: 'task2',
          title: 'Blocked Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {}, // Missing REQUIRES relation
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item1);
        graph.addItem(item2);
        final issues = doctor.fullDiagnosis();

        expect(issues, hasLength(1));
        expect(issues.first.type, equals(IssueType.incompleteChain));
        expect(issues.first.itemId, equals('task1'));
        expect(issues.first.message, contains('blocks'));
        expect(issues.first.message, contains('task2'));
        expect(issues.first.message, contains("isn't required by it"));
      });

      test('should detect item that requires but is not blocked', () {
        final item1 = GianttItem(
          id: 'task1',
          title: 'Required Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {}, // Missing BLOCKS relation
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final item2 = GianttItem(
          id: 'task2',
          title: 'Requiring Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'REQUIRES': ['task1']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item1);
        graph.addItem(item2);
        final issues = doctor.fullDiagnosis();

        expect(issues, hasLength(1));
        expect(issues.first.type, equals(IssueType.incompleteChain));
        expect(issues.first.itemId, equals('task2'));
        expect(issues.first.message, contains('requires'));
        expect(issues.first.message, contains('task1'));
        expect(issues.first.message, contains("isn't blocked by it"));
      });
    });

    group('Issue Fixing', () {
      test('should fix dangling reference by removing it', () {
        final item = GianttItem(
          id: 'task1',
          title: 'Task with bad dependency',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'REQUIRES': ['nonexistent', 'valid_task']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final validItem = GianttItem(
          id: 'valid_task',
          title: 'Valid Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item);
        graph.addItem(validItem);

        final issues = doctor.fullDiagnosis();
        expect(issues, hasLength(1));

        final fixedIssues = doctor.fixIssues();
        expect(fixedIssues, hasLength(1));

        // Check that the dangling reference was removed
        final updatedItem = graph.items['task1']!;
        expect(updatedItem.relations['REQUIRES'], equals(['valid_task']));
        expect(updatedItem.relations['REQUIRES'], isNot(contains('nonexistent')));

        // Verify no more issues
        final remainingIssues = doctor.fullDiagnosis();
        expect(remainingIssues, isEmpty);
      });

      test('should not fix issues that require manual intervention', () {
        // Create an incomplete chain that would require adding a relation
        final item1 = GianttItem(
          id: 'task1',
          title: 'Blocking Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'BLOCKS': ['task2']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final item2 = GianttItem(
          id: 'task2',
          title: 'Blocked Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item1);
        graph.addItem(item2);

        final issues = doctor.fullDiagnosis();
        expect(issues, hasLength(1));

        // This type of issue typically can't be auto-fixed
        final fixedIssues = doctor.fixIssues();
        expect(fixedIssues, isEmpty);

        // Issue should still remain
        final remainingIssues = doctor.fullDiagnosis();
        expect(remainingIssues, hasLength(1));
      });
    });

    group('Quick Check', () {
      test('should return issue count without detailed analysis', () {
        final item = GianttItem(
          id: 'task1',
          title: 'Task with bad dependency',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'REQUIRES': ['nonexistent']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item);
        final issueCount = doctor.quickCheck();

        expect(issueCount, equals(1));
      });
    });

    group('Issue Type Filtering', () {
      test('should get issues by specific type', () {
        final item1 = GianttItem(
          id: 'task1',
          title: 'Task with dangling ref',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'REQUIRES': ['nonexistent']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final item2 = GianttItem(
          id: 'task2',
          title: 'Blocking Task',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {'BLOCKS': ['task3']},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        final item3 = GianttItem(
          id: 'task3',
          title: 'Task without proper requires',
          status: GianttStatus.notStarted,
          priority: GianttPriority.neutral,
          duration: GianttDuration.zero(),
          charts: [],
          tags: [],
          relations: {},
          timeConstraints: const [],
          userComment: null,
          autoComment: null,
          occlude: false,
        );

        graph.addItem(item1);
        graph.addItem(item2);
        graph.addItem(item3);

        doctor.fullDiagnosis();

        final danglingIssues = doctor.getIssuesByType(IssueType.danglingReference);
        final chainIssues = doctor.getIssuesByType(IssueType.incompleteChain);

        expect(danglingIssues, hasLength(1));
        expect(chainIssues, hasLength(1));
        expect(danglingIssues.first.itemId, equals('task1'));
        expect(chainIssues.first.itemId, equals('task2'));
      });
    });
  });
}
