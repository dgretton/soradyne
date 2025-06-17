#!/usr/bin/env dart

import 'dart:io';
import 'package:args/args.dart';
import 'package:giantt_core/giantt_core.dart';
import 'package:giantt_core/src/commands/command_interface.dart';

void main(List<String> arguments) async {
  final parser = ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help information', negatable: false)
    ..addFlag('version', abbr: 'v', help: 'Show version information', negatable: false);

  // Add subcommands
  parser.addCommand('init', _createInitCommand());
  parser.addCommand('add', _createAddCommand());
  parser.addCommand('show', _createShowCommand());
  parser.addCommand('modify', _createModifyCommand());
  parser.addCommand('remove', _createRemoveCommand());
  parser.addCommand('set-status', _createSetStatusCommand());
  parser.addCommand('sort', _createSortCommand());
  parser.addCommand('touch', _createTouchCommand());
  parser.addCommand('insert', _createInsertCommand());
  parser.addCommand('includes', _createIncludesCommand());
  parser.addCommand('clean', _createCleanCommand());
  parser.addCommand('log', _createLogCommand());
  parser.addCommand('occlude', _createOccludeCommand());
  parser.addCommand('doctor', _createDoctorCommand());

  try {
    final results = parser.parse(arguments);

    if (results['help'] as bool) {
      _printUsage(parser);
      return;
    }

    if (results['version'] as bool) {
      print('giantt version 1.0.0');
      return;
    }

    if (results.command == null) {
      _printUsage(parser);
      exit(1);
    }

    // Check for subcommand help
    if (results.command!['help'] as bool? ?? false) {
      _printSubcommandHelp(results.command!.name!, parser);
      return;
    }

    await _executeCommand(results.command!);
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

void _printUsage(ArgParser parser) {
  print('Giantt command line utility for managing task dependencies.');
  print('');
  print('Usage: giantt <command> [arguments]');
  print('');
  print('Global options:');
  print(parser.usage);
  print('');
  print('Available commands:');
  print('  init        Initialize Giantt directory structure and files');
  print('  add         Add a new item to the Giantt chart');
  print('  show        Show details of an item matching the substring');
  print('  modify      Modify any property of a Giantt item');
  print('  remove      Remove an item from the Giantt chart');
  print('  set-status  Set the status of an item');
  print('  sort        Sort items in topological order and save');
  print('  touch       Touch items and logs files to trigger a reload');
  print('  insert      Insert a new item between two existing items');
  print('  includes    Show the include structure of a Giantt items file');
  print('  clean       Clean up backup files');
  print('  log         Create a log entry with session tag and message');
  print('  occlude     Occlude items or logs');
  print('  doctor      Check the health of the Giantt graph and fix issues');
  print('');
  print('Run "giantt <command> --help" for more information on a command.');
}

void _printSubcommandHelp(String commandName, ArgParser parser) {
  final command = parser.commands[commandName];
  if (command == null) {
    stderr.writeln('Unknown command: $commandName');
    exit(1);
  }

  switch (commandName) {
    case 'init':
      print('Initialize Giantt directory structure and files.');
      print('');
      print('Usage: giantt init [options]');
      print('');
      print('Options:');
      print(command.usage);
      break;
    case 'add':
      print('Add a new item to the Giantt chart.');
      print('');
      print('Usage: giantt add <id> <title> [options]');
      print('');
      print('Options:');
      print(command.usage);
      break;
    case 'show':
      print('Show details of an item matching the substring.');
      print('');
      print('Usage: giantt show <substring> [options]');
      print('');
      print('Options:');
      print(command.usage);
      break;
    case 'doctor':
      print('Check the health of the Giantt graph and fix issues.');
      print('');
      print('Usage: giantt doctor [subcommand] [options]');
      print('');
      print('Subcommands:');
      print('  check       Check for issues (default)');
      print('  fix         Fix issues automatically');
      print('  list-types  List available issue types');
      print('');
      print('Options:');
      print(command.usage);
      break;
    default:
      print('Help for $commandName command.');
      print('');
      print('Usage: giantt $commandName [options]');
      print('');
      print('Options:');
      print(command.usage);
  }
}

ArgParser _createInitCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addFlag('dev', help: 'Initialize for development', negatable: false)
    ..addOption('data-dir', help: 'Custom data directory location');
}

ArgParser _createAddCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('duration', defaultsTo: '1d', help: 'Duration (e.g., 1d, 2w, 3mo)')
    ..addOption('priority', defaultsTo: 'NEUTRAL', help: 'Priority level')
    ..addOption('charts', help: 'Comma-separated list of chart names')
    ..addOption('tags', help: 'Comma-separated list of tags')
    ..addOption('status', defaultsTo: 'NOT_STARTED', help: 'Initial status')
    ..addOption('requires', help: 'Comma-separated list of item IDs that this item requires')
    ..addOption('any-of', help: 'Comma-separated list of item IDs that are individually sufficient');
}

ArgParser _createShowCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('log-file', abbr: 'l', help: 'Giantt log file to use')
    ..addOption('occlude-log-file', help: 'Giantt occlude log file to use')
    ..addFlag('chart', help: 'Search in chart names', negatable: false)
    ..addFlag('log', help: 'Search in logs and log sessions', negatable: false);
}

ArgParser _createModifyCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addFlag('add', help: 'Add a relation', negatable: false)
    ..addFlag('remove', help: 'Remove a relation', negatable: false);
}

ArgParser _createRemoveCommand() {
  return ArgParser()
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addFlag('force', abbr: 'F', help: 'Force removal without confirmation', negatable: false)
    ..addFlag('keep-relations', help: 'Keep relations to other items', negatable: false);
}

ArgParser _createSetStatusCommand() {
  return ArgParser()
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use');
}

ArgParser _createSortCommand() {
  return ArgParser()
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use');
}

ArgParser _createTouchCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('log-file', abbr: 'l', help: 'Giantt log file to use')
    ..addOption('occlude-log-file', help: 'Giantt occlude log file to use');
}

ArgParser _createInsertCommand() {
  return ArgParser()
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('charts', help: 'Comma-separated list of charts')
    ..addOption('tags', help: 'Comma-separated list of tags')
    ..addOption('duration', defaultsTo: '1d', help: 'Duration (e.g., 1d, 2w, 3mo2w5d3s)')
    ..addOption('priority', defaultsTo: 'NEUTRAL', help: 'Priority level');
}

ArgParser _createIncludesCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addFlag('recursive', abbr: 'r', help: 'Show recursive includes', negatable: false);
}

ArgParser _createCleanCommand() {
  return ArgParser()
    ..addFlag('yes', abbr: 'y', help: 'Skip confirmation prompt', negatable: false)
    ..addOption('keep', abbr: 'k', defaultsTo: '3', help: 'Number of recent backups to keep');
}

ArgParser _createLogCommand() {
  return ArgParser()
    ..addOption('file', abbr: 'f', help: 'Logs file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Occluded logs file to use')
    ..addOption('tags', help: 'Additional comma-separated tags');
}

ArgParser _createOccludeCommand() {
  final parser = ArgParser();
  
  // Add subcommands for occlude
  parser.addCommand('items', ArgParser()
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addMultiOption('tag', abbr: 't', help: 'Occlude items with specific tags')
    ..addFlag('dry-run', help: 'Show what would be occluded without making changes', negatable: true, defaultsTo: false));
    
  parser.addCommand('logs', ArgParser()
    ..addOption('file', abbr: 'f', help: 'Giantt logs file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occlude logs file to use')
    ..addMultiOption('tag', abbr: 't', help: 'Occlude logs with specific tags')
    ..addFlag('dry-run', help: 'Show what would be occluded without making changes', negatable: true, defaultsTo: false));
    
  return parser;
}

ArgParser _createDoctorCommand() {
  final parser = ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use');
  
  // Add subcommands for doctor
  parser.addCommand('check', ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this subcommand', negatable: false));
  
  parser.addCommand('fix', ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this subcommand', negatable: false)
    ..addOption('type', abbr: 't', help: 'Type of issue to fix (e.g., dangling_reference)')
    ..addOption('item', abbr: 'i', help: 'Fix issues for a specific item ID')
    ..addFlag('all', abbr: 'a', help: 'Fix all fixable issues', negatable: false)
    ..addFlag('dry-run', help: 'Show what would be fixed without making changes', negatable: false));
    
  parser.addCommand('list-types', ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this subcommand', negatable: false));
  
  return parser;
}

Future<void> _executeCommand(ArgResults command) async {
  switch (command.name) {
    case 'init':
      await _executeInit(command);
      break;
    case 'add':
      await _executeAdd(command);
      break;
    case 'show':
      await _executeShow(command);
      break;
    case 'modify':
      await _executeModify(command);
      break;
    case 'remove':
      await _executeRemove(command);
      break;
    case 'set-status':
      await _executeSetStatus(command);
      break;
    case 'sort':
      await _executeSort(command);
      break;
    case 'touch':
      await _executeTouch(command);
      break;
    case 'insert':
      await _executeInsert(command);
      break;
    case 'includes':
      await _executeIncludes(command);
      break;
    case 'clean':
      await _executeClean(command);
      break;
    case 'log':
      await _executeLog(command);
      break;
    case 'occlude':
      await _executeOcclude(command);
      break;
    case 'doctor':
      await _executeDoctor(command);
      break;
    default:
      throw ArgumentError('Unknown command: ${command.name}');
  }
}

// Command implementations - these will be filled in with actual logic
Future<void> _executeInit(ArgResults args) async {
  final dev = args['dev'] as bool;
  final dataDir = args['data-dir'] as String?;
  
  try {
    // Determine base directory
    late Directory baseDir;
    if (dataDir != null) {
      baseDir = Directory(dataDir);
    } else if (dev) {
      baseDir = Directory('.giantt');
    } else {
      final homeDir = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
      if (homeDir == null) {
        stderr.writeln('Error: Unable to determine home directory');
        exit(1);
      }
      baseDir = Directory('$homeDir/.giantt');
    }
    
    // Create directory structure
    final includeDir = Directory('${baseDir.path}/include');
    final occludeDir = Directory('${baseDir.path}/occlude');
    
    await includeDir.create(recursive: true);
    await occludeDir.create(recursive: true);
    
    // Create initial files if they don't exist
    final files = {
      '${includeDir.path}/items.txt': _createItemsBanner(),
      '${includeDir.path}/metadata.json': '{}',
      '${includeDir.path}/logs.jsonl': '',
      '${occludeDir.path}/items.txt': _createOccludeItemsBanner(),
      '${occludeDir.path}/metadata.json': '{}',
      '${occludeDir.path}/logs.jsonl': '',
    };
    
    final alreadyExists = <String>[];
    
    for (final entry in files.entries) {
      final file = File(entry.key);
      if (await file.exists()) {
        alreadyExists.add(entry.key);
      } else {
        await file.writeAsString(entry.value);
      }
    }
    
    if (alreadyExists.length == files.length) {
      print('Giantt is already initialized at ${baseDir.path}. Enjoy!');
    } else {
      print('Initialized Giantt at ${baseDir.path}');
    }
  } catch (e) {
    stderr.writeln('Error initializing Giantt: $e');
    exit(1);
  }
}

String _createItemsBanner() {
  return '''
##############################################
#                                            #
#                Giantt Items                #
#                                            #
#   This file contains all include Giantt   #
#   items in topological order according    #
#   to the REQUIRES (⊢) relation.           #
#   You can use #include directives at the  #
#   top of this file to include other       #
#   Giantt item files.                      #
#   Edit this file manually at your own     #
#   risk.                                    #
#                                            #
##############################################

''';
}

String _createOccludeItemsBanner() {
  return '''
##############################################
#                                            #
#            Giantt Occluded Items           #
#                                            #
#   This file contains all occluded Giantt  #
#   items in topological order according    #
#   to the REQUIRES (⊢) relation.           #
#   Edit this file manually at your own     #
#   risk.                                    #
#                                            #
##############################################

''';
}

Future<void> _executeAdd(ArgResults args) async {
  if (args.rest.length < 2) {
    stderr.writeln('Error: Please provide both ID and title');
    stderr.writeln('Usage: giantt add <id> <title> [options]');
    exit(1);
  }
  
  final id = args.rest[0];
  final title = args.rest[1];
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final duration = args['duration'] as String;
  final priority = args['priority'] as String;
  final charts = args['charts'] as String?;
  final tags = args['tags'] as String?;
  final status = args['status'] as String;
  final requires = args['requires'] as String?;
  final anyOf = args['any-of'] as String?;
  
  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
    
    // Load existing graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
    
    // Check if ID already exists
    if (graph.items.containsKey(id)) {
      stderr.writeln('Error: Item with ID "$id" already exists');
      exit(1);
    }
    
    // Parse duration
    final parsedDuration = GianttDuration.parse(duration);
    
    // Parse priority
    final parsedPriority = GianttPriority.fromName(priority);
    
    // Parse status
    final parsedStatus = GianttStatus.fromName(status);
    
    // Parse charts
    final chartList = charts?.split(',').map((c) => c.trim()).toList() ?? <String>[];
    
    // Parse tags
    final tagList = tags?.split(',').map((t) => t.trim()).toList() ?? <String>[];
    
    // Parse relations
    final relations = <String, List<String>>{};
    if (requires != null) {
      relations['REQUIRES'] = requires.split(',').map((r) => r.trim()).toList();
    }
    if (anyOf != null) {
      relations['ANYOF'] = anyOf.split(',').map((a) => a.trim()).toList();
    }
    
    // Validate that required items exist
    for (final entry in relations.entries) {
      for (final targetId in entry.value) {
        if (!graph.items.containsKey(targetId)) {
          stderr.writeln('Error: Referenced item "$targetId" does not exist');
          exit(1);
        }
      }
    }
    
    // Create new item
    final newItem = GianttItem(
      id: id,
      title: title,
      status: parsedStatus,
      priority: parsedPriority,
      duration: parsedDuration,
      charts: chartList,
      tags: tagList,
      relations: relations,
    );
    
    // Add to graph
    graph.addItem(newItem);
    
    // Check for cycles
    try {
      graph.topologicalSort();
    } catch (e) {
      stderr.writeln('Error: Adding this item would create a cycle in dependencies');
      exit(1);
    }
    
    // Save graph
    FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
    
    print('Successfully added item "$id"');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeShow(ArgResults args) async {
  if (args.rest.isEmpty) {
    stderr.writeln('Error: Please provide a substring to search for');
    exit(1);
  }
  
  final substring = args.rest.first;
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final logFile = args['log-file'] as String?;
  final occludeLogFile = args['occlude-log-file'] as String?;
  final searchChart = args['chart'] as bool;
  final searchLog = args['log'] as bool;
  
  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
    
    // Load graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
    
    if (!searchChart && !searchLog) {
      // Show item details
      await _showOneItem(graph, substring);
    }
    
    if (searchChart) {
      await _showChart(graph, substring);
    }
    
    if (searchLog) {
      final logsPath = logFile ?? _getDefaultGianttPath('logs.jsonl');
      final occludeLogsPath = occludeLogFile ?? _getDefaultGianttPath('logs.jsonl', occlude: true);
      final logs = FileRepository.loadLogs(logsPath, occludeLogsPath);
      await _showLogs(logs, substring);
    }
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _showOneItem(GianttGraph graph, String substring) async {
  try {
    GianttItem item;
    
    // If there's an exact match to an ID, select that item
    if (graph.items.containsKey(substring)) {
      item = graph.items[substring]!;
    } else {
      // Otherwise, find by title substring
      item = graph.findBySubstring(substring);
    }
    
    print('Title: ${item.title}');
    print('ID: ${item.id}');
    print('Status: ${item.status.name}');
    print('Priority: ${item.priority.name}');
    print('Duration: ${item.duration}');
    print('Charts: ${item.charts.join(', ')}');
    print('Tags: ${item.tags.isNotEmpty ? item.tags.join(', ') : 'None'}');
    print('Time Constraint: ${item.timeConstraint ?? 'None'}');
    print('Relations:');
    
    if (item.relations.isEmpty) {
      // Don't print anything for empty relations to match Python
    } else {
      for (final entry in item.relations.entries) {
        print('  - ${entry.key}: ${entry.value.join(', ')}');
      }
    }
    
    print('Comment: ${item.userComment ?? 'None'}');
    print('Auto Comment: ${item.autoComment ?? 'None'}');
  } catch (e) {
    print('Error: $e');
  }
}

Future<void> _showChart(GianttGraph graph, String substring) async {
  final chartItems = <String, List<GianttItem>>{};
  
  for (final item in graph.items.values) {
    for (final chart in item.charts) {
      if (chart.toLowerCase().contains(substring.toLowerCase())) {
        chartItems.putIfAbsent(chart, () => []).add(item);
      }
    }
  }
  
  if (chartItems.isEmpty) {
    print("No items found in chart '$substring'");
    return;
  }
  
  for (final entry in chartItems.entries) {
    print("Chart '${entry.key}':");
    for (final item in entry.value) {
      print('  - ${item.id} ${item.title}');
    }
  }
}

Future<void> _showLogs(LogCollection logs, String substring) async {
  // Search in logs by session
  final sessionEntries = logs.getBySession(substring);
  if (sessionEntries.isNotEmpty) {
    print("Logs for session '$substring':");
    for (final entry in sessionEntries) {
      print('  - $entry');
    }
  }
  
  // Search in logs by substring
  final substringEntries = logs.getBySubstring(substring);
  if (substringEntries.isNotEmpty) {
    print("Logs matching '$substring':");
    for (final entry in substringEntries) {
      print('  - $entry');
    }
  }
  
  if (sessionEntries.isEmpty && substringEntries.isEmpty) {
    print("No logs found for '$substring'");
  }
}

Future<void> _executeModify(ArgResults args) async {
  print('TODO: Implement modify command');
  // TODO: Implement modify logic
}

Future<void> _executeRemove(ArgResults args) async {
  print('TODO: Implement remove command');
  // TODO: Implement remove logic
}

Future<void> _executeSetStatus(ArgResults args) async {
  print('TODO: Implement set-status command');
  // TODO: Implement set-status logic
}

Future<void> _executeSort(ArgResults args) async {
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  
  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
    
    // Check if files exist
    if (!File(itemsPath).existsSync()) {
      stderr.writeln('Error: File not found: $itemsPath');
      exit(1);
    }
    if (!File(occludeItemsPath).existsSync()) {
      stderr.writeln('Error: File not found: $occludeItemsPath');
      exit(1);
    }
    
    // Load graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
    
    // Perform topological sort (this validates the graph)
    final sortedItems = graph.topologicalSort();
    
    // Save the sorted graph
    FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
    
    print('Successfully sorted and saved items.');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeTouch(ArgResults args) async {
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final logFile = args['log-file'] as String?;
  final occludeLogFile = args['occlude-log-file'] as String?;
  
  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
    
    // Touch items files
    final itemsFile = File(itemsPath);
    final occludeItemsFile = File(occludeItemsPath);
    
    if (itemsFile.existsSync()) {
      await itemsFile.setLastModified(DateTime.now());
      print('Touched: $itemsPath');
    } else {
      print('File not found: $itemsPath');
    }
    
    if (occludeItemsFile.existsSync()) {
      await occludeItemsFile.setLastModified(DateTime.now());
      print('Touched: $occludeItemsPath');
    } else {
      print('File not found: $occludeItemsPath');
    }
    
    // Touch log files if specified
    if (logFile != null || occludeLogFile != null) {
      final logsPath = logFile ?? _getDefaultGianttPath('logs.jsonl');
      final occludeLogsPath = occludeLogFile ?? _getDefaultGianttPath('logs.jsonl', occlude: true);
      
      final logsFile = File(logsPath);
      final occludeLogsFile = File(occludeLogsPath);
      
      if (logsFile.existsSync()) {
        await logsFile.setLastModified(DateTime.now());
        print('Touched: $logsPath');
      } else {
        print('File not found: $logsPath');
      }
      
      if (occludeLogsFile.existsSync()) {
        await occludeLogsFile.setLastModified(DateTime.now());
        print('Touched: $occludeLogsPath');
      } else {
        print('File not found: $occludeLogsPath');
      }
    }
    
    print('Touch operation completed.');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeInsert(ArgResults args) async {
  print('TODO: Implement insert command');
  // TODO: Implement insert logic
}

Future<void> _executeIncludes(ArgResults args) async {
  final file = args['file'] as String?;
  final recursive = args['recursive'] as bool;
  
  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    
    if (!File(itemsPath).existsSync()) {
      stderr.writeln('Error: File not found: $itemsPath');
      exit(1);
    }
    
    print('Include structure for: $itemsPath');
    FileRepository.showIncludeStructure(itemsPath, recursive: recursive);
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeClean(ArgResults args) async {
  print('TODO: Implement clean command');
  // TODO: Implement clean logic
}

Future<void> _executeLog(ArgResults args) async {
  print('TODO: Implement log command');
  // TODO: Implement log logic
}

Future<void> _executeOcclude(ArgResults args) async {
  print('TODO: Implement occlude command');
  // TODO: Implement occlude logic
}

Future<void> _executeDoctor(ArgResults args) async {
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  
  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
    
    // Load graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);
    final doctor = GraphDoctor(graph);
    
    if (args.command == null) {
      // Default to check command
      await _executeDoctorCheck(doctor);
    } else {
      switch (args.command!.name) {
        case 'check':
          await _executeDoctorCheck(doctor);
          break;
        case 'fix':
          await _executeDoctorFix(doctor, args.command!, itemsPath, occludeItemsPath);
          break;
        case 'list-types':
          await _executeDoctorListTypes();
          break;
        default:
          throw ArgumentError('Unknown doctor subcommand: ${args.command!.name}');
      }
    }
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeDoctorCheck(GraphDoctor doctor) async {
  final issues = doctor.fullDiagnosis();
  
  if (issues.isEmpty) {
    // Don't print anything for healthy graph to match Python
    return;
  }
  
  // Group issues by type
  final issuesByType = <IssueType, List<Issue>>{};
  for (final issue in issues) {
    issuesByType.putIfAbsent(issue.type, () => []).add(issue);
  }
  
  print('Found ${issues.length} issue${issues.length != 1 ? 's' : ''}:');
  
  for (final entry in issuesByType.entries) {
    final issueType = entry.key;
    final typeIssues = entry.value;
    print('${issueType.value} (${typeIssues.length} issues):');
    
    for (final issue in typeIssues) {
      print('  • ${issue.itemId}: ${issue.message}');
      if (issue.suggestedFix != null) {
        print('    Suggested fix: ${issue.suggestedFix}');
      }
    }
  }
  
  // Exit with error code to match Python behavior
  exit(2);
}

Future<void> _executeDoctorFix(GraphDoctor doctor, ArgResults fixArgs, String itemsPath, String occludeItemsPath) async {
  final issueTypeStr = fixArgs['type'] as String?;
  final itemId = fixArgs['item'] as String?;
  final fixAll = fixArgs['all'] as bool;
  final dryRun = fixArgs['dry-run'] as bool;
  
  // Run diagnosis first
  final issues = doctor.fullDiagnosis();
  
  if (issues.isEmpty) {
    print('✓ Graph is healthy! No issues to fix.');
    return;
  }
  
  // Filter issues based on options
  List<Issue> issuesToFix = [];
  
  if (issueTypeStr != null) {
    try {
      final issueType = IssueType.fromString(issueTypeStr);
      issuesToFix = doctor.getIssuesByType(issueType);
      if (issuesToFix.isEmpty) {
        print("No issues of type '$issueTypeStr' found.");
        return;
      }
    } catch (e) {
      final validTypes = IssueType.values.map((t) => t.value).join(', ');
      print("Invalid issue type: $issueTypeStr. Valid types are: $validTypes");
      return;
    }
  } else if (itemId != null) {
    issuesToFix = issues.where((i) => i.itemId == itemId).toList();
    if (issuesToFix.isEmpty) {
      print("No issues found for item '$itemId'.");
      return;
    }
  } else if (fixAll) {
    issuesToFix = issues;
  } else {
    print('Please specify --type, --item, or --all to indicate which issues to fix.');
    return;
  }
  
  // Show what would be fixed
  print('\nFound ${issuesToFix.length} issue(s) that can be fixed:');
  for (final issue in issuesToFix) {
    print('  • ${issue.itemId}: ${issue.message}');
    if (issue.suggestedFix != null) {
      print('    Suggested fix: ${issue.suggestedFix}');
    }
  }
  
  if (dryRun) {
    print('\nDry run - no changes made.');
    return;
  }
  
  // Confirm before fixing
  stdout.write('Do you want to fix these issues? [y/N]: ');
  final input = stdin.readLineSync()?.trim().toLowerCase() ?? '';
  if (input != 'y' && input != 'yes') {
    print('Aborted. No changes made.');
    return;
  }
  
  // Fix issues
  final fixedIssues = doctor.fixIssues(
    issueType: issueTypeStr != null ? IssueType.fromString(issueTypeStr) : null,
    itemId: itemId,
  );
  
  if (fixedIssues.isNotEmpty) {
    // Save changes
    FileRepository.saveGraph(itemsPath, occludeItemsPath, doctor.graph);
    
    print('\nSuccessfully fixed ${fixedIssues.length} issue(s):');
    for (final issue in fixedIssues) {
      print('  • ${issue.itemId}: ${issue.message}');
    }
  } else {
    print('\nNo issues were fixed. Some issues may require manual intervention.');
  }
}

Future<void> _executeDoctorListTypes() async {
  print('Available issue types:');
  for (final issueType in IssueType.values) {
    print('  • ${issueType.value}');
  }
}

/// Get the default path for Giantt files
String _getDefaultGianttPath(String filename, {bool occlude = false}) {
  final filepath = '${occlude ? 'occlude' : 'include'}${Platform.pathSeparator}$filename';
  
  // First check for local .giantt directory
  final localDir = Directory('.giantt');
  if (localDir.existsSync()) {
    return '.giantt${Platform.pathSeparator}$filepath';
  }
  
  // Fall back to home directory
  final homeDir = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  if (homeDir != null) {
    final homeGianttDir = Directory('$homeDir${Platform.pathSeparator}.giantt');
    if (homeGianttDir.existsSync()) {
      return '$homeDir${Platform.pathSeparator}.giantt${Platform.pathSeparator}$filepath';
    }
  }
  
  // If neither exists, return local path for creation
  return '.giantt${Platform.pathSeparator}$filepath';
}
