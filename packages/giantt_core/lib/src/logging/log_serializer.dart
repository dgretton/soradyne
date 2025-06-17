import 'dart:convert';
import '../models/log_entry.dart';

/// Handles JSONL serialization and deserialization for log entries
class LogSerializer {
  /// Serialize a log entry to JSONL format
  static String serialize(LogEntry entry) {
    return entry.toJsonLine();
  }

  /// Deserialize a JSONL line to a log entry
  static LogEntry deserialize(String jsonLine, {bool occlude = false}) {
    return LogEntry.fromJsonLine(jsonLine, occlude: occlude);
  }

  /// Serialize multiple log entries to JSONL format
  static String serializeMultiple(List<LogEntry> entries) {
    return entries.map(serialize).join('\n');
  }

  /// Deserialize multiple JSONL lines to log entries
  static List<LogEntry> deserializeMultiple(String jsonLines, {bool occlude = false}) {
    final lines = jsonLines.split('\n');
    final entries = <LogEntry>[];
    
    for (final line in lines) {
      final trimmed = line.trim();
      if (trimmed.isNotEmpty && !trimmed.startsWith('#')) {
        try {
          entries.add(deserialize(trimmed, occlude: occlude));
        } catch (e) {
          // Skip invalid lines
          print('Warning: Skipping invalid log line: $e');
        }
      }
    }
    
    return entries;
  }

  /// Validate that a string is valid JSONL format
  static bool isValidJsonLine(String line) {
    try {
      final data = json.decode(line);
      if (data is! Map<String, dynamic>) return false;
      
      // Check required fields
      return data.containsKey('s') &&  // session
             data.containsKey('t') &&  // timestamp
             data.containsKey('m') &&  // message
             data.containsKey('tags'); // tags
    } catch (e) {
      return false;
    }
  }

  /// Extract metadata from a JSONL line without full deserialization
  static Map<String, dynamic>? extractMetadata(String jsonLine) {
    try {
      final data = json.decode(jsonLine) as Map<String, dynamic>;
      return {
        'session': data['s'],
        'timestamp': data['t'],
        'tags': data['tags'],
        'hasMetadata': data.containsKey('meta') && (data['meta'] as Map).isNotEmpty,
      };
    } catch (e) {
      return null;
    }
  }

  /// Convert log entry to a pretty-printed JSON format (for debugging)
  static String toPrettyJson(LogEntry entry) {
    final data = {
      'session': entry.session,
      'timestamp': entry.timestamp.toIso8601String(),
      'message': entry.message,
      'tags': entry.tags.toList()..sort(),
      'metadata': entry.metadata,
      'occlude': entry.occlude,
    };
    
    const encoder = JsonEncoder.withIndent('  ');
    return encoder.convert(data);
  }

  /// Batch serialize entries with optional filtering
  static String serializeBatch(
    List<LogEntry> entries, {
    bool includeOccluded = true,
    Set<String>? sessionFilter,
    Set<String>? tagFilter,
  }) {
    var filteredEntries = entries;
    
    if (!includeOccluded) {
      filteredEntries = filteredEntries.where((e) => !e.occlude).toList();
    }
    
    if (sessionFilter != null) {
      filteredEntries = filteredEntries
        .where((e) => sessionFilter.contains(e.session))
        .toList();
    }
    
    if (tagFilter != null) {
      filteredEntries = filteredEntries
        .where((e) => e.tags.any(tagFilter.contains))
        .toList();
    }
    
    return serializeMultiple(filteredEntries);
  }
}
