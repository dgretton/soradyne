import 'command_interface.dart';
import '../models/giantt_item.dart';
import '../models/status.dart';
import '../models/priority.dart';
import '../models/duration.dart';
import '../models/time_constraint.dart';
import '../storage/dual_file_manager.dart';
import '../parser/giantt_parser.dart';
import '../graph/giantt_graph.dart';
import '../models/graph_exceptions.dart';

/// Arguments for add command
class AddArgs {
  const AddArgs({
    required this.id,
    required this.title,
    this.status = GianttStatus.notStarted,
    this.priority = GianttPriority.neutral,
    this.duration,
    this.charts = const [],
    this.tags = const [],
    this.relations = const {},
    this.timeConstraints = const [],
  });

  final String id;
  final String title;
  final GianttStatus status;
  final GianttPriority priority;
  final GianttDuration? duration;
  final List<String> charts;
  final List<String> tags;
  final Map<String, List<String>> relations;
  final List<TimeConstraint> timeConstraints;
}

/// Add a new item to the graph
class AddCommand extends CliCommand<AddArgs> {
  const AddCommand();

  @override
  String get name => 'add';

  @override
  String get description => 'Add a new item to the graph';

  @override
  String get usage => 'add <id> "<title>" [options]';

  @override
  AddArgs parseArgs(List<String> args) {
    if (args.length < 2) {
      throw ArgumentError('add requires at least id and title');
    }

    final id = args[0];
    final title = args[1];
    
    // Parse optional arguments
    GianttStatus status = GianttStatus.notStarted;
    GianttPriority priority = GianttPriority.neutral;
    GianttDuration? duration;
    List<String> charts = [];
    List<String> tags = [];
    Map<String, List<String>> relations = {};
    List<TimeConstraint> timeConstraints = [];

    for (int i = 2; i < args.length; i++) {
      final arg = args[i];
      
      if (arg.startsWith('--status=')) {
        final statusStr = arg.substring(9);
        status = GianttStatus.fromSymbol(statusStr);
      } else if (arg.startsWith('--priority=')) {
        final priorityStr = arg.substring(11);
        priority = GianttPriority.fromSymbol(priorityStr);
      } else if (arg.startsWith('--duration=')) {
        final durationStr = arg.substring(11);
        duration = GianttDuration.parse(durationStr);
      } else if (arg.startsWith('--charts=')) {
        final chartsStr = arg.substring(9);
        charts = chartsStr.split(',').map((c) => c.trim()).toList();
      } else if (arg.startsWith('--tags=')) {
        final tagsStr = arg.substring(7);
        tags = tagsStr.split(',').map((t) => t.trim()).toList();
      } else if (arg.startsWith('--requires=')) {
        final requiresStr = arg.substring(11);
        relations['REQUIRES'] = requiresStr.split(',').map((r) => r.trim()).toList();
      } else if (arg.startsWith('--blocks=')) {
        final blocksStr = arg.substring(9);
        relations['BLOCKS'] = blocksStr.split(',').map((b) => b.trim()).toList();
      } else if (arg.startsWith('--constraints=')) {
        final constraintsStr = arg.substring(14);
        for (final constraintStr in constraintsStr.split(' ')) {
          if (constraintStr.trim().isNotEmpty) {
            timeConstraints.add(TimeConstraint.parse(constraintStr.trim()));
          }
        }
      }
    }

    return AddArgs(
      id: id,
      title: title,
      status: status,
      priority: priority,
      duration: duration,
      charts: charts,
      tags: tags,
      relations: relations,
      timeConstraints: timeConstraints,
    );
  }

  @override
  Future<CommandResult<AddArgs>> execute(CommandContext context) async {
    try {
      // Load existing graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = AddArgs(
        id: 'temp_id', // This will be set by parseArgs in CLI usage
        title: 'temp_title',
      );

      // Validate ID is unique and doesn't conflict with titles (matching Python logic)
      if (context.graph!.items.containsKey(args.id)) {
        final existingItem = context.graph!.items[args.id]!;
        return CommandResult.failure('Item with ID "${args.id}" already exists\nExisting item: ${existingItem.id} - ${existingItem.title}');
      }
      
      // Check if ID conflicts with any existing item titles
      for (final item in context.graph!.items.values) {
        if (args.id.toLowerCase() == item.title.toLowerCase()) {
          return CommandResult.failure('Item ID "${args.id}" conflicts with title of another item\nConflicting item: ${item.id} - ${item.title}');
        }
        if (item.title.toLowerCase().contains(args.id.toLowerCase())) {
          return CommandResult.failure('Item ID "${args.id}" conflicts with title of another item\nConflicting item: ${item.id} - ${item.title}');
        }
      }
      
      // Check if title conflicts with any existing item titles
      for (final item in context.graph!.items.values) {
        if (args.title.toLowerCase() == item.title.toLowerCase()) {
          return CommandResult.failure('Title "${args.title}" conflicts with title of another item\nConflicting item: ${item.id} - ${item.title}');
        }
        if (item.title.toLowerCase().contains(args.title.toLowerCase()) || 
            args.title.toLowerCase().contains(item.title.toLowerCase())) {
          return CommandResult.failure('Title "${args.title}" conflicts with title of another item\nConflicting item: ${item.id} - ${item.title}');
        }
      }
      
      // Validate that all referenced items exist
      for (final entry in args.relations.entries) {
        for (final targetId in entry.value) {
          if (!context.graph!.items.containsKey(targetId)) {
            return CommandResult.failure('Referenced item "$targetId" does not exist');
          }
        }
      }

      // Create new item
      final newItem = GianttItem(
        id: args.id,
        title: args.title,
        status: args.status,
        priority: args.priority,
        duration: args.duration ?? GianttDuration.zero(),
        charts: args.charts,
        tags: args.tags,
        relations: args.relations,
        timeConstraints: args.timeConstraints,
        userComment: null,
        autoComment: null,
        occlude: false,
      );

      if (context.dryRun) {
        return CommandResult.message(
          'Would add item: ${newItem.toFileString()}'
        );
      }

      // Add to graph
      context.graph!.addItem(newItem);

      // Check for cycles after adding the item
      try {
        context.graph!.topologicalSort();
      } on CycleDetectedException catch (e) {
        // Remove the item we just added since it creates a cycle
        context.graph!.items.remove(args.id);
        return CommandResult.failure('Adding this item would create a dependency cycle: ${e.cycleItems.join(' -> ')}');
      }

      // Save graph
      DualFileManager.saveGraph(
        context.itemsPath,
        context.occludeItemsPath,
        context.graph!,
      );

      return CommandResult.success(
        args,
        'Added item "${args.id}" successfully'
      );

    } catch (e) {
      return CommandResult.failure('Failed to add item: $e');
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<GianttItem>> addItem(
    String workspacePath,
    String id,
    String title, {
    GianttStatus status = GianttStatus.notStarted,
    GianttPriority priority = GianttPriority.neutral,
    GianttDuration? duration,
    List<String> charts = const [],
    List<String> tags = const [],
    Map<String, List<String>> relations = const {},
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    // Load existing graph
    context.graph = DualFileManager.loadGraph(
      context.itemsPath,
      context.occludeItemsPath,
    );

    // Check if item already exists
    if (context.graph!.items.containsKey(id)) {
      return CommandResult.failure('Item with ID "$id" already exists');
    }

    // Create new item
    final newItem = GianttItem(
      id: id,
      title: title,
      status: status,
      priority: priority,
      duration: duration ?? GianttDuration.zero(),
      charts: charts,
      tags: tags,
      relations: relations,
      timeConstraints: const [],
      userComment: null,
      autoComment: null,
      occlude: false,
    );

    // Add to graph
    context.graph!.addItem(newItem);

    // Save graph
    DualFileManager.saveGraph(
      context.itemsPath,
      context.occludeItemsPath,
      context.graph!,
    );

    return CommandResult.success(newItem, 'Added item "$id" successfully');
  }
}
