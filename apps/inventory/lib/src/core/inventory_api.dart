import 'dart:convert';
import 'dart:io';
import 'crdt/add_item_op.dart';
import 'crdt/delete_item_op.dart';
import 'crdt/genesis_op.dart';
import 'crdt/inventory_crdt.dart';
import 'crdt/operation.dart';
import 'package:uuid/uuid.dart';
import 'models/inventory_entry.dart';

// This is a sketch of the new CRDT-based API.
// It is not a complete implementation.
class InventoryApi {
  final String operationLogPath;
  late final String _opLogPath;
  final InventoryCRDT _crdt = InventoryCRDT();
  final String _nodeId = const Uuid().v4();

  InventoryApi({required this.operationLogPath});

  /// Initializes the API by loading operations and migrating legacy data if needed.
  Future<void> initialize(String legacyFilePath) async {
    _opLogPath = legacyFilePath.replaceFirst('inventory.txt', 'inventory_ops.jsonl');

    final ops = await _loadOpsFromFile(_opLogPath);
    if (ops.isNotEmpty) {
      print('CRDT: Loaded ${ops.length} operations from log.');
      _crdt.load(ops);
    } else {
      // If no operations exist, this is a first run. Check for a legacy file.
      final legacyFile = File(legacyFilePath);
      if (await legacyFile.exists()) {
        print('CRDT: No operations log found. Migrating from legacy inventory.txt...');
        await _migrateFromLegacyFile(legacyFile);
      }
    }
  }

  /// Migrates data from the old inventory.txt format into a CRDT Genesis operation.
  Future<void> _migrateFromLegacyFile(File legacyFile) async {
    final lines = await legacyFile.readAsLines();
    final initialItems = lines
        .where((line) => line.trim().isNotEmpty && !line.trim().startsWith('#'))
        .map((line) {
      try {
        // fromLine will generate a UUID for legacy entries that don't have one.
        return InventoryEntry.fromLine(line);
      } catch (e) {
        print('Skipping malformed line during migration: $line');
        return null;
      }
    }).whereType<InventoryEntry>().toList();

    if (initialItems.isNotEmpty) {
      final genesisOp = GenesisOp(nodeId: _nodeId, initialItems: initialItems);
      _crdt.apply(genesisOp);
      await _persistOperation(genesisOp);
    }
  }

  /// Persists a single operation to the operation log.
  Future<void> _persistOperation(Operation op) async {
    final file = File(_opLogPath);
    await file.writeAsString(jsonEncode(op.toJson()) + '\n', mode: FileMode.append);
  }

  Future<List<Operation>> _loadOpsFromFile(String path) async {
    final file = File(path);
    if (!await file.exists()) {
      return [];
    }

    final lines = await file.readAsLines();
    final ops = <Operation>[];
    for (final line in lines) {
      if (line.trim().isEmpty) continue;
      try {
        final json = jsonDecode(line) as Map<String, dynamic>;
        ops.add(_operationFromJson(json));
      } catch (e) {
        print('Error decoding operation from log: $e');
      }
    }
    return ops;
  }

  Operation _operationFromJson(Map<String, dynamic> json) {
    final type = json['type'] as String;
    switch (type) {
      case 'AddItemOp':
        return AddItemOp.fromJson(json);
      case 'DeleteItemOp':
        return DeleteItemOp.fromJson(json);
      case 'GenesisOp':
        return GenesisOp.fromJson(json);
      default:
        throw ArgumentError('Unknown operation type: $type');
    }
  }

  /// Adds a new item by creating and applying an AddItemOp.
  Future<void> addItem({
    required String category,
    required String description,
    required String location,
    List<String> tags = const [],
    bool isContainer = false,
    String? containerId,
  }) async {
    final allTags = List<String>.from(tags);
    if (isContainer) {
      // The chat processor should have already validated this.
      // This is an internal consistency check.
      if (containerId == null || containerId.isEmpty) {
        throw ArgumentError('A containerId must be provided when creating a container.');
      }
      final containerTag = 'container_$containerId';
      if (!allTags.contains(containerTag)) {
        allTags.add(containerTag);
      }
    }

    // If an item is being added to a container, its location should be updated to be consistent.
    final containerTag = allTags.firstWhere((t) => t.startsWith('container_'), orElse: () => '');
    String finalLocation = location;
    if (containerTag.isNotEmpty && !isContainer) {
      final inContainerId = containerTag.substring('container_'.length);
      if (!await containerExists(inContainerId)) {
        throw StateError('Attempted to add item to a non-existent container: "$inContainerId"');
      }
      finalLocation = 'container $inContainerId';
    }

    final entry = InventoryEntry(
      id: const Uuid().v4(),
      category: category,
      description: description,
      location: finalLocation,
      tags: allTags,
    );

    final op = AddItemOp(nodeId: _nodeId, item: entry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  /// Searches the current materialized state of the inventory.
  Future<List<InventoryEntry>> search(String searchStr) async {
    final currentState = _crdt.currentState.values;
    if (searchStr.isEmpty) {
      return currentState.toList();
    }

    final searchTerm = searchStr.toLowerCase();
    return currentState.where((entry) {
      return entry.description.toLowerCase().contains(searchTerm) ||
          entry.location.toLowerCase().contains(searchTerm) ||
          entry.category.toLowerCase().contains(searchTerm) ||
          entry.tags.any((tag) => tag.toLowerCase().contains(searchTerm));
    }).toList();
  }

  /// Returns true if no existing item's description conflicts with [description].
  /// Conflict = bidirectional case-insensitive substring match.
  bool descriptionIsUnique(String description) {
    final descLower = description.toLowerCase();
    for (final entry in _crdt.currentState.values) {
      final existingLower = entry.description.toLowerCase();
      if (descLower.contains(existingLower) || existingLower.contains(descLower)) {
        return false;
      }
    }
    return true;
  }

  /// Finds the first existing item whose description conflicts with [description].
  /// Returns null if no conflict found.
  InventoryEntry? findDescriptionConflict(String description) {
    final descLower = description.toLowerCase();
    for (final entry in _crdt.currentState.values) {
      final existingLower = entry.description.toLowerCase();
      if (descLower.contains(existingLower) || existingLower.contains(descLower)) {
        return entry;
      }
    }
    return null;
  }

  /// Finds an entry by exact or prefix match on its UUID.
  /// Throws if zero or multiple entries match the prefix.
  InventoryEntry findByIdPrefix(String idPrefix) {
    final prefixLower = idPrefix.toLowerCase();
    final matches = _crdt.currentState.values
        .where((entry) => entry.id.toLowerCase().startsWith(prefixLower))
        .toList();

    if (matches.isEmpty) {
      throw StateError('No entries found with ID starting with "$idPrefix"');
    }
    if (matches.length > 1) {
      final ids = matches.map((e) => '"${e.id.substring(0, 8)}..."').join(', ');
      throw StateError('Ambiguous ID prefix "$idPrefix" matches $ids. Use a longer prefix.');
    }
    return matches.first;
  }

  Future<InventoryEntry> findUniqueEntry(String searchStr) async {
    // Try ID prefix match first if searchStr looks like a UUID prefix
    if (_looksLikeIdPrefix(searchStr)) {
      try {
        return findByIdPrefix(searchStr);
      } on StateError {
        // Fall through to description search
      }
    }

    final matches = _crdt.currentState.values
        .where((entry) =>
            entry.description.toLowerCase().contains(searchStr.toLowerCase()))
        .toList();

    if (matches.isEmpty) {
      throw StateError('No entries found matching "$searchStr"');
    }
    if (matches.length > 1) {
      final details = matches.map((e) =>
          '[${e.id.substring(0, 8)}] "${e.description}"').join(', ');
      throw StateError(
          'Multiple entries found matching "$searchStr": $details. '
          'Use a short ID prefix to select a specific one.');
    }

    return matches.first;
  }

  /// Returns true if the string looks like it could be a UUID prefix.
  /// UUIDs are hex + hyphens, e.g. "f47ac10b-58cc-..."
  bool _looksLikeIdPrefix(String s) {
    return s.length >= 4 && RegExp(r'^[0-9a-fA-F-]+$').hasMatch(s);
  }


  // Other methods like deleteItem, moveItem, etc. would be refactored
  // to create and apply their corresponding Operation types.
  // This is left as an exercise for the full implementation.

  Future<void> deleteItem({required String searchStr}) async {
    final entryToDelete = await findUniqueEntry(searchStr);
    final op = DeleteItemOp(nodeId: _nodeId, itemId: entryToDelete.id);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  Future<void> moveItem({
    required String searchStr,
    required String newLocation,
  }) async {
    final entryToMove = await findUniqueEntry(searchStr);
    final updatedEntry = entryToMove.copyWith(location: newLocation);
    final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  Future<void> putInContainer({
    required String searchStr,
    required String containerId,
  }) async {
    final entryToMove = await findUniqueEntry(searchStr);

    if (!await containerExists(containerId)) {
      throw StateError('Container with ID "$containerId" not found.');
    }

    final newTags =
        entryToMove.tags.where((t) => !t.startsWith('container_')).toList();
    newTags.add('container_$containerId');

    final updatedEntry = entryToMove.copyWith(
      location: 'container $containerId',
      tags: newTags,
    );

    final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  Future<void> removeFromContainer({
    required String searchStr,
  }) async {
    final entryToRemove = await findUniqueEntry(searchStr);

    final containerTag = entryToRemove.tags.firstWhere(
      (t) => t.startsWith('container_'),
      orElse: () => '',
    );

    if (containerTag.isEmpty) {
      throw StateError('Item "$searchStr" is not in a container.');
    }

    final containerId = containerTag.substring('container_'.length);

    final containers = _crdt.currentState.values
        .where((entry) =>
            entry.category == 'Containers' &&
            entry.tags.contains('container_$containerId'))
        .toList();

    if (containers.isEmpty) {
      throw StateError(
          'Container with ID "$containerId" not found, but item "$searchStr" references it.');
    }
    final container = containers.first;

    final newTags =
        entryToRemove.tags.where((t) => !t.startsWith('container_')).toList();

    final updatedEntry = entryToRemove.copyWith(
      location: container.location,
      tags: newTags,
    );

    final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  Future<bool> containerExists(String containerId) async {
    final containerTag = 'container_$containerId';
    return _crdt.currentState.values.any((entry) =>
        entry.category == 'Containers' && entry.tags.contains(containerTag));
  }

  /// Creates a new storage container.
  Future<void> createContainer({
    required String containerId,
    required String location,
    String? description,
  }) async {
    // Check if container already exists
    if (await containerExists(containerId)) {
      throw StateError('Container "$containerId" already exists.');
    }

    final desc = description ?? 'Storage container $containerId';
    final entry = InventoryEntry(
      id: const Uuid().v4(),
      category: 'Containers',
      description: desc,
      location: location,
      tags: ['container_$containerId'],
    );

    final op = AddItemOp(nodeId: _nodeId, item: entry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  /// Adds a tag to an item.
  Future<void> addTag({
    required String searchStr,
    required String tag,
  }) async {
    final entry = await findUniqueEntry(searchStr);

    if (entry.tags.contains(tag)) {
      throw StateError('Item "${entry.description}" already has tag "$tag".');
    }

    final newTags = List<String>.from(entry.tags)..add(tag);
    final updatedEntry = entry.copyWith(tags: newTags);

    final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  /// Removes a tag from an item.
  Future<void> removeTag({
    required String searchStr,
    required String tag,
  }) async {
    final entry = await findUniqueEntry(searchStr);

    if (!entry.tags.contains(tag)) {
      throw StateError('Item "${entry.description}" does not have tag "$tag".');
    }

    final newTags = entry.tags.where((t) => t != tag).toList();
    final updatedEntry = entry.copyWith(tags: newTags);

    final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  /// Puts all items with a specific tag into a container.
  Future<void> groupPutIn({
    required String tag,
    required String containerId,
  }) async {
    if (!await containerExists(containerId)) {
      throw StateError('Container "$containerId" not found.');
    }

    final items = _crdt.currentState.values
        .where((entry) => entry.tags.contains(tag))
        .toList();

    if (items.isEmpty) {
      throw StateError('No items found with tag "$tag".');
    }

    for (final entry in items) {
      // Remove any existing container tags, add new one
      final newTags = entry.tags.where((t) => !t.startsWith('container_')).toList();
      newTags.add('container_$containerId');

      final updatedEntry = entry.copyWith(
        location: 'container $containerId',
        tags: newTags,
      );

      final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
      _crdt.apply(op);
      await _persistOperation(op);
    }
  }

  /// Removes a tag from all items that have it.
  Future<void> groupRemoveTag({
    required String tag,
  }) async {
    final items = _crdt.currentState.values
        .where((entry) => entry.tags.contains(tag))
        .toList();

    if (items.isEmpty) {
      throw StateError('No items found with tag "$tag".');
    }

    for (final entry in items) {
      final newTags = entry.tags.where((t) => t != tag).toList();
      final updatedEntry = entry.copyWith(tags: newTags);

      final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
      _crdt.apply(op);
      await _persistOperation(op);
    }
  }

  Future<void> editDescription({
    required String searchStr,
    required String newDescription,
  }) async {
    final entryToEdit = await findUniqueEntry(searchStr);

    final updatedEntry = entryToEdit.copyWith(description: newDescription);
    final op = AddItemOp(nodeId: _nodeId, item: updatedEntry);
    _crdt.apply(op);
    await _persistOperation(op);
  }

  /// Exports the current CRDT state in the legacy inventory.txt format.
  /// Groups items by category and formats them with category headers.
  String exportToLegacyFormat() {
    final currentState = _crdt.currentState.values.toList();

    if (currentState.isEmpty) {
      return '# Empty inventory\n';
    }

    // Group by category
    final Map<String, List<InventoryEntry>> byCategory = {};
    for (final entry in currentState) {
      if (!byCategory.containsKey(entry.category)) {
        byCategory[entry.category] = [];
      }
      byCategory[entry.category]!.add(entry);
    }

    // Sort categories alphabetically
    final sortedCategories = byCategory.keys.toList()..sort();

    final buffer = StringBuffer();
    for (final category in sortedCategories) {
      buffer.writeln('# $category');
      final entries = byCategory[category]!;

      // Sort entries within category by description
      entries.sort((a, b) => a.description.compareTo(b.description));

      for (final entry in entries) {
        final tagsStr = entry.tags.map((t) => '"$t"').join(',');
        buffer.writeln('{"category":"${entry.category}","tags":[$tagsStr]} ${entry.description} -> ${entry.location}');
      }
      buffer.writeln();
    }

    return buffer.toString();
  }
}
