import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';
import '../models/tracked_command.dart';

/// Manages tracked commands across a chat session.
/// Commands are parsed from LLM responses, assigned IDs, and their
/// execution state is tracked.
class CommandManager {
  static const _storageKey = 'tracked_commands';

  final List<TrackedCommand> _commands = [];

  List<TrackedCommand> get commands => List.unmodifiable(_commands);

  /// Non-archived commands — the active panel data source.
  List<TrackedCommand> get activeCommands =>
      _commands.where((c) => !c.archived).toList();

  /// Archived commands — shown in the "Previously..." section.
  List<TrackedCommand> get archivedCommands =>
      _commands.where((c) => c.archived).toList();

  List<TrackedCommand> get pendingCommands =>
      _commands.where((c) => !c.archived && c.status == CommandStatus.pending).toList();

  List<TrackedCommand> get selectedCommands =>
      _commands.where((c) => !c.archived && (c.status == CommandStatus.pending || c.status == CommandStatus.errored)).toList();

  List<TrackedCommand> get skippedCommands =>
      _commands.where((c) => !c.archived && c.status == CommandStatus.skipped).toList();

  List<TrackedCommand> get executedCommands =>
      _commands.where((c) => !c.archived && c.status == CommandStatus.executed).toList();

  int get totalCount => activeCommands.length;
  int get executedCount => executedCommands.length;
  int get pendingCount => pendingCommands.length;
  int get skippedCount => skippedCommands.length;

  /// Adds a new command parsed from LLM response.
  TrackedCommand addCommand(Map<String, dynamic> commandJson, {String? replacesId}) {
    final tracked = TrackedCommand(command: commandJson, replacesId: replacesId);
    _commands.add(tracked);
    return tracked;
  }

  /// Parses commands from LLM response text and adds them.
  /// Returns the list of newly added TrackedCommands.
  List<TrackedCommand> parseAndAddCommands(String responseText) {
    final newCommands = <TrackedCommand>[];

    // Match JSON code blocks
    final codeBlockPattern = RegExp(r'```json\s*([\s\S]*?)\s*```');
    final matches = codeBlockPattern.allMatches(responseText);

    for (final match in matches) {
      final jsonStr = match.group(1)?.trim();
      if (jsonStr == null || jsonStr.isEmpty) continue;

      try {
        final parsed = jsonDecode(jsonStr);
        if (parsed is Map<String, dynamic> && parsed.containsKey('command')) {
          final replacesId = parsed['replaces'] as String?;
          final tracked = addCommand(parsed, replacesId: replacesId);
          newCommands.add(tracked);
        }
      } catch (e) {
        // Skip malformed JSON
        continue;
      }
    }

    return newCommands;
  }

  /// Finds a command by ID.
  TrackedCommand? findById(String id) {
    try {
      return _commands.firstWhere((c) => c.id == id);
    } catch (_) {
      return null;
    }
  }

  /// Marks a command as executed.
  void markExecuted(String id) {
    final cmd = findById(id);
    if (cmd != null) {
      cmd.status = CommandStatus.executed;
      cmd.executedAt = DateTime.now();
    }
  }

  /// Marks a command as errored.
  void markErrored(String id, String errorMessage) {
    final cmd = findById(id);
    if (cmd != null) {
      cmd.status = CommandStatus.errored;
      cmd.errorMessage = errorMessage;
    }
  }

  /// Toggles a command between pending and skipped.
  void toggleSkipped(String id) {
    final cmd = findById(id);
    if (cmd != null) {
      if (cmd.status == CommandStatus.skipped) {
        cmd.status = CommandStatus.pending;
      } else if (cmd.status == CommandStatus.pending) {
        cmd.status = CommandStatus.skipped;
      }
      // Don't toggle executed or errored commands
    }
  }

  /// Skips a command.
  void skip(String id) {
    final cmd = findById(id);
    if (cmd != null && cmd.status == CommandStatus.pending) {
      cmd.status = CommandStatus.skipped;
    }
  }

  /// Unskips a command (back to pending).
  void unskip(String id) {
    final cmd = findById(id);
    if (cmd != null && cmd.status == CommandStatus.skipped) {
      cmd.status = CommandStatus.pending;
    }
  }

  /// Gets the next pending (non-skipped) command.
  TrackedCommand? getNextPending() {
    try {
      return _commands.firstWhere((c) => c.status == CommandStatus.pending);
    } catch (_) {
      return null;
    }
  }

  /// Builds context string for LLM showing active commands and their statuses.
  String buildContextForLLM() {
    final active = activeCommands;
    if (active.isEmpty) {
      return 'COMMANDS IN THIS SESSION: (none)';
    }

    final buffer = StringBuffer('COMMANDS IN THIS SESSION:\n');
    for (final cmd in active) {
      final statusStr = '[${cmd.status.name}]';
      buffer.writeln('${cmd.id} $statusStr ${cmd.summary}');
      if (cmd.errorMessage != null) {
        buffer.writeln('  Error: ${cmd.errorMessage}');
      }
      if (cmd.replacesId != null) {
        buffer.writeln('  (replaces ${cmd.replacesId})');
      }
    }
    return buffer.toString();
  }

  /// Formats selected commands for sending to chat.
  String formatCommandsForChat(List<String> commandIds) {
    final cmds = commandIds.map(findById).whereType<TrackedCommand>().toList();
    if (cmds.isEmpty) return '';

    final ids = cmds.map((c) => c.id).join(', ');
    return 'Commands needing revision: $ids';
  }

  /// Archives all executed and skipped commands.
  void archiveCompleted() {
    for (final cmd in _commands) {
      if (cmd.status == CommandStatus.executed || cmd.status == CommandStatus.skipped) {
        cmd.archived = true;
      }
    }
  }

  /// Archives all commands (e.g., on new chat).
  void archiveAll() {
    for (final cmd in _commands) {
      cmd.archived = true;
    }
  }

  /// Clears all commands (e.g., for new session).
  void clear() {
    _commands.clear();
  }

  /// Persists commands to storage.
  Future<void> save() async {
    final prefs = await SharedPreferences.getInstance();
    final jsonList = _commands.map((c) => c.toJson()).toList();
    await prefs.setString(_storageKey, jsonEncode(jsonList));
  }

  /// Loads commands from storage.
  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final jsonStr = prefs.getString(_storageKey);
    if (jsonStr == null) return;

    try {
      final jsonList = jsonDecode(jsonStr) as List<dynamic>;
      _commands.clear();
      for (final item in jsonList) {
        _commands.add(TrackedCommand.fromJson(item as Map<String, dynamic>));
      }
    } catch (e) {
      // If loading fails, start fresh
      _commands.clear();
    }
  }
}
