import 'package:equatable/equatable.dart';
import 'package:uuid/uuid.dart';

class InventoryEntry extends Equatable {
  const InventoryEntry({
    required this.id,
    required this.category,
    required this.description,
    required this.location,
    this.tags = const [],
  });

  final String id;
  final String category;
  final String description;
  final String location;
  final List<String> tags;

  @override
  List<Object?> get props => [id, category, description, location, tags];

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'category': category,
      'description': description,
      'location': location,
      'tags': tags,
    };
  }

  factory InventoryEntry.fromJson(Map<String, dynamic> json) {
    return InventoryEntry(
      id: json['id'] as String,
      category: json['category'] as String,
      description: json['description'] as String,
      location: json['location'] as String,
      tags: (json['tags'] as List<dynamic>).cast<String>(),
    );
  }

  InventoryEntry copyWith({
    String? id,
    String? category,
    String? description,
    String? location,
    List<String>? tags,
  }) {
    return InventoryEntry(
      id: id ?? this.id,
      category: category ?? this.category,
      description: description ?? this.description,
      location: location ?? this.location,
      tags: tags ?? this.tags,
    );
  }

  String toLine() {
    final tagsStr = tags.map((t) => '"$t"').join(',');
    return '{"id":"$id","category":"$category","tags":[$tagsStr]} $description -> $location';
  }

  static InventoryEntry fromLine(String line) {
    final idMatch = RegExp(r'"id":"([^"]+)"').firstMatch(line);
    final categoryMatch = RegExp(r'"category":"([^"]+)"').firstMatch(line);
    final tagsMatch = RegExp(r'"tags":\[(.*?)\]').firstMatch(line);
    final contentMatch = RegExp(r'} (.*?) -> (.*)').firstMatch(line);

    if (categoryMatch == null || contentMatch == null) {
      throw FormatException('Invalid inventory entry format: $line');
    }

    final id = idMatch?.group(1) ?? const Uuid().v4();

    final tagsRaw = tagsMatch?.group(1);
    final tags = (tagsRaw == null || tagsRaw.isEmpty)
        ? <String>[]
        : tagsRaw.split(',').map((t) => t.trim().replaceAll('"', '')).toList();

    return InventoryEntry(
      id: id,
      category: categoryMatch.group(1)!,
      description: contentMatch.group(1)!,
      location: contentMatch.group(2)!,
      tags: tags,
    );
  }
}
