import 'package:meta/meta.dart';
import 'duration.dart';

/// Type of time constraint
enum TimeConstraintType {
  window('window'),
  deadline('deadline'),
  recurring('recurring');

  const TimeConstraintType(this.value);
  final String value;
}

/// Type of consequence for time constraint violations
enum ConsequenceType {
  severe('severe'),
  warning('warn'),
  escalating('escalating');

  const ConsequenceType(this.value);
  final String value;

  static ConsequenceType fromString(String value) {
    for (final type in ConsequenceType.values) {
      if (type.value == value) return type;
    }
    throw ArgumentError('Invalid consequence type: $value');
  }
}

/// Escalation rate for escalating consequences
enum EscalationRate {
  lowest(',,,'),
  low('...'),
  neutral(''),
  unsure('?'),
  medium('!'),
  high('!!'),
  critical('!!!');

  const EscalationRate(this.symbol);
  final String symbol;

  static EscalationRate fromString(String symbol) {
    for (final rate in EscalationRate.values) {
      if (rate.symbol == symbol) return rate;
    }
    throw ArgumentError('Invalid escalation rate: $symbol');
  }
}

/// Represents a time constraint on a Giantt item
@immutable
class TimeConstraint {
  const TimeConstraint({
    required this.type,
    required this.duration,
    this.gracePeriod,
    this.consequenceType = ConsequenceType.warning,
    this.escalationRate = EscalationRate.neutral,
    this.dueDate,
    this.interval,
    this.stack = false,
  });

  /// The type of time constraint
  final TimeConstraintType type;
  
  /// The main duration for the constraint
  final GianttDuration duration;
  
  /// Optional grace period
  final GianttDuration? gracePeriod;
  
  /// Type of consequence for violations
  final ConsequenceType consequenceType;
  
  /// Escalation rate for escalating consequences
  final EscalationRate escalationRate;
  
  /// Due date for deadline constraints (YYYY-MM-DD format)
  final String? dueDate;
  
  /// Interval for recurring constraints
  final GianttDuration? interval;
  
  /// Whether recurring constraints should stack
  final bool stack;

  /// Parse a time constraint from a string (alias for fromString)
  static TimeConstraint? parse(String? constraintStr) => fromString(constraintStr);

  /// Parse a time constraint from a string
  static TimeConstraint? fromString(String? constraintStr) {
    if (constraintStr == null || constraintStr.isEmpty) {
      return null;
    }

    // Parse window constraints: window(5d:2d,severe)
    final windowMatch = RegExp(r'window\((\d+[smhdwy]+)(?::(\d+[smhdwy]+))?,([^)]+)\)').firstMatch(constraintStr);
    if (windowMatch != null) {
      final window = GianttDuration.parse(windowMatch.group(1)!);
      final grace = windowMatch.group(2) != null ? GianttDuration.parse(windowMatch.group(2)!) : null;
      final consequence = _parseConsequence(windowMatch.group(3)!);

      return TimeConstraint(
        type: TimeConstraintType.window,
        duration: window,
        gracePeriod: grace,
        consequenceType: consequence['type']!,
        escalationRate: consequence['rate']!,
      );
    }

    // Parse deadline constraints: due(2024-12-31:2d,severe)
    final deadlineMatch = RegExp(r'due\((\d{4}-\d{2}-\d{2})(?::(\d+[smhdwy]+))?,([^)]+)\)').firstMatch(constraintStr);
    if (deadlineMatch != null) {
      final dueDate = deadlineMatch.group(1)!;
      final grace = deadlineMatch.group(2) != null ? GianttDuration.parse(deadlineMatch.group(2)!) : null;
      final consequence = _parseConsequence(deadlineMatch.group(3)!);

      return TimeConstraint(
        type: TimeConstraintType.deadline,
        duration: GianttDuration.parse('1d'), // Default to 1 day for deadline
        gracePeriod: grace,
        consequenceType: consequence['type']!,
        escalationRate: consequence['rate']!,
        dueDate: dueDate,
      );
    }

    // Parse recurring constraints: every(7d:1d,warn,stack)
    final recurringMatch = RegExp(r'every\((\d+[smhdwy]+)(?::(\d+[smhdwy]+))?,([^)]+)\)').firstMatch(constraintStr);
    if (recurringMatch != null) {
      final interval = GianttDuration.parse(recurringMatch.group(1)!);
      final grace = recurringMatch.group(2) != null ? GianttDuration.parse(recurringMatch.group(2)!) : null;
      final consequenceStr = recurringMatch.group(3)!;

      final stack = consequenceStr.contains('stack');
      final cleanConsequenceStr = consequenceStr.replaceAll(',stack', '').replaceAll('stack,', '').replaceAll('stack', '');
      final consequence = _parseConsequence(cleanConsequenceStr);

      return TimeConstraint(
        type: TimeConstraintType.recurring,
        duration: interval,
        gracePeriod: grace,
        consequenceType: consequence['type']!,
        escalationRate: consequence['rate']!,
        interval: interval,
        stack: stack,
      );
    }

    throw ArgumentError('Invalid time constraint format: $constraintStr');
  }

  /// Parse consequence information from a string
  static Map<String, dynamic> _parseConsequence(String consequenceStr) {
    final parts = consequenceStr.split(',').map((s) => s.trim()).toList();
    final baseConsequence = parts[0];

    for (final part in parts) {
      if (part.startsWith('escalate:')) {
        final rateStr = part.substring(9); // Remove 'escalate:'
        return {
          'type': ConsequenceType.escalating,
          'rate': rateStr.isNotEmpty ? EscalationRate.fromString(rateStr) : EscalationRate.neutral,
        };
      }
    }

    return {
      'type': ConsequenceType.fromString(baseConsequence),
      'rate': EscalationRate.neutral,
    };
  }

  @override
  String toString() {
    final baseStr = switch (type) {
      TimeConstraintType.window => 'window($duration',
      TimeConstraintType.deadline => 'due($dueDate',
      TimeConstraintType.recurring => 'every($interval',
    };

    final buffer = StringBuffer(baseStr);
    
    if (gracePeriod != null) {
      buffer.write(':$gracePeriod');
    }
    
    buffer.write(',${consequenceType.value}');
    
    if (escalationRate != EscalationRate.neutral) {
      buffer.write(',escalate:${escalationRate.symbol}');
    }
    
    if (type == TimeConstraintType.recurring && stack) {
      buffer.write(',stack');
    }
    
    buffer.write(')');
    return buffer.toString();
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! TimeConstraint) return false;
    return type == other.type &&
           duration == other.duration &&
           gracePeriod == other.gracePeriod &&
           consequenceType == other.consequenceType &&
           escalationRate == other.escalationRate &&
           dueDate == other.dueDate &&
           interval == other.interval &&
           stack == other.stack;
  }

  @override
  int get hashCode => Object.hash(
    type, duration, gracePeriod, consequenceType, 
    escalationRate, dueDate, interval, stack
  );

  /// Create a copy with modified properties
  TimeConstraint copyWith({
    TimeConstraintType? type,
    GianttDuration? duration,
    GianttDuration? gracePeriod,
    ConsequenceType? consequenceType,
    EscalationRate? escalationRate,
    String? dueDate,
    GianttDuration? interval,
    bool? stack,
  }) {
    return TimeConstraint(
      type: type ?? this.type,
      duration: duration ?? this.duration,
      gracePeriod: gracePeriod ?? this.gracePeriod,
      consequenceType: consequenceType ?? this.consequenceType,
      escalationRate: escalationRate ?? this.escalationRate,
      dueDate: dueDate ?? this.dueDate,
      interval: interval ?? this.interval,
      stack: stack ?? this.stack,
    );
  }
}
