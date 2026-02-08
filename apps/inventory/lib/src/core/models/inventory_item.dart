import 'package:equatable/equatable.dart';

class InventoryItem extends Equatable {
  const InventoryItem({
    required this.category,
    required this.tags,
    required this.description,
    required this.location,
  });

  final String category;
  final List<String> tags;
  final String description;
  final String location;

  @override
  List<Object?> get props => [category, tags, description, location];
}
