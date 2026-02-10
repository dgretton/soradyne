import 'package:args/command_runner.dart';
import 'commands/add_command.dart';
import 'commands/delete_command.dart';
import 'commands/create_container_command.dart';
import 'commands/move_command.dart';
import 'commands/put_in_command.dart';
import 'commands/remove_from_container_command.dart';
import 'commands/edit_description_command.dart';
import 'commands/search_command.dart';
import 'commands/export_command.dart';

class InventoryCommandRunner extends CommandRunner<void> {
  InventoryCommandRunner()
      : super('inv', 'A command-line tool for managing personal inventory.') {
    argParser.addOption(
      'inventory',
      defaultsTo: 'seed_inventory.txt',
      help: 'Inventory file path',
    );
    addCommand(AddCommand());
    addCommand(DeleteCommand());
    addCommand(CreateContainerCommand());
    addCommand(MoveCommand());
    addCommand(PutInCommand());
    addCommand(RemoveFromContainerCommand());
    addCommand(EditDescriptionCommand());
    addCommand(SearchCommand());
    addCommand(ExportCommand());
  }
}

Future<void> runCli(List<String> args) async {
  await InventoryCommandRunner().run(args);
}
