#!/usr/bin/env dart

import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;
import 'package:args/args.dart';
import 'package:giantt_core/giantt_core.dart';

// ---------------------------------------------------------------------------
// Flow system state (initialised once at startup)
// ---------------------------------------------------------------------------

bool _flowAvailable = false;
bool _syncEnabled = false;

/// Load the device UUID from ~/.soradyne/device_identity.json.
///
/// Returns null if the file doesn't exist or can't be parsed.
String? _loadDeviceId() {
  final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  if (home == null) return null;
  final identityFile = File('$home/.soradyne/device_identity.json');
  if (!identityFile.existsSync()) return null;
  try {
    final json = jsonDecode(identityFile.readAsStringSync());
    return json['device_id'] as String?;
  } catch (_) {
    return null;
  }
}

/// Try to wire up the Soradyne FFI flow system.
///
/// Silently falls back to file-based storage if the native library is not
/// found, so the CLI still works without a Rust build.
void _tryInitFlow() {
  if (Platform.environment['GIANTT_NO_FLOW'] == '1') return;
  final deviceId = _loadDeviceId();
  if (deviceId == null) {
    stderr.writeln(
        '[soradyne] No device identity found — using file-based storage.\n'
        '           Run: soradyne-cli device-id  (creates identity on first run)');
    return;
  }
  _flowAvailable = FlowRepository.initializeIfAvailable(deviceId);
  if (!_flowAvailable) {
    stderr.writeln(
        '[soradyne] Native library unavailable — using file-based storage.\n'
        '           Run: cargo build --release --no-default-features\n'
        '           and ensure libsoradyne is on the library search path.');
  }
}

/// Path to the giantt flow config file: ~/.config/giantt/flows.json
String _gianttConfigPath() {
  final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'] ?? '.';
  return '$home/.config/giantt/flows.json';
}

/// Read all configured flow UUIDs from ~/.config/giantt/flows.json.
///
/// Returns an empty list if the file doesn't exist, the flow system is
/// unavailable, or the file cannot be parsed.
List<String> _getFlowIds() {
  if (!_flowAvailable) return [];
  try {
    final file = File(_gianttConfigPath());
    if (!file.existsSync()) return [];
    final list = jsonDecode(file.readAsStringSync()) as List<dynamic>;
    return list.map((e) => e.toString()).toList();
  } catch (e) {
    stderr.writeln('[soradyne] Could not read flow config: $e');
    return [];
  }
}

/// Return the primary (first) flow UUID, or null if none are configured.
String? _getFlowId() {
  final ids = _getFlowIds();
  return ids.isEmpty ? null : ids.first;
}

/// Enable peer-to-peer sync for all configured flows.
void _tryEnableSync() {
  if (!_flowAvailable) return;
  final flowIds = _getFlowIds();
  if (flowIds.isEmpty) return;
  final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  for (final flowId in flowIds) {
    try {
      FlowRepository.enableSync(flowId, dataDir: home != null ? '$home/.soradyne' : null);
    } catch (e) {
      stderr.writeln('[soradyne] Sync not available for $flowId: $e');
    }
  }
  _syncEnabled = true;
}

/// Generate a random UUID v4.
String _generateUuid() {
  final rng = math.Random.secure();
  final bytes = List<int>.generate(16, (_) => rng.nextInt(256));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  final hex = bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
      '${hex.substring(12, 16)}-${hex.substring(16, 20)}-'
      '${hex.substring(20, 32)}';
}

void main(List<String> arguments) async {
  _tryInitFlow();
  _tryEnableSync();
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
  parser.addCommand('snapshot', _createSnapshotCommand());
  parser.addCommand('watch', _createWatchCommand());
  parser.addCommand('summary', _createSummaryCommand());
  parser.addCommand('load', _createLoadCommand());
  parser.addCommand('deps', _createDepsCommand());
  parser.addCommand('blocked', _createBlockedCommand());

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

    // If sync is enabled, wait for the ensemble to connect and deliver
    // any pending broadcasts. The TCP static peer connector and horizon
    // exchange run asynchronously on the bridge runtime.
    if (_syncEnabled) {
      await Future.delayed(Duration(seconds: 2));
    }
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
  print('Query commands (supports --json):');
  print('  summary     Per-chart project overview');
  print('  load        Temporal load analysis in a date window');
  print('  deps        Dependency chain for an item');
  print('  blocked     All currently blocked items');
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
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('duration', defaultsTo: '1d', help: 'Duration (e.g., 1d, 2w, 3mo)')
    ..addOption('priority', defaultsTo: 'NEUTRAL', help: 'Priority level')
    ..addOption('charts', help: 'Comma-separated list of chart names')
    ..addOption('tags', help: 'Comma-separated list of tags')
    ..addOption('status', defaultsTo: 'NOT_STARTED', help: 'Initial status')
    ..addOption('requires', help: 'Comma-separated list of item IDs that this item requires')
    ..addOption('any-of', help: 'Comma-separated list of item IDs that are individually sufficient')
    ..addOption('constraints', help: 'Space-separated time constraints (e.g., "due(2024-12-31,warn) window(5d,severe)")');
}

ArgParser _createShowCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
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
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addFlag('add', help: 'Add a value (for collection properties)', negatable: false)
    ..addFlag('remove', help: 'Remove a value (for collection properties)', negatable: false)
    ..addFlag('clear', help: 'Clear all values (for collection properties)', negatable: false)
    ..addOption('add-constraints', help: 'Add time constraints (space-separated, e.g. "due(2026-04-30,severe)")')
    ..addOption('remove-constraints', help: 'Remove time constraints (space-separated)')
    ..addOption('set-constraints', help: 'Replace all time constraints (space-separated)')
    ..addFlag('clear-constraints', help: 'Remove all time constraints', negatable: false)
    ..addOption('title', help: 'Set item title')
    ..addOption('status', help: 'Set item status')
    ..addOption('priority', help: 'Set item priority')
    ..addOption('duration', help: 'Set item duration')
    ..addOption('add-charts', help: 'Add charts (comma-separated)')
    ..addOption('remove-charts', help: 'Remove charts (comma-separated)')
    ..addOption('add-tags', help: 'Add tags (comma-separated)')
    ..addOption('remove-tags', help: 'Remove tags (comma-separated)')
    ..addOption('add-requires', help: 'Add REQUIRES relations (comma-separated)')
    ..addOption('remove-requires', help: 'Remove REQUIRES relations (comma-separated)')
    ..addOption('add-blocks', help: 'Add BLOCKS relations (comma-separated)')
    ..addOption('remove-blocks', help: 'Remove BLOCKS relations (comma-separated)')
    ..addOption('comment', help: 'Set user comment');
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
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
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
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('charts', help: 'Comma-separated list of charts')
    ..addOption('tags', help: 'Comma-separated list of tags')
    ..addOption('duration', defaultsTo: '1d', help: 'Duration (e.g., 1d, 2w, 3mo2w5d3s)')
    ..addOption('priority', defaultsTo: 'NEUTRAL', help: 'Priority level')
    ..addOption('constraints', help: 'Space-separated time constraints (e.g., "due(2024-12-31,warn) window(5d,severe)")');
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
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
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
    case 'snapshot':
      _executeSnapshot(command);
      break;
    case 'watch':
      await _executeWatch(command);
      break;
    case 'summary':
      _executeSummary(command);
      break;
    case 'load':
      _executeLoad(command);
      break;
    case 'deps':
      _executeDeps(command);
      break;
    case 'blocked':
      _executeBlocked(command);
      break;
    default:
      throw ArgumentError('Unknown command: ${command.name}');
  }
}

Future<void> _executeInit(ArgResults args) async {
  try {
    final configFile = File(_gianttConfigPath());
    if (configFile.existsSync()) {
      final ids = _getFlowIds();
      print('Giantt already configured at ${configFile.path}');
      if (ids.isNotEmpty) print('Flows: ${ids.join(', ')}');
      return;
    }

    configFile.parent.createSync(recursive: true);
    final uuid = _generateUuid();
    configFile.writeAsStringSync(jsonEncode([uuid]));

    // Touch the flow so soradyne initialises its storage directory.
    if (_flowAvailable) {
      try { FlowRepository.loadGraph(uuid); } catch (_) {}
    }

    print('Initialized Giantt.');
    print('Flow UUID: $uuid');
    print('Config:    ${configFile.path}');
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
  final constraints = args['constraints'] as String?;
  final jsonOutput = args['json'] as bool;
  
  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
    
    // Load existing graph (prefer flow for up-to-date state)
    final flowId = _getFlowId();
    final graph = flowId != null
        ? FlowRepository.loadGraph(flowId)
        : FileRepository.loadGraph(itemsPath, occludeItemsPath);

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

    // Parse constraints
    final timeConstraints = <TimeConstraint>[];
    if (constraints != null) {
      for (final cs in constraints.split(' ')) {
        if (cs.trim().isNotEmpty) {
          final tc = TimeConstraint.parse(cs.trim());
          if (tc != null) timeConstraints.add(tc);
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
      timeConstraints: timeConstraints,
    );

    // Add to graph (for cycle check)
    graph.addItem(newItem);

    // Check for cycles
    try {
      graph.topologicalSort();
    } catch (e) {
      stderr.writeln('Error: Adding this item would create a cycle in dependencies');
      exit(1);
    }

    // Persist: prefer Flow CRDT; fall back to file
    if (flowId != null) {
      final ops = GianttOp.fromItem(newItem);
      FlowRepository.saveOperations(flowId, ops);
    } else {
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
    }

    if (jsonOutput) {
      print(jsonEncode({'ok': true, 'command': 'add', 'data': _itemToJson(newItem)}));
    } else {
      print('Successfully added item "$id"');
    }
  } catch (e) {
    if (args['json'] as bool? ?? false) {
      print(jsonEncode({'ok': false, 'command': 'add', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
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
  final jsonOutput = args['json'] as bool;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph — prefer Flow CRDT so synced state is visible; fall back to file
    final flowId = _getFlowId();
    final graph = flowId != null
        ? FlowRepository.loadGraph(flowId)
        : FileRepository.loadGraph(itemsPath, occludeItemsPath);

    if (jsonOutput && !searchChart && !searchLog) {
      // JSON output for item lookup.
      GianttItem item;
      if (graph.items.containsKey(substring)) {
        item = graph.items[substring]!;
      } else {
        item = graph.findBySubstring(substring);
      }
      print(jsonEncode({'ok': true, 'command': 'show', 'data': {'items': [_itemToJson(item)], 'count': 1}}));
      return;
    }

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
    if (args['json'] as bool? ?? false) {
      print(jsonEncode({'ok': false, 'command': 'show', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
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
  final clearMode = args['clear'] as bool;

  // Check for named-option style: giantt modify <id> --add-constraints="..."
  final hasNamedOptions = _hasNamedModifyOptions(args);

  if (!hasNamedOptions && (args.rest.length < 2 || (!clearMode && args.rest.length < 3))) {
    stderr.writeln('Error: Please provide item ID, property, and value');
    stderr.writeln('Usage: giantt modify <id> <property> <value> [--add|--remove|--clear]');
    stderr.writeln('   or: giantt modify <id> --add-constraints="due(2026-04-30,severe)"');
    exit(1);
  }

  final itemId = args.rest[0];
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final jsonOutput = args['json'] as bool;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph — prefer Flow CRDT; fall back to file
    final flowId = _getFlowId();
    final graph = flowId != null
        ? FlowRepository.loadGraph(flowId)
        : FileRepository.loadGraph(itemsPath, occludeItemsPath);

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

    // Collect all modifications to apply (supports multiple named options at once)
    final modifications = <_ModifyAction>[];

    if (hasNamedOptions) {
      // Named-option style: --add-constraints, --title, etc.
      modifications.addAll(_collectNamedModifications(args, item));
    } else {
      // Positional style: modify <id> <property> <value> [--add|--remove|--clear]
      final property = args.rest[1];
      final value = args.rest.length >= 3 ? args.rest[2] : '';
      final addMode = args['add'] as bool;
      final removeMode = args['remove'] as bool;
      modifications.add(_ModifyAction(property, value, addMode, removeMode, clearMode));
    }

    if (modifications.isEmpty) {
      stderr.writeln('Error: No modifications specified');
      exit(1);
    }

    // Apply all modifications sequentially
    var modifiedItem = item;
    final modifiedProperties = <String>[];
    for (final mod in modifications) {
      modifiedItem = _applyModification(modifiedItem, mod);
      modifiedProperties.add(mod.property);
    }

    // Update in graph (for cycle check)
    graph.addItem(modifiedItem);

    // Check for cycles if relations were modified
    if (modifiedProperties.any((p) => ['requires', 'anyof', 'blocks'].contains(p.toLowerCase()))) {
      try {
        graph.topologicalSort();
      } catch (e) {
        stderr.writeln('Error: Modification would create a cycle in dependencies');
        exit(1);
      }
    }

    // Build CRDT ops from the diff between old item and modified item
    if (flowId != null) {
      final allOps = <GianttOp>[];
      for (final mod in modifications) {
        allOps.addAll(_buildModifyOps(item, modifiedItem, mod.property, mod.addMode, mod.removeMode));
      }
      FlowRepository.saveOperations(flowId, allOps);
    } else {
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
    }

    final propertyDesc = modifiedProperties.join(', ');
    if (jsonOutput) {
      print(jsonEncode({
        'ok': true,
        'command': 'modify',
        'data': {
          'id': modifiedItem.id,
          'property': propertyDesc,
          'item': _itemToJson(modifiedItem),
        }
      }));
    } else {
      print('Successfully modified "$propertyDesc" for item "${item.id}"');
    }
  } catch (e) {
    if (args['json'] as bool? ?? false) {
      print(jsonEncode({'ok': false, 'command': 'modify', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
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

    // Load graph — prefer Flow CRDT; fall back to file
    final flowId = _getFlowId();
    final graph = flowId != null
        ? FlowRepository.loadGraph(flowId)
        : FileRepository.loadGraph(itemsPath, occludeItemsPath);

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

    // Collect relation cleanup ops BEFORE mutating the graph
    final relCleanupOps = <GianttOp>[];
    if (!keepRelations) {
      for (final item in graph.items.values) {
        if (item.id == itemId) continue;
        for (final entry in item.relations.entries) {
          if (entry.value.contains(itemId)) {
            relCleanupOps.add(
              GianttOp.removeFromSet(item.id, entry.key.toLowerCase(), itemId, []),
            );
          }
        }
      }
    }

    // Remove the item from the local graph and clean up relations for file-based path
    graph.removeItem(itemId);
    if (!keepRelations) {
      for (final item in graph.items.values) {
        bool modified = false;
        final newRelations = <String, List<String>>{};
        for (final entry in item.relations.entries) {
          final cleanedTargets = entry.value.where((t) => t != itemId).toList();
          if (cleanedTargets.length != entry.value.length) modified = true;
          if (cleanedTargets.isNotEmpty) newRelations[entry.key] = cleanedTargets;
        }
        if (modified) graph.addItem(item.copyWith(relations: newRelations));
      }
    }

    // Persist: prefer Flow CRDT; fall back to file
    if (flowId != null) {
      FlowRepository.saveOperation(flowId, GianttOp.removeItem(itemId));
      for (final op in relCleanupOps) {
        FlowRepository.saveOperation(flowId, op);
      }
    } else {
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
    }

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
  final jsonOutput = args['json'] as bool;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph — prefer Flow CRDT; fall back to file
    final flowId = _getFlowId();
    final graph = flowId != null
        ? FlowRepository.loadGraph(flowId)
        : FileRepository.loadGraph(itemsPath, occludeItemsPath);

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

    // Persist: prefer Flow CRDT; fall back to file
    if (flowId != null) {
      FlowRepository.saveOperation(
          flowId, GianttOp.setStatus(item.id, newStatus));
    } else {
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
    }

    if (jsonOutput) {
      print(jsonEncode({
        'ok': true,
        'command': 'set-status',
        'data': {
          'id': item.id,
          'previous_status': item.status.name,
          'new_status': newStatus.name,
        }
      }));
    } else {
      print('Set status of "${item.id}" to ${newStatus.name}');
    }
  } catch (e) {
    if (args['json'] as bool? ?? false) {
      print(jsonEncode({'ok': false, 'command': 'set-status', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
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
  final constraints = args['constraints'] as String?;
  final jsonOutput = args['json'] as bool;

  try {
    final itemsPath = file ?? _getDefaultGianttPath('items.txt');
    final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

    // Load graph (prefer flow for up-to-date state)
    final flowId = _getFlowId();
    final graph = flowId != null
        ? FlowRepository.loadGraph(flowId)
        : FileRepository.loadGraph(itemsPath, occludeItemsPath);

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

    // Parse constraints
    final timeConstraints = <TimeConstraint>[];
    if (constraints != null) {
      for (final cs in constraints.split(' ')) {
        if (cs.trim().isNotEmpty) {
          final tc = TimeConstraint.parse(cs.trim());
          if (tc != null) timeConstraints.add(tc);
        }
      }
    }

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
      timeConstraints: timeConstraints,
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

    // Persist: prefer Flow CRDT; fall back to file
    if (flowId != null) {
      final ops = <GianttOp>[];
      // Add the new item
      ops.addAll(GianttOp.fromItem(newItem));
      // Update beforeId's relations: remove old REQUIRES afterId, add REQUIRES newId
      ops.add(GianttOp.removeFromSet(beforeId, 'requires', afterId, []));
      ops.add(GianttOp.addToSet(beforeId, 'requires', newId));
      FlowRepository.saveOperations(flowId, ops);
    } else {
      FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
    }

    if (jsonOutput) {
      print(jsonEncode({'ok': true, 'command': 'insert', 'data': _itemToJson(newItem)}));
    } else {
      print('Successfully inserted "$newId" between "$beforeId" and "$afterId"');
    }
  } catch (e) {
    if (args['json'] as bool? ?? false) {
      print(jsonEncode({'ok': false, 'command': 'insert', 'error': e.toString()}));
    } else {
      stderr.writeln('Error: $e');
    }
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
  final jsonOutput = subcommand.options.contains('json') && (subcommand['json'] as bool? ?? false);

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
        if (jsonOutput) {
          print(jsonEncode({'ok': true, 'command': 'occlude', 'data': {'occluded_items': [], 'occluded_count': 0, 'dry_run': dryRun}}));
        } else {
          print('No items to occlude.');
        }
        return;
      }

      for (final item in itemsToOcclude) {
        if (!dryRun) {
          final occludedItem = item.setOcclude(true);
          graph.addItem(occludedItem);
        }
      }

      if (!dryRun) {
        FileRepository.saveGraph(itemsPath, occludeItemsPath, graph);
      }

      if (jsonOutput) {
        print(jsonEncode({
          'ok': true,
          'command': 'occlude',
          'data': {
            'occluded_items': itemsToOcclude.map((i) => i.id).toList(),
            'occluded_count': itemsToOcclude.length,
            'dry_run': dryRun,
          }
        }));
      } else {
        final action = dryRun ? 'Would occlude' : 'Occluded';
        print('$action ${itemsToOcclude.length} items:');
        for (final item in itemsToOcclude) {
          print('  - ${item.id}: ${item.title}');
        }
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
    
    // Load graph — use flow if available, like all other commands
    final graph = (_flowAvailable && file == null)
        ? FlowRepository.loadGraph(_getFlowId()!)
        : FileRepository.loadGraph(itemsPath, occludeItemsPath);
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
    print('✓ Graph is healthy — no issues found.');
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

/// A single property modification to apply.
class _ModifyAction {
  final String property;
  final String value;
  final bool addMode;
  final bool removeMode;
  final bool clearMode;
  const _ModifyAction(this.property, this.value, this.addMode, this.removeMode, this.clearMode);
}

/// Check whether any named modify options (--add-constraints, --title, etc.) are set.
bool _hasNamedModifyOptions(ArgResults args) {
  const namedOptions = [
    'add-constraints', 'remove-constraints', 'set-constraints', 'clear-constraints',
    'title', 'status', 'priority', 'duration', 'comment',
    'add-charts', 'remove-charts', 'add-tags', 'remove-tags',
    'add-requires', 'remove-requires', 'add-blocks', 'remove-blocks',
  ];
  for (final opt in namedOptions) {
    if (args.wasParsed(opt)) return true;
  }
  return false;
}

/// Collect modifications from named options into a list of actions.
List<_ModifyAction> _collectNamedModifications(ArgResults args, GianttItem item) {
  final mods = <_ModifyAction>[];

  // Simple property setters
  if (args.wasParsed('title')) {
    mods.add(_ModifyAction('title', args['title'] as String, false, false, false));
  }
  if (args.wasParsed('status')) {
    mods.add(_ModifyAction('status', args['status'] as String, false, false, false));
  }
  if (args.wasParsed('priority')) {
    mods.add(_ModifyAction('priority', args['priority'] as String, false, false, false));
  }
  if (args.wasParsed('duration')) {
    mods.add(_ModifyAction('duration', args['duration'] as String, false, false, false));
  }
  if (args.wasParsed('comment')) {
    mods.add(_ModifyAction('comment', args['comment'] as String, false, false, false));
  }

  // Collection modifiers: charts
  if (args.wasParsed('add-charts')) {
    mods.add(_ModifyAction('charts', args['add-charts'] as String, true, false, false));
  }
  if (args.wasParsed('remove-charts')) {
    mods.add(_ModifyAction('charts', args['remove-charts'] as String, false, true, false));
  }

  // Collection modifiers: tags
  if (args.wasParsed('add-tags')) {
    mods.add(_ModifyAction('tags', args['add-tags'] as String, true, false, false));
  }
  if (args.wasParsed('remove-tags')) {
    mods.add(_ModifyAction('tags', args['remove-tags'] as String, false, true, false));
  }

  // Collection modifiers: relations
  if (args.wasParsed('add-requires')) {
    mods.add(_ModifyAction('requires', args['add-requires'] as String, true, false, false));
  }
  if (args.wasParsed('remove-requires')) {
    mods.add(_ModifyAction('requires', args['remove-requires'] as String, false, true, false));
  }
  if (args.wasParsed('add-blocks')) {
    mods.add(_ModifyAction('blocks', args['add-blocks'] as String, true, false, false));
  }
  if (args.wasParsed('remove-blocks')) {
    mods.add(_ModifyAction('blocks', args['remove-blocks'] as String, false, true, false));
  }

  // Constraints
  if (args.wasParsed('clear-constraints') && args['clear-constraints'] as bool) {
    mods.add(_ModifyAction('constraints', '', false, false, true));
  }
  if (args.wasParsed('set-constraints')) {
    mods.add(_ModifyAction('constraints', args['set-constraints'] as String, false, false, false));
  }
  if (args.wasParsed('add-constraints')) {
    mods.add(_ModifyAction('constraints', args['add-constraints'] as String, true, false, false));
  }
  if (args.wasParsed('remove-constraints')) {
    mods.add(_ModifyAction('constraints', args['remove-constraints'] as String, false, true, false));
  }

  return mods;
}

/// Apply a single modification action to an item and return the modified item.
GianttItem _applyModification(GianttItem item, _ModifyAction mod) {
  final value = mod.value;
  switch (mod.property.toLowerCase()) {
    case 'title':
      return item.copyWith(title: value);
    case 'status':
      return item.copyWith(status: GianttStatus.fromName(value.toUpperCase()));
    case 'priority':
      return item.copyWith(priority: GianttPriority.fromName(value.toUpperCase()));
    case 'duration':
      return item.copyWith(duration: GianttDuration.parse(value));
    case 'charts':
      final chartsList = value.split(',').map((c) => c.trim()).toList();
      if (mod.addMode) {
        return item.copyWith(charts: [...item.charts, ...chartsList]);
      } else if (mod.removeMode) {
        return item.copyWith(charts: item.charts.where((c) => !chartsList.contains(c)).toList());
      } else {
        return item.copyWith(charts: chartsList);
      }
    case 'tags':
      final tagsList = value.split(',').map((t) => t.trim()).toList();
      if (mod.addMode) {
        return item.copyWith(tags: [...item.tags, ...tagsList]);
      } else if (mod.removeMode) {
        return item.copyWith(tags: item.tags.where((t) => !tagsList.contains(t)).toList());
      } else {
        return item.copyWith(tags: tagsList);
      }
    case 'requires':
      final requiresList = value.split(',').map((r) => r.trim()).toList();
      final newRelations = Map<String, List<String>>.from(item.relations);
      if (mod.addMode) {
        newRelations['REQUIRES'] = [...(newRelations['REQUIRES'] ?? []), ...requiresList];
      } else if (mod.removeMode) {
        newRelations['REQUIRES'] = (newRelations['REQUIRES'] ?? []).where((r) => !requiresList.contains(r)).toList();
        if (newRelations['REQUIRES']!.isEmpty) newRelations.remove('REQUIRES');
      } else {
        newRelations['REQUIRES'] = requiresList;
      }
      return item.copyWith(relations: newRelations);
    case 'anyof':
      final anyOfList = value.split(',').map((a) => a.trim()).toList();
      final newRelations = Map<String, List<String>>.from(item.relations);
      if (mod.addMode) {
        newRelations['ANYOF'] = [...(newRelations['ANYOF'] ?? []), ...anyOfList];
      } else if (mod.removeMode) {
        newRelations['ANYOF'] = (newRelations['ANYOF'] ?? []).where((a) => !anyOfList.contains(a)).toList();
        if (newRelations['ANYOF']!.isEmpty) newRelations.remove('ANYOF');
      } else {
        newRelations['ANYOF'] = anyOfList;
      }
      return item.copyWith(relations: newRelations);
    case 'blocks':
      final blocksList = value.split(',').map((b) => b.trim()).toList();
      final newRelations = Map<String, List<String>>.from(item.relations);
      if (mod.addMode) {
        newRelations['BLOCKS'] = [...(newRelations['BLOCKS'] ?? []), ...blocksList];
      } else if (mod.removeMode) {
        newRelations['BLOCKS'] = (newRelations['BLOCKS'] ?? []).where((b) => !blocksList.contains(b)).toList();
        if (newRelations['BLOCKS']!.isEmpty) newRelations.remove('BLOCKS');
      } else {
        newRelations['BLOCKS'] = blocksList;
      }
      return item.copyWith(relations: newRelations);
    case 'comment':
      return item.copyWith(userComment: value);
    case 'constraints':
      final constraintList = value.split(' ').where((s) => s.trim().isNotEmpty).toList();
      if (mod.clearMode) {
        return item.copyWith(timeConstraints: []);
      } else if (mod.addMode) {
        final newConstraints = <TimeConstraint>[...item.timeConstraints];
        for (final cs in constraintList) {
          final tc = TimeConstraint.parse(cs.trim());
          if (tc != null) newConstraints.add(tc);
        }
        return item.copyWith(timeConstraints: newConstraints);
      } else if (mod.removeMode) {
        final toRemove = <TimeConstraint>[];
        for (final cs in constraintList) {
          final tc = TimeConstraint.parse(cs.trim());
          if (tc != null) toRemove.add(tc);
        }
        return item.copyWith(
          timeConstraints: item.timeConstraints.where((tc) => !toRemove.contains(tc)).toList(),
        );
      } else {
        // Replace all constraints
        final newConstraints = <TimeConstraint>[];
        for (final cs in constraintList) {
          final tc = TimeConstraint.parse(cs.trim());
          if (tc != null) newConstraints.add(tc);
        }
        return item.copyWith(timeConstraints: newConstraints);
      }
    default:
      throw ArgumentError('Unknown property "${mod.property}"');
  }
}

/// Build the minimal list of CRDT ops that transforms [oldItem] into
/// [modifiedItem] for the given [property].
List<GianttOp> _buildModifyOps(
  GianttItem oldItem,
  GianttItem modifiedItem,
  String property,
  bool addMode,
  bool removeMode,
) {
  final ops = <GianttOp>[];
  final id = oldItem.id;

  switch (property.toLowerCase()) {
    case 'title':
      ops.add(GianttOp.setTitle(id, modifiedItem.title));
    case 'status':
      ops.add(GianttOp.setStatus(id, modifiedItem.status));
    case 'priority':
      ops.add(GianttOp.setPriority(id, modifiedItem.priority));
    case 'duration':
      ops.add(GianttOp.setDuration(id, modifiedItem.duration));
    case 'comment':
      ops.add(GianttOp.setComment(id, modifiedItem.userComment));
    case 'charts':
      ops.addAll(_setDiffOps(id, 'charts', oldItem.charts.toSet(),
          modifiedItem.charts.toSet(), addMode, removeMode));
    case 'tags':
      ops.addAll(_setDiffOps(id, 'tags', oldItem.tags.toSet(),
          modifiedItem.tags.toSet(), addMode, removeMode));
    case 'requires':
      ops.addAll(_setDiffOps(
          id,
          'requires',
          (oldItem.relations['REQUIRES'] ?? []).toSet(),
          (modifiedItem.relations['REQUIRES'] ?? []).toSet(),
          addMode,
          removeMode));
    case 'anyof':
      ops.addAll(_setDiffOps(
          id,
          'anyof',
          (oldItem.relations['ANYOF'] ?? []).toSet(),
          (modifiedItem.relations['ANYOF'] ?? []).toSet(),
          addMode,
          removeMode));
    case 'blocks':
      ops.addAll(_setDiffOps(
          id,
          'blocks',
          (oldItem.relations['BLOCKS'] ?? []).toSet(),
          (modifiedItem.relations['BLOCKS'] ?? []).toSet(),
          addMode,
          removeMode));
    case 'constraints':
      ops.addAll(_setDiffOps(
          id,
          'timeConstraints',
          oldItem.timeConstraints.map((tc) => tc.toString()).toSet(),
          modifiedItem.timeConstraints.map((tc) => tc.toString()).toSet(),
          addMode,
          removeMode));
  }

  return ops;
}

/// Compute AddToSet / RemoveFromSet ops for a set field.
List<GianttOp> _setDiffOps(
  String itemId,
  String setName,
  Set<String> oldSet,
  Set<String> newSet,
  bool addMode,
  bool removeMode,
) {
  final ops = <GianttOp>[];
  if (addMode) {
    // Add only the new elements
    for (final e in newSet.difference(oldSet)) {
      ops.add(GianttOp.addToSet(itemId, setName, e));
    }
  } else if (removeMode) {
    // Remove only the specified elements
    for (final e in oldSet.difference(newSet)) {
      ops.add(GianttOp.removeFromSet(itemId, setName, e, []));
    }
  } else {
    // Replace: remove elements no longer in the set, add new ones
    for (final e in oldSet.difference(newSet)) {
      ops.add(GianttOp.removeFromSet(itemId, setName, e, []));
    }
    for (final e in newSet.difference(oldSet)) {
      ops.add(GianttOp.addToSet(itemId, setName, e));
    }
  }
  return ops;
}


ArgParser _createSnapshotCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help', negatable: false)
    ..addOption('output', abbr: 'o', help: 'Output file path (default: stdout)');
}

void _executeSnapshot(ArgResults args) {
  if (!_flowAvailable) {
    stderr.writeln('Error: Flow system unavailable — native library not found.');
    exit(1);
  }

  final flowId = _getFlowId();
  if (flowId == null) {
    stderr.writeln('Error: Could not determine flow ID for this workspace.');
    exit(1);
  }

  final outputPath = args['output'] as String?;

  if (outputPath != null) {
    FlowRepository.writeSnapshot(flowId, outputPath);
    print('Snapshot written to $outputPath');
  } else {
    // Print to stdout — lets the user pipe it wherever they want
    final client = FlowClient.open(flowId);
    try {
      print(client.readDrip());
    } finally {
      client.close();
    }
  }
}

ArgParser _createWatchCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help', negatable: false)
    ..addOption('output', abbr: 'o',
        help: 'File to overwrite on each refresh',
        defaultsTo: '/tmp/giantt_watch.txt')
    ..addOption('interval', abbr: 'i',
        help: 'Refresh interval in seconds',
        defaultsTo: '2');
}

Future<void> _executeWatch(ArgResults args) async {
  if (!_flowAvailable) {
    stderr.writeln('Error: Flow system unavailable — native library not found.');
    exit(1);
  }

  final flowId = _getFlowId();
  if (flowId == null) {
    stderr.writeln('Error: Could not determine flow ID for this workspace.');
    exit(1);
  }

  final outputPath = args['output'] as String;
  final intervalSecs = int.tryParse(args['interval'] as String) ?? 2;

  print('Watching flow $flowId');
  print('Writing snapshot to $outputPath every ${intervalSecs}s. Ctrl+C to stop.');

  bool running = true;
  ProcessSignal.sigint.watch().first.then((_) => running = false);

  while (running) {
    try {
      FlowRepository.writeSnapshot(flowId, outputPath);
    } catch (e) {
      stderr.writeln('Warning: snapshot failed: $e');
    }
    await Future.delayed(Duration(seconds: intervalSecs));
  }

  print('\nWatch stopped.');
}

// ---------------------------------------------------------------------------
// JSON helper
// ---------------------------------------------------------------------------

Map<String, dynamic> _itemToJson(GianttItem item) => {
      'id': item.id,
      'title': item.title,
      'status': item.status.name,
      'priority': item.priority.name,
      'duration_seconds': item.duration.totalSeconds,
      'charts': item.charts,
      'tags': item.tags,
      'occlude': item.occlude,
      'relations': item.relations,
      'time_constraints': item.timeConstraints.map((tc) {
        final m = <String, dynamic>{
          'type': tc.type.name,
          'consequence': tc.consequenceType.value,
          'grace_period_seconds':
              tc.gracePeriod?.totalSeconds,
        };
        if (tc.dueDate != null) m['due_date'] = tc.dueDate;
        if (tc.type == TimeConstraintType.window) {
          m['window_seconds'] = tc.duration.totalSeconds;
        }
        if (tc.type == TimeConstraintType.recurring && tc.interval != null) {
          m['interval_seconds'] = tc.interval!.totalSeconds;
          m['stack'] = tc.stack;
        }
        return m;
      }).toList(),
      'user_comment': item.userComment,
      'auto_comment': item.autoComment,
    };

// ---------------------------------------------------------------------------
// Query command ArgParsers
// ---------------------------------------------------------------------------

ArgParser _createSummaryCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('charts', help: 'Include only these charts (comma-separated)')
    ..addOption('exclude-charts', help: 'Exclude these charts (comma-separated)')
    ..addOption('min-priority', help: 'Minimum priority (e.g. MEDIUM, HIGH)')
    ..addOption('today', help: 'Override today date (YYYY-MM-DD)');
}

ArgParser _createLoadCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('charts', help: 'Include only these charts (comma-separated)')
    ..addOption('exclude-charts', help: 'Exclude these charts (comma-separated)')
    ..addOption('min-priority', help: 'Minimum priority (e.g. MEDIUM, HIGH)')
    ..addOption('today', help: 'Override today date (YYYY-MM-DD)');
}

ArgParser _createDepsCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
    ..addFlag('upstream-only', help: 'Show only upstream (prerequisite) dependencies', negatable: false)
    ..addFlag('downstream-only', help: 'Show only downstream (blocked) dependents', negatable: false)
    ..addOption('depth', defaultsTo: '20', help: 'Maximum traversal depth in each direction')
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use');
}

ArgParser _createBlockedCommand() {
  return ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show help for this command', negatable: false)
    ..addFlag('json', abbr: 'j', help: 'Output result as JSON', negatable: false)
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('charts', help: 'Include only these charts (comma-separated)')
    ..addOption('exclude-charts', help: 'Exclude these charts (comma-separated)')
    ..addOption('min-priority', help: 'Minimum priority (e.g. MEDIUM, HIGH)');
}

// ---------------------------------------------------------------------------
// Query command execute functions
// ---------------------------------------------------------------------------

/// Parse a date spec: YYYY-MM-DD, TODAY, TODAY+Nd, TODAY-Nd.
DateTime _parseLoadDate(String spec, DateTime today) {
  final s = spec.trim().toUpperCase();
  if (s == 'TODAY') return DateTime(today.year, today.month, today.day);

  final relMatch = RegExp(r'^TODAY([+-])(\d+)D$').firstMatch(s);
  if (relMatch != null) {
    final sign = relMatch.group(1) == '+' ? 1 : -1;
    final days = int.parse(relMatch.group(2)!);
    final base = DateTime(today.year, today.month, today.day);
    return base.add(Duration(days: sign * days));
  }

  // Bare YYYY-MM-DD
  return DateTime.parse(spec.trim());
}

GianttPriority? _parsePriority(String? s) {
  if (s == null) return null;
  return GianttPriority.fromName(s.trim().toUpperCase());
}

List<String>? _parseCommaList(String? s) {
  if (s == null) return null;
  return s.split(',').map((v) => v.trim()).where((v) => v.isNotEmpty).toList();
}

void _executeSummary(ArgResults args) {
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final itemsPath = file ?? _getDefaultGianttPath('items.txt');
  final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
  final todayStr = args['today'] as String?;
  final today = todayStr != null ? DateTime.parse(todayStr) : null;

  // Prefer flow CRDT (merged across all configured flows); fall back to file
  final flowIds = _getFlowIds();
  final graph = flowIds.isNotEmpty ? FlowRepository.loadMergedGraph(flowIds).graph : null;

  executeSummaryCommand(
    itemsPath: itemsPath,
    occludeItemsPath: occludeItemsPath,
    graph: graph,
    today: today,
    charts: _parseCommaList(args['charts'] as String?),
    excludeCharts: _parseCommaList(args['exclude-charts'] as String?),
    minPriority: _parsePriority(args['min-priority'] as String?),
    jsonOutput: args['json'] as bool,
  );
}

void _executeLoad(ArgResults args) {
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final itemsPath = file ?? _getDefaultGianttPath('items.txt');
  final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
  final todayStr = args['today'] as String?;
  final today = todayStr != null ? DateTime.parse(todayStr) : DateTime.now();
  final todayDate = DateTime(today.year, today.month, today.day);

  // Positional args: [start] [end] — both optional.
  final rest = args.rest;
  final windowStart = rest.isNotEmpty ? _parseLoadDate(rest[0], todayDate) : todayDate;
  final windowEnd = rest.length > 1
      ? _parseLoadDate(rest[1], todayDate)
      : todayDate.add(const Duration(days: 30));

  // Prefer flow CRDT (merged across all configured flows); fall back to file
  final flowIds = _getFlowIds();
  final graph = flowIds.isNotEmpty ? FlowRepository.loadMergedGraph(flowIds).graph : null;

  executeLoadCommand(
    itemsPath: itemsPath,
    occludeItemsPath: occludeItemsPath,
    graph: graph,
    windowStart: windowStart,
    windowEnd: windowEnd,
    today: todayDate,
    charts: _parseCommaList(args['charts'] as String?),
    excludeCharts: _parseCommaList(args['exclude-charts'] as String?),
    minPriority: _parsePriority(args['min-priority'] as String?),
    jsonOutput: args['json'] as bool,
  );
}

void _executeDeps(ArgResults args) {
  if (args.rest.isEmpty) {
    stderr.writeln('Error: Please provide an item ID or title substring');
    stderr.writeln('Usage: giantt deps <id_or_substring> [options]');
    exit(1);
  }
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final itemsPath = file ?? _getDefaultGianttPath('items.txt');
  final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);
  final depth = int.tryParse(args['depth'] as String? ?? '20') ?? 20;

  // Prefer flow CRDT (merged across all configured flows); fall back to file
  final flowIds = _getFlowIds();
  final graph = flowIds.isNotEmpty ? FlowRepository.loadMergedGraph(flowIds).graph : null;

  executeDepsCommand(
    itemsPath: itemsPath,
    occludeItemsPath: occludeItemsPath,
    graph: graph,
    itemId: args.rest.first,
    maxDepth: depth,
    upstreamOnly: args['upstream-only'] as bool,
    downstreamOnly: args['downstream-only'] as bool,
    jsonOutput: args['json'] as bool,
  );
}

void _executeBlocked(ArgResults args) {
  final file = args['file'] as String?;
  final occludeFile = args['occlude-file'] as String?;
  final itemsPath = file ?? _getDefaultGianttPath('items.txt');
  final occludeItemsPath = occludeFile ?? _getDefaultGianttPath('items.txt', occlude: true);

  // Prefer flow CRDT (merged across all configured flows); fall back to file
  final flowIds = _getFlowIds();
  final graph = flowIds.isNotEmpty ? FlowRepository.loadMergedGraph(flowIds).graph : null;

  executeBlockedCommand(
    itemsPath: itemsPath,
    occludeItemsPath: occludeItemsPath,
    graph: graph,
    charts: _parseCommaList(args['charts'] as String?),
    excludeCharts: _parseCommaList(args['exclude-charts'] as String?),
    minPriority: _parsePriority(args['min-priority'] as String?),
    jsonOutput: args['json'] as bool,
  );
}

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
