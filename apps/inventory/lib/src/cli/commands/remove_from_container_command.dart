import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class RemoveFromContainerCommand extends Command<void> {
  @override
  final String name = 'remove-from-container';

  @override
  final String description = 'Remove an item from its container.';

  RemoveFromContainerCommand();

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.isEmpty) {
      usageException(
          'remove-from-container command requires a search string for the item.');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final searchStr = argResults!.rest.join(' ');

    try {
      await api.removeFromContainer(searchStr: searchStr);
      print('Removed item matching "$searchStr" from its container.');
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    }
  }
}
