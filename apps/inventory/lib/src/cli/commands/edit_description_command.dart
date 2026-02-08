import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class EditDescriptionCommand extends Command<void> {
  @override
  final String name = 'edit-description';

  @override
  final String description = 'Edit the description of an existing item.';

  EditDescriptionCommand();

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.length < 2) {
      usageException(
          'edit-description command requires a search string and a new description.');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final searchStr = argResults!.rest[0];
    final newDescription = argResults!.rest[1];

    try {
      await api.editDescription(
          searchStr: searchStr, newDescription: newDescription);
      print(
          'Changed description from "$searchStr" to "$newDescription".');
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    }
  }
}
