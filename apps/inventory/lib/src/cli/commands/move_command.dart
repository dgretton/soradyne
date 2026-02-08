import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class MoveCommand extends Command<void> {
  @override
  final String name = 'move';

  @override
  final String description = 'Move an item to a new location.';

  MoveCommand();

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.length < 2) {
      usageException(
          'Move command requires a search string and a new location.');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final searchStr = argResults!.rest[0];
    final newLocation = argResults!.rest[1];

    try {
      await api.moveItem(searchStr: searchStr, newLocation: newLocation);
      print('Moved item matching "$searchStr" to "$newLocation".');
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    }
  }
}
