import 'command_interface.dart';
import '../models/giantt_item.dart';
import '../models/status.dart';
import '../storage/dual_file_manager.dart';

/// Arguments for set-status command
class SetStatusArgs {
  const SetStatusArgs({
    required this.itemId,
    required this.status,
  });

  final String itemId;
  final GianttStatus status;
}

/// Set the status of an item
class SetStatusCommand extends CliCommand<SetStatusArgs> {
  const SetStatusCommand();

  @override
  String get name => 'set-status';

  @override
  String get description => 'Set the status of an item';

  @override
  String get usage => 'set-status <id> <status>';

  @override
  SetStatusArgs parseArgs(List<String> args) {
    if (args.length < 2) {
      throw ArgumentError('set-status requires item ID and status');
    }

    final itemId = args[0];
    final statusStr = args[1];
    
    GianttStatus status;
    try {
      // Try parsing as symbol first
      status = GianttStatus.fromSymbol(statusStr);
    } catch (e) {
      try {
        // Try parsing as name
        status = GianttStatus.fromName(statusStr.toUpperCase());
      } catch (e) {
        throw ArgumentError('Invalid status: $statusStr. Valid statuses: ○ (not_started), ◑ (in_progress), ⊘ (blocked), ● (completed)');
      }
    }

    return SetStatusArgs(itemId: itemId, status: status);
  }

  @override
  Future<CommandResult<SetStatusArgs>> execute(CommandContext context) async {
    try {
      // Load existing graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = SetStatusArgs(itemId: 'temp', status: GianttStatus.notStarted); // This will be set by parseArgs in CLI usage

      // Find the item
      final existingItem = context.graph!.items[args.itemId];
      if (existingItem == null) {
        return CommandResult.failure('Item with ID "${args.itemId}" not found');
      }

      if (context.dryRun) {
        return CommandResult.message(
          'Would set status of "${args.itemId}" from ${existingItem.status.symbol} to ${args.status.symbol}'
        );
      }

      // Update status
      final updatedItem = existingItem.copyWith(status: args.status);
      context.graph!.addItem(updatedItem);

      // Save graph
      DualFileManager.saveGraph(
        context.itemsPath,
        context.occludeItemsPath,
        context.graph!,
      );

      return CommandResult.success(
        args,
        'Set status of "${args.itemId}" to ${args.status.name} (${args.status.symbol})'
      );

    } catch (e) {
      return CommandResult.failure('Failed to set status: $e');
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<GianttItem>> setItemStatus(
    String workspacePath,
    String itemId,
    GianttStatus status,
  ) async {
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

    // Update status
    final updatedItem = existingItem.copyWith(status: status);
    context.graph!.addItem(updatedItem);

    // Save graph
    DualFileManager.saveGraph(
      context.itemsPath,
      context.occludeItemsPath,
      context.graph!,
    );

    return CommandResult.success(
      updatedItem,
      'Set status of "$itemId" to ${status.name} (${status.symbol})'
    );
  }
}
