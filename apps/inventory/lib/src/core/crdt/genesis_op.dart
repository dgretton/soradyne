import '../models/inventory_entry.dart';
import 'operation.dart';

class GenesisOp extends Operation {
  final List<InventoryEntry> initialItems;

  GenesisOp({
    String? id,
    required String nodeId,
    required this.initialItems,
    DateTime? timestamp,
  }) : super(id: id, nodeId: nodeId, timestamp: timestamp);

  @override
  List<Object?> get props => [...super.props, initialItems];

  @override
  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'timestamp': timestamp.toIso8601String(),
      'nodeId': nodeId,
      'type': 'GenesisOp',
      'initialItems': initialItems.map((item) => item.toJson()).toList(),
    };
  }

  factory GenesisOp.fromJson(Map<String, dynamic> json) {
    return GenesisOp(
      id: json['id'] as String,
      timestamp: DateTime.parse(json['timestamp'] as String),
      nodeId: json['nodeId'] as String,
      initialItems: (json['initialItems'] as List<dynamic>)
          .map((itemJson) =>
              InventoryEntry.fromJson(itemJson as Map<String, dynamic>))
          .toList(),
    );
  }
}
