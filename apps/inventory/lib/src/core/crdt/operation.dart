import 'package:equatable/equatable.dart';
import 'package:uuid/uuid.dart';

abstract class Operation extends Equatable {
  final String id;
  final DateTime timestamp;
  final String nodeId; // To identify the device of origin

  Operation({String? id, DateTime? timestamp, required this.nodeId})
      : id = id ?? const Uuid().v4(),
        timestamp = timestamp ?? DateTime.now();

  @override
  List<Object?> get props => [id, timestamp, nodeId];

  Map<String, dynamic> toJson();
}
