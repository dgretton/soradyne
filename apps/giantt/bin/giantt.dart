#!/usr/bin/env dart

import 'dart:io';
import 'package:args/args.dart';
import 'package:giantt_core/giantt_core.dart';

/// Main entry point for the giantt CLI
void main(List<String> arguments) async {
  final parser = ArgParser()
    ..addFlag('help', abbr: 'h', help: 'Show usage information')
    ..addFlag('version', abbr: 'v', help: 'Show version information');

  try {
    final results = parser.parse(arguments);
    
    if (results['help'] as bool) {
      print('Giantt command line utility for managing task dependencies.');
      print('');
      print('Usage: giantt <command> [arguments]');
      print('');
      print('Available commands:');
      print('  init     Initialize Giantt directory structure');
      print('  add      Add a new item');
      print('  show     Show item details');
      print('  modify   Modify item properties');
      print('  remove   Remove an item');
      print('  sort     Sort items topologically');
      print('  doctor   Check graph health');
      print('');
      print('Run "giantt <command> --help" for more information about a command.');
      return;
    }
    
    if (results['version'] as bool) {
      print('giantt version 1.0.0');
      return;
    }
    
    if (results.rest.isEmpty) {
      print('Error: No command specified.');
      print('Run "giantt --help" for usage information.');
      exit(1);
    }
    
    final command = results.rest.first;
    final commandArgs = results.rest.skip(1).toList();
    
    // TODO: Implement command routing
    print('Command "$command" not yet implemented.');
    print('Arguments: $commandArgs');
    
  } catch (e) {
    print('Error: $e');
    exit(1);
  }
}
