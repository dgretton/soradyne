import 'dart:io';
import 'package:args/command_runner.dart';
import '../../core/inventory_api.dart';

class ExportCommand extends Command<void> {
  ExportCommand() {
    argParser.addOption(
      'output',
      abbr: 'o',
      help: 'Output file path (default: prints to stdout)',
    );
  }

  @override
  final String name = 'export';

  @override
  final String description = 'Export the current inventory state in legacy format.';

  @override
  Future<void> run() async {
    final inventoryPath = globalResults!['inventory'] as String;
    final api = InventoryApi(operationLogPath: inventoryPath);
    await api.initialize(inventoryPath);

    try {
      final exportedContent = api.exportToLegacyFormat();

      final outputPath = argResults!['output'] as String?;
      if (outputPath != null) {
        final file = File(outputPath);
        await file.writeAsString(exportedContent);
        print('Inventory exported to: $outputPath');
      } else {
        print(exportedContent);
      }
    } catch (e) {
      throw UsageException('Failed to export inventory: $e', usage);
    }
  }
}
