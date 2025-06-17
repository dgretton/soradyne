import 'command_interface.dart';
import '../storage/dual_file_manager.dart';
import '../logging/log_occluder.dart';

/// Arguments for occlude command
class OccludeArgs {
  const OccludeArgs({
    this.itemIds = const [],
    this.tags = const [],
    this.sessionTags = const [],
    this.occludeItems = true,
    this.occludeLogs = false,
    this.dryRun = false,
    this.verbose = false,
  });

  final List<String> itemIds;
  final List<String> tags;
  final List<String> sessionTags;
  final bool occludeItems;
  final bool occludeLogs;
  final bool dryRun;
  final bool verbose;
}

/// Occlude items and/or logs with dry-run support
class OccludeCommand extends CliCommand<OccludeArgs> {
  const OccludeCommand();

  @override
  String get name => 'occlude';

  @override
  String get description => 'Occlude items and/or logs with dry-run support';

  @override
  String get usage => 'occlude [items|logs] [--ids=id1,id2] [--tags=tag1,tag2] [--sessions=s1,s2] [--dry-run] [--verbose]';

  @override
  OccludeArgs parseArgs(List<String> args) {
    List<String> itemIds = [];
    List<String> tags = [];
    List<String> sessionTags = [];
    bool occludeItems = true;
    bool occludeLogs = false;
    bool dryRun = false;
    bool verbose = false;

    for (final arg in args) {
      if (arg == 'items') {
        occludeItems = true;
        occludeLogs = false;
      } else if (arg == 'logs') {
        occludeItems = false;
        occludeLogs = true;
      } else if (arg.startsWith('--ids=')) {
        final idsStr = arg.substring(6);
        itemIds = idsStr.split(',').map((id) => id.trim()).where((id) => id.isNotEmpty).toList();
      } else if (arg.startsWith('--tags=')) {
        final tagsStr = arg.substring(7);
        tags = tagsStr.split(',').map((tag) => tag.trim()).where((tag) => tag.isNotEmpty).toList();
      } else if (arg.startsWith('--sessions=')) {
        final sessionsStr = arg.substring(11);
        sessionTags = sessionsStr.split(',').map((s) => s.trim()).where((s) => s.isNotEmpty).toList();
      } else if (arg == '--dry-run' || arg == '-n') {
        dryRun = true;
      } else if (arg == '--verbose' || arg == '-v') {
        verbose = true;
      } else {
        throw ArgumentError('Unknown argument: $arg');
      }
    }

    return OccludeArgs(
      itemIds: itemIds,
      tags: tags,
      sessionTags: sessionTags,
      occludeItems: occludeItems,
      occludeLogs: occludeLogs,
      dryRun: dryRun,
      verbose: verbose,
    );
  }

  @override
  Future<CommandResult<OccludeArgs>> execute(CommandContext context) async {
    try {
      final args = OccludeArgs(); // This will be set by parseArgs in CLI usage
      final results = <String>[];

      if (args.occludeItems) {
        // Load graph
        context.graph ??= DualFileManager.loadGraph(
          context.itemsPath,
          context.occludeItemsPath,
        );

        OccludeResult? itemResult;

        if (args.itemIds.isNotEmpty) {
          // Occlude by IDs
          itemResult = DualFileManager.occludeItems(
            context.graph!,
            args.itemIds,
            dryRun: context.dryRun || args.dryRun,
          );
        } else if (args.tags.isNotEmpty) {
          // Occlude by tags
          itemResult = DualFileManager.occludeItemsByTags(
            context.graph!,
            args.tags,
            dryRun: context.dryRun || args.dryRun,
          );
        }

        if (itemResult != null) {
          if (itemResult.hasOccluded) {
            final action = itemResult.dryRun ? 'Would occlude' : 'Occluded';
            results.add('$action ${itemResult.occludedCount} items');
            
            if (args.verbose || context.verbose) {
              results.add('  Items: ${itemResult.occludedItems.join(', ')}');
            }
          }

          if (itemResult.hasNotFound) {
            results.add('Items not found: ${itemResult.notFoundItems.join(', ')}');
          }

          // Save graph if not dry run
          if (!itemResult.dryRun) {
            DualFileManager.saveGraph(
              context.itemsPath,
              context.occludeItemsPath,
              context.graph!,
            );
          }
        } else {
          results.add('No items specified for occlusion');
        }
      }

      if (args.occludeLogs) {
        // Load logs
        context.logs ??= DualFileManager.loadLogs(
          context.logsPath,
          context.occludeLogsPath,
        );

        LogOccludeResult? logResult;

        if (args.sessionTags.isNotEmpty || args.tags.isNotEmpty) {
          // Occlude logs by session tags and/or regular tags
          logResult = LogOccluder.occludeBySession(
            context.logs!,
            args.sessionTags,
            dryRun: context.dryRun || args.dryRun,
          );

          // Also occlude by regular tags if specified
          if (args.tags.isNotEmpty) {
            final tagResult = LogOccluder.occludeByTags(
              context.logs!,
              args.tags,
              dryRun: context.dryRun || args.dryRun,
            );
            
            // Combine results (simplified)
            if (logResult.hasOccluded || tagResult.hasOccluded) {
              logResult = LogOccludeResult(
                occludedCount: logResult.occludedCount + tagResult.occludedCount,
                occludedLogs: [...logResult.occludedLogs, ...tagResult.occludedLogs],
                dryRun: logResult.dryRun,
              );
            }
          }
        }

        if (logResult != null) {
          if (logResult.hasOccluded) {
            final action = logResult.dryRun ? 'Would occlude' : 'Occluded';
            results.add('$action ${logResult.occludedCount} log entries');
            
            if (args.verbose || context.verbose) {
              results.add('  Sessions: ${logResult.occludedLogs.map((l) => l.session).toSet().join(', ')}');
            }
          }

          // Save logs if not dry run
          if (!logResult.dryRun) {
            DualFileManager.saveLogs(
              context.logsPath,
              context.occludeLogsPath,
              context.logs!,
            );
          }
        } else {
          results.add('No logs specified for occlusion');
        }
      }

      if (results.isEmpty) {
        return CommandResult.message('Nothing to occlude. Specify items or logs with appropriate filters.');
      }

      return CommandResult.success(args, results.join('\n'));

    } catch (e) {
      return CommandResult.failure('Failed to occlude: $e');
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<Map<String, dynamic>>> occludeContent(
    String workspacePath, {
    List<String> itemIds = const [],
    List<String> tags = const [],
    List<String> sessionTags = const [],
    bool occludeItems = true,
    bool occludeLogs = false,
    bool dryRun = false,
  }) async {
    final context = CommandContext(workspacePath: workspacePath, dryRun: dryRun);
    final results = <String, dynamic>{};

    if (occludeItems) {
      // Load graph
      context.graph = DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      OccludeResult? itemResult;

      if (itemIds.isNotEmpty) {
        itemResult = DualFileManager.occludeItems(
          context.graph!,
          itemIds,
          dryRun: dryRun,
        );
      } else if (tags.isNotEmpty) {
        itemResult = DualFileManager.occludeItemsByTags(
          context.graph!,
          tags,
          dryRun: dryRun,
        );
      }

      if (itemResult != null) {
        results['items'] = {
          'occluded_count': itemResult.occludedCount,
          'occluded_items': itemResult.occludedItems,
          'not_found_items': itemResult.notFoundItems,
          'dry_run': itemResult.dryRun,
        };

        // Save graph if not dry run
        if (!itemResult.dryRun) {
          DualFileManager.saveGraph(
            context.itemsPath,
            context.occludeItemsPath,
            context.graph!,
          );
        }
      }
    }

    if (occludeLogs) {
      // Load logs
      context.logs = DualFileManager.loadLogs(
        context.logsPath,
        context.occludeLogsPath,
      );

      if (sessionTags.isNotEmpty || tags.isNotEmpty) {
        final logResult = LogOccluder.occludeBySession(
          context.logs!,
          sessionTags,
          dryRun: dryRun,
        );

        results['logs'] = {
          'occluded_count': logResult.occludedCount,
          'occluded_sessions': logResult.occludedLogs.map((l) => l.session).toSet().toList(),
          'dry_run': logResult.dryRun,
        };

        // Save logs if not dry run
        if (!logResult.dryRun) {
          DualFileManager.saveLogs(
            context.logsPath,
            context.occludeLogsPath,
            context.logs!,
          );
        }
      }
    }

    final message = dryRun ? 'Analyzed occlusion candidates' : 'Occlusion completed';
    return CommandResult.success(results, message);
  }
}
