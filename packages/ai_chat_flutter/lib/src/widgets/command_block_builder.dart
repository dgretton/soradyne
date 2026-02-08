import 'dart:convert';
import 'package:flutter/material.dart';
import '../models/tracked_command.dart';
import '../services/command_manager.dart';
import 'command_card.dart';

class CommandBlockParser {
  static List<Widget> parseMarkdownWithCommands(
    String content,
    BuildContext context,
    Future<void> Function(Map<String, dynamic>, {String? trackedId}) onExecute,
    bool isSending, {
    List<String> commandIds = const [],
    CommandManager? commandManager,
    Future<void> Function(Map<String, dynamic>, {String? trackedId})? onView,
  }) {
    final widgets = <Widget>[];
    int commandIndex = 0;

    // Split content by code blocks to handle them separately
    final codeBlockRegex = RegExp(r'```json\s*\n(.*?)\n```', dotAll: true);
    int lastEnd = 0;

    for (final match in codeBlockRegex.allMatches(content)) {
      // Add text before this code block
      if (match.start > lastEnd) {
        final textBefore = content.substring(lastEnd, match.start).trim();
        if (textBefore.isNotEmpty) {
          widgets.add(_buildTextWidget(textBefore, context));
        }
      }

      // Process the JSON code block
      final jsonContent = match.group(1)?.trim() ?? '';
      final result = _processJsonBlock(
        jsonContent, context, onExecute, isSending,
        commandIndex: commandIndex,
        commandIds: commandIds,
        commandManager: commandManager,
        onView: onView,
      );
      widgets.add(result.widget);
      if (result.isCommand) commandIndex++;

      lastEnd = match.end;
    }

    // Add remaining text after last code block
    if (lastEnd < content.length) {
      final remainingText = content.substring(lastEnd).trim();
      if (remainingText.isNotEmpty) {
        widgets.add(_buildTextWidget(remainingText, context));
      }
    }

    // If no code blocks were found, just return the text
    if (widgets.isEmpty) {
      widgets.add(_buildTextWidget(content, context));
    }

    return widgets;
  }

  static ({Widget widget, bool isCommand}) _processJsonBlock(
    String jsonContent,
    BuildContext context,
    Future<void> Function(Map<String, dynamic>, {String? trackedId}) onExecute,
    bool isSending, {
    required int commandIndex,
    required List<String> commandIds,
    required CommandManager? commandManager,
    Future<void> Function(Map<String, dynamic>, {String? trackedId})? onView,
  }) {
    try {
      final command = jsonDecode(jsonContent) as Map<String, dynamic>;
      if (command.containsKey('command') && command.containsKey('arguments')) {
        // Look up the tracked command by index into commandIds
        TrackedCommand? tracked;
        String? trackedId;
        if (commandIndex < commandIds.length && commandManager != null) {
          trackedId = commandIds[commandIndex];
          tracked = commandManager.findById(trackedId);
        }

        if (tracked != null) {
          return (
            widget: CommandCard(
              tracked: tracked,
              isSending: isSending,
              showId: false,
              onExecute: () => onExecute(command, trackedId: trackedId),
              onView: onView != null
                  ? () => onView(command, trackedId: trackedId)
                  : null,
            ),
            isCommand: true,
          );
        }

        // Fallback for old messages without tracked IDs
        return (
          widget: _buildUnlinkedCommandWidget(command, onExecute, isSending, onView: onView),
          isCommand: true,
        );
      }
    } catch (e) {
      debugPrint('Failed to parse JSON command: $e');
    }

    // Fallback to code block display
    return (widget: _buildCodeBlock(jsonContent, context), isCommand: false);
  }

  static Widget _buildUnlinkedCommandWidget(
    Map<String, dynamic> command,
    Future<void> Function(Map<String, dynamic>, {String? trackedId}) onExecute,
    bool isSending, {
    Future<void> Function(Map<String, dynamic>, {String? trackedId})? onView,
  }) {
    return Builder(
      builder: (context) {
        final commandName = command['command'] as String? ?? 'unknown';
        final args = command['arguments'] as Map<String, dynamic>? ?? {};

        final descVal = args['description'];
        final searchStrVal = args['search_str'] ?? args['searchStr'];
        final preview = (descVal is String ? descVal : null) ??
                        (searchStrVal is String ? searchStrVal : null) ?? '';
        final dividerColor = Theme.of(context).colorScheme.outline.withValues(alpha: 0.2);

        return Card(
          margin: const EdgeInsets.symmetric(vertical: 4),
          elevation: 2,
          child: InkWell(
            onTap: isSending
                ? (onView != null ? () => onView(command) : null)
                : () => onExecute(command),
            borderRadius: BorderRadius.circular(12),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: IntrinsicHeight(
                child: Row(
                  children: [
                    Icon(Icons.play_circle_outline, size: 20,
                         color: Theme.of(context).colorScheme.secondary),
                    VerticalDivider(width: 16, thickness: 1, color: dividerColor),
                    Text(
                      commandName,
                      style: const TextStyle(
                        fontWeight: FontWeight.w600,
                        fontSize: 13,
                      ),
                    ),
                    if (preview.isNotEmpty) ...[
                      VerticalDivider(width: 16, thickness: 1, color: dividerColor),
                      Expanded(
                        child: Text(
                          preview,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: 12,
                            color: Theme.of(context).colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ),
                    ] else
                      const Spacer(),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  static Widget _buildCodeBlock(String code, BuildContext context) {
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 8),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: Theme.of(context).colorScheme.outline.withValues(alpha: 0.2),
        ),
      ),
      width: double.infinity,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: Text(
          code,
          style: const TextStyle(fontFamily: 'monospace'),
        ),
      ),
    );
  }

  static Widget _buildTextWidget(String text, BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: SelectableText(
        text,
        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
          color: Theme.of(context).colorScheme.onSurface,
        ),
      ),
    );
  }
}
