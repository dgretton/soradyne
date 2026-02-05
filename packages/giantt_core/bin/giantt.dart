#!/usr/bin/env dart

import 'dart:io';
import 'package:args/args.dart';
import 'package:giantt_core/giantt_core.dart';

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
  parser.addCommand('add-include', _createAddIncludeCommand());

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
  print('  add-include Add an include directive to a Giantt items file');
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
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addFlag('force', abbr: 'F', help: 'Force removal without confirmation', negatable: false)
    ..addFlag('keep-relations', help: 'Keep relations to other items', negatable: false);
}

ArgParser _createSetStatusCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use');
}

ArgParser _createSortCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
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
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
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
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addFlag('yes', abbr: 'y', help: 'Skip confirmation prompt', negatable: false)
    ..addOption('keep', abbr: 'k', defaultsTo: '3', help: 'Number of recent backups to keep');
}

ArgParser _createLogCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Logs file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Occluded logs file to use')
    ..addOption('tags', help: 'Additional comma-separated tags');
}

ArgParser _createOccludeCommand() {
  final parser = ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false);

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

ArgParser _createAddIncludeCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use');
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
    case 'add-include':
      await _executeAddInclude(command);
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
  return FileHeaderGenerator.generateItemsFileHeader();
}

String _createOccludeItemsBanner() {
  return FileHeaderGenerator.generateOccludedItemsFileHeader();
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
    
    // Validate ID is unique and doesn't conflict with titles (matching Python logic)
    if (graph.items.containsKey(id)) {
      final existingItem = graph.items[id]!;
      stderr.writeln('Error: Item with ID "$id" already exists');
      stderr.writeln('Existing item: ${existingItem.id} - ${existingItem.title}');
      exit(1);
    }
    
    // Check if ID conflicts with any existing item titles
    for (final item in graph.items.values) {
      if (id.toLowerCase() == item.title.toLowerCase()) {
        stderr.writeln('Error: Item ID "$id" conflicts with title of another item');
        stderr.writeln('Conflicting item: ${item.id} - ${item.title}');
        exit(1);
      }
      if (item.title.toLowerCase().contains(id.toLowerCase())) {
        stderr.writeln('Error: Item ID "$id" conflicts with title of another item');
        stderr.writeln('Conflicting item: ${item.id} - ${item.title}');
        exit(1);
      }
    }
    
    // Check if title conflicts with any existing item titles
    for (final item in graph.items.values) {
      if (title.toLowerCase() == item.title.toLowerCase()) {
        stderr.writeln('Error: Title "$title" conflicts with title of another item');
        stderr.writeln('Conflicting item: ${item.id} - ${item.title}');
        exit(1);
      }
      if (item.title.toLowerCase().contains(title.toLowerCase()) || 
          title.toLowerCase().contains(item.title.toLowerCase())) {
        stderr.writeln('Error: Title "$title" conflicts with title of another item');
        stderr.writeln('Conflicting item: ${item.id} - ${item.title}');
        exit(1);
      }
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
    print('Time Constraints: ${item.timeConstraints.isNotEmpty ? item.timeConstraints.join(', ') : 'None'}');
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
  if (args.rest.length < 3) {
    stderr.writeln('Error: Please provide item ID, property, and value');
    stderr.writeln('Usage: giantt modify <id> <property> <value> [--add|--remove]');
    exit(1);
  }

  final itemId = args.rest[0];
  final property = args.rest[1];
  final value = args.rest[2];
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final addMode = args['add'] as bool;
  final removeMode = args['remove'] as bool;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);

    // Find item by ID or substring
    GianttItem? item;
    if (graph.items.containsKey(itemId)) {
      item = graph.items[itemId];
    } else {
      try {
        item = graph.findBySubstring(itemId);
      } catch (e) {
        stderr.writeln('Error: Item "$itemId" not found');
        exit(1);
      }
    }

    if (item == null) {
      stderr.writeln('Error: Item "$itemId" not found');
      exit(1);
    }

    // Apply modification based on property
    GianttItem modifiedItem;
    switch (property.toLowerCase()) {
      case 'title':
        modifiedItem = item.copyWith(title: value);
        break;
      case 'status':
        modifiedItem = item.copyWith(status: GianttStatus.fromName(value.toUpperCase()));
        break;
      case 'priority':
        modifiedItem = item.copyWith(priority: GianttPriority.fromName(value.toUpperCase()));
        break;
      case 'duration':
        modifiedItem = item.copyWith(duration: GianttDuration.parse(value));
        break;
      case 'charts':
        final chartsList = value.split(',').map((c) => c.trim()).toList();
        if (addMode) {
          modifiedItem = item.copyWith(charts: [...item.charts, ...chartsList]);
        } else if (removeMode) {
          modifiedItem = item.copyWith(charts: item.charts.where((c) => !chartsList.contains(c)).toList());
        } else {
          modifiedItem = item.copyWith(charts: chartsList);
        }
        break;
      case 'tags':
        final tagsList = value.split(',').map((t) => t.trim()).toList();
        if (addMode) {
          modifiedItem = item.copyWith(tags: [...item.tags, ...tagsList]);
        } else if (removeMode) {
          modifiedItem = item.copyWith(tags: item.tags.where((t) => !tagsList.contains(t)).toList());
        } else {
          modifiedItem = item.copyWith(tags: tagsList);
        }
        break;
      case 'requires':
        final requiresList = value.split(',').map((r) => r.trim()).toList();
        final newRelations = Map<String, List<String>>.from(item.relations);
        if (addMode) {
          newRelations['REQUIRES'] = [...(newRelations['REQUIRES'] ?? []), ...requiresList];
        } else if (removeMode) {
          newRelations['REQUIRES'] = (newRelations['REQUIRES'] ?? []).where((r) => !requiresList.contains(r)).toList();
          if (newRelations['REQUIRES']!.isEmpty) newRelations.remove('REQUIRES');
        } else {
          newRelations['REQUIRES'] = requiresList;
        }
        modifiedItem = item.copyWith(relations: newRelations);
        break;
      case 'anyof':
        final anyOfList = value.split(',').map((a) => a.trim()).toList();
        final newRelations = Map<String, List<String>>.from(item.relations);
        if (addMode) {
          newRelations['ANYOF'] = [...(newRelations['ANYOF'] ?? []), ...anyOfList];
        } else if (removeMode) {
          newRelations['ANYOF'] = (newRelations['ANYOF'] ?? []).where((a) => !anyOfList.contains(a)).toList();
          if (newRelations['ANYOF']!.isEmpty) newRelations.remove('ANYOF');
        } else {
          newRelations['ANYOF'] = anyOfList;
        }
        modifiedItem = item.copyWith(relations: newRelations);
        break;
      case 'blocks':
        final blocksList = value.split(',').map((b) => b.trim()).toList();
        final newRelations = Map<String, List<String>>.from(item.relations);
        if (addMode) {
          newRelations['BLOCKS'] = [...(newRelations['BLOCKS'] ?? []), ...blocksList];
        } else if (removeMode) {
          newRelations['BLOCKS'] = (newRelations['BLOCKS'] ?? []).where((b) => !blocksList.contains(b)).toList();
          if (newRelations['BLOCKS']!.isEmpty) newRelations.remove('BLOCKS');
        } else {
          newRelations['BLOCKS'] = blocksList;
        }
        modifiedItem = item.copyWith(relations: newRelations);
        break;
      case 'comment':
        modifiedItem = item.copyWith(userComment: value);
        break;
      default:
        stderr.writeln('Error: Unknown property "$property"');
        stderr.writeln('Valid properties: title, status, priority, duration, charts, tags, requires, anyof, blocks, comment');
        exit(1);
    }

    // Update in graph
    graph.addItem(modifiedItem);

    // Check for cycles if relations were modified
    if (['requires', 'anyof', 'blocks'].contains(property.toLowerCase())) {
      try {
        graph.topologicalSort();
      } catch (e) {
        stderr.writeln('Error: Modification would create a cycle in dependencies');
        exit(1);
      }
    }

    // Save graph
    FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);

    print('Successfully modified "$property" for item "${item.id}"');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeRemove(ArgResults args) async {
  if (args.rest.isEmpty) {
    stderr.writeln('Error: Please provide an item ID to remove');
    stderr.writeln('Usage: giantt remove <id> [--force]');
    exit(1);
  }

  final itemId = args.rest[0];
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final force = args['force'] as bool;
  final keepRelations = args['keep-relations'] as bool;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);

    // Find the item
    if (!graph.items.containsKey(itemId)) {
      stderr.writeln('Error: Item "$itemId" not found');
      exit(1);
    }

    // Check for dependencies unless force is used
    if (!force) {
      final dependentItems = <String>[];
      for (final item in graph.items.values) {
        final requires = item.relations['REQUIRES'] ?? [];
        final anyOf = item.relations['ANYOF'] ?? [];
        if (requires.contains(itemId) || anyOf.contains(itemId)) {
          dependentItems.add(item.id);
        }
      }

      if (dependentItems.isNotEmpty) {
        stderr.writeln('Error: Cannot remove item "$itemId" because it is required by: ${dependentItems.join(", ")}');
        stderr.writeln('Use --force to remove anyway.');
        exit(1);
      }
    }

    // Remove the item
    graph.removeItem(itemId);

    // Clean up relations that reference this item (unless keep-relations is set)
    if (!keepRelations) {
      for (final item in graph.items.values) {
        bool modified = false;
        final newRelations = <String, List<String>>{};

        for (final entry in item.relations.entries) {
          final cleanedTargets = entry.value.where((target) => target != itemId).toList();
          if (cleanedTargets.length != entry.value.length) {
            modified = true;
          }
          if (cleanedTargets.isNotEmpty) {
            newRelations[entry.key] = cleanedTargets;
          }
        }

        if (modified) {
          final updatedItem = item.copyWith(relations: newRelations);
          graph.addItem(updatedItem);
        }
      }
    }

    // Save graph
    FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);

    print('Successfully removed item "$itemId"');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeSetStatus(ArgResults args) async {
  if (args.rest.length < 2) {
    stderr.writeln('Error: Please provide item ID/substring and new status');
    stderr.writeln('Usage: giantt set-status <id> <status>');
    stderr.writeln('Valid statuses: NOT_STARTED, IN_PROGRESS, BLOCKED, COMPLETED');
    exit(1);
  }

  final itemId = args.rest[0];
  final statusStr = args.rest[1];
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);

    // Find item by ID or substring
    GianttItem? item;
    if (graph.items.containsKey(itemId)) {
      item = graph.items[itemId];
    } else {
      try {
        item = graph.findBySubstring(itemId);
      } catch (e) {
        stderr.writeln('Error: Item "$itemId" not found');
        exit(1);
      }
    }

    if (item == null) {
      stderr.writeln('Error: Item "$itemId" not found');
      exit(1);
    }

    // Parse the new status
    GianttStatus newStatus;
    try {
      newStatus = GianttStatus.fromName(statusStr.toUpperCase());
    } catch (e) {
      try {
        newStatus = GianttStatus.fromSymbol(statusStr);
      } catch (e) {
        stderr.writeln('Error: Invalid status "$statusStr"');
        stderr.writeln('Valid statuses: NOT_STARTED, IN_PROGRESS, BLOCKED, COMPLETED');
        exit(1);
      }
    }

    // Update the item
    final updatedItem = item.copyWith(status: newStatus);
    graph.addItem(updatedItem);

    // Save graph
    FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);

    print('Set status of "${item.id}" to ${newStatus.name}');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
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
    graph.topologicalSort();
    
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
  if (args.rest.length < 4) {
    stderr.writeln('Error: Please provide new_id, title, before_id, and after_id');
    stderr.writeln('Usage: giantt insert <new_id> <title> <before_id> <after_id> [options]');
    exit(1);
  }

  final newId = args.rest[0];
  final title = args.rest[1];
  final beforeId = args.rest[2];
  final afterId = args.rest[3];
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final duration = args['duration'] as String;
  final priority = args['priority'] as String;
  final charts = args['charts'] as String?;
  final tags = args['tags'] as String?;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph
    final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);

    // Check if new ID already exists
    if (graph.items.containsKey(newId)) {
      stderr.writeln('Error: Item with ID "$newId" already exists');
      exit(1);
    }

    // Check if before and after items exist
    if (!graph.items.containsKey(beforeId)) {
      stderr.writeln('Error: Before item "$beforeId" not found');
      exit(1);
    }
    if (!graph.items.containsKey(afterId)) {
      stderr.writeln('Error: After item "$afterId" not found');
      exit(1);
    }

    // Parse duration and priority
    final parsedDuration = GianttDuration.parse(duration);
    final parsedPriority = GianttPriority.fromName(priority);

    // Parse charts and tags
    final chartList = charts?.split(',').map((c) => c.trim()).toList() ?? <String>[];
    final tagList = tags?.split(',').map((t) => t.trim()).toList() ?? <String>[];

    // Create the new item with relations to insert it between the two existing items
    // The new item REQUIRES the afterId (it depends on after)
    // The beforeId should REQUIRE the new item (before depends on new)
    final newItem = GianttItem(
      id: newId,
      title: title,
      status: GianttStatus.notStarted,
      priority: parsedPriority,
      duration: parsedDuration,
      charts: chartList,
      tags: tagList,
      relations: {'REQUIRES': [afterId]},
    );

    // Add the new item
    graph.addItem(newItem);

    // Update beforeId to require newId instead of afterId
    final beforeItem = graph.items[beforeId]!;
    final beforeRelations = Map<String, List<String>>.from(beforeItem.relations);

    // Remove afterId from REQUIRES and add newId
    if (beforeRelations.containsKey('REQUIRES')) {
      beforeRelations['REQUIRES'] = beforeRelations['REQUIRES']!
          .where((r) => r != afterId)
          .toList();
      beforeRelations['REQUIRES']!.add(newId);
    } else {
      beforeRelations['REQUIRES'] = [newId];
    }

    final updatedBeforeItem = beforeItem.copyWith(relations: beforeRelations);
    graph.addItem(updatedBeforeItem);

    // Check for cycles
    try {
      graph.topologicalSort();
    } catch (e) {
      stderr.writeln('Error: Insert would create a cycle in dependencies');
      exit(1);
    }

    // Save graph
    FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);

    print('Successfully inserted "$newId" between "$beforeId" and "$afterId"');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
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
  final skipConfirm = args['yes'] as bool;
  final keepStr = args['keep'] as String;
  final keep = int.tryParse(keepStr) ?? 3;

  try {
    // Find giantt directory
    final gianttDir = _findGianttDirectory();
    if (gianttDir == null) {
      stderr.writeln('Error: No Giantt workspace found. Run "giantt init" first.');
      exit(1);
    }

    // Find all backup files
    final backupFiles = <File>[];
    final includeDir = Directory('$gianttDir/include');
    final occludeDir = Directory('$gianttDir/occlude');

    for (final dir in [includeDir, occludeDir]) {
      if (dir.existsSync()) {
        for (final entity in dir.listSync()) {
          if (entity is File && entity.path.endsWith('.backup')) {
            backupFiles.add(entity);
          }
        }
      }
    }

    if (backupFiles.isEmpty) {
      print('No backup files found.');
      return;
    }

    // Group backups by base file
    final backupsByFile = <String, List<File>>{};
    for (final backup in backupFiles) {
      final name = backup.path;
      // Extract base filename (remove .N.backup suffix)
      final match = RegExp(r'^(.+)\.\d+\.backup$').firstMatch(name);
      if (match != null) {
        final baseFile = match.group(1)!;
        backupsByFile.putIfAbsent(baseFile, () => []).add(backup);
      }
    }

    // Calculate what would be removed
    int totalToRemove = 0;
    final filesToRemove = <File>[];

    for (final entry in backupsByFile.entries) {
      final backups = entry.value;
      // Sort by backup number (newest first)
      backups.sort((a, b) {
        final aNum = int.tryParse(RegExp(r'\.(\d+)\.backup$').firstMatch(a.path)?.group(1) ?? '0') ?? 0;
        final bNum = int.tryParse(RegExp(r'\.(\d+)\.backup$').firstMatch(b.path)?.group(1) ?? '0') ?? 0;
        return bNum.compareTo(aNum);
      });

      // Mark files beyond the retention count for removal
      if (backups.length > keep) {
        final toRemove = backups.sublist(keep);
        filesToRemove.addAll(toRemove);
        totalToRemove += toRemove.length;
      }
    }

    if (filesToRemove.isEmpty) {
      print('No backup files need to be removed (keeping $keep most recent).');
      return;
    }

    print('Found ${backupFiles.length} backup files, $totalToRemove would be removed (keeping $keep most recent).');

    if (!skipConfirm) {
      stdout.write('Do you want to proceed? [y/N]: ');
      final input = stdin.readLineSync()?.trim().toLowerCase() ?? '';
      if (input != 'y' && input != 'yes') {
        print('Aborted.');
        return;
      }
    }

    // Remove the files
    for (final file in filesToRemove) {
      file.deleteSync();
    }

    print('Removed $totalToRemove backup files.');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeAddInclude(ArgResults args) async {
  if (args.rest.isEmpty) {
    stderr.writeln('Error: Please provide include path');
    stderr.writeln('Usage: giantt add-include <include_path> [--file items.txt]');
    exit(1);
  }

  final includePath = args.rest[0];
  final file = args['file'] as String?;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');

    // Check if file exists
    final itemsFile = File(itemsPath);
    if (!itemsFile.existsSync()) {
      stderr.writeln('Error: File not found: $itemsPath');
      exit(1);
    }

    // Read the current file content
    final lines = itemsFile.readAsLinesSync();

    // Find where to insert the include directive
    // Include directives should be at the top of the file, after the banner
    int insertPos = 0;
    for (int i = 0; i < lines.length; i++) {
      final line = lines[i].trim();
      if (line.startsWith('#include ')) {
        insertPos = i + 1;
      } else if (line.isNotEmpty && !line.startsWith('#')) {
        // Found first non-comment, non-empty line - insert before this
        break;
      }
    }

    // Create a backup first
    final backupPath = _incrementBackupName(itemsPath);
    itemsFile.copySync(backupPath);

    // Insert the include directive
    lines.insert(insertPos, '#include $includePath');

    // Write the updated content
    itemsFile.writeAsStringSync(lines.join('\n'));

    print('Added include directive for "$includePath" to $itemsPath');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

String _incrementBackupName(String filepath) {
  int backupNum = 1;
  while (true) {
    final backupPath = '$filepath.$backupNum.backup';
    if (!File(backupPath).existsSync()) {
      return backupPath;
    }
    backupNum++;
  }
}

String? _findGianttDirectory() {
  // Check for local .giantt directory
  final localDir = Directory('.giantt');
  if (localDir.existsSync()) {
    return localDir.path;
  }

  // Check home directory
  final homeDir = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  if (homeDir != null) {
    final homeGianttDir = Directory('$homeDir/.giantt');
    if (homeGianttDir.existsSync()) {
      return homeGianttDir.path;
    }
  }

  return null;
}

Future<void> _executeLog(ArgResults args) async {
  if (args.rest.length < 2) {
    stderr.writeln('Error: Please provide session and message');
    stderr.writeln('Usage: giantt log <session> <message> [--tags tag1,tag2]');
    exit(1);
  }

  final session = args.rest[0];
  final message = args.rest[1];
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final tagsStr = args['tags'] as String?;

  try {
    final logsPath = file ?? _getDefaultGianttPath('logs.jsonl');
    final occludeLogsPath = occludeFile ?? _getDefaultGianttPath('logs.jsonl', occlude: true);

    // Load logs
    final logs = FileRepository.loadLogs(logsPath, occludeLogsPath);

    // Parse additional tags
    final additionalTags = tagsStr?.split(',').map((t) => t.trim()).toList();

    // Create the log entry
    logs.createEntry(session, message, additionalTags: additionalTags);

    // Save logs
    FileRepository.saveLogs(logsPath, occludeLogsPath, logs);

    print('Log entry created with session tag "$session"');
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
}

Future<void> _executeOcclude(ArgResults args) async {
  if (args.command == null) {
    stderr.writeln('Usage: giantt occlude [items|logs] [options]');
    stderr.writeln('');
    stderr.writeln('Subcommands:');
    stderr.writeln('  items   Occlude items by ID or tag');
    stderr.writeln('  logs    Occlude logs by session or tag');
    exit(1);
  }

  final subcommand = args.command!;
  final dryRun = subcommand['dry-run'] as bool;
  final tags = (subcommand['tag'] as List<String>?) ?? [];

  try {
    if (subcommand.name == 'items') {
      final file = subcommand['file'] as String?;
      final occludeFile = subcommand['occlude-file'] as String?;
      final itemIds = subcommand.rest;

      final itemsPath = file ?? _getDefaultGianttPath('items.txt');
      final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

      // Load graph
      final graph = FileRepository.loadGraph(itemsPath, occludeItemsPath);

      // Find items to occlude
      final itemsToOcclude = <GianttItem>[];

      if (itemIds.isNotEmpty) {
        // Occlude by ID
        for (final itemId in itemIds) {
          final item = graph.items[itemId];
          if (item != null && !item.occlude) {
            itemsToOcclude.add(item);
          } else if (item == null) {
            stderr.writeln('Warning: Item "$itemId" not found');
          }
        }
      }

      if (tags.isNotEmpty) {
        // Occlude by tag
        for (final item in graph.items.values) {
          if (!item.occlude && item.tags.any((t) => tags.contains(t))) {
            if (!itemsToOcclude.contains(item)) {
              itemsToOcclude.add(item);
            }
          }
        }
      }

      if (itemsToOcclude.isEmpty) {
        print('No items to occlude.');
        return;
      }

      final action = dryRun ? 'Would occlude' : 'Occluded';
      print('$action ${itemsToOcclude.length} items:');
      for (final item in itemsToOcclude) {
        print('  - ${item.id}: ${item.title}');
        if (!dryRun) {
          final occludedItem = item.setOcclude(true);
          graph.addItem(occludedItem);
        }
      }

      if (!dryRun) {
        FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
      }

    } else if (subcommand.name == 'logs') {
      final file = subcommand['file'] as String?;
      final occludeFile = subcommand['occlude-file'] as String?;
      final sessionIdentifiers = subcommand.rest;

      final logsPath = file ?? _getDefaultGianttPath('logs.jsonl');
      final occludeLogsPath = occludeFile ?? _getDefaultGianttPath('logs.jsonl', occlude: true);

      // Load logs
      final logs = FileRepository.loadLogs(logsPath, occludeLogsPath);

      // Find logs to occlude
      final logsToOcclude = <LogEntry>[];

      if (sessionIdentifiers.isNotEmpty) {
        // Occlude by session
        for (final session in sessionIdentifiers) {
          for (final entry in logs.entries) {
            if (!entry.occlude && entry.session == session) {
              logsToOcclude.add(entry);
            }
          }
        }
      }

      if (tags.isNotEmpty) {
        // Occlude by tag
        for (final entry in logs.entries) {
          if (!entry.occlude && entry.tags.any((t) => tags.contains(t))) {
            if (!logsToOcclude.contains(entry)) {
              logsToOcclude.add(entry);
            }
          }
        }
      }

      if (logsToOcclude.isEmpty) {
        print('No logs to occlude.');
        return;
      }

      final action = dryRun ? 'Would occlude' : 'Occluded';
      print('$action ${logsToOcclude.length} log entries:');
      final sessions = logsToOcclude.map((e) => e.session).toSet();
      print('  Sessions: ${sessions.join(", ")}');

      if (!dryRun) {
        for (final entry in logsToOcclude) {
          final occludedEntry = entry.setOcclude(true);
          logs.replaceEntry(entry, occludedEntry);
        }
        FileRepository.saveLogs(logsPath, occludeLogsPath, logs);
      }
    }
  } catch (e) {
    stderr.writeln('Error: $e');
    exit(1);
  }
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
      // Require a subcommand, just like Python
      stderr.writeln('Usage: giantt doctor [OPTIONS] COMMAND [ARGS]...');
      stderr.writeln("Try 'giantt doctor --help' for help.");
      stderr.writeln('');
      stderr.writeln('Error: Missing command.');
      exit(2);
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
