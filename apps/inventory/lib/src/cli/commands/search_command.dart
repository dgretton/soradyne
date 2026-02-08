import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class SearchCommand extends Command<void> {
  @override
  final String name = 'search';

  @override
  final String description = 'Search inventory for items containing a string.';

  SearchCommand();

  @override
  Future<void> run() async {
    if (argResults == null || argResults!.rest.isEmpty) {
      usageException('Search command requires a search string.');
    }

    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    final searchStr = argResults!.rest.join(' ');

    try {
      final results = await api.search(searchStr);
      if (results.isEmpty) {
        print('No items found matching "$searchStr".');
        return;
      }
      for (final entry in results) {
        print('${entry.description} -> ${entry.location}');
      }
    } on StateError catch (e) {
      throw UsageException(e.message, usage);
    }
  }
}
