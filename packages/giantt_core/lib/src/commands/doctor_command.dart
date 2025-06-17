import 'command_interface.dart';
import '../storage/dual_file_manager.dart';
import '../validation/graph_doctor.dart';

/// Arguments for doctor command
class DoctorArgs {
  const DoctorArgs({
    this.autoFix = false,
    this.issueType,
    this.itemId,
    this.verbose = false,
  });

  final bool autoFix;
  final String? issueType;
  final String? itemId;
  final bool verbose;
}

/// Graph health checking with auto-fix capabilities
class DoctorCommand extends CliCommand<DoctorArgs> {
  const DoctorCommand();

  @override
  String get name => 'doctor';

  @override
  String get description => 'Graph health checking with auto-fix capabilities';

  @override
  String get usage => 'doctor [--fix] [--type=issue_type] [--item=item_id] [--verbose]';

  @override
  DoctorArgs parseArgs(List<String> args) {
    bool autoFix = false;
    String? issueType;
    String? itemId;
    bool verbose = false;

    for (final arg in args) {
      if (arg == '--fix') {
        autoFix = true;
      } else if (arg.startsWith('--type=')) {
        issueType = arg.substring(7);
      } else if (arg.startsWith('--item=')) {
        itemId = arg.substring(7);
      } else if (arg == '--verbose' || arg == '-v') {
        verbose = true;
      } else {
        throw ArgumentError('Unknown argument: $arg');
      }
    }

    return DoctorArgs(
      autoFix: autoFix,
      issueType: issueType,
      itemId: itemId,
      verbose: verbose,
    );
  }

  @override
  Future<CommandResult<DoctorArgs>> execute(CommandContext context) async {
    try {
      // Load graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = DoctorArgs(); // This will be set by parseArgs in CLI usage
      final doctor = GraphDoctor(context.graph!);

      // Run diagnosis
      final issues = doctor.fullDiagnosis();
      
      if (issues.isEmpty) {
        return CommandResult.success(args, '✓ No issues found. Graph is healthy!');
      }

      final buffer = StringBuffer();
      buffer.writeln('Found ${issues.length} issue(s):');
      buffer.writeln();

      // Group issues by type
      final issuesByType = <IssueType, List<Issue>>{};
      for (final issue in issues) {
        issuesByType.putIfAbsent(issue.type, () => []).add(issue);
      }

      // Display issues
      for (final entry in issuesByType.entries) {
        final type = entry.key;
        final typeIssues = entry.value;
        
        buffer.writeln('${_getIssueIcon(type)} ${_getIssueTypeName(type)} (${typeIssues.length})');
        
        for (final issue in typeIssues) {
          buffer.writeln('  • ${issue.itemId}: ${issue.message}');
          if (issue.suggestedFix != null && (args.verbose || context.verbose)) {
            buffer.writeln('    Fix: ${issue.suggestedFix}');
          }
        }
        buffer.writeln();
      }

      // Auto-fix if requested
      if (args.autoFix) {
        final issueTypeFilter = args.issueType != null 
          ? IssueType.fromString(args.issueType!)
          : null;
        
        final fixedIssues = doctor.fixIssues(
          issueType: issueTypeFilter,
          itemId: args.itemId,
        );

        if (fixedIssues.isNotEmpty) {
          buffer.writeln('🔧 Fixed ${fixedIssues.length} issue(s):');
          for (final issue in fixedIssues) {
            buffer.writeln('  ✓ ${issue.itemId}: ${issue.message}');
          }
          buffer.writeln();

          // Save the fixed graph
          DualFileManager.saveGraph(
            context.itemsPath,
            context.occludeItemsPath,
            context.graph!,
          );

          buffer.writeln('Graph saved with fixes applied.');
        } else {
          buffer.writeln('No issues could be automatically fixed.');
        }
      } else {
        buffer.writeln('Run with --fix to automatically repair issues where possible.');
      }

      return CommandResult.success(args, buffer.toString());

    } catch (e) {
      return CommandResult.failure('Failed to run health check: $e');
    }
  }

  String _getIssueIcon(IssueType type) {
    switch (type) {
      case IssueType.danglingReference:
        return '🔗';
      case IssueType.orphanedItem:
        return '🏝️';
      case IssueType.incompleteChain:
        return '⛓️';
      case IssueType.chartInconsistency:
        return '📊';
      case IssueType.tagInconsistency:
        return '🏷️';
    }
  }

  String _getIssueTypeName(IssueType type) {
    switch (type) {
      case IssueType.danglingReference:
        return 'Dangling References';
      case IssueType.orphanedItem:
        return 'Orphaned Items';
      case IssueType.incompleteChain:
        return 'Incomplete Chains';
      case IssueType.chartInconsistency:
        return 'Chart Inconsistencies';
      case IssueType.tagInconsistency:
        return 'Tag Inconsistencies';
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<Map<String, dynamic>>> checkHealth(
    String workspacePath, {
    bool autoFix = false,
    String? issueType,
    String? itemId,
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    // Load graph
    context.graph = DualFileManager.loadGraph(
      context.itemsPath,
      context.occludeItemsPath,
    );

    final doctor = GraphDoctor(context.graph!);
    final issues = doctor.fullDiagnosis();

    final results = <String, dynamic>{
      'total_issues': issues.length,
      'issues_by_type': <String, int>{},
      'issues': <Map<String, dynamic>>[],
      'fixed_issues': <Map<String, dynamic>>[],
    };

    // Group issues by type
    final issuesByType = <IssueType, List<Issue>>{};
    for (final issue in issues) {
      issuesByType.putIfAbsent(issue.type, () => []).add(issue);
    }

    // Convert to results format
    for (final entry in issuesByType.entries) {
      results['issues_by_type'][entry.key.value] = entry.value.length;
    }

    for (final issue in issues) {
      results['issues'].add({
        'type': issue.type.value,
        'item_id': issue.itemId,
        'message': issue.message,
        'related_ids': issue.relatedIds,
        'suggested_fix': issue.suggestedFix,
      });
    }

    // Auto-fix if requested
    if (autoFix && issues.isNotEmpty) {
      final issueTypeFilter = issueType != null 
        ? IssueType.fromString(issueType)
        : null;
      
      final fixedIssues = doctor.fixIssues(
        issueType: issueTypeFilter,
        itemId: itemId,
      );

      for (final issue in fixedIssues) {
        results['fixed_issues'].add({
          'type': issue.type.value,
          'item_id': issue.itemId,
          'message': issue.message,
        });
      }

      if (fixedIssues.isNotEmpty) {
        // Save the fixed graph
        DualFileManager.saveGraph(
          context.itemsPath,
          context.occludeItemsPath,
          context.graph!,
        );
        results['graph_saved'] = true;
      }
    }

    final message = issues.isEmpty 
      ? 'No issues found. Graph is healthy!'
      : 'Found ${issues.length} issue(s)${autoFix && results['fixed_issues'].isNotEmpty ? ', fixed ${results['fixed_issues'].length}' : ''}';

    return CommandResult.success(results, message);
  }
}
