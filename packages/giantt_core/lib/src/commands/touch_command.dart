import 'dart:io';
import 'command_interface.dart';
import '../storage/dual_file_manager.dart';
import '../storage/path_resolver.dart';

/// Arguments for touch command
class TouchArgs {
  const TouchArgs({
    this.validate = true,
    this.verbose = false,
  });

  final bool validate;
  final bool verbose;
}

/// Reload files and check consistency
class TouchCommand extends CliCommand<TouchArgs> {
  const TouchCommand();

  @override
  String get name => 'touch';

  @override
  String get description => 'Reload files and check consistency';

  @override
  String get usage => 'touch [--no-validate] [--verbose]';

  @override
  TouchArgs parseArgs(List<String> args) {
    bool validate = true;
    bool verbose = false;

    for (final arg in args) {
      switch (arg) {
        case '--no-validate':
          validate = false;
          break;
        case '--verbose':
        case '-v':
          verbose = true;
          break;
        default:
          throw ArgumentError('Unknown argument: $arg');
      }
    }

    return TouchArgs(validate: validate, verbose: verbose);
  }

  @override
  Future<CommandResult<TouchArgs>> execute(CommandContext context) async {
    try {
      final args = TouchArgs(); // This will be set by parseArgs in CLI usage
      final results = <String>[];

      // Check if workspace exists
      if (!PathResolver.gianttWorkspaceExists(context.workspacePath)) {
        return CommandResult.failure('No giantt workspace found at ${context.workspacePath}');
      }

      // Check file existence and accessibility
      final filesToCheck = [
        context.itemsPath,
        context.occludeItemsPath,
        context.logsPath,
        context.occludeLogsPath,
      ];

      for (final filepath in filesToCheck) {
        final file = File(filepath);
        if (file.existsSync()) {
          try {
            // Try to read the file to check accessibility
            final content = file.readAsStringSync();
            final lines = content.split('\n').length;
            results.add('✓ $filepath ($lines lines)');
          } catch (e) {
            results.add('✗ $filepath (read error: $e)');
          }
        } else {
          results.add('? $filepath (missing)');
        }
      }

      // Reload graph to check parsing
      if (args.validate) {
        try {
          final graph = DualFileManager.loadGraph(
            context.itemsPath,
            context.occludeItemsPath,
          );
          
          final itemCount = graph.items.length;
          final includedCount = graph.includedItems.length;
          final occludedCount = graph.occludedItems.length;
          
          results.add('✓ Graph loaded: $itemCount items ($includedCount included, $occludedCount occluded)');

          // Try topological sort to validate graph structure
          try {
            final sortedItems = graph.topologicalSort();
            results.add('✓ Graph structure valid (${sortedItems.length} items sorted)');
          } catch (e) {
            results.add('✗ Graph structure invalid: $e');
          }

          // Load logs
          try {
            final logs = DualFileManager.loadLogs(
              context.logsPath,
              context.occludeLogsPath,
            );
            final logCount = logs.length;
            final includedLogCount = logs.includedEntries.length;
            final occludedLogCount = logs.occludedEntries.length;
            
            results.add('✓ Logs loaded: $logCount entries ($includedLogCount included, $occludedLogCount occluded)');
          } catch (e) {
            results.add('✗ Log loading failed: $e');
          }

        } catch (e) {
          results.add('✗ Graph loading failed: $e');
        }
      }

      final message = args.verbose 
        ? 'File consistency check completed:\n${results.join('\n')}'
        : 'File consistency check completed (${results.where((r) => r.startsWith('✓')).length}/${results.length} checks passed)';

      return CommandResult.success(args, message);

    } catch (e) {
      return CommandResult.failure('Failed to check file consistency: $e');
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<Map<String, dynamic>>> checkConsistency(
    String workspacePath, {
    bool validate = true,
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    final results = <String, dynamic>{
      'workspace_exists': PathResolver.gianttWorkspaceExists(workspacePath),
      'files': <String, dynamic>{},
      'graph_valid': false,
      'logs_valid': false,
    };

    // Check files
    final filesToCheck = [
      context.itemsPath,
      context.occludeItemsPath,
      context.logsPath,
      context.occludeLogsPath,
    ];

    for (final filepath in filesToCheck) {
      final file = File(filepath);
      final filename = filepath.split('/').last;
      
      if (file.existsSync()) {
        try {
          final content = file.readAsStringSync();
          results['files'][filename] = {
            'exists': true,
            'readable': true,
            'lines': content.split('\n').length,
          };
        } catch (e) {
          results['files'][filename] = {
            'exists': true,
            'readable': false,
            'error': e.toString(),
          };
        }
      } else {
        results['files'][filename] = {
          'exists': false,
          'readable': false,
        };
      }
    }

    if (validate) {
      // Check graph
      try {
        final graph = DualFileManager.loadGraph(
          context.itemsPath,
          context.occludeItemsPath,
        );
        
        results['graph_valid'] = true;
        results['graph_stats'] = {
          'total_items': graph.items.length,
          'included_items': graph.includedItems.length,
          'occluded_items': graph.occludedItems.length,
        };

        // Check graph structure
        try {
          final sortedItems = graph.topologicalSort();
          results['graph_structure_valid'] = true;
          results['sorted_items_count'] = sortedItems.length;
        } catch (e) {
          results['graph_structure_valid'] = false;
          results['graph_structure_error'] = e.toString();
        }

      } catch (e) {
        results['graph_valid'] = false;
        results['graph_error'] = e.toString();
      }

      // Check logs
      try {
        final logs = DualFileManager.loadLogs(
          context.logsPath,
          context.occludeLogsPath,
        );
        
        results['logs_valid'] = true;
        results['log_stats'] = {
          'total_entries': logs.length,
          'included_entries': logs.includedEntries.length,
          'occluded_entries': logs.occludedEntries.length,
        };

      } catch (e) {
        results['logs_valid'] = false;
        results['logs_error'] = e.toString();
      }
    }

    return CommandResult.success(results, 'Consistency check completed');
  }
}
