import 'dart:io';
import 'package:args/args.dart';
import '../models/giantt_item.dart';
import '../models/log_entry.dart';
import '../graph/giantt_graph.dart';
import '../logging/log_collection.dart';

/// Base interface for all CLI commands
abstract class Command {
  String get name;
  String get description;
  ArgParser get argParser;

  Future<void> execute(ArgResults args);
}

/// Generic base class for CLI commands with typed arguments
abstract class CliCommand<T> {
  const CliCommand();

  String get name;
  String get description;
  String get usage;

  /// Parse command-line arguments into typed args object
  T parseArgs(List<String> args);

  /// Execute the command with context
  Future<CommandResult<T>> execute(CommandContext context);
}

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

/// Exception thrown when a command fails
class CommandException implements Exception {
  const CommandException(this.message, [this.exitCode = 1]);
  
  final String message;
  final int exitCode;
  
  @override
  String toString() => message;
}

/// Utility functions for CLI commands
class CommandUtils {
  /// Get the default path for Giantt files
  static String getDefaultGianttPath({String filename = 'items.txt', bool occlude = false}) {
    final filepath = '${occlude ? 'occlude' : 'include'}/$filename';
    
    // First check for local .giantt directory
    final localPath = Directory('.giantt/$filepath');
    if (localPath.existsSync()) {
      return localPath.path;
    }
    
    // Fall back to home directory
    final homeDir = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
    if (homeDir != null) {
      final homePath = Directory('$homeDir/.giantt/$filepath');
      if (homePath.existsSync()) {
        return homePath.path;
      }
    }
    
    // If neither exists, throw an error
    throw CommandException(
      'No Giantt $filepath found. Please run \'giantt init\' or \'giantt init --dev\' first.'
    );
  }
  
  /// Confirm an action with the user
  static bool confirm(String message, {bool defaultValue = false}) {
    stdout.write('$message ${defaultValue ? '[Y/n]' : '[y/N]'}: ');
    final input = stdin.readLineSync()?.trim().toLowerCase() ?? '';
    
    if (input.isEmpty) return defaultValue;
    return input == 'y' || input == 'yes';
  }
  
  /// Print an error message to stderr
  static void printError(String message) {
    stderr.writeln('Error: $message');
  }
  
  /// Print a warning message
  static void printWarning(String message) {
    stderr.writeln('Warning: $message');
  }
  
  /// Print a success message
  static void printSuccess(String message) {
    print(message);
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
