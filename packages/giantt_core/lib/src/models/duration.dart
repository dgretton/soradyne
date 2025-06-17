import 'package:meta/meta.dart';

/// A single part of a duration with an amount and unit
@immutable
class DurationPart {
  const DurationPart({
    required this.amount,
    required this.unit,
  });

  /// The numeric amount for this duration part
  final double amount;
  
  /// The unit (s, min, h, d, w, mo, y)
  final String unit;

  /// Unit conversion to seconds
  static const Map<String, int> _unitSeconds = {
    's': 1,
    'min': 60,
    'h': 3600,
    'hr': 3600,
    'd': 86400,
    'w': 604800,
    'mo': 2592000, // 30 days
    'y': 31536000, // 365 days
  };

  /// Unit normalization mapping
  static const Map<String, String> _unitNormalize = {
    'hr': 'h',
    'minute': 'min',
    'minutes': 'min',
    'hour': 'h',
    'hours': 'h',
    'day': 'd',
    'days': 'd',
    'week': 'w',
    'weeks': 'w',
    'month': 'mo',
    'months': 'mo',
    'year': 'y',
    'years': 'y',
  };

  /// Factory method to create a normalized DurationPart
  factory DurationPart.create(double amount, String unit) {
    final normalizedUnit = _unitNormalize[unit] ?? unit;
    
    if (!_unitSeconds.containsKey(normalizedUnit)) {
      throw ArgumentError('Invalid duration unit: $unit');
    }
    
    return DurationPart(amount: amount, unit: normalizedUnit);
  }

  /// Parse a duration part from a string like "5d" or "3.5h"
  static DurationPart parse(String durationStr) {
    final match = RegExp(r'^(\d+\.?\d*)([a-zA-Z]+)$').firstMatch(durationStr);
    if (match == null) {
      throw ArgumentError('Invalid duration part format: $durationStr');
    }
    
    final amount = double.parse(match.group(1)!);
    final unit = match.group(2)!;
    
    return DurationPart.create(amount, unit);
  }

  /// Get total seconds for this duration part
  double get totalSeconds => amount * (_unitSeconds[unit] ?? 0);

  @override
  String toString() {
    // For whole numbers, display as integers
    final amountStr = amount == amount.toInt() 
        ? amount.toInt().toString() 
        : amount.toString();
    return '$amountStr$unit';
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! DurationPart) return false;
    return amount == other.amount && unit == other.unit;
  }

  @override
  int get hashCode => Object.hash(amount, unit);
}

/// Handles compound durations like '6mo8d3.5s'
@immutable
class GianttDuration {
  const GianttDuration({this.parts = const []});

  /// The individual duration parts that make up this duration
  final List<DurationPart> parts;

  /// Parse a duration string into a GianttDuration object
  static GianttDuration parse(String durationStr) {
    if (durationStr.isEmpty) {
      throw ArgumentError('Empty duration string');
    }

    final pattern = RegExp(r'(\d+\.?\d*)([a-zA-Z]+)');
    final matches = pattern.allMatches(durationStr);
    final parts = <DurationPart>[];

    for (final match in matches) {
      final amount = double.parse(match.group(1)!);
      final unit = match.group(2)!;
      parts.add(DurationPart.create(amount, unit));
    }

    if (parts.isEmpty) {
      throw ArgumentError('No valid duration parts found in: $durationStr');
    }

    return GianttDuration(parts: parts);
  }

  /// Get total duration in seconds
  double get totalSeconds => parts.fold(0.0, (sum, part) => sum + part.totalSeconds);

  /// Create a duration with a single part
  factory GianttDuration.single(double amount, String unit) {
    return GianttDuration(parts: [DurationPart.create(amount, unit)]);
  }

  /// Create an empty duration (0 seconds)
  factory GianttDuration.zero() {
    return const GianttDuration(parts: []);
  }

  @override
  String toString() {
    if (parts.isEmpty) return '0s';
    return parts.map((part) => part.toString()).join('');
  }

  /// Add two durations
  GianttDuration operator +(GianttDuration other) {
    final totalSecs = totalSeconds + other.totalSeconds;
    
    // Convert back to largest sensible unit
    const unitOrder = ['y', 'mo', 'w', 'd', 'h', 'min', 's'];
    for (final unit in unitOrder) {
      final seconds = DurationPart._unitSeconds[unit]!;
      if (totalSecs >= seconds) {
        final amount = totalSecs / seconds;
        final finalAmount = amount == amount.toInt() ? amount.toInt().toDouble() : amount;
        return GianttDuration(parts: [DurationPart(amount: finalAmount, unit: unit)]);
      }
    }
    
    return GianttDuration(parts: [DurationPart(amount: totalSecs, unit: 's')]);
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! GianttDuration) return false;
    return totalSeconds == other.totalSeconds;
  }

  bool operator <(GianttDuration other) => totalSeconds < other.totalSeconds;
  bool operator >(GianttDuration other) => totalSeconds > other.totalSeconds;
  bool operator <=(GianttDuration other) => totalSeconds <= other.totalSeconds;
  bool operator >=(GianttDuration other) => totalSeconds >= other.totalSeconds;

  @override
  int get hashCode => Object.hashAll(parts);

  /// Create a copy with modified parts
  GianttDuration copyWith({List<DurationPart>? parts}) {
    return GianttDuration(parts: parts ?? this.parts);
  }
}
