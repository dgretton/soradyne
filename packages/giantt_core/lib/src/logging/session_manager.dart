import '../models/log_entry.dart';
import 'log_collection.dart';

/// Manages session tags and session-based operations
class SessionManager {
  final LogCollection _logs;
  
  SessionManager(this._logs);

  /// Get all unique session tags
  Set<String> getAllSessions() {
    return _logs.entries.map((entry) => entry.session).toSet();
  }

  /// Get all entries for a specific session
  List<LogEntry> getSessionEntries(String sessionTag) {
    return _logs.getBySession(sessionTag);
  }

  /// Get the most recent session tag
  String? getMostRecentSession() {
    if (_logs.isEmpty) return null;
    return _logs.entries.last.session;
  }

  /// Get the oldest session tag
  String? getOldestSession() {
    if (_logs.isEmpty) return null;
    return _logs.entries.first.session;
  }

  /// Get sessions within a date range
  Set<String> getSessionsInDateRange(DateTime start, [DateTime? end]) {
    final entriesInRange = _logs.getByDateRange(start, end);
    return entriesInRange.map((entry) => entry.session).toSet();
  }

  /// Get session statistics
  Map<String, SessionStats> getSessionStats() {
    final stats = <String, SessionStats>{};
    
    for (final entry in _logs.entries) {
      final session = entry.session;
      if (!stats.containsKey(session)) {
        stats[session] = SessionStats(
          sessionTag: session,
          entryCount: 0,
          firstEntry: entry.timestamp,
          lastEntry: entry.timestamp,
          totalTags: <String>{},
        );
      }
      
      final sessionStats = stats[session]!;
      stats[session] = SessionStats(
        sessionTag: session,
        entryCount: sessionStats.entryCount + 1,
        firstEntry: entry.timestamp.isBefore(sessionStats.firstEntry) 
          ? entry.timestamp 
          : sessionStats.firstEntry,
        lastEntry: entry.timestamp.isAfter(sessionStats.lastEntry) 
          ? entry.timestamp 
          : sessionStats.lastEntry,
        totalTags: sessionStats.totalTags..addAll(entry.tags),
      );
    }
    
    return stats;
  }

  /// Generate a new session tag based on current timestamp
  static String generateSessionTag([String? prefix]) {
    final now = DateTime.now().toUtc();
    final timestamp = now.toIso8601String().replaceAll(RegExp(r'[:\-.]'), '');
    return prefix != null ? '${prefix}_$timestamp' : 'session_$timestamp';
  }

  /// Check if a session exists
  bool sessionExists(String sessionTag) {
    return _logs.entries.any((entry) => entry.session == sessionTag);
  }

  /// Get the duration of a session (time between first and last entry)
  Duration? getSessionDuration(String sessionTag) {
    final sessionEntries = getSessionEntries(sessionTag);
    if (sessionEntries.isEmpty) return null;
    if (sessionEntries.length == 1) return Duration.zero;
    
    sessionEntries.sort((a, b) => a.timestamp.compareTo(b.timestamp));
    return sessionEntries.last.timestamp.difference(sessionEntries.first.timestamp);
  }

  /// Get sessions that contain specific tags
  Set<String> getSessionsWithTags(List<String> tags, {bool requireAll = false}) {
    final matchingSessions = <String>{};
    
    for (final session in getAllSessions()) {
      final sessionEntries = getSessionEntries(session);
      final sessionTags = sessionEntries
        .expand((entry) => entry.tags)
        .toSet();
      
      if (requireAll) {
        if (tags.every(sessionTags.contains)) {
          matchingSessions.add(session);
        }
      } else {
        if (tags.any(sessionTags.contains)) {
          matchingSessions.add(session);
        }
      }
    }
    
    return matchingSessions;
  }

  /// Get the most active sessions (by entry count)
  List<String> getMostActiveSessions([int limit = 10]) {
    final stats = getSessionStats();
    final sortedSessions = stats.entries.toList()
      ..sort((a, b) => b.value.entryCount.compareTo(a.value.entryCount));
    
    return sortedSessions
      .take(limit)
      .map((entry) => entry.key)
      .toList();
  }
}

/// Statistics for a session
class SessionStats {
  const SessionStats({
    required this.sessionTag,
    required this.entryCount,
    required this.firstEntry,
    required this.lastEntry,
    required this.totalTags,
  });

  final String sessionTag;
  final int entryCount;
  final DateTime firstEntry;
  final DateTime lastEntry;
  final Set<String> totalTags;

  Duration get duration => lastEntry.difference(firstEntry);
  
  @override
  String toString() {
    return 'SessionStats(session: $sessionTag, entries: $entryCount, '
           'duration: $duration, tags: ${totalTags.length})';
  }
}
