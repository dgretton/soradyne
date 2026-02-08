import '../models/inventory_entry.dart';
import 'operation.dart';

class AddItemOp extends Operation {
  final InventoryEntry item;

  AddItemOp({
    String? id,
    required String nodeId,
    required this.item,
    DateTime? timestamp,
  }) : super(id: id, nodeId: nodeId, timestamp: timestamp);

  @override
  List<Object?> get props => [...super.props, item];

  @override
  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'timestamp': timestamp.toIso8601String(),
      'nodeId': nodeId,
      'type': 'AddItemOp',
      'item': item.toJson(),
    };
  }

  factory AddItemOp.fromJson(Map<String, dynamic> json) {
    return AddItemOp(
      id: json['id'] as String,
      timestamp: DateTime.parse(json['timestamp'] as String),
      nodeId: json['nodeId'] as String,
      item: InventoryEntry.fromJson(json['item'] as Map<String, dynamic>),
    );
  }
}
