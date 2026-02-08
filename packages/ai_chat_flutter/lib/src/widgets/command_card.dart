import 'package:flutter/material.dart';
import '../models/tracked_command.dart';

class CommandCard extends StatelessWidget {
  final TrackedCommand tracked;
  final Future<void> Function()? onExecute;
  final Future<void> Function()? onView;
  final VoidCallback? onToggleSkip;
  final bool isSending;
  final bool showId;

  const CommandCard({
    super.key,
    required this.tracked,
    this.onExecute,
    this.onView,
    this.onToggleSkip,
    this.isSending = false,
    this.showId = true,
  });

  @override
  Widget build(BuildContext context) {
    final isExecuted = tracked.status == CommandStatus.executed;
    final isSkipped = tracked.status == CommandStatus.skipped;
    final isErrored = tracked.status == CommandStatus.errored;
    final isPending = tracked.status == CommandStatus.pending;
    final canExecute = (isPending || isErrored) && !isSending;

    Color? tint;
    if (isExecuted) {
      tint = Colors.green.withValues(alpha: 0.08);
    } else if (isErrored) {
      tint = Colors.red.withValues(alpha: 0.08);
    } else if (isSkipped) {
      tint = Colors.grey.withValues(alpha: 0.08);
    }

    IconData statusIcon;
    Color statusColor;
    if (isExecuted) {
      statusIcon = Icons.check_circle;
      statusColor = Colors.green;
    } else if (isErrored) {
      statusIcon = Icons.error;
      statusColor = Colors.red;
    } else if (isSkipped) {
      statusIcon = Icons.remove_circle_outline;
      statusColor = Theme.of(context).colorScheme.outline;
    } else {
      statusIcon = Icons.play_circle_outline;
      statusColor = Theme.of(context).colorScheme.secondary;
    }

    final dividerColor = Theme.of(context).colorScheme.outline.withValues(alpha: 0.2);
    final preview = tracked.preview;

    return Card(
      margin: const EdgeInsets.symmetric(vertical: 4, horizontal: 0),
      elevation: isPending ? 2 : 0,
      color: tint ?? Theme.of(context).cardColor,
      child: InkWell(
        onTap: canExecute
            ? onExecute
            : onView,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: IntrinsicHeight(
            child: Row(
              children: [
                if (onToggleSkip != null) ...[
                  SizedBox(
                    width: 28,
                    height: 28,
                    child: Checkbox(
                      value: !isSkipped,
                      onChanged: (isPending || isSkipped) ? (_) => onToggleSkip!() : null,
                    ),
                  ),
                  VerticalDivider(width: 16, thickness: 1, color: dividerColor),
                ],
                // Status icon
                Icon(statusIcon, size: 20, color: statusColor),
                VerticalDivider(width: 16, thickness: 1, color: dividerColor),
                // Command name + tiny ID (when shown)
                Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      tracked.commandName,
                      style: TextStyle(
                        fontWeight: FontWeight.w600,
                        fontSize: 13,
                        color: isSkipped
                            ? Theme.of(context).colorScheme.outline
                            : null,
                      ),
                    ),
                    if (showId)
                      Text(
                        tracked.id,
                        style: TextStyle(
                          fontSize: 9,
                          color: Theme.of(context).colorScheme.outline,
                          fontFamily: 'monospace',
                        ),
                      ),
                  ],
                ),
                if (preview.isNotEmpty) ...[
                  VerticalDivider(width: 16, thickness: 1, color: dividerColor),
                  // Preview text
                  Expanded(
                    child: Text(
                      preview,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 12,
                        decoration: isSkipped ? TextDecoration.lineThrough : null,
                        color: isSkipped
                            ? Theme.of(context).colorScheme.outline
                            : Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ] else
                  const Spacer(),
                if (isErrored && tracked.errorMessage != null) ...[
                  const SizedBox(width: 8),
                  Tooltip(
                    message: tracked.errorMessage!,
                    child: Icon(Icons.warning_amber_rounded, size: 16,
                        color: Theme.of(context).colorScheme.error),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
