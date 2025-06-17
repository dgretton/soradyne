import 'package:meta/meta.dart';

/// Represents chart metadata and view settings
@immutable
class Chart {
  const Chart({
    required this.name,
    this.description = '',
    this.color,
    this.isVisible = true,
  });

  /// Chart name
  final String name;
  
  /// Optional description
  final String description;
  
  /// Optional color for visualization
  final String? color;
  
  /// Whether this chart is currently visible
  final bool isVisible;

  /// Create a copy with modified properties
  Chart copyWith({
    String? name,
    String? description,
    String? color,
    bool? isVisible,
  }) {
    return Chart(
      name: name ?? this.name,
      description: description ?? this.description,
      color: color ?? this.color,
      isVisible: isVisible ?? this.isVisible,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! Chart) return false;
    return name == other.name &&
           description == other.description &&
           color == other.color &&
           isVisible == other.isVisible;
  }

  @override
  int get hashCode => Object.hash(name, description, color, isVisible);

  @override
  String toString() => 'Chart(name: $name, visible: $isVisible)';
}
