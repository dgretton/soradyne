import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import '../core/inventory_api.dart';
import '../core/models/inventory_entry.dart';

/// Query commands that are auto-executed in the ReAct loop.
const _queryCommands = {
  'search',
  'show',
  'count',
  'list-tags',
  'list-containers',
};

/// Implements [ChatCommandProcessor] for the inventory app.
///
/// Handles query classification, execution, and summary/preview formatting.
/// The existing [ChatProcessor] is kept for action execution via `processCommand`.
class InventoryChatCommandProcessor implements ChatCommandProcessor {
  final InventoryApi _inventoryApi;

  InventoryChatCommandProcessor(this._inventoryApi);

  @override
  bool isQuery(String commandName) => _queryCommands.contains(commandName);

  @override
  Future<String?> executeQuery(Map<String, dynamic> command) async {
    final name = command['command'] as String?;
    if (name == null || !isQuery(name)) return null;

    final args = command['arguments'] as Map<String, dynamic>? ?? {};

    try {
      switch (name) {
        case 'search':
          return await _search(args);
        case 'show':
          return await _show(args);
        case 'count':
          return _count(args);
        case 'list-tags':
          return _listTags();
        case 'list-containers':
          return _listContainers();
        default:
          return 'Unknown query command: $name';
      }
    } catch (e) {
      return 'Error executing $name: $e';
    }
  }

  @override
  String commandSummary(String commandName, Map<String, dynamic> args) {
    switch (commandName) {
      // Action commands (existing inventory commands)
      case 'add':
        final desc = args['description'] ?? '?';
        final loc = args['location'] ?? '?';
        return 'add "$desc" to $loc';
      case 'delete':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        return 'delete "$search"';
      case 'move':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final loc = args['new_location'] ?? args['newLocation'] ?? '?';
        return 'move "$search" to $loc';
      case 'edit-description':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final newDesc = args['new_description'] ?? args['newDescription'] ?? '?';
        return 'edit "$search" to "$newDesc"';
      case 'put-in':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final container = args['container_id'] ?? args['containerId'] ?? '?';
        return 'put "$search" in $container';
      case 'remove-from-container':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        return 'remove "$search" from container';
      case 'create-container':
        final containerId = args['container_id'] ?? args['containerId'] ?? '?';
        final loc = args['location'] ?? '?';
        return 'create container $containerId at $loc';
      case 'add-tag':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final tag = args['tag'] ?? '?';
        return 'add tag "$tag" to "$search"';
      case 'remove-tag':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final tag = args['tag'] ?? '?';
        return 'remove tag "$tag" from "$search"';
      case 'group-put-in':
        final tag = args['tag'] ?? '?';
        final container = args['container_id'] ?? args['containerId'] ?? '?';
        return 'put all tagged "$tag" in $container';
      case 'group-remove-tag':
        final tag = args['tag'] ?? '?';
        return 'remove tag "$tag" from all';
      // Query commands
      case 'search':
        return 'search "${args['query'] ?? '?'}"';
      case 'show':
        return 'show "${args['search_str'] ?? args['searchStr'] ?? '?'}"';
      case 'count':
        final filters = <String>[];
        if (args['tag'] != null) filters.add('tag:${args['tag']}');
        if (args['category'] != null) filters.add('category:${args['category']}');
        return 'count${filters.isEmpty ? '' : ' (${filters.join(', ')})'}';
      case 'list-tags':
        return 'list-tags';
      case 'list-containers':
        return 'list-containers';
      default:
        return '$commandName(...)';
    }
  }

  @override
  String commandPreview(String commandName, Map<String, dynamic> args) {
    final desc = args['description'];
    final search = args['search_str'] ?? args['searchStr'];
    final query = args['query'];
    final tag = args['tag'];
    return (desc is String ? desc : null) ??
        (search is String ? search : null) ??
        (query is String ? query : null) ??
        (tag is String ? tag : null) ??
        '';
  }

  // -- Query implementations --

  Future<String> _search(Map<String, dynamic> args) async {
    final query = args['query'] as String? ?? '';
    final results = await _inventoryApi.search(query);
    if (results.isEmpty) return 'No items found matching "$query".';
    return _formatCompactList(results);
  }

  Future<String> _show(Map<String, dynamic> args) async {
    final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
    if (searchStr == null) return 'Missing search_str argument.';
    final entry = await _inventoryApi.findUniqueEntry(searchStr);
    return _formatItemDetail(entry);
  }

  String _count(Map<String, dynamic> args) {
    final tag = args['tag'] as String?;
    final category = args['category'] as String?;
    var entries = _inventoryApi.currentState.values;

    if (tag != null) {
      entries = entries.where((e) => e.tags.contains(tag));
    }
    if (category != null) {
      entries = entries.where(
        (e) => e.category.toLowerCase() == category.toLowerCase(),
      );
    }

    final count = entries.length;
    final filters = <String>[];
    if (tag != null) filters.add('tag:"$tag"');
    if (category != null) filters.add('category:"$category"');
    final filterStr = filters.isEmpty ? '' : ' (${filters.join(', ')})';
    return '$count items$filterStr';
  }

  String _listTags() {
    final allTags = <String>{};
    for (final entry in _inventoryApi.currentState.values) {
      allTags.addAll(entry.tags);
    }
    if (allTags.isEmpty) return 'No tags in use.';
    final sorted = allTags.toList()..sort();
    return 'Tags in use (${sorted.length}): ${sorted.join(', ')}';
  }

  String _listContainers() {
    final containerEntries = _inventoryApi.currentState.values
        .where((e) => e.category == 'Containers')
        .toList();
    if (containerEntries.isEmpty) return 'No containers found.';
    final lines = containerEntries.map(
      (e) => '[${e.id.substring(0, 8)}] ${e.description} -> ${e.location}',
    );
    return 'Containers (${containerEntries.length}):\n${lines.join('\n')}';
  }

  // -- Formatting helpers --

  String _formatCompactList(List<InventoryEntry> entries) {
    final lines = entries.map(
      (e) => '[${e.id.substring(0, 8)}] ${e.description} -> ${e.location}',
    );
    return 'Found ${entries.length} items:\n${lines.join('\n')}';
  }

  String _formatItemDetail(InventoryEntry entry) {
    final lines = <String>[
      '[${entry.id.substring(0, 8)}] ${entry.description}',
      '  Category: ${entry.category}',
      '  Location: ${entry.location}',
    ];
    if (entry.tags.isNotEmpty) {
      lines.add('  Tags: ${entry.tags.join(', ')}');
    }
    return lines.join('\n');
  }
}
