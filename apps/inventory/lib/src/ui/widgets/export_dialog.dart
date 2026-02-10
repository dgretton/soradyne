import 'dart:convert';
import 'package:flutter/material.dart';
import '../../core/inventory_api.dart';
import '../../services/export_service.dart';

/// Modal bottom sheet for exporting inventory data or edit history.
class ExportDialog extends StatelessWidget {
  final InventoryApi api;

  const ExportDialog({super.key, required this.api});

  static Future<void> show(BuildContext context, InventoryApi api) {
    return showModalBottomSheet(
      context: context,
      builder: (_) => ExportDialog(api: api),
    );
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Text(
                'Export',
                style: Theme.of(context).textTheme.titleLarge,
              ),
            ),
            const SizedBox(height: 8),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Text(
                'Inventory',
                style: Theme.of(context).textTheme.labelMedium?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ),
            _ExportTile(
              icon: Icons.copy,
              title: 'Copy to clipboard',
              onTap: () => _exportInventoryClipboard(context),
            ),
            _ExportTile(
              icon: Icons.save_alt,
              title: 'Save to file',
              onTap: () => _exportInventoryFile(context),
            ),
            _ExportTile(
              icon: Icons.share,
              title: 'Share...',
              onTap: () => _shareInventory(context),
            ),
            const Divider(height: 1, indent: 16, endIndent: 16),
            const SizedBox(height: 4),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Text(
                'Edit history',
                style: Theme.of(context).textTheme.labelMedium?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ),
            _ExportTile(
              icon: Icons.copy,
              title: 'Copy to clipboard',
              onTap: () => _exportHistoryClipboard(context),
            ),
            _ExportTile(
              icon: Icons.save_alt,
              title: 'Save to file',
              onTap: () => _exportHistoryFile(context),
            ),
            _ExportTile(
              icon: Icons.share,
              title: 'Share...',
              onTap: () => _shareHistory(context),
            ),
          ],
        ),
      ),
    );
  }

  String _getInventoryContent() {
    return api.exportToLegacyFormat();
  }

  String _getHistoryContent() {
    final rawJson = api.getOperationsJson();
    final decoded = jsonDecode(rawJson);
    return const JsonEncoder.withIndent('  ').convert(decoded);
  }

  String _timestamp() {
    return DateTime.now().toIso8601String().replaceAll(':', '-').split('.')[0];
  }

  Future<void> _exportInventoryClipboard(BuildContext context) async {
    Navigator.pop(context);
    try {
      await ExportService.copyToClipboard(_getInventoryContent());
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Inventory copied to clipboard'), backgroundColor: Colors.green),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }

  Future<void> _exportInventoryFile(BuildContext context) async {
    Navigator.pop(context);
    try {
      final path = await ExportService.saveToFile(_getInventoryContent(), 'inventory_export_${_timestamp()}.txt');
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Saved to: $path'), backgroundColor: Colors.green, duration: const Duration(seconds: 4)),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }

  Future<void> _shareInventory(BuildContext context) async {
    Navigator.pop(context);
    try {
      await ExportService.shareFile(_getInventoryContent(), 'inventory_export_${_timestamp()}.txt');
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }

  Future<void> _exportHistoryClipboard(BuildContext context) async {
    Navigator.pop(context);
    try {
      await ExportService.copyToClipboard(_getHistoryContent());
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Edit history copied to clipboard'), backgroundColor: Colors.green),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }

  Future<void> _exportHistoryFile(BuildContext context) async {
    Navigator.pop(context);
    try {
      final path = await ExportService.saveToFile(_getHistoryContent(), 'inventory_history_${_timestamp()}.json');
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Saved to: $path'), backgroundColor: Colors.green, duration: const Duration(seconds: 4)),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }

  Future<void> _shareHistory(BuildContext context) async {
    Navigator.pop(context);
    try {
      await ExportService.shareFile(_getHistoryContent(), 'inventory_history_${_timestamp()}.json');
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }
}

class _ExportTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final VoidCallback onTap;

  const _ExportTile({required this.icon, required this.title, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Icon(icon, size: 20),
      title: Text(title),
      dense: true,
      visualDensity: VisualDensity.compact,
      onTap: onTap,
    );
  }
}
