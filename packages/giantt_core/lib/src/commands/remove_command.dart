import 'command_interface.dart';
import '../models/giantt_item.dart';
import '../graph/giantt_graph.dart';
import '../storage/dual_file_manager.dart';

/// Arguments for remove command
class RemoveArgs {
  const RemoveArgs({
    required this.itemId,
    this.force = false,
  });

  final String itemId;
  final bool force;
}

/// Remove an item from the graph
class RemoveCommand extends CliCommand<RemoveArgs> {
  const RemoveCommand();

  @override
  String get name => 'remove';

  @override
  String get description => 'Remove an item from the graph';

  @override
  String get usage => 'remove <id> [--force]';

  @override
  RemoveArgs parseArgs(List<String> args) {
    if (args.isEmpty) {
      throw ArgumentError('remove requires an item ID');
    }

    final itemId = args[0];
    bool force = false;

    for (int i = 1; i < args.length; i++) {
      final arg = args[i];
      if (arg == '--force' || arg == '-f') {
        force = true;
      }
    }

    return RemoveArgs(itemId: itemId, force: force);
  }

  @override
  Future<CommandResult<RemoveArgs>> execute(CommandContext context) async {
    try {
      // Load existing graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = RemoveArgs(itemId: 'temp'); // This will be set by parseArgs in CLI usage

      // Find the item
      final itemToRemove = context.graph!.items[args.itemId];
      if (itemToRemove == null) {
        return CommandResult.failure('Item with ID "${args.itemId}" not found');
      }

      // Check for dependencies unless force is used
      if (!args.force) {
        final dependentItems = _findDependentItems(context.graph!, args.itemId);
        if (dependentItems.isNotEmpty) {
          final dependentIds = dependentItems.map((item) => item.id).join(', ');
          return CommandResult.failure(
            'Cannot remove item "${args.itemId}" because it is required by: $dependentIds. '
            'Use --force to remove anyway.'
          );
        }
      }

      if (context.dryRun) {
        return CommandResult.message(
          'Would remove item "${args.itemId}" and clean up ${_countRelationReferences(context.graph!, args.itemId)} relation references'
        );
      }

      // Remove the item
      context.graph!.items.remove(args.itemId);

      // Clean up relations that reference this item
      _cleanupRelationReferences(context.graph!, args.itemId);

      // Save graph
      DualFileManager.saveGraph(
        context.itemsPath,
        context.occludeItemsPath,
        context.graph!,
      );

      return CommandResult.success(
        args,
        'Removed item "${args.itemId}" successfully'
      );

    } catch (e) {
      return CommandResult.failure('Failed to remove item: $e');
    }
  }

  /// Find items that depend on the given item
  List<GianttItem> _findDependentItems(GianttGraph graph, String itemId) {
    final dependentItems = <GianttItem>[];

    for (final item in graph.items.values) {
      // Check if this item requires the item we want to remove
      final requires = item.relations['REQUIRES'] ?? [];
      if (requires.contains(itemId)) {
        dependentItems.add(item);
      }

      // Check if this item has any-of relation with the item we want to remove
      final anyOf = item.relations['ANYOF'] ?? [];
      if (anyOf.contains(itemId)) {
        dependentItems.add(item);
      }
    }

    return dependentItems;
  }

  /// Count how many relation references would be cleaned up
  int _countRelationReferences(GianttGraph graph, String itemId) {
    int count = 0;

    for (final item in graph.items.values) {
      for (final relations in item.relations.values) {
        if (relations.contains(itemId)) {
          count++;
        }
      }
    }

    return count;
  }

  /// Clean up all relation references to the removed item
  void _cleanupRelationReferences(GianttGraph graph, String itemId) {
    for (final item in graph.items.values) {
      bool modified = false;
      final newRelations = <String, List<String>>{};

      for (final entry in item.relations.entries) {
        final relType = entry.key;
        final targets = entry.value;
        final cleanedTargets = targets.where((target) => target != itemId).toList();
        
        if (cleanedTargets.length != targets.length) {
          modified = true;
        }
        
        if (cleanedTargets.isNotEmpty) {
          newRelations[relType] = cleanedTargets;
        }
      }

      if (modified) {
        final updatedItem = item.copyWith(relations: newRelations);
        graph.addItem(updatedItem);
      }
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<void>> removeItem(
    String workspacePath,
    String itemId, {
    bool force = false,
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    // Load existing graph
    context.graph = DualFileManager.loadGraph(
      context.itemsPath,
      context.occludeItemsPath,
    );

    // Find the item
    final itemToRemove = context.graph!.items[itemId];
    if (itemToRemove == null) {
      return CommandResult.failure('Item with ID "$itemId" not found');
    }

    final command = RemoveCommand();

    // Check for dependencies unless force is used
    if (!force) {
      final dependentItems = command._findDependentItems(context.graph!, itemId);
      if (dependentItems.isNotEmpty) {
        final dependentIds = dependentItems.map((item) => item.id).join(', ');
        return CommandResult.failure(
          'Cannot remove item "$itemId" because it is required by: $dependentIds. '
          'Use force=true to remove anyway.'
        );
      }
    }

    // Remove the item
    context.graph!.items.remove(itemId);

    // Clean up relations that reference this item
    command._cleanupRelationReferences(context.graph!, itemId);

    // Save graph
    DualFileManager.saveGraph(
      context.itemsPath,
      context.occludeItemsPath,
      context.graph!,
    );

    return CommandResult.success(null, 'Removed item "$itemId" successfully');
  }
}
