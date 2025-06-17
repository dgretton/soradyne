import 'package:meta/meta.dart';

/// Type of relation between Giantt items with corresponding symbols
@immutable
enum RelationType {
  requires('⊢', 'REQUIRES'),
  anyof('⋲', 'ANYOF'),
  supercharges('≫', 'SUPERCHARGES'),
  indicates('∴', 'INDICATES'),
  together('∪', 'TOGETHER'),
  conflicts('⊟', 'CONFLICTS'),
  blocks('►', 'BLOCKS'),
  sufficient('≻', 'SUFFICIENT');

  const RelationType(this.symbol, this.name);

  /// The Unicode symbol representing this relation type
  final String symbol;
  
  /// The string name for this relation type
  final String name;

  /// Create a RelationType from its symbol
  static RelationType fromSymbol(String symbol) {
    for (final relationType in RelationType.values) {
      if (relationType.symbol == symbol) {
        return relationType;
      }
    }
    throw ArgumentError('Invalid relation symbol: $symbol');
  }

  /// Create a RelationType from its name
  static RelationType fromName(String name) {
    for (final relationType in RelationType.values) {
      if (relationType.name == name) {
        return relationType;
      }
    }
    throw ArgumentError('Invalid relation name: $name');
  }

  @override
  String toString() => symbol;
}

/// Represents a relation between two Giantt items
@immutable
class Relation {
  const Relation({
    required this.type,
    required this.targetIds,
  });

  /// The type of relation
  final RelationType type;
  
  /// The IDs of the target items
  final List<String> targetIds;

  /// Create a copy with modified properties
  Relation copyWith({
    RelationType? type,
    List<String>? targetIds,
  }) {
    return Relation(
      type: type ?? this.type,
      targetIds: targetIds ?? this.targetIds,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! Relation) return false;
    return type == other.type && 
           targetIds.length == other.targetIds.length &&
           targetIds.every((id) => other.targetIds.contains(id));
  }

  @override
  int get hashCode => Object.hash(type, Object.hashAll(targetIds));

  @override
  String toString() => '${type.symbol}[${targetIds.join(',')}]';
}
