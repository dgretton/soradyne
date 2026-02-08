import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class DeleteCommand extends Command<void> {
  @override
  final String name = 'delete';

  @override
  final String description = 'Delete an item from the inventory.';

  DeleteCommand();

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.isEmpty) {
      usageException(
          'Delete command requires a search string for the item to delete.');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final searchStr = argResults!.rest.join(' ');

    try {
      await api.deleteItem(searchStr: searchStr);
      print('Deleted item matching "$searchStr".');
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    }
  }
}
