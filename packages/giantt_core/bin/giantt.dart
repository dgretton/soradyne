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

ArgParser _createInitCommand() {
  return ArgParser()
    ..addFlag('dev', help: 'Initialize for development', negatable: false)
    ..addOption('data-dir', help: 'Custom data directory location');
}

ArgParser _createAddCommand() {
  return ArgParser()
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
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('log-file', abbr: 'l', help: 'Giantt log file to use')
    ..addOption('occlude-log-file', abbr: 'al', help: 'Giantt occlude log file to use')
    ..addFlag('chart', help: 'Search in chart names', negatable: false)
    ..addFlag('log', help: 'Search in logs and log sessions', negatable: false);
}

ArgParser _createModifyCommand() {
  return ArgParser()
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
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use')
    ..addOption('log-file', abbr: 'l', help: 'Giantt log file to use')
    ..addOption('occlude-log-file', abbr: 'al', help: 'Giantt occlude log file to use');
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
    ..addOption('file', abbr: 'f', help: 'Giantt items file to use')
    ..addOption('occlude-file', abbr: 'a', help: 'Giantt occluded items file to use');
  
  // Add subcommands for doctor
  parser.addCommand('check', ArgParser());
  
  parser.addCommand('fix', ArgParser()
    ..addOption('type', abbr: 't', help: 'Type of issue to fix (e.g., dangling_reference)')
    ..addOption('item', abbr: 'i', help: 'Fix issues for a specific item ID')
    ..addFlag('all', abbr: 'a', help: 'Fix all fixable issues', negatable: false)
    ..addFlag('dry-run', help: 'Show what would be fixed without making changes', negatable: false));
    
  parser.addCommand('list-types', ArgParser());
  
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
  print('TODO: Implement init command');
  // TODO: Implement initialization logic
}

Future<void> _executeAdd(ArgResults args) async {
  print('TODO: Implement add command');
  // TODO: Implement add logic
}

Future<void> _executeShow(ArgResults args) async {
  print('TODO: Implement show command');
  // TODO: Implement show logic
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
  print('TODO: Implement sort command');
  // TODO: Implement sort logic
}

Future<void> _executeTouch(ArgResults args) async {
  print('TODO: Implement touch command');
  // TODO: Implement touch logic
}

Future<void> _executeInsert(ArgResults args) async {
  print('TODO: Implement insert command');
  // TODO: Implement insert logic
}

Future<void> _executeIncludes(ArgResults args) async {
  print('TODO: Implement includes command');
  // TODO: Implement includes logic
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
  print('TODO: Implement doctor command');
  // TODO: Implement doctor logic
}
