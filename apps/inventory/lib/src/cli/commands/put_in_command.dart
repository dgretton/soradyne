import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class PutInCommand extends Command<void> {
  @override
  final String name = 'put-in';

  @override
  final String description = 'Put an item into a container.';

  PutInCommand();

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.length < 2) {
      usageException(
          'put-in command requires a container ID and an item search string.');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final containerId = argResults!.rest[0];
    final searchStr = argResults!.rest[1];

    try {
      await api.putInContainer(searchStr: searchStr, containerId: containerId);
      print('Put "$searchStr" in container "$containerId".');
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    }
  }
}
