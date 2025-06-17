import 'command_interface.dart';
import '../models/giantt_item.dart';
import '../storage/dual_file_manager.dart';

/// Arguments for show command
class ShowArgs {
  const ShowArgs({
    this.itemId,
    this.substring,
    this.includeOccluded = false,
    this.format = ShowFormat.detailed,
  });

  final String? itemId;
  final String? substring;
  final bool includeOccluded;
  final ShowFormat format;
}

/// Output format for show command
enum ShowFormat {
  detailed,
  brief,
  raw,
}

/// Show items from the graph
class ShowCommand extends CliCommand<ShowArgs> {
  const ShowCommand();

  @override
  String get name => 'show';

  @override
  String get description => 'Show items from the graph';

  @override
  String get usage => 'show [<id_or_substring>] [--occluded] [--brief] [--raw]';

  @override
  ShowArgs parseArgs(List<String> args) {
    String? itemId;
    String? substring;
    bool includeOccluded = false;
    ShowFormat format = ShowFormat.detailed;

    for (int i = 0; i < args.length; i++) {
      final arg = args[i];
      
      if (arg == '--occluded') {
        includeOccluded = true;
      } else if (arg == '--brief') {
        format = ShowFormat.brief;
      } else if (arg == '--raw') {
        format = ShowFormat.raw;
      } else if (!arg.startsWith('--')) {
        // First non-flag argument is the ID or substring
        if (itemId == null && substring == null) {
          // Try to determine if it's an exact ID or substring
          itemId = arg;
          substring = arg;
        }
      }
    }

    return ShowArgs(
      itemId: itemId,
      substring: substring,
      includeOccluded: includeOccluded,
      format: format,
    );
  }

  @override
  Future<CommandResult<ShowArgs>> execute(CommandContext context) async {
    try {
      // Load graph
      context.graph ??= DualFileManager.loadGraph(
        context.itemsPath,
        context.occludeItemsPath,
      );

      final args = ShowArgs(); // This will be set by parseArgs in CLI usage
      
      List<GianttItem> itemsToShow = [];

      if (args.itemId != null) {
        // Show specific item or search by substring
        try {
          final item = context.graph!.findBySubstring(args.itemId!);
          itemsToShow = [item];
        } catch (e) {
          // If not found by substring, try exact ID match
          final item = context.graph!.items[args.itemId];
          if (item != null) {
            itemsToShow = [item];
          } else {
            return CommandResult.failure('No item found with ID or substring "${args.itemId}"');
          }
        }
      } else {
        // Show all items
        itemsToShow = context.graph!.items.values.toList();
      }

      // Filter by occlusion status
      if (!args.includeOccluded) {
        itemsToShow = itemsToShow.where((item) => !item.occlude).toList();
      }

      if (itemsToShow.isEmpty) {
        return CommandResult.message('No items to show');
      }

      // Format output
      final output = _formatItems(itemsToShow, args.format);

      return CommandResult.success(args, output);

    } catch (e) {
      return CommandResult.failure('Failed to show items: $e');
    }
  }

  String _formatItems(List<GianttItem> items, ShowFormat format) {
    switch (format) {
      case ShowFormat.raw:
        return items.map((item) => item.toFileString()).join('\n');
      
      case ShowFormat.brief:
        return items.map((item) => 
          '${item.status.symbol} ${item.id}${item.priority.symbol} - ${item.title}'
        ).join('\n');
      
      case ShowFormat.detailed:
        return items.map((item) => _formatItemDetailed(item)).join('\n\n');
    }
  }

  String _formatItemDetailed(GianttItem item) {
    final buffer = StringBuffer();
    
    buffer.writeln('ID: ${item.id}');
    buffer.writeln('Title: ${item.title}');
    buffer.writeln('Status: ${item.status.name} (${item.status.symbol})');
    buffer.writeln('Priority: ${item.priority.name} (${item.priority.symbol})');
    buffer.writeln('Duration: ${item.duration}');
    
    if (item.charts.isNotEmpty) {
      buffer.writeln('Charts: ${item.charts.join(', ')}');
    }
    
    if (item.tags.isNotEmpty) {
      buffer.writeln('Tags: ${item.tags.join(', ')}');
    }
    
    if (item.relations.isNotEmpty) {
      buffer.writeln('Relations:');
      for (final entry in item.relations.entries) {
        buffer.writeln('  ${entry.key}: ${entry.value.join(', ')}');
      }
    }
    
    if (item.timeConstraints.isNotEmpty) {
      buffer.writeln('Time Constraints: ${item.timeConstraints.length}');
    }
    
    if (item.userComment != null) {
      buffer.writeln('Comment: ${item.userComment}');
    }
    
    if (item.occlude) {
      buffer.writeln('Status: OCCLUDED');
    }
    
    return buffer.toString();
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<List<GianttItem>>> getItems(
    String workspacePath, {
    String? itemId,
    String? substring,
    bool includeOccluded = false,
  }) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    // Load graph
    context.graph = DualFileManager.loadGraph(
      context.itemsPath,
      context.occludeItemsPath,
    );

    List<GianttItem> items = [];

    if (itemId != null || substring != null) {
      final searchTerm = itemId ?? substring!;
      try {
        final item = context.graph!.findBySubstring(searchTerm);
        items = [item];
      } catch (e) {
        final item = context.graph!.items[searchTerm];
        if (item != null) {
          items = [item];
        } else {
          return CommandResult.failure('No item found with ID or substring "$searchTerm"');
        }
      }
    } else {
      items = context.graph!.items.values.toList();
    }

    // Filter by occlusion status
    if (!includeOccluded) {
      items = items.where((item) => !item.occlude).toList();
    }

    return CommandResult.success(items);
  }
}
