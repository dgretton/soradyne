import 'command_interface.dart';
import '../storage/backup_manager.dart';
import '../storage/path_resolver.dart';

/// Arguments for clean command
class CleanArgs {
  const CleanArgs({
    this.retentionCount = 3,
    this.dryRun = false,
    this.verbose = false,
  });

  final int retentionCount;
  final bool dryRun;
  final bool verbose;
}

/// Clean up backup files with configurable retention
class CleanCommand extends CliCommand<CleanArgs> {
  const CleanCommand();

  @override
  String get name => 'clean';

  @override
  String get description => 'Clean up backup files with configurable retention';

  @override
  String get usage => 'clean [--keep=N] [--dry-run] [--verbose]';

  @override
  CleanArgs parseArgs(List<String> args) {
    int retentionCount = 3;
    bool dryRun = false;
    bool verbose = false;

    for (final arg in args) {
      if (arg.startsWith('--keep=')) {
        final countStr = arg.substring(7);
        retentionCount = int.tryParse(countStr) ?? 3;
      } else if (arg == '--dry-run' || arg == '-n') {
        dryRun = true;
      } else if (arg == '--verbose' || arg == '-v') {
        verbose = true;
      } else {
        throw ArgumentError('Unknown argument: $arg');
      }
    }

    return CleanArgs(
      retentionCount: retentionCount,
      dryRun: dryRun,
      verbose: verbose,
    );
  }

  @override
  Future<CommandResult<CleanArgs>> execute(CommandContext context) async {
    try {
      final args = CleanArgs(); // This will be set by parseArgs in CLI usage

      // Check if workspace exists
      if (!PathResolver.gianttWorkspaceExists(context.workspacePath)) {
        return CommandResult.failure('No giantt workspace found at ${context.workspacePath}');
      }

      // Files to clean backups for
      final filesToClean = [
        context.itemsPath,
        context.occludeItemsPath,
        context.logsPath,
        context.occludeLogsPath,
      ];

      final results = <String>[];
      int totalBackupsFound = 0;
      int totalBackupsToRemove = 0;

      // Analyze what would be cleaned
      for (final filepath in filesToClean) {
        final allBackups = BackupManager.getAllBackups(filepath);
        totalBackupsFound += allBackups.length;
        
        if (allBackups.length > args.retentionCount) {
          final toRemove = allBackups.length - args.retentionCount;
          totalBackupsToRemove += toRemove;
          
          if (args.verbose || context.verbose) {
            results.add('$filepath: ${allBackups.length} backups, would remove $toRemove');
          }
        } else if (args.verbose || context.verbose) {
          results.add('$filepath: ${allBackups.length} backups, none to remove');
        }
      }

      if (context.dryRun || args.dryRun) {
        final message = StringBuffer();
        message.writeln('Would clean $totalBackupsToRemove of $totalBackupsFound backup files (keeping ${args.retentionCount} most recent)');
        if (results.isNotEmpty) {
          message.writeln('Details:');
          message.writeln(results.join('\n'));
        }
        return CommandResult.message(message.toString());
      }

      // Actually clean the backups
      BackupManager.cleanupAllBackups(filesToClean, retentionCount: args.retentionCount);

      final message = args.verbose || context.verbose
        ? 'Cleaned $totalBackupsToRemove of $totalBackupsFound backup files:\n${results.join('\n')}'
        : 'Cleaned $totalBackupsToRemove of $totalBackupsFound backup files (kept ${args.retentionCount} most recent)';

      return CommandResult.success(args, message);

    } catch (e) {
      return CommandResult.failure('Failed to clean backup files: $e');
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<Map<String, int>>> cleanBackups(
    String workspacePath, {
    int retentionCount = 3,
    bool dryRun = false,
  }) async {
    final context = CommandContext(workspacePath: workspacePath, dryRun: dryRun);
    
    // Check if workspace exists
    if (!PathResolver.gianttWorkspaceExists(workspacePath)) {
      return CommandResult.failure('No giantt workspace found at $workspacePath');
    }

    // Files to clean backups for
    final filesToClean = [
      context.itemsPath,
      context.occludeItemsPath,
      context.logsPath,
      context.occludeLogsPath,
    ];

    int totalBackupsFound = 0;
    int totalBackupsToRemove = 0;
    final fileStats = <String, int>{};

    // Analyze what would be cleaned
    for (final filepath in filesToClean) {
      final allBackups = BackupManager.getAllBackups(filepath);
      totalBackupsFound += allBackups.length;
      
      if (allBackups.length > retentionCount) {
        final toRemove = allBackups.length - retentionCount;
        totalBackupsToRemove += toRemove;
        fileStats[filepath] = toRemove;
      } else {
        fileStats[filepath] = 0;
      }
    }

    if (!dryRun) {
      // Actually clean the backups
      BackupManager.cleanupAllBackups(filesToClean, retentionCount: retentionCount);
    }

    final message = dryRun
      ? 'Would clean $totalBackupsToRemove of $totalBackupsFound backup files'
      : 'Cleaned $totalBackupsToRemove of $totalBackupsFound backup files';

    return CommandResult.message(message);
  }
}
