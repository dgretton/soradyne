import 'dart:io';
import 'command_interface.dart';
import '../storage/path_resolver.dart';
import '../storage/file_header_generator.dart';

/// Arguments for init command
class InitArgs {
  const InitArgs({
    this.force = false,
    this.homeMode = false,
  });

  final bool force;
  final bool homeMode;
}

/// Initialize a new giantt workspace
class InitCommand extends CliCommand<InitArgs> {
  const InitCommand();

  @override
  String get name => 'init';

  @override
  String get description => 'Initialize a new giantt workspace';

  @override
  String get usage => 'init [--force] [--home]';

  @override
  InitArgs parseArgs(List<String> args) {
    bool force = false;
    bool homeMode = false;

    for (final arg in args) {
      switch (arg) {
        case '--force':
        case '-f':
          force = true;
          break;
        case '--home':
        case '-h':
          homeMode = true;
          break;
        default:
          throw ArgumentError('Unknown argument: $arg');
      }
    }

    return InitArgs(force: force, homeMode: homeMode);
  }

  @override
  Future<CommandResult<InitArgs>> execute(CommandContext context) async {
    try {
      final workspacePath = context.workspacePath;
      final workspaceDir = Directory(workspacePath);

      // Check if workspace already exists
      if (workspaceDir.existsSync() && !context.dryRun) {
        final itemsFile = File('${workspacePath}/items.txt');
        if (itemsFile.existsSync()) {
          return CommandResult.failure(
            'Workspace already exists at $workspacePath. Use --force to reinitialize.'
          );
        }
      }

      if (context.dryRun) {
        return CommandResult.message(
          'Would initialize workspace at $workspacePath'
        );
      }

      // Create directory structure
      await _createDirectoryStructure(workspacePath);

      // Create initial files
      await _createInitialFiles(workspacePath);

      return CommandResult.success(
        InitArgs(),
        'Initialized giantt workspace at $workspacePath'
      );

    } catch (e) {
      return CommandResult.failure('Failed to initialize workspace: $e');
    }
  }

  /// Create the directory structure for a giantt workspace
  Future<void> _createDirectoryStructure(String workspacePath) async {
    final directories = [
      workspacePath,
      '$workspacePath/include',
      '$workspacePath/occlude',
    ];

    for (final dir in directories) {
      await Directory(dir).create(recursive: true);
    }
  }

  /// Create initial files with headers
  Future<void> _createInitialFiles(String workspacePath) async {
    // Create items.txt
    final itemsFile = File('$workspacePath/items.txt');
    await itemsFile.writeAsString(
      '${FileHeaderGenerator.generateItemsFileHeader()}\n\n'
    );

    // Create occlude/items.txt
    final occludeItemsFile = File('$workspacePath/occlude/items.txt');
    await occludeItemsFile.writeAsString(
      '${FileHeaderGenerator.generateOccludedItemsFileHeader()}\n\n'
    );

    // Create logs.txt
    final logsFile = File('$workspacePath/logs.txt');
    await logsFile.writeAsString(
      '${FileHeaderGenerator.generateLogsFileHeader()}\n\n'
    );

    // Create occlude/logs.txt
    final occludeLogsFile = File('$workspacePath/occlude/logs.txt');
    await occludeLogsFile.writeAsString(
      '${FileHeaderGenerator.generateOccludedLogsFileHeader()}\n\n'
    );
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<void>> initializeWorkspace(
    String workspacePath, {
    bool force = false,
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    final command = InitCommand();
    final result = await command.execute(context);
    
    return CommandResult<void>(
      success: result.success,
      message: result.message,
      error: result.error,
    );
  }
}
