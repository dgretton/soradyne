import 'operation.dart';

class DeleteItemOp extends Operation {
  final String itemId;

  DeleteItemOp({
    String? id,
    required String nodeId,
    required this.itemId,
    DateTime? timestamp,
  }) : super(id: id, nodeId: nodeId, timestamp: timestamp);

  @override
  List<Object?> get props => [...super.props, itemId];

  @override
  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'timestamp': timestamp.toIso8601String(),
      'nodeId': nodeId,
      'type': 'DeleteItemOp',
      'itemId': itemId,
    };
  }

  factory DeleteItemOp.fromJson(Map<String, dynamic> json) {
    return DeleteItemOp(
      id: json['id'] as String,
      timestamp: DateTime.parse(json['timestamp'] as String),
      nodeId: json['nodeId'] as String,
      itemId: json['itemId'] as String,
    );
  }
}
