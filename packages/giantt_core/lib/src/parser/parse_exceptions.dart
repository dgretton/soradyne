import '../models/graph_exceptions.dart';

/// Exception thrown when parsing a specific section fails
class SectionParseException extends GianttParseException {
  const SectionParseException(this.section, String message, [String? input])
      : super('$section: $message', input);

  /// The section that failed to parse
  final String section;
}

/// Exception thrown when a required field is missing
class MissingFieldException extends GianttParseException {
  const MissingFieldException(this.fieldName, [String? input])
      : super('Missing required field: $fieldName', input);

  /// The name of the missing field
  final String fieldName;
}

/// Exception thrown when a field has an invalid format
class InvalidFormatException extends GianttParseException {
  const InvalidFormatException(this.fieldName, this.expectedFormat, [String? input])
      : super('Invalid format for $fieldName. Expected: $expectedFormat', input);

  /// The name of the field with invalid format
  final String fieldName;
  
  /// Description of the expected format
  final String expectedFormat;
}

/// Exception thrown when an unknown symbol is encountered
class UnknownSymbolException extends GianttParseException {
  const UnknownSymbolException(this.symbol, this.context, [String? input])
      : super('Unknown symbol "$symbol" in $context', input);

  /// The unknown symbol
  final String symbol;
  
  /// The context where the symbol was found
  final String context;
}

/// Exception thrown when quotes are not properly balanced
class UnbalancedQuotesException extends GianttParseException {
  const UnbalancedQuotesException([String? input])
      : super('Unbalanced quotes in title', input);
}

/// Exception thrown when JSON parsing fails
class JsonParseException extends GianttParseException {
  const JsonParseException(this.jsonError, [String? input])
      : super('JSON parse error: $jsonError', input);

  /// The underlying JSON error
  final String jsonError;
}
