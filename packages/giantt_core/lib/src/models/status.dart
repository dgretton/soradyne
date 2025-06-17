import 'package:meta/meta.dart';

/// Status of a Giantt item with corresponding symbols
enum GianttStatus {
  notStarted('○', 'NOT_STARTED'),
  inProgress('◑', 'IN_PROGRESS'), 
  blocked('⊘', 'BLOCKED'),
  completed('●', 'COMPLETED');

  const GianttStatus(this.symbol, this.name);

  /// The Unicode symbol representing this status
  final String symbol;
  
  /// The string name for this status
  final String name;

  /// Create a GianttStatus from its symbol
  static GianttStatus fromSymbol(String symbol) {
    for (final status in GianttStatus.values) {
      if (status.symbol == symbol) {
        return status;
      }
    }
    throw ArgumentError('Invalid status symbol: $symbol');
  }

  /// Create a GianttStatus from its name
  static GianttStatus fromName(String name) {
    for (final status in GianttStatus.values) {
      if (status.name == name) {
        return status;
      }
    }
    throw ArgumentError('Invalid status name: $name');
  }

  @override
  String toString() => symbol;
}
