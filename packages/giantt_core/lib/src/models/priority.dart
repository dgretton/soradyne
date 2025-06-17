import 'package:meta/meta.dart';

/// Priority level of a Giantt item with corresponding symbols
enum GianttPriority {
  lowest(',,,', 'LOWEST'),
  low('...', 'LOW'),
  neutral('', 'NEUTRAL'),
  unsure('?', 'UNSURE'),
  medium('!', 'MEDIUM'),
  high('!!', 'HIGH'),
  critical('!!!', 'CRITICAL');

  const GianttPriority(this.symbol, this.name);

  /// The symbol representing this priority level
  final String symbol;
  
  /// The string name for this priority
  final String name;

  /// Create a GianttPriority from its symbol
  static GianttPriority fromSymbol(String symbol) {
    for (final priority in GianttPriority.values) {
      if (priority.symbol == symbol) {
        return priority;
      }
    }
    throw ArgumentError('Invalid priority symbol: $symbol');
  }

  /// Create a GianttPriority from its name
  static GianttPriority fromName(String name) {
    for (final priority in GianttPriority.values) {
      if (priority.name == name) {
        return priority;
      }
    }
    throw ArgumentError('Invalid priority name: $name');
  }

  @override
  String toString() => symbol;
}
