import 'dart:io';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/chat/chat_processor.dart';
import 'package:inventory/src/core/inventory_api.dart';

void main() {
  group('ChatProcessor', () {
    late ChatProcessor chatProcessor;
    late InventoryApi inventoryApi;
    late File testFile;
    late File opsLogFile;
    const testFilePath = 'test_inventory.txt';
    const opsLogPath = 'test_inventory_ops.jsonl';

    setUp(() async {
      // Clean up both the legacy file and the CRDT operations log
      testFile = File(testFilePath);
      opsLogFile = File(opsLogPath);
      if (testFile.existsSync()) {
        testFile.deleteSync();
      }
      if (opsLogFile.existsSync()) {
        opsLogFile.deleteSync();
      }
      testFile.createSync();
      inventoryApi = InventoryApi(operationLogPath: testFilePath);
      await inventoryApi.initialize(testFilePath);
      chatProcessor = ChatProcessor(inventoryApi);
    });

    tearDown(() {
      if (testFile.existsSync()) {
        testFile.deleteSync();
      }
      if (opsLogFile.existsSync()) {
        opsLogFile.deleteSync();
      }
    });

    test('can process an "add" command', () async {
      const command = '''
      {
        "command": "add",
        "arguments": {
          "category": "Decor",
          "description": "Crystal vase",
          "location": "display cabinet",
          "tags": ["fragile", "valuable"],
          "isContainer": false
        }
      }
      ''';

      await chatProcessor.processCommand(command);

      final results = await inventoryApi.search('Crystal vase');
      expect(results, hasLength(1));
      final entry = results.first;

      expect(entry.category, 'Decor');
      expect(entry.description, 'Crystal vase');
      expect(entry.tags, containsAll(['fragile', 'valuable']));
    });

    test('can process an "add" command to create a container', () async {
      const command = '''
      {
        "command": "add",
        "arguments": {
          "category": "Containers",
          "description": "Big Box",
          "location": "attic",
          "isContainer": true,
          "containerId": "Big Box"
        }
      }
      ''';

      await chatProcessor.processCommand(command);

      final results = await inventoryApi.search('Big Box');
      expect(results, hasLength(1));
      final entry = results.first;

      expect(entry.category, 'Containers');
      expect(entry.description, 'Big Box');
      expect(entry.tags, contains('container_Big Box'));
    });

    test('add command rejects conflicting description (exact match)', () async {
      await inventoryApi.addItem(
        category: 'Decor',
        description: 'Crystal vase',
        location: 'display cabinet',
      );

      const command = '''
      {
        "command": "add",
        "arguments": {
          "category": "Decor",
          "description": "Crystal vase",
          "location": "kitchen shelf"
        }
      }
      ''';

      expect(
        () => chatProcessor.processCommand(command),
        throwsA(isA<StateError>()),
      );
    });

    test('add command rejects substring conflict (new is substring of existing)', () async {
      await inventoryApi.addItem(
        category: 'Decor',
        description: 'Crystal vase',
        location: 'display cabinet',
      );

      const command = '''
      {
        "command": "add",
        "arguments": {
          "category": "Decor",
          "description": "vase",
          "location": "kitchen shelf"
        }
      }
      ''';

      expect(
        () => chatProcessor.processCommand(command),
        throwsA(isA<StateError>()),
      );
    });

    test('add command rejects substring conflict (existing is substring of new)', () async {
      await inventoryApi.addItem(
        category: 'Decor',
        description: 'Crystal vase',
        location: 'display cabinet',
      );

      const command = '''
      {
        "command": "add",
        "arguments": {
          "category": "Decor",
          "description": "Large Crystal vase with flowers",
          "location": "kitchen shelf"
        }
      }
      ''';

      expect(
        () => chatProcessor.processCommand(command),
        throwsA(isA<StateError>()),
      );
    });

    test('can process a "delete" command', () async {
      await inventoryApi.addItem(
        category: 'Tools',
        description: 'Hammer',
        location: 'Toolbox',
      );

      const command = '''
      {
        "command": "delete",
        "arguments": {
          "search_str": "Hammer"
        }
      }
      ''';

      await chatProcessor.processCommand(command);

      final results = await inventoryApi.search('Hammer');
      expect(results, isEmpty);
    });

    test('can process a "move" command', () async {
      await inventoryApi.addItem(
        category: 'Tools',
        description: 'Hammer',
        location: 'Toolbox',
      );

      const command = '''
      {
        "command": "move",
        "arguments": {
          "search_str": "Hammer",
          "new_location": "Garage"
        }
      }
      ''';

      await chatProcessor.processCommand(command);

      final entry = await inventoryApi.findUniqueEntry('Hammer');
      expect(entry.location, 'Garage');
    });

    test('can process a "put-in" command', () async {
      await inventoryApi.addItem(
        category: 'Tools',
        description: 'Screwdriver',
        location: 'Toolbox',
      );
      await inventoryApi.addItem(
        category: 'Containers',
        description: 'Tool Chest',
        location: 'Garage',
        isContainer: true,
        containerId: 'Tool Chest',
      );

      const command = '''
      {
        "command": "put-in",
        "arguments": {
          "search_str": "Screwdriver",
          "container_id": "Tool Chest"
        }
      }
      ''';

      await chatProcessor.processCommand(command);

      final entry = await inventoryApi.findUniqueEntry('Screwdriver');
      expect(entry.location, 'container Tool Chest');
      expect(entry.tags, contains('container_Tool Chest'));
    });

    test('can process a "remove-from-container" command', () async {
      await inventoryApi.addItem(
        category: 'Tools',
        description: 'Screwdriver',
        location: 'Toolbox',
      );
      await inventoryApi.addItem(
        category: 'Containers',
        description: 'Tool Chest',
        location: 'Garage',
        isContainer: true,
        containerId: 'C1',
      );
      await inventoryApi.putInContainer(searchStr: 'Screwdriver', containerId: 'C1');

      const command = '''
      {
        "command": "remove-from-container",
        "arguments": {
          "search_str": "Screwdriver"
        }
      }
      ''';

      await chatProcessor.processCommand(command);

      final entry = await inventoryApi.findUniqueEntry('Screwdriver');
      expect(entry.location, 'Garage');
      expect(entry.tags, isNot(contains('container_C1')));
    });

    test('can process an "edit-description" command', () async {
      await inventoryApi.addItem(
        category: 'Tools',
        description: 'Hammer',
        location: 'Toolbox',
      );

      const command = '''
      {
        "command": "edit-description",
        "arguments": {
          "search_str": "Hammer",
          "new_description": "Claw Hammer"
        }
      }
      ''';

      await chatProcessor.processCommand(command);

      final entry = await inventoryApi.findUniqueEntry('Claw Hammer');
      expect(entry.description, 'Claw Hammer');
    });
  });
}
