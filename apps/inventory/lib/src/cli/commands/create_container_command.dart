import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class CreateContainerCommand extends Command<void> {
  CreateContainerCommand() {
    argParser.addOption(
      'description',
      abbr: 'd',
      help: 'Optional description for the container.',
    );
  }

  @override
  final String name = 'create-container';

  @override
  final String description = 'Create a new storage container.';

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.length < 2) {
      usageException('create-container command requires an ID and a location.');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final containerId = argResults!.rest[0];
    final location = argResults!.rest[1];
    final description =
        argResults!['description'] as String? ?? 'Storage container $containerId';

    try {
      await api.createContainer(
        containerId: containerId,
        location: location,
        description: description,
      );

      print('Created container "$containerId" at "$location".');
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    }
  }
}
