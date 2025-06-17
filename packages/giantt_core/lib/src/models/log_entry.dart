import 'dart:convert';
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
    try {
      final data = json.decode(jsonLine) as Map<String, dynamic>;
      return LogEntry(
        session: data['s'] as String,
        timestamp: DateTime.parse(data['t'] as String),
        message: data['m'] as String,
        tags: Set<String>.from(data['tags'] as List),
        metadata: Map<String, String>.from(data['meta'] as Map? ?? {}),
        occlude: occlude,
      );
    } catch (e) {
      throw FormatException('Invalid log entry format: $e');
    }
  }

  /// Convert to JSON line format
  String toJsonLine() {
    final data = {
      's': session,
      't': timestamp.toIso8601String(),
      'm': message,
      'tags': tags.toList()..sort(),
      'meta': metadata,
    };
    return json.encode(data);
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

  /// Factory constructor to create a new log entry with current timestamp
  factory LogEntry.create(
    String sessionTag,
    String message, {
    List<String>? additionalTags,
    Map<String, String>? metadata,
    bool occlude = false,
  }) {
    final tags = <String>{sessionTag};
    if (additionalTags != null) {
      tags.addAll(additionalTags);
    }

    return LogEntry(
      session: sessionTag,
      timestamp: DateTime.now().toUtc(),
      message: message,
      tags: tags,
      metadata: metadata ?? {},
      occlude: occlude,
    );
  }

  /// Check if entry has a specific tag
  bool hasTag(String tag) => tags.contains(tag);

  /// Check if entry has any of the specified tags
  bool hasAnyTags(List<String> tagList) => tags.any(tagList.contains);

  /// Check if entry has all of the specified tags
  bool hasAllTags(List<String> tagList) => tagList.every(tags.contains);

  /// Add a tag to the entry
  LogEntry addTag(String tag) {
    final newTags = Set<String>.from(tags)..add(tag);
    return copyWith(tags: newTags);
  }

  /// Remove a tag from the entry
  LogEntry removeTag(String tag) {
    final newTags = Set<String>.from(tags)..remove(tag);
    return copyWith(tags: newTags);
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
