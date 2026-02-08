import 'package:flutter/material.dart';
import 'command_edit_dialog.dart';

class CommandWidget extends StatelessWidget {
  final Map<String, dynamic> command;
  final Future<void> Function(Map<String, dynamic> command) onExecute;
  final bool isSending;

  const CommandWidget({
    super.key,
    required this.command,
    required this.onExecute,
    required this.isSending,
  });

  @override
  Widget build(BuildContext context) {
    final commandName = command['command'] as String? ?? 'unknown';
    final args = command['arguments'] as Map<String, dynamic>? ?? {};

    final descVal = args['description'];
    final searchStrVal = args['search_str'] ?? args['searchStr'];
    final description = (descVal is String ? descVal : null) ?? (searchStrVal is String ? searchStrVal : null) ?? '';

    return Card(
      margin: const EdgeInsets.symmetric(vertical: 8),
      elevation: 2,
      child: InkWell(
        onTap: isSending
            ? null
            : () async {
                final editedCommand = await showDialog<Map<String, dynamic>>(
                  context: context,
                  builder: (context) => CommandEditDialog(command: command),
                );
                if (editedCommand != null) {
                  await onExecute(editedCommand);
                }
              },
        child: Padding(
          padding: const EdgeInsets.all(12.0),
          child: Row(
            children: [
              Icon(Icons.play_circle_outline, color: Theme.of(context).colorScheme.secondary),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Command: $commandName',
                      style: Theme.of(context).textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                    ),
                    if (description.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Text(
                        description,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ],
                ),
              ),
              const Icon(Icons.edit, size: 16),
            ],
          ),
        ),
      ),
    );
  }
}
