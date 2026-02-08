import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class AddCommand extends Command<void> {
  AddCommand() {
    argParser.addOption(
      'tags',
      abbr: 't',
      help: 'Comma-separated list of tags',
      defaultsTo: '',
    );
    argParser.addFlag(
      'container',
      abbr: 'c',
      help: 'Specify if the item is a container',
      defaultsTo: false,
    );
  }

  @override
  final String name = 'add';

  @override
  final String description = 'Add a new item to the inventory';

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.length < 3) {
      usageException('Add command requires category, description, and location');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final category = argResults!.rest[0];
    final description = argResults!.rest[1];
    final location = argResults!.rest[2];

    final tagsInput = argResults!['tags'] as String;
    final tags = tagsInput.isNotEmpty
        ? tagsInput.split(',').map((tag) => tag.trim()).toList()
        : <String>[];

    final isContainer = argResults!['container'] as bool;

    try {
      final conflict = api.findDescriptionConflict(description);
      if (conflict != null) {
        throw UsageException(
          'Description "$description" conflicts with existing item "${conflict.description}" (substring match).',
          usage);
      }

      await api.addItem(
        category: category,
        description: description,
        location: location,
        tags: tags,
        isContainer: isContainer,
        containerId: isContainer ? description : null,
      );

      print('Added: $description -> $location');
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    } catch (e) {
      throw UsageException('Failed to add item: $e', usage);
    }
  }
}
