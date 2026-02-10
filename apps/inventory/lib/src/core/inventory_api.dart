import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'package:uuid/uuid.dart';
import 'package:soradyne_flutter/soradyne_flutter.dart';
import 'models/inventory_entry.dart';

/// Inventory API backed by Soradyne's Rust ConvergentDocument via FFI.
///
/// The Rust side owns the CRDT engine and persistence (operations.jsonl).
/// This class owns the business logic: search, containers, tag management.
class InventoryApi {
  final String operationLogPath;
  final FlowClient _flowClient = FlowClient.instance;
  late final Pointer<Void> _handle;
  final String _nodeId = const Uuid().v4();
  String _flowUuid = '';
  bool _initialized = false;

  InventoryApi({required this.operationLogPath});

  /// Initializes the API: determines the flow UUID, migrates legacy data if
  /// needed, and opens the Rust-side flow.
  Future<void> initialize(String legacyFilePath) async {
    // Determine flow UUID from a config file stored alongside the inventory
    final configDir = File(legacyFilePath).parent.path;
    final flowConfigFile = File('$configDir/inventory_flow_uuid.txt');

    if (await flowConfigFile.exists()) {
      _flowUuid = (await flowConfigFile.readAsString()).trim();
    } else {
      _flowUuid = const Uuid().v4();
      await flowConfigFile.writeAsString(_flowUuid);
    }

    // Initialize the flow system and open this flow
    _flowClient.inventoryInit(_nodeId);
    _handle = _flowClient.inventoryOpen(_flowUuid);
    _initialized = true;

    // Check if we need to migrate from old CRDT format
    final oldOpsPath =
        legacyFilePath.replaceFirst('seed_inventory.txt', 'inventory_ops.jsonl');
    final oldOpsFile = File(oldOpsPath);

    // Only migrate if:
    // 1. The old ops file exists
    // 2. The flow is empty (no operations yet — first time opening this UUID)
    if (await oldOpsFile.exists()) {
      final currentState = _readState();
      if (currentState.isEmpty) {
        print('CRDT: Migrating from old Dart CRDT format...');
        await _migrateFromOldOps(oldOpsFile);
        // Rename old file so we don't re-migrate
        await oldOpsFile.rename('$oldOpsPath.migrated');
        print('CRDT: Migration complete.');
      }
    } else {
      // No old ops file — check for legacy seed_inventory.txt
      final legacyFile = File(legacyFilePath);
      if (await legacyFile.exists()) {
        final currentState = _readState();
        if (currentState.isEmpty) {
          print('CRDT: Migrating from seed_inventory.txt...');
          await _migrateFromLegacyFile(legacyFile);
        }
      }
    }
  }

  // ===========================================================================
  // State access
  // ===========================================================================

  /// The current materialized inventory state.
  Map<String, InventoryEntry> get currentState => _readState();

  Map<String, InventoryEntry> _readState() {
    final json = _flowClient.inventoryReadDrip(_handle);
    return _parseState(json);
  }

  Map<String, InventoryEntry> _parseState(String json) {
    final parsed = jsonDecode(json) as Map<String, dynamic>;
    final items = parsed['items'] as Map<String, dynamic>;
    return items.map((id, itemJson) {
      final item = itemJson as Map<String, dynamic>;
      return MapEntry(
        id,
        InventoryEntry(
          id: item['id'] as String,
          category: item['category'] as String? ?? '',
          description: item['description'] as String? ?? '',
          location: item['location'] as String? ?? '',
          tags: (item['tags'] as List<dynamic>?)?.cast<String>() ?? [],
        ),
      );
    });
  }

  // ===========================================================================
  // Write helpers
  // ===========================================================================

  void _writeOp(Map<String, dynamic> op) {
    _flowClient.inventoryWriteOp(_handle, jsonEncode(op));
  }

  /// Add a new item via the 5-primitive convergent operations.
  void _addItemOps(String itemId, String category, String description,
      String location, List<String> tags) {
    _writeOp({
      'AddItem': {'item_id': itemId, 'item_type': 'InventoryItem'}
    });
    _writeOp({
      'SetField': {
        'item_id': itemId,
        'field': 'category',
        'value': category,
      }
    });
    _writeOp({
      'SetField': {
        'item_id': itemId,
        'field': 'description',
        'value': description,
      }
    });
    _writeOp({
      'SetField': {
        'item_id': itemId,
        'field': 'location',
        'value': location,
      }
    });
    for (final tag in tags) {
      _writeOp({
        'AddToSet': {
          'item_id': itemId,
          'set_name': 'tags',
          'element': tag,
        }
      });
    }
  }

  // ===========================================================================
  // CRUD operations
  // ===========================================================================

  /// Adds a new item.
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
      if (containerId == null || containerId.isEmpty) {
        throw ArgumentError(
            'A containerId must be provided when creating a container.');
      }
      final containerTag = 'container_$containerId';
      if (!allTags.contains(containerTag)) {
        allTags.add(containerTag);
      }
    }

    final containerTag =
        allTags.firstWhere((t) => t.startsWith('container_'), orElse: () => '');
    String finalLocation = location;
    if (containerTag.isNotEmpty && !isContainer) {
      final inContainerId = containerTag.substring('container_'.length);
      if (!await containerExists(inContainerId)) {
        throw StateError(
            'Attempted to add item to a non-existent container: "$inContainerId"');
      }
      finalLocation = 'container $inContainerId';
    }

    final itemId = const Uuid().v4();
    _addItemOps(itemId, category, description, finalLocation, allTags);
  }

  Future<void> deleteItem({required String searchStr}) async {
    final entryToDelete = await findUniqueEntry(searchStr);
    _writeOp({
      'RemoveItem': {'item_id': entryToDelete.id}
    });
  }

  Future<void> moveItem({
    required String searchStr,
    required String newLocation,
  }) async {
    final entryToMove = await findUniqueEntry(searchStr);
    _writeOp({
      'SetField': {
        'item_id': entryToMove.id,
        'field': 'location',
        'value': newLocation,
      }
    });
  }

  Future<void> editDescription({
    required String searchStr,
    required String newDescription,
  }) async {
    final entryToEdit = await findUniqueEntry(searchStr);
    _writeOp({
      'SetField': {
        'item_id': entryToEdit.id,
        'field': 'description',
        'value': newDescription,
      }
    });
  }

  // ===========================================================================
  // Tag operations
  // ===========================================================================

  Future<void> addTag({
    required String searchStr,
    required String tag,
  }) async {
    final entry = await findUniqueEntry(searchStr);

    if (entry.tags.contains(tag)) {
      throw StateError('Item "${entry.description}" already has tag "$tag".');
    }

    _writeOp({
      'AddToSet': {
        'item_id': entry.id,
        'set_name': 'tags',
        'element': tag,
      }
    });
  }

  Future<void> removeTag({
    required String searchStr,
    required String tag,
  }) async {
    final entry = await findUniqueEntry(searchStr);

    if (!entry.tags.contains(tag)) {
      throw StateError(
          'Item "${entry.description}" does not have tag "$tag".');
    }

    // Send RemoveFromSet with empty observed_add_ids —
    // the Rust side auto-fills them.
    _writeOp({
      'RemoveFromSet': {
        'item_id': entry.id,
        'set_name': 'tags',
        'element': tag,
        'observed_add_ids': <String>[],
      }
    });
  }

  Future<void> groupRemoveTag({required String tag}) async {
    final state = currentState;
    final items =
        state.values.where((entry) => entry.tags.contains(tag)).toList();

    if (items.isEmpty) {
      throw StateError('No items found with tag "$tag".');
    }

    for (final entry in items) {
      _writeOp({
        'RemoveFromSet': {
          'item_id': entry.id,
          'set_name': 'tags',
          'element': tag,
          'observed_add_ids': <String>[],
        }
      });
    }
  }

  // ===========================================================================
  // Container operations
  // ===========================================================================

  Future<bool> containerExists(String containerId) async {
    final containerTag = 'container_$containerId';
    return currentState.values.any((entry) =>
        entry.category == 'Containers' && entry.tags.contains(containerTag));
  }

  Future<void> createContainer({
    required String containerId,
    required String location,
    String? description,
  }) async {
    if (await containerExists(containerId)) {
      throw StateError('Container "$containerId" already exists.');
    }

    final desc = description ?? 'Storage container $containerId';
    final itemId = const Uuid().v4();
    _addItemOps(
        itemId, 'Containers', desc, location, ['container_$containerId']);
  }

  Future<void> putInContainer({
    required String searchStr,
    required String containerId,
  }) async {
    final entryToMove = await findUniqueEntry(searchStr);

    if (!await containerExists(containerId)) {
      throw StateError('Container with ID "$containerId" not found.');
    }

    // Remove old container tags
    for (final tag in entryToMove.tags) {
      if (tag.startsWith('container_')) {
        _writeOp({
          'RemoveFromSet': {
            'item_id': entryToMove.id,
            'set_name': 'tags',
            'element': tag,
            'observed_add_ids': <String>[],
          }
        });
      }
    }

    // Add new container tag
    _writeOp({
      'AddToSet': {
        'item_id': entryToMove.id,
        'set_name': 'tags',
        'element': 'container_$containerId',
      }
    });

    // Update location
    _writeOp({
      'SetField': {
        'item_id': entryToMove.id,
        'field': 'location',
        'value': 'container $containerId',
      }
    });
  }

  Future<void> removeFromContainer({required String searchStr}) async {
    final entryToRemove = await findUniqueEntry(searchStr);

    final containerTag = entryToRemove.tags.firstWhere(
      (t) => t.startsWith('container_'),
      orElse: () => '',
    );

    if (containerTag.isEmpty) {
      throw StateError('Item "$searchStr" is not in a container.');
    }

    final containerId = containerTag.substring('container_'.length);
    final state = currentState;

    final containers = state.values
        .where((entry) =>
            entry.category == 'Containers' &&
            entry.tags.contains('container_$containerId'))
        .toList();

    if (containers.isEmpty) {
      throw StateError(
          'Container with ID "$containerId" not found, but item "$searchStr" references it.');
    }
    final container = containers.first;

    // Remove container tag
    _writeOp({
      'RemoveFromSet': {
        'item_id': entryToRemove.id,
        'set_name': 'tags',
        'element': containerTag,
        'observed_add_ids': <String>[],
      }
    });

    // Inherit container's location
    _writeOp({
      'SetField': {
        'item_id': entryToRemove.id,
        'field': 'location',
        'value': container.location,
      }
    });
  }

  Future<void> groupPutIn({
    required String tag,
    required String containerId,
  }) async {
    if (!await containerExists(containerId)) {
      throw StateError('Container "$containerId" not found.');
    }

    final state = currentState;
    final items =
        state.values.where((entry) => entry.tags.contains(tag)).toList();

    if (items.isEmpty) {
      throw StateError('No items found with tag "$tag".');
    }

    for (final entry in items) {
      // Remove old container tags
      for (final t in entry.tags) {
        if (t.startsWith('container_')) {
          _writeOp({
            'RemoveFromSet': {
              'item_id': entry.id,
              'set_name': 'tags',
              'element': t,
              'observed_add_ids': <String>[],
            }
          });
        }
      }

      // Add new container tag
      _writeOp({
        'AddToSet': {
          'item_id': entry.id,
          'set_name': 'tags',
          'element': 'container_$containerId',
        }
      });

      // Update location
      _writeOp({
        'SetField': {
          'item_id': entry.id,
          'field': 'location',
          'value': 'container $containerId',
        }
      });
    }
  }

  // ===========================================================================
  // Search / query
  // ===========================================================================

  Future<List<InventoryEntry>> search(String searchStr) async {
    final state = currentState.values;
    if (searchStr.isEmpty) {
      return state.toList();
    }

    final searchTerm = searchStr.toLowerCase();
    return state.where((entry) {
      return entry.description.toLowerCase().contains(searchTerm) ||
          entry.location.toLowerCase().contains(searchTerm) ||
          entry.category.toLowerCase().contains(searchTerm) ||
          entry.tags.any((tag) => tag.toLowerCase().contains(searchTerm));
    }).toList();
  }

  bool descriptionIsUnique(String description) {
    final descLower = description.toLowerCase();
    for (final entry in currentState.values) {
      final existingLower = entry.description.toLowerCase();
      if (descLower.contains(existingLower) ||
          existingLower.contains(descLower)) {
        return false;
      }
    }
    return true;
  }

  InventoryEntry? findDescriptionConflict(String description) {
    final descLower = description.toLowerCase();
    for (final entry in currentState.values) {
      final existingLower = entry.description.toLowerCase();
      if (descLower.contains(existingLower) ||
          existingLower.contains(descLower)) {
        return entry;
      }
    }
    return null;
  }

  InventoryEntry findByIdPrefix(String idPrefix) {
    final prefixLower = idPrefix.toLowerCase();
    final matches = currentState.values
        .where((entry) => entry.id.toLowerCase().startsWith(prefixLower))
        .toList();

    if (matches.isEmpty) {
      throw StateError('No entries found with ID starting with "$idPrefix"');
    }
    if (matches.length > 1) {
      final ids =
          matches.map((e) => '"${e.id.substring(0, 8)}..."').join(', ');
      throw StateError(
          'Ambiguous ID prefix "$idPrefix" matches $ids. Use a longer prefix.');
    }
    return matches.first;
  }

  Future<InventoryEntry> findUniqueEntry(String searchStr) async {
    if (_looksLikeIdPrefix(searchStr)) {
      try {
        return findByIdPrefix(searchStr);
      } on StateError {
        // Fall through to description search
      }
    }

    final matches = currentState.values
        .where((entry) =>
            entry.description.toLowerCase().contains(searchStr.toLowerCase()))
        .toList();

    if (matches.isEmpty) {
      throw StateError('No entries found matching "$searchStr"');
    }
    if (matches.length > 1) {
      final details = matches
          .map((e) => '[${e.id.substring(0, 8)}] "${e.description}"')
          .join(', ');
      throw StateError(
          'Multiple entries found matching "$searchStr": $details. '
          'Use a short ID prefix to select a specific one.');
    }

    return matches.first;
  }

  bool _looksLikeIdPrefix(String s) {
    return s.length >= 4 && RegExp(r'^[0-9a-fA-F-]+$').hasMatch(s);
  }

  // ===========================================================================
  // Export
  // ===========================================================================

  /// Returns the raw CRDT operations as a JSON string (for history export/sync).
  String getOperationsJson() {
    return _flowClient.inventoryGetOperations(_handle);
  }

  String exportToLegacyFormat() {
    final state = currentState.values.toList();

    if (state.isEmpty) {
      return '# Empty inventory\n';
    }

    final Map<String, List<InventoryEntry>> byCategory = {};
    for (final entry in state) {
      byCategory.putIfAbsent(entry.category, () => []).add(entry);
    }

    final sortedCategories = byCategory.keys.toList()..sort();

    final buffer = StringBuffer();
    for (final category in sortedCategories) {
      buffer.writeln('# $category');
      final entries = byCategory[category]!;
      entries.sort((a, b) => a.description.compareTo(b.description));

      for (final entry in entries) {
        final tagsStr = entry.tags.map((t) => '"$t"').join(',');
        buffer.writeln(
            '{"category":"${entry.category}","tags":[$tagsStr]} ${entry.description} -> ${entry.location}');
      }
      buffer.writeln();
    }

    return buffer.toString();
  }

  // ===========================================================================
  // Migration
  // ===========================================================================

  /// Migrate from old Dart CRDT ops file (inventory_ops.jsonl).
  ///
  /// Parses the JSON directly rather than importing the old type classes.
  Future<void> _migrateFromOldOps(File oldOpsFile) async {
    final lines = await oldOpsFile.readAsLines();
    for (final line in lines) {
      if (line.trim().isEmpty) continue;
      try {
        final json = jsonDecode(line) as Map<String, dynamic>;
        final type = json['type'] as String;

        switch (type) {
          case 'GenesisOp':
            final items = json['initialItems'] as List<dynamic>? ?? [];
            for (final itemJson in items) {
              final item = itemJson as Map<String, dynamic>;
              _addItemOps(
                item['id'] as String,
                item['category'] as String? ?? '',
                item['description'] as String? ?? '',
                item['location'] as String? ?? '',
                (item['tags'] as List<dynamic>?)?.cast<String>() ?? [],
              );
            }
            break;
          case 'AddItemOp':
            final item = json['item'] as Map<String, dynamic>;
            _addItemOps(
              item['id'] as String,
              item['category'] as String? ?? '',
              item['description'] as String? ?? '',
              item['location'] as String? ?? '',
              (item['tags'] as List<dynamic>?)?.cast<String>() ?? [],
            );
            break;
          case 'DeleteItemOp':
            final itemId = json['itemId'] as String;
            _writeOp({
              'RemoveItem': {'item_id': itemId}
            });
            break;
          default:
            print('Skipping unknown operation type during migration: $type');
        }
      } catch (e) {
        print('Error migrating operation: $e');
      }
    }
  }

  /// Migrate from legacy seed_inventory.txt (plain text format)
  Future<void> _migrateFromLegacyFile(File legacyFile) async {
    final lines = await legacyFile.readAsLines();
    final entries = lines
        .where((line) => line.trim().isNotEmpty && !line.trim().startsWith('#'))
        .map((line) {
      try {
        return InventoryEntry.fromLine(line);
      } catch (e) {
        print('Skipping malformed line during migration: $line');
        return null;
      }
    }).whereType<InventoryEntry>();

    for (final item in entries) {
      _addItemOps(
          item.id, item.category, item.description, item.location, item.tags);
    }
  }
}
