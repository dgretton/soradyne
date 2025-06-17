import 'command_interface.dart';
import '../models/giantt_item.dart';
import '../models/status.dart';
import '../models/priority.dart';
import '../models/duration.dart';
import '../storage/dual_file_manager.dart';

/// Arguments for insert command
class InsertArgs {
  const InsertArgs({
    required this.newId,
    required this.title,
    required this.beforeId,
    required this.afterId,
    this.status = GianttStatus.notStarted,
    this.priority = GianttPriority.neutral,
    this.duration,
  });

  final String newId;
  final String title;
  final String beforeId;
  final String afterId;
  final GianttStatus status;
  final GianttPriority priority;
  final GianttDuration? duration;
}

/// Insert a new item between two existing items in the dependency chain
class InsertCommand extends CliCommand<InsertArgs> {
  const InsertCommand();

  @override
  String get name => 'insert';

  @override
  String get description => 'Insert a new item between two existing items in the dependency chain';

  @override
  String get usage => 'insert <new_id> "<title>" <before_id> <after_id> [options]';

  @override
  InsertArgs parseArgs(List<String> args) {
    if (args.length < 4) {
      throw ArgumentError('insert requires new_id, title, before_id, and after_id');
    }

    final newId = args[0];
    final title = args[1];
    final beforeId = args[2];
    final afterId = args[3];

    // Parse optional arguments
    GianttStatus status = GianttStatus.notStarted;
    GianttPriority priority = GianttPriority.neutral;
    GianttDuration? duration;

    for (int i = 4; i < args.length; i++) {
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
      }
    }

    return InsertArgs(
      newId: newId,
      title: title,
      beforeId: beforeId,
      afterId: afterId,
      status: status,
      priority: priority,
      duration: duration,
    );
  }

  @override
  Future<CommandResult<InsertArgs>> execute(CommandContext context) async {
    try {
      // Load graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = InsertArgs(
        newId: 'temp',
        title: 'temp',
        beforeId: 'temp',
        afterId: 'temp',
      ); // This will be set by parseArgs in CLI usage

      // Check if new item ID already exists
      if (context.graph!.items.containsKey(args.newId)) {
        return CommandResult.failure('Item with ID "${args.newId}" already exists');
      }

      // Check if before and after items exist
      if (!context.graph!.items.containsKey(args.beforeId)) {
        return CommandResult.failure('Before item "${args.beforeId}" not found');
      }
      if (!context.graph!.items.containsKey(args.afterId)) {
        return CommandResult.failure('After item "${args.afterId}" not found');
      }

      // Create new item
      final newItem = GianttItem(
        id: args.newId,
        title: args.title,
        status: args.status,
        priority: args.priority,
        duration: args.duration ?? GianttDuration.zero(),
        charts: [],
        tags: [],
        relations: {},
        timeConstraints: const [],
        userComment: null,
        autoComment: null,
        occlude: false,
      );

      if (context.dryRun) {
        return CommandResult.message(
          'Would insert item "${args.newId}" between "${args.beforeId}" and "${args.afterId}":\n'
          '${newItem.toFileString()}'
        );
      }

      // Insert the item between the two existing items
      context.graph!.insertBetween(newItem, args.beforeId, args.afterId);

      // Save graph
      DualFileManager.saveGraph(
        context.itemsPath,
        context.occludeItemsPath,
        context.graph!,
      );

      return CommandResult.success(
        args,
        'Inserted item "${args.newId}" between "${args.beforeId}" and "${args.afterId}"'
      );

    } catch (e) {
      return CommandResult.failure('Failed to insert item: $e');
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<GianttItem>> insertItem(
    String workspacePath,
    String newId,
    String title,
    String beforeId,
    String afterId, {
    GianttStatus status = GianttStatus.notStarted,
    GianttPriority priority = GianttPriority.neutral,
    GianttDuration? duration,
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    // Load graph
    context.graph = DualFileManager.loadGraph(
      context.itemsPath,
      context.occludeItemsPath,
    );

    // Check if new item ID already exists
    if (context.graph!.items.containsKey(newId)) {
      return CommandResult.failure('Item with ID "$newId" already exists');
    }

    // Check if before and after items exist
    if (!context.graph!.items.containsKey(beforeId)) {
      return CommandResult.failure('Before item "$beforeId" not found');
    }
    if (!context.graph!.items.containsKey(afterId)) {
      return CommandResult.failure('After item "$afterId" not found');
    }

    // Create new item
    final newItem = GianttItem(
      id: newId,
      title: title,
      status: status,
      priority: priority,
      duration: duration ?? GianttDuration.zero(),
      charts: [],
      tags: [],
      relations: {},
      timeConstraints: const [],
      userComment: null,
      autoComment: null,
      occlude: false,
    );

    // Insert the item between the two existing items
    context.graph!.insertBetween(newItem, beforeId, afterId);

    // Save graph
    DualFileManager.saveGraph(
      context.itemsPath,
      context.occludeItemsPath,
      context.graph!,
    );

    return CommandResult.success(
      newItem,
      'Inserted item "$newId" between "$beforeId" and "$afterId"'
    );
  }
}
