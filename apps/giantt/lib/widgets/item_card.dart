import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';

class ItemCard extends StatelessWidget {
  final GianttItem item;
  final VoidCallback? onTap;

  const ItemCard({
    super.key,
    required this.item,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 4.0),
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: _getStatusColor(),
          child: Text(
            item.status.symbol,
            style: const TextStyle(color: Colors.white),
          ),
        ),
        title: Row(
          children: [
            Expanded(
              child: Text(
                item.title,
                style: TextStyle(
                  decoration: item.occlude ? TextDecoration.lineThrough : null,
                  color: item.occlude ? Colors.grey : null,
                ),
              ),
            ),
            if (item.priority.symbol.isNotEmpty)
              Text(
                item.priority.symbol,
                style: TextStyle(
                  fontWeight: FontWeight.bold,
                  color: _getPriorityColor(),
                ),
              ),
          ],
        ),
        subtitle: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('ID: ${item.id}'),
            if (item.duration.totalSeconds > 0)
              Text('Duration: ${item.duration}'),
            if (item.charts.isNotEmpty)
              Text('Charts: ${item.charts.join(', ')}'),
            if (item.tags.isNotEmpty)
              Wrap(
                spacing: 4,
                children: item.tags.take(3).map((tag) {
                  return Chip(
                    label: Text(
                      tag,
                      style: const TextStyle(fontSize: 10),
                    ),
                    materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    visualDensity: VisualDensity.compact,
                  );
                }).toList(),
              ),
          ],
        ),
        trailing: item.occlude
            ? Icon(Icons.visibility_off, color: Colors.grey[400])
            : const Icon(Icons.chevron_right),
        onTap: onTap,
      ),
    );
  }

  Color _getStatusColor() {
    switch (item.status) {
      case GianttStatus.notStarted:
        return Colors.grey;
      case GianttStatus.inProgress:
        return Colors.blue;
      case GianttStatus.blocked:
        return Colors.red;
      case GianttStatus.completed:
        return Colors.green;
    }
  }

  Color _getPriorityColor() {
    switch (item.priority) {
      case GianttPriority.lowest:
      case GianttPriority.low:
        return Colors.green;
      case GianttPriority.neutral:
        return Colors.grey;
      case GianttPriority.unsure:
        return Colors.orange;
      case GianttPriority.medium:
        return Colors.amber;
      case GianttPriority.high:
        return Colors.deepOrange;
      case GianttPriority.critical:
        return Colors.red;
    }
  }
}
