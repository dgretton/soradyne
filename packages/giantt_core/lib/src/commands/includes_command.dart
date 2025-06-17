import 'dart:io';
import 'command_interface.dart';
import '../storage/path_resolver.dart';

/// Arguments for includes command
class IncludesArgs {
  const IncludesArgs({
    this.verbose = false,
  });

  final bool verbose;
}

/// Show include structure visualization
class IncludesCommand extends CliCommand<IncludesArgs> {
  const IncludesCommand();

  @override
  String get name => 'includes';

  @override
  String get description => 'Show include structure visualization';

  @override
  String get usage => 'includes [--verbose]';

  @override
  IncludesArgs parseArgs(List<String> args) {
    bool verbose = false;

    for (final arg in args) {
      switch (arg) {
        case '--verbose':
        case '-v':
          verbose = true;
          break;
        default:
          throw ArgumentError('Unknown argument: $arg');
      }
    }

    return IncludesArgs(verbose: verbose);
  }

  @override
  Future<CommandResult<IncludesArgs>> execute(CommandContext context) async {
    try {
      final args = IncludesArgs(); // This will be set by parseArgs in CLI usage

      // Check if workspace exists
      if (!PathResolver.gianttWorkspaceExists(context.workspacePath)) {
        return CommandResult.failure('No giantt workspace found at ${context.workspacePath}');
      }

      // Build include structure starting from main files
      final includeStructure = _buildIncludeStructure(context.itemsPath);
      final occludeStructure = _buildIncludeStructure(context.occludeItemsPath);

      final buffer = StringBuffer();
      
      // Show include file structure
      if (includeStructure.isNotEmpty) {
        buffer.writeln('Include file structure:');
        _formatIncludeTree(buffer, includeStructure, '', <String>{});
        buffer.writeln();
      }

      // Show occlude file structure
      if (occludeStructure.isNotEmpty) {
        buffer.writeln('Occlude file structure:');
        _formatIncludeTree(buffer, occludeStructure, '', <String>{});
        buffer.writeln();
      }

      if (includeStructure.isEmpty && occludeStructure.isEmpty) {
        buffer.writeln('No include directives found in workspace files.');
      }

      // Show additional details if verbose
      if (args.verbose || context.verbose) {
        buffer.writeln('Workspace files:');
        final files = [
          context.itemsPath,
          context.occludeItemsPath,
          context.logsPath,
          context.occludeLogsPath,
        ];

        for (final filepath in files) {
          final file = File(filepath);
          if (file.existsSync()) {
            final includes = _parseIncludeDirectives(filepath);
            buffer.writeln('  $filepath (${includes.length} includes)');
            for (final include in includes) {
              buffer.writeln('    → $include');
            }
          } else {
            buffer.writeln('  $filepath (missing)');
          }
        }
      }

      return CommandResult.success(args, buffer.toString());

    } catch (e) {
      return CommandResult.failure('Failed to show include structure: $e');
    }
  }

  /// Build include structure tree
  Map<String, List<String>> _buildIncludeStructure(String rootFile) {
    final structure = <String, List<String>>{};
    final visited = <String>{};

    void buildTree(String filepath) {
      if (visited.contains(filepath)) return;
      visited.add(filepath);

      final includes = _parseIncludeDirectives(filepath);
      if (includes.isNotEmpty) {
        structure[filepath] = includes;
        for (final include in includes) {
          final resolvedPath = PathResolver.resolvePath(
            PathResolver._getDirectoryPath(filepath),
            include,
          );
          buildTree(resolvedPath);
        }
      }
    }

    buildTree(rootFile);
    return structure;
  }

  /// Parse include directives from a file
  List<String> _parseIncludeDirectives(String filepath) {
    final includes = <String>[];
    
    try {
      final file = File(filepath);
      if (!file.existsSync()) return includes;

      final lines = file.readAsLinesSync();
      
      for (final line in lines) {
        final trimmed = line.trim();
        
        // Stop parsing includes when we hit non-directive content
        if (trimmed.isNotEmpty && !trimmed.startsWith('#')) {
          break;
        }
        
        // Parse include directive
        if (trimmed.startsWith('#include ')) {
          final includePath = trimmed.substring(9).trim();
          includes.add(includePath);
        }
      }
    } catch (e) {
      // Ignore file read errors
    }
    
    return includes;
  }

  /// Format include tree with proper indentation and cycle detection
  void _formatIncludeTree(
    StringBuffer buffer,
    Map<String, List<String>> structure,
    String indent,
    Set<String> currentPath,
  ) {
    for (final entry in structure.entries) {
      final filepath = entry.key;
      final includes = entry.value;
      
      buffer.writeln('${indent}└─ $filepath');
      
      if (currentPath.contains(filepath)) {
        buffer.writeln('$indent  └─ (circular include, skipping)');
        continue;
      }
      
      final newPath = Set<String>.from(currentPath)..add(filepath);
      final newIndent = '$indent  ';
      
      for (final include in includes) {
        final resolvedPath = PathResolver.resolvePath(
          PathResolver._getDirectoryPath(filepath),
          include,
        );
        
        if (structure.containsKey(resolvedPath)) {
          _formatIncludeTree(buffer, {resolvedPath: structure[resolvedPath]!}, newIndent, newPath);
        } else {
          buffer.writeln('${newIndent}└─ $resolvedPath');
        }
      }
    }
  }

  /// Static method for programmatic use (Flutter)
  static Future<CommandResult<Map<String, dynamic>>> getIncludeStructure(
    String workspacePath,
  ) async {
    final context = CommandContext(workspacePath: workspacePath);
    
    // Check if workspace exists
    if (!PathResolver.gianttWorkspaceExists(workspacePath)) {
      return CommandResult.failure('No giantt workspace found at $workspacePath');
    }

    final command = IncludesCommand();
    
    // Build include structures
    final includeStructure = command._buildIncludeStructure(context.itemsPath);
    final occludeStructure = command._buildIncludeStructure(context.occludeItemsPath);

    // Get file details
    final files = [
      context.itemsPath,
      context.occludeItemsPath,
      context.logsPath,
      context.occludeLogsPath,
    ];

    final fileDetails = <String, dynamic>{};
    for (final filepath in files) {
      final file = File(filepath);
      if (file.existsSync()) {
        final includes = command._parseIncludeDirectives(filepath);
        fileDetails[filepath] = {
          'exists': true,
          'includes': includes,
        };
      } else {
        fileDetails[filepath] = {
          'exists': false,
          'includes': <String>[],
        };
      }
    }

    final results = {
      'include_structure': includeStructure,
      'occlude_structure': occludeStructure,
      'file_details': fileDetails,
    };

    return CommandResult.success(results, 'Include structure analyzed');
  }
}

/// Extension to access private method
extension PathResolverExtension on PathResolver {
  static String _getDirectoryPath(String filepath) {
    final lastSeparator = filepath.lastIndexOf(Platform.pathSeparator);
    if (lastSeparator == -1) {
      return '.'; // Current directory
    }
    return filepath.substring(0, lastSeparator);
  }
}
