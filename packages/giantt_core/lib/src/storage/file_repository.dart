import 'dart:io';
import '../models/giantt_item.dart';
import '../models/log_entry.dart';
import '../models/graph_exceptions.dart';
import '../graph/giantt_graph.dart';
import '../parser/giantt_parser.dart';
import '../logging/log_collection.dart';
import 'atomic_file_writer.dart';
import 'backup_manager.dart';
import 'file_header_generator.dart';
import 'path_resolver.dart';

/// Repository for file I/O operations with include support
class FileRepository {

  /// Parse include directives from a file
  /// 
  /// Include directives should be at the top of the file in the format:
  /// #include path/to/file.txt
  static List<String> parseIncludeDirectives(String filepath) {
    final includes = <String>[];
    
    try {
      final file = File(filepath);
      if (!file.existsSync()) {
        return includes;
      }
      
      final lines = file.readAsLinesSync();
      for (final line in lines) {
        final trimmed = line.trim();
        if (trimmed.isEmpty || !trimmed.startsWith('#include ')) {
          break; // Only process directives at the top
        }
        final includePath = trimmed.substring(9).trim(); // Remove '#include ' prefix
        includes.add(includePath);
      }
    } catch (e) {
      throw GraphException('Error reading include directives from $filepath: $e');
    }
    
    return includes;
  }

  /// Load a graph from a file, processing include directives
  /// 
  /// Args:
  ///   filepath: Path to the file to load
  ///   loadedFiles: Set of files already loaded (to prevent circular includes)
  ///   
  /// Returns:
  ///   GianttGraph object
  static GianttGraph loadGraphFromFile(String filepath, [Set<String>? loadedFiles]) {
    loadedFiles ??= <String>{};
    
    // Prevent circular includes
    if (loadedFiles.contains(filepath)) {
      throw GraphException('Circular include detected for $filepath');
    }
    
    loadedFiles.add(filepath);
    
    final file = File(filepath);
    if (!file.existsSync()) {
      throw GraphException('File not found: $filepath');
    }
    
    // Process include directives
    final includes = parseIncludeDirectives(filepath);
    
    // Create the graph
    final graph = GianttGraph();
    
    // Load included files first
    for (final includePath in includes) {
      // Handle relative paths
      String resolvedPath;
      if (_isAbsolutePath(includePath)) {
        resolvedPath = includePath;
      } else {
        final baseDir = _getDirectoryPath(filepath);
        resolvedPath = _joinPaths(baseDir, includePath);
      }
      
      try {
        final includeGraph = loadGraphFromFile(resolvedPath, Set.from(loadedFiles));
        // Merge the included graph
        for (final item in includeGraph.items.values) {
          graph.addItem(item);
        }
      } catch (e) {
        throw GraphException('Error loading include $resolvedPath: $e');
      }
    }
    
    // Now load the main file content
    final lines = file.readAsLinesSync();
    final isOccludeFile = filepath.contains('occlude');
    
    for (final line in lines) {
      final trimmed = line.trim();
      if (trimmed.isNotEmpty && !trimmed.startsWith('#')) {
        try {
          final item = GianttParser.fromString(trimmed, occlude: isOccludeFile);
          graph.addItem(item);
        } catch (e) {
          // Skip invalid lines with warning
          print('Warning: Skipping invalid line in $filepath: $e');
        }
      }
    }
    
    return graph;
  }

  /// Load a graph from main and occluded files, processing includes
  static GianttGraph loadGraph(String filepath, String occludeFilepath) {
    final loadedFiles = <String>{};
    final mainGraph = loadGraphFromFile(filepath, loadedFiles);
    final occludeGraph = loadGraphFromFile(occludeFilepath, Set.from(loadedFiles));
    
    // Merge graphs
    for (final item in occludeGraph.items.values) {
      mainGraph.addItem(item);
    }
    
    return mainGraph;
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

  /// Initialize a Giantt workspace with proper directory structure
  static void initializeWorkspace(String basePath, {bool dev = false}) {
    try {
      // Create directory structure
      final includeDir = '$basePath${Platform.pathSeparator}include';
      final occludeDir = '$basePath${Platform.pathSeparator}occlude';
      
      PathResolver.ensureDirectoryExists(includeDir);
      PathResolver.ensureDirectoryExists(occludeDir);

      // Create initial files with headers
      final files = {
        '$includeDir${Platform.pathSeparator}items.txt': 
            FileHeaderGenerator.generateItemsFileHeader(),
        '$includeDir${Platform.pathSeparator}logs.jsonl': 
            FileHeaderGenerator.generateLogsFileHeader(),
        '$includeDir${Platform.pathSeparator}metadata.json': 
            FileHeaderGenerator.generateMetadataFileHeader() + '\n{}\n',
        '$occludeDir${Platform.pathSeparator}items.txt': 
            FileHeaderGenerator.generateOccludedItemsFileHeader(),
        '$occludeDir${Platform.pathSeparator}logs.jsonl': 
            FileHeaderGenerator.generateOccludedLogsFileHeader(),
        '$occludeDir${Platform.pathSeparator}metadata.json': 
            FileHeaderGenerator.generateMetadataFileHeader() + '\n{}\n',
      };

      // Only create files that don't already exist
      final filesToCreate = <String, String>{};
      for (final entry in files.entries) {
        if (!File(entry.key).existsSync()) {
          filesToCreate[entry.key] = entry.value;
        }
      }

      if (filesToCreate.isNotEmpty) {
        AtomicFileWriter.writeFiles(filesToCreate, createBackup: false);
      }

    } catch (e) {
      throw GraphException('Failed to initialize workspace: $e');
    }
  }

  /// Check if a workspace is properly initialized
  static bool isWorkspaceInitialized(String basePath) {
    return PathResolver.gianttWorkspaceExists(basePath);
  }

  /// Get the default file paths for a workspace
  static Map<String, String> getDefaultFilePaths([String? basePath]) {
    basePath ??= PathResolver.getActiveGianttWorkspace();
    
    return {
      'items': '$basePath${Platform.pathSeparator}include${Platform.pathSeparator}items.txt',
      'occlude_items': '$basePath${Platform.pathSeparator}occlude${Platform.pathSeparator}items.txt',
      'logs': '$basePath${Platform.pathSeparator}include${Platform.pathSeparator}logs.jsonl',
      'occlude_logs': '$basePath${Platform.pathSeparator}occlude${Platform.pathSeparator}logs.jsonl',
      'metadata': '$basePath${Platform.pathSeparator}include${Platform.pathSeparator}metadata.json',
      'occlude_metadata': '$basePath${Platform.pathSeparator}occlude${Platform.pathSeparator}metadata.json',
    };
  }

  /// Validate that all required files exist and are accessible
  static void validateWorkspace(String basePath) {
    final paths = getDefaultFilePaths(basePath);
    
    for (final entry in paths.entries) {
      final file = File(entry.value);
      if (!file.existsSync()) {
        throw GraphException('Required file missing: ${entry.value}');
      }
      
      // Check if file is readable
      try {
        file.readAsStringSync();
      } catch (e) {
        throw GraphException('Cannot read file ${entry.value}: $e');
      }
    }
  }

  /// Check if a path is absolute
  static bool _isAbsolutePath(String path) {
    return path.startsWith('/') || (path.length > 1 && path[1] == ':'); // Unix or Windows
  }

  /// Get the directory part of a file path
  static String _getDirectoryPath(String filepath) {
    final lastSeparator = filepath.lastIndexOf(Platform.pathSeparator);
    if (lastSeparator == -1) {
      return '.'; // Current directory
    }
    return filepath.substring(0, lastSeparator);
  }

  /// Join two path components
  static String _joinPaths(String dir, String filename) {
    if (dir.endsWith(Platform.pathSeparator)) {
      return dir + filename;
    }
    return dir + Platform.pathSeparator + filename;
  }

  /// Get the include structure of a file
  static void showIncludeStructure(String filepath, {bool recursive = false, int depth = 0, Set<String>? visited}) {
    visited ??= <String>{};
    
    final indent = '  ' * depth;
    
    if (visited.contains(filepath)) {
      print('${indent}└─ $filepath (circular include, skipping)');
      return;
    }
    
    visited.add(filepath);
    
    if (!File(filepath).existsSync()) {
      print('${indent}└─ $filepath (file not found)');
      return;
    }
    
    print('${indent}└─ $filepath');
    
    if (recursive) {
      final includes = parseIncludeDirectives(filepath);
      for (final includePath in includes) {
        // Handle relative paths
        String resolvedPath;
        if (_isAbsolutePath(includePath)) {
          resolvedPath = includePath;
        } else {
          final baseDir = _getDirectoryPath(filepath);
          resolvedPath = _joinPaths(baseDir, includePath);
        }
        
        showIncludeStructure(resolvedPath, recursive: true, depth: depth + 1, visited: Set.from(visited));
      }
    }
  }
}
