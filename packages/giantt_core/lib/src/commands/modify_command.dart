import 'command_interface.dart';
import '../models/giantt_item.dart';
import '../models/status.dart';
import '../models/priority.dart';
import '../models/duration.dart';
import '../storage/dual_file_manager.dart';
import '../graph/giantt_graph.dart';
import '../models/graph_exceptions.dart';

/// Arguments for modify command
class ModifyArgs {
  const ModifyArgs({
    required this.itemId,
    this.title,
    this.status,
    this.priority,
    this.duration,
    this.addCharts = const [],
    this.removeCharts = const [],
    this.addTags = const [],
    this.removeTags = const [],
    this.addRelations = const {},
    this.removeRelations = const {},
    this.userComment,
  });

  final String itemId;
  final String? title;
  final GianttStatus? status;
  final GianttPriority? priority;
  final GianttDuration? duration;
  final List<String> addCharts;
  final List<String> removeCharts;
  final List<String> addTags;
  final List<String> removeTags;
  final Map<String, List<String>> addRelations;
  final Map<String, List<String>> removeRelations;
  final String? userComment;
}

/// Modify an existing item in the graph
class ModifyCommand extends CliCommand<ModifyArgs> {
  const ModifyCommand();

  @override
  String get name => 'modify';

  @override
  String get description => 'Modify an existing item in the graph';

  @override
  String get usage => 'modify <id> [options]';

  @override
  ModifyArgs parseArgs(List<String> args) {
    if (args.isEmpty) {
      throw ArgumentError('modify requires an item ID');
    }

    final itemId = args[0];
    String? title;
    GianttStatus? status;
    GianttPriority? priority;
    GianttDuration? duration;
    List<String> addCharts = [];
    List<String> removeCharts = [];
    List<String> addTags = [];
    List<String> removeTags = [];
    Map<String, List<String>> addRelations = {};
    Map<String, List<String>> removeRelations = {};
    String? userComment;

    for (int i = 1; i < args.length; i++) {
      final arg = args[i];
      
      if (arg.startsWith('--title=')) {
        title = arg.substring(8);
      } else if (arg.startsWith('--status=')) {
        final statusStr = arg.substring(9);
        status = GianttStatus.fromSymbol(statusStr);
      } else if (arg.startsWith('--priority=')) {
        final priorityStr = arg.substring(11);
        priority = GianttPriority.fromSymbol(priorityStr);
      } else if (arg.startsWith('--duration=')) {
        final durationStr = arg.substring(11);
        duration = GianttDuration.parse(durationStr);
      } else if (arg.startsWith('--add-charts=')) {
        final chartsStr = arg.substring(13);
        addCharts = chartsStr.split(',').map((c) => c.trim()).toList();
      } else if (arg.startsWith('--remove-charts=')) {
        final chartsStr = arg.substring(16);
        removeCharts = chartsStr.split(',').map((c) => c.trim()).toList();
      } else if (arg.startsWith('--add-tags=')) {
        final tagsStr = arg.substring(11);
        addTags = tagsStr.split(',').map((t) => t.trim()).toList();
      } else if (arg.startsWith('--remove-tags=')) {
        final tagsStr = arg.substring(14);
        removeTags = tagsStr.split(',').map((t) => t.trim()).toList();
      } else if (arg.startsWith('--add-requires=')) {
        final requiresStr = arg.substring(15);
        addRelations['REQUIRES'] = requiresStr.split(',').map((r) => r.trim()).toList();
      } else if (arg.startsWith('--remove-requires=')) {
        final requiresStr = arg.substring(18);
        removeRelations['REQUIRES'] = requiresStr.split(',').map((r) => r.trim()).toList();
      } else if (arg.startsWith('--add-blocks=')) {
        final blocksStr = arg.substring(13);
        addRelations['BLOCKS'] = blocksStr.split(',').map((b) => b.trim()).toList();
      } else if (arg.startsWith('--remove-blocks=')) {
        final blocksStr = arg.substring(16);
        removeRelations['BLOCKS'] = blocksStr.split(',').map((b) => b.trim()).toList();
      } else if (arg.startsWith('--comment=')) {
        userComment = arg.substring(10);
      }
    }

    return ModifyArgs(
      itemId: itemId,
      title: title,
      status: status,
      priority: priority,
      duration: duration,
      addCharts: addCharts,
      removeCharts: removeCharts,
      addTags: addTags,
      removeTags: removeTags,
      addRelations: addRelations,
      removeRelations: removeRelations,
      userComment: userComment,
    );
  }

  @override
  Future<CommandResult<ModifyArgs>> execute(CommandContext context) async {
    try {
      // Load existing graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = ModifyArgs(itemId: 'temp'); // This will be set by parseArgs in CLI usage

      // Find the item
      final existingItem = context.graph!.items[args.itemId];
      if (existingItem == null) {
        return CommandResult.failure('Item with ID "${args.itemId}" not found');
      }

      // Apply modifications
      final modifiedItem = _applyModifications(existingItem, args);

      if (context.dryRun) {
        return CommandResult.message(
          'Would modify item: ${modifiedItem.toFileString()}'
        );
      }

      // Check for cycles before applying changes (especially for relation modifications)
      if (args.addRelations.isNotEmpty || args.removeRelations.isNotEmpty) {
        // Create a temporary graph to test the changes
        final tempGraph = GianttGraph();
        for (final item in context.graph!.items.values) {
          tempGraph.addItem(item);
        }
        tempGraph.addItem(modifiedItem); // Replace with modified version
        
        try {
          tempGraph.topologicalSort();
        } on CycleDetectedException catch (e) {
          return CommandResult.failure('Modifying relations would create a dependency cycle: ${e.cycleItems.join(' -> ')}');
        }
      }

      // Update in graph
      context.graph!.addItem(modifiedItem);

      // Save graph
      DualFileManager.saveGraph(
        context.itemsPath,
        context.occludeItemsPath,
        context.graph!,
      );

      return CommandResult.success(
        args,
        'Modified item "${args.itemId}" successfully'
      );

    } catch (e) {
      return CommandResult.failure('Failed to modify item: $e');
    }
  }

  GianttItem _applyModifications(GianttItem item, ModifyArgs args) {
    // Start with existing item
    var modified = item;

    // Apply basic property changes
    if (args.title != null) {
      modified = modified.copyWith(title: args.title);
    }
    if (args.status != null) {
      modified = modified.copyWith(status: args.status);
    }
    if (args.priority != null) {
      modified = modified.copyWith(priority: args.priority);
    }
    if (args.duration != null) {
      modified = modified.copyWith(duration: args.duration);
    }
    if (args.userComment != null) {
      modified = modified.copyWith(userComment: args.userComment);
    }

    // Apply chart modifications
    var newCharts = List<String>.from(modified.charts);
    newCharts.addAll(args.addCharts);
    newCharts.removeWhere((chart) => args.removeCharts.contains(chart));
    modified = modified.copyWith(charts: newCharts);

    // Apply tag modifications
    var newTags = List<String>.from(modified.tags);
    newTags.addAll(args.addTags);
    newTags.removeWhere((tag) => args.removeTags.contains(tag));
    modified = modified.copyWith(tags: newTags);

    // Apply relation modifications
    var newRelations = Map<String, List<String>>.from(modified.relations);
    
    // Add relations
    for (final entry in args.addRelations.entries) {
      final relType = entry.key;
      final targets = entry.value;
      newRelations[relType] = (newRelations[relType] ?? [])..addAll(targets);
    }
    
    // Remove relations
    for (final entry in args.removeRelations.entries) {
      final relType = entry.key;
      final targets = entry.value;
      if (newRelations.containsKey(relType)) {
        newRelations[relType]!.removeWhere((target) => targets.contains(target));
        if (newRelations[relType]!.isEmpty) {
          newRelations.remove(relType);
        }
      }
    }
    
    modified = modified.copyWith(relations: newRelations);

    return modified;
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<GianttItem>> modifyItem(
    String workspacePath,
    String itemId, {
    String? title,
    GianttStatus? status,
    GianttPriority? priority,
    GianttDuration? duration,
    List<String> addCharts = const [],
    List<String> removeCharts = const [],
    List<String> addTags = const [],
    List<String> removeTags = const [],
    Map<String, List<String>> addRelations = const {},
    Map<String, List<String>> removeRelations = const {},
    String? userComment,
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    // Load existing graph
    context.graph = DualFileManager.loadGraph(
      context.itemsPath,
      context.occludeItemsPath,
    );

    // Find the item
    final existingItem = context.graph!.items[itemId];
    if (existingItem == null) {
      return CommandResult.failure('Item with ID "$itemId" not found');
    }

    final args = ModifyArgs(
      itemId: itemId,
      title: title,
      status: status,
      priority: priority,
      duration: duration,
      addCharts: addCharts,
      removeCharts: removeCharts,
      addTags: addTags,
      removeTags: removeTags,
      addRelations: addRelations,
      removeRelations: removeRelations,
      userComment: userComment,
    );

    final command = ModifyCommand();
    final modifiedItem = command._applyModifications(existingItem, args);

    // Update in graph
    context.graph!.addItem(modifiedItem);

    // Save graph
    DualFileManager.saveGraph(
      context.itemsPath,
      context.occludeItemsPath,
      context.graph!,
    );

    return CommandResult.success(modifiedItem, 'Modified item "$itemId" successfully');
  }
}
