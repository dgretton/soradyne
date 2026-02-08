import '../models/inventory_entry.dart';
import 'add_item_op.dart';
import 'delete_item_op.dart';
import 'genesis_op.dart';
import 'operation.dart';

class InventoryCRDT {
  final List<Operation> _operations = [];
  Map<String, InventoryEntry> _materializedView = {};

  List<Operation> get operations => List.unmodifiable(_operations);

  void apply(Operation op) {
    _operations.add(op);
    _recomputeState(); // For simplicity. Can be optimized.
  }

  void load(List<Operation> ops) {
    _operations.clear();
    _operations.addAll(ops);
    _recomputeState();
  }

  Map<String, InventoryEntry> get currentState =>
      Map.unmodifiable(_materializedView);

  void _recomputeState() {
    final newState = <String, InventoryEntry>{};
    // Sort operations by timestamp to ensure deterministic state
    final sortedOps = List<Operation>.from(_operations)
      ..sort((a, b) => a.timestamp.compareTo(b.timestamp));

    for (final op in sortedOps) {
      if (op is GenesisOp) {
        newState.clear(); // Genesis resets the state
        for (final item in op.initialItems) {
          newState[item.id] = item;
        }
      } else if (op is AddItemOp) {
        newState[op.item.id] = op.item;
      } else if (op is DeleteItemOp) {
        newState.remove(op.itemId);
      }
      // TODO: Implement other operations like MoveItemOp etc.
    }
    _materializedView = newState;
  }
}
