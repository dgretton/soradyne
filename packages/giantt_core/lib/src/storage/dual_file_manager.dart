import 'dart:io';
import '../models/giantt_item.dart';
import '../models/log_entry.dart';
import '../models/graph_exceptions.dart';
import '../graph/giantt_graph.dart';
import '../parser/giantt_parser.dart';
import '../logging/log_collection.dart';
import '../logging/log_occluder.dart';
import 'atomic_file_writer.dart';
import 'backup_manager.dart';
import 'file_header_generator.dart';
import 'file_repository.dart';
import 'path_resolver.dart';

/// Manages dual-file operations for include/occlude system
class DualFileManager {
  /// Load a graph from main and occluded files, processing includes
  static GianttGraph loadGraph(String filepath, String occludeFilepath) {
    final loadedFiles = <String>{};
    final mainGraph = FileRepository.loadGraphFromFile(filepath, loadedFiles);
    final occludeGraph = FileRepository.loadGraphFromFile(occludeFilepath, Set.from(loadedFiles));
    
    // Merge graphs
    for (final item in occludeGraph.items.values) {
      mainGraph.addItem(item);
    }
    
    return mainGraph;
  }

  /// Save a graph to files with atomic operations and proper headers
  static void saveGraph(String filepath, String occludeFilepath, GianttGraph graph) {
    try {
      // First perform topological sort to validate the graph
      final sortedItems = graph.topologicalSort();

      // Prepare file contents
      final includeContent = StringBuffer();
      final occludeContent = StringBuffer();

      // Add headers
      includeContent.writeln(FileHeaderGenerator.generateItemsFileHeader());
      includeContent.writeln();
      
      occludeContent.writeln(FileHeaderGenerator.generateOccludedItemsFileHeader());
      occludeContent.writeln();

      // Add items to appropriate files
      for (final item in sortedItems) {
        final itemString = item.toFileString();
        if (item.occlude) {
          occludeContent.writeln(itemString);
        } else {
          includeContent.writeln(itemString);
        }
      }

      // Write both files atomically
      final fileContents = {
        filepath: includeContent.toString(),
        occludeFilepath: occludeContent.toString(),
      };

      AtomicFileWriter.writeFiles(fileContents);

    } catch (e) {
      if (e is GraphException) rethrow;
      throw GraphException('Failed to save graph: $e');
    }
  }

  /// Occlude items by ID with optional dry-run
  static OccludeResult occludeItems(
    GianttGraph graph, 
    List<String> itemIds, 
    {bool dryRun = false}
  ) {
    final toOcclude = <String>[];
    final notFound = <String>[];

    // Find items to occlude
    for (final id in itemIds) {
      final item = graph.items[id];
      if (item != null && !item.occlude) {
        toOcclude.add(id);
      } else if (item == null) {
        notFound.add(id);
      }
    }

    if (dryRun) {
      return OccludeResult(
        occludedCount: toOcclude.length,
        occludedItems: toOcclude,
        notFoundItems: notFound,
        dryRun: true,
      );
    }

    // Actually occlude the items
    for (final id in toOcclude) {
      final item = graph.items[id]!;
      final occludedItem = item.copyWith(occlude: true);
      graph.addItem(occludedItem);
    }

    return OccludeResult(
      occludedCount: toOcclude.length,
      occludedItems: toOcclude,
      notFoundItems: notFound,
      dryRun: false,
    );
  }

  /// Occlude items by tags with optional dry-run
  static OccludeResult occludeItemsByTags(
    GianttGraph graph, 
    List<String> tags, 
    {bool dryRun = false}
  ) {
    final toOcclude = <String>[];

    // Find items with matching tags
    for (final item in graph.includedItems.values) {
      if (tags.any((tag) => item.tags.contains(tag))) {
        toOcclude.add(item.id);
      }
    }

    return occludeItems(graph, toOcclude, dryRun: dryRun);
  }

  /// Include (un-occlude) items by ID with optional dry-run
  static OccludeResult includeItems(
    GianttGraph graph, 
    List<String> itemIds, 
    {bool dryRun = false}
  ) {
    final toInclude = <String>[];
    final notFound = <String>[];

    // Find items to include
    for (final id in itemIds) {
      final item = graph.items[id];
      if (item != null && item.occlude) {
        toInclude.add(id);
      } else if (item == null) {
        notFound.add(id);
      }
    }

    if (dryRun) {
      return OccludeResult(
        occludedCount: toInclude.length,
        occludedItems: toInclude,
        notFoundItems: notFound,
        dryRun: true,
      );
    }

    // Actually include the items
    for (final id in toInclude) {
      final item = graph.items[id]!;
      final includedItem = item.copyWith(occlude: false);
      graph.addItem(includedItem);
    }

    return OccludeResult(
      occludedCount: toInclude.length,
      occludedItems: toInclude,
      notFoundItems: notFound,
      dryRun: false,
    );
  }

  /// Load logs from main and occluded files
  static LogCollection loadLogs(String filepath, String occludeFilepath) {
    final logs = LogCollection();
    
    // Load include logs
    logs.addEntries(_loadLogsFromFile(filepath, occlude: false));
    
    // Load occlude logs
    logs.addEntries(_loadLogsFromFile(occludeFilepath, occlude: true));
    
    return logs;
  }

  /// Save logs to files with atomic operations
  static void saveLogs(String filepath, String occludeFilepath, LogCollection logs) {
    try {
      // Prepare file contents
      final includeContent = StringBuffer();
      final occludeContent = StringBuffer();

      // Add headers
      includeContent.writeln(FileHeaderGenerator.generateLogsFileHeader());
      occludeContent.writeln(FileHeaderGenerator.generateOccludedLogsFileHeader());

      // Add logs to appropriate files
      for (final log in logs.entries) {
        final logLine = log.toJsonLine();
        if (log.occlude) {
          occludeContent.writeln(logLine);
        } else {
          includeContent.writeln(logLine);
        }
      }

      // Write both files atomically
      final fileContents = {
        filepath: includeContent.toString(),
        occludeFilepath: occludeContent.toString(),
      };

      AtomicFileWriter.writeFiles(fileContents);

    } catch (e) {
      throw GraphException('Failed to save logs: $e');
    }
  }

  /// Occlude logs by session tags with optional dry-run
  static LogOccludeResult occludeLogs(
    LogCollection logs, 
    List<String> sessionTags, 
    List<String> tags,
    {bool dryRun = false}
  ) {
    final toOcclude = <LogEntry>[];

    for (final log in logs.includedEntries) {
      bool shouldOcclude = false;

      // Check session tags
      if (sessionTags.contains(log.session)) {
        shouldOcclude = true;
      }

      // Check other tags
      if (tags.any((tag) => log.tags.contains(tag))) {
        shouldOcclude = true;
      }

      if (shouldOcclude) {
        toOcclude.add(log);
      }
    }

    if (dryRun) {
      return LogOccludeResult(
        occludedCount: toOcclude.length,
        occludedLogs: toOcclude,
        dryRun: true,
      );
    }

    // Actually occlude the logs
    for (final log in toOcclude) {
      final occludedLog = log.copyWith(occlude: true);
      logs.replaceEntry(log, occludedLog);
    }

    return LogOccludeResult(
      occludedCount: toOcclude.length,
      occludedLogs: toOcclude,
      dryRun: false,
    );
  }

  /// Load logs from a single file
  static List<LogEntry> _loadLogsFromFile(String filepath, {required bool occlude}) {
    final logs = <LogEntry>[];
    
    try {
      final file = File(filepath);
      if (!file.existsSync()) {
        return logs;
      }

      final lines = file.readAsLinesSync();
      for (final line in lines) {
        final trimmed = line.trim();
        if (trimmed.isNotEmpty && !trimmed.startsWith('#')) {
          try {
            final log = LogEntry.fromJsonLine(trimmed, occlude: occlude);
            logs.add(log);
          } catch (e) {
            // Skip invalid lines with warning
            print('Warning: Skipping invalid log line in $filepath: $e');
          }
        }
      }
    } catch (e) {
      throw GraphException('Error loading logs from $filepath: $e');
    }
    
    return logs;
  }
}

/// Result of an occlude operation
class OccludeResult {
  const OccludeResult({
    required this.occludedCount,
    required this.occludedItems,
    required this.notFoundItems,
    required this.dryRun,
  });

  final int occludedCount;
  final List<String> occludedItems;
  final List<String> notFoundItems;
  final bool dryRun;

  bool get hasNotFound => notFoundItems.isNotEmpty;
  bool get hasOccluded => occludedCount > 0;
}

// LogOccludeResult is imported from log_occluder.dart

