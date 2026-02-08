import 'package:flutter/material.dart';

class CommandEditDialog extends StatefulWidget {
  final Map<String, dynamic> command;
  final bool readOnly;
  final String? trackedId;

  const CommandEditDialog({super.key, required this.command, this.readOnly = false, this.trackedId});

  @override
  State<CommandEditDialog> createState() => _CommandEditDialogState();
}

class _CommandEditDialogState extends State<CommandEditDialog> {
  late Map<String, TextEditingController> _controllers;
  late String _commandName;

  @override
  void initState() {
    super.initState();
    _commandName = widget.command['command'] as String? ?? 'unknown';
    final arguments = widget.command['arguments'] as Map<String, dynamic>? ?? {};
    _controllers = {};
    arguments.forEach((key, value) {
      if (value is List) {
        _controllers[key] = TextEditingController(text: value.join(', '));
      } else {
        _controllers[key] = TextEditingController(text: value?.toString() ?? '');
      }
    });
  }

  @override
  void dispose() {
    _controllers.values.forEach((controller) => controller.dispose());
    super.dispose();
  }

  void _saveAndClose() {
    final newArguments = <String, dynamic>{};
    final originalArguments = widget.command['arguments'] as Map<String, dynamic>? ?? {};

    _controllers.forEach((key, controller) {
      final originalValue = originalArguments[key];
      if (originalValue is bool) {
        newArguments[key] = controller.text.toLowerCase() == 'true';
      } else if (originalValue is List) {
        newArguments[key] = controller.text.split(',').map((s) => s.trim()).where((s) => s.isNotEmpty).toList();
      } else if (originalValue is num) {
        newArguments[key] = num.tryParse(controller.text) ?? controller.text;
      } else {
        newArguments[key] = controller.text;
      }
    });

    final newCommand = {
      'command': _commandName,
      'arguments': newArguments,
    };
    Navigator.of(context).pop(newCommand);
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      child: Padding(
        padding: const EdgeInsets.all(24.0),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              widget.readOnly ? 'Command: $_commandName' : 'Edit Command: $_commandName',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            if (widget.trackedId != null)
              Text(
                widget.trackedId!,
                style: TextStyle(
                  fontSize: 11,
                  fontFamily: 'monospace',
                  color: Theme.of(context).colorScheme.outline,
                ),
              ),
            const SizedBox(height: 24),
            Flexible(
              child: SingleChildScrollView(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: _controllers.entries.map((entry) {
                    return Padding(
                      padding: const EdgeInsets.only(bottom: 16.0),
                      child: TextField(
                        controller: entry.value,
                        readOnly: widget.readOnly,
                        decoration: InputDecoration(
                          labelText: entry.key,
                          border: const OutlineInputBorder(),
                        ),
                        maxLines: null,
                      ),
                    );
                  }).toList(),
                ),
              ),
            ),
            const SizedBox(height: 24),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: Text(widget.readOnly ? 'Close' : 'Cancel'),
                ),
                if (!widget.readOnly) ...[
                  const SizedBox(width: 8),
                  ElevatedButton(
                    onPressed: _saveAndClose,
                    child: const Text('Execute'),
                  ),
                ],
              ],
            )
          ],
        ),
      ),
    );
  }
}
