/// Exception thrown when a cycle is detected in the dependency graph
class CycleDetectedException implements Exception {
  const CycleDetectedException(this.cycleItems);

  /// The items that form the cycle
  final List<String> cycleItems;

  @override
  String toString() {
    final cycleStr = cycleItems.join(' -> ');
    return 'Cycle detected in dependencies: $cycleStr';
  }
}

/// Exception thrown when parsing fails
class GianttParseException implements Exception {
  const GianttParseException(this.message, [this.input]);

  /// Error message
  final String message;
  
  /// The input that caused the error
  final String? input;

  @override
  String toString() {
    if (input != null) {
      return 'Parse error: $message\nInput: $input';
    }
    return 'Parse error: $message';
  }
}

/// Exception thrown when graph operations fail
class GraphException implements Exception {
  const GraphException(this.message);

  /// Error message
  final String message;

  @override
  String toString() => 'Graph error: $message';
}
