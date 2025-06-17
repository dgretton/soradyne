import 'command_interface.dart';
import '../models/giantt_item.dart';
import '../models/graph_exceptions.dart';
import '../storage/dual_file_manager.dart';

/// Arguments for sort command
class SortArgs {
  const SortArgs({
    this.dryRun = false,
    this.verbose = false,
  });

  final bool dryRun;
  final bool verbose;
}

/// Sort items using topological sort with cycle detection
class SortCommand extends CliCommand<SortArgs> {
  const SortCommand();

  @override
  String get name => 'sort';

  @override
  String get description => 'Sort items using topological sort with cycle detection';

  @override
  String get usage => 'sort [--dry-run] [--verbose]';

  @override
  SortArgs parseArgs(List<String> args) {
    bool dryRun = false;
    bool verbose = false;

    for (final arg in args) {
      switch (arg) {
        case '--dry-run':
        case '-n':
          dryRun = true;
          break;
        case '--verbose':
        case '-v':
          verbose = true;
          break;
        default:
          throw ArgumentError('Unknown argument: $arg');
      }
    }

    return SortArgs(dryRun: dryRun, verbose: verbose);
  }

  @override
  Future<CommandResult<SortArgs>> execute(CommandContext context) async {
    try {
      // Load graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = SortArgs(); // This will be set by parseArgs in CLI usage

      // Perform topological sort
      List<GianttItem> sortedItems;
      try {
        sortedItems = context.graph!.topologicalSort();
      } catch (e) {
        if (e is CycleDetectedException) {
          return CommandResult.failure(
            'Cannot sort: ${e.toString()}\n'
            'Please resolve the cycle before sorting.'
          );
        }
        rethrow;
      }

      if (context.dryRun || args.dryRun) {
        final buffer = StringBuffer();
        buffer.writeln('Would sort ${sortedItems.length} items:');
        for (int i = 0; i < sortedItems.length; i++) {
          final item = sortedItems[i];
          buffer.writeln('${i + 1}. ${item.status.symbol} ${item.id} - ${item.title}');
        }
        return CommandResult.message(buffer.toString());
      }

      // Save the sorted graph
      DualFileManager.saveGraph(
        context.itemsPath,
        context.occludeItemsPath,
        context.graph!,
      );

      final message = args.verbose 
        ? 'Sorted ${sortedItems.length} items successfully:\n${_formatSortedItems(sortedItems)}'
        : 'Sorted ${sortedItems.length} items successfully';

      return CommandResult.success(args, message);

    } catch (e) {
      return CommandResult.failure('Failed to sort items: $e');
    }
  }

  String _formatSortedItems(List<GianttItem> items) {
    final buffer = StringBuffer();
    for (int i = 0; i < items.length; i++) {
      final item = items[i];
      buffer.writeln('${i + 1}. ${item.status.symbol} ${item.id} - ${item.title}');
    }
    return buffer.toString();
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<List<GianttItem>>> sortItems(
    String workspacePath, {
    bool dryRun = false,
  }) async {
    final context = CommandContext(workspacePath: workspacePath, dryRun: dryRun);
    
    // Load graph
    context.graph = DualFileManager.loadGraph(
      context.itemsPath,
      context.occludeItemsPath,
    );

    // Perform topological sort
    List<GianttItem> sortedItems;
    try {
      sortedItems = context.graph!.topologicalSort();
    } catch (e) {
      if (e is CycleDetectedException) {
        return CommandResult.failure('Cannot sort: ${e.toString()}');
      }
      return CommandResult.failure('Failed to sort: $e');
    }

    if (!dryRun) {
      // Save the sorted graph
      DualFileManager.saveGraph(
        context.itemsPath,
        context.occludeItemsPath,
        context.graph!,
      );
    }

    return CommandResult.success(sortedItems, 'Sorted ${sortedItems.length} items successfully');
  }
}
