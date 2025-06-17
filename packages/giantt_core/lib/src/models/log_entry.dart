import 'package:meta/meta.dart';

/// Represents a single log entry
@immutable
class LogEntry {
  const LogEntry({
    required this.session,
    required this.timestamp,
    required this.message,
    this.tags = const {},
    this.metadata = const {},
    this.occlude = false,
  });

  /// Session identifier
  final String session;
  
  /// When this entry was created
  final DateTime timestamp;
  
  /// Log message content
  final String message;
  
  /// Associated tags
  final Set<String> tags;
  
  /// Additional metadata
  final Map<String, String> metadata;
  
  /// Whether this entry is occluded
  final bool occlude;

  /// Set the occlude status of this entry
  LogEntry setOcclude(bool occlude) {
    return copyWith(occlude: occlude);
  }

  /// Create a LogEntry from a JSON line
  static LogEntry fromJsonLine(String jsonLine, {bool occlude = false}) {
    // This would parse the JSONL format from Python
    // For now, placeholder implementation
    throw UnimplementedError('LogEntry.fromJsonLine not yet implemented');
  }

  /// Convert to JSON line format
  String toJsonLine() {
    // This would serialize to JSONL format matching Python
    // For now, placeholder implementation
    throw UnimplementedError('LogEntry.toJsonLine not yet implemented');
  }

  /// Create a copy with modified properties
  LogEntry copyWith({
    String? session,
    DateTime? timestamp,
    String? message,
    Set<String>? tags,
    Map<String, String>? metadata,
    bool? occlude,
  }) {
    return LogEntry(
      session: session ?? this.session,
      timestamp: timestamp ?? this.timestamp,
      message: message ?? this.message,
      tags: tags ?? this.tags,
      metadata: metadata ?? this.metadata,
      occlude: occlude ?? this.occlude,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! LogEntry) return false;
    return session == other.session &&
           timestamp == other.timestamp &&
           message == other.message &&
           tags.length == other.tags.length &&
           tags.every((tag) => other.tags.contains(tag)) &&
           metadata.length == other.metadata.length &&
           occlude == other.occlude;
  }

  @override
  int get hashCode => Object.hash(
    session, timestamp, message, 
    Object.hashAll(tags), Object.hashAll(metadata.entries), occlude
  );

  @override
  String toString() => 'LogEntry(session: $session, message: $message)';
}
