import '../models/giantt_item.dart';
import '../models/log_entry.dart';
import '../graph/giantt_graph.dart';
import '../logging/log_collection.dart';

/// Result of a command execution
class CommandResult<T> {
  const CommandResult({
    required this.success,
    this.data,
    this.message,
    this.error,
  });

  final bool success;
  final T? data;
  final String? message;
  final String? error;

  factory CommandResult.success(T data, [String? message]) {
    return CommandResult(success: true, data: data, message: message);
  }

  factory CommandResult.failure(String error) {
    return CommandResult(success: false, error: error);
  }

  factory CommandResult.message(String message) {
    return CommandResult(success: true, message: message);
  }
}

/// Context for command execution
class CommandContext {
  CommandContext({
    required this.workspacePath,
    this.graph,
    this.logs,
    this.dryRun = false,
    this.verbose = false,
  });

  final String workspacePath;
  GianttGraph? graph;
  LogCollection? logs;
  final bool dryRun;
  final bool verbose;

  String get itemsPath => '$workspacePath/items.txt';
  String get occludeItemsPath => '$workspacePath/occlude/items.txt';
  String get logsPath => '$workspacePath/logs.txt';
  String get occludeLogsPath => '$workspacePath/occlude/logs.txt';
}

/// Base interface for all commands
abstract class Command<T> {
  const Command();

  /// Execute the command
  Future<CommandResult<T>> execute(CommandContext context);

  /// Get command name for CLI
  String get name;

  /// Get command description for help
  String get description;

  /// Get command usage for help
  String get usage;
}

/// Interface for commands that can be used in CLI
abstract class CliCommand<T> extends Command<T> {
  const CliCommand();

  /// Parse arguments from CLI
  T parseArgs(List<String> args);

  /// Execute with parsed arguments
  Future<CommandResult<T>> executeWithArgs(CommandContext context, List<String> args) async {
    try {
      final parsedArgs = parseArgs(args);
      return await execute(context);
    } catch (e) {
      return CommandResult.failure('Invalid arguments: $e');
    }
  }
}
