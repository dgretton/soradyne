import '../models/log_entry.dart';
import 'log_collection.dart';

/// Handles log occlusion operations
class LogOccluder {
  /// Occlude logs by session tags with optional dry-run
  static LogOccludeResult occludeBySession(
    LogCollection logs,
    List<String> sessionTags, {
    bool dryRun = false,
  }) {
    final toOcclude = <LogEntry>[];

    for (final log in logs.includedEntries) {
      if (sessionTags.contains(log.session)) {
        toOcclude.add(log);
      }
    }

    if (dryRun) {
      return LogOccludeResult(
        occludedCount: toOcclude.length,
        occludedLogs: toOcclude,
        dryRun: true,
      );
    }

    // Actually occlude the logs
    for (final log in toOcclude) {
      final occludedLog = log.setOcclude(true);
      logs.replaceEntry(log, occludedLog);
    }

    return LogOccludeResult(
      occludedCount: toOcclude.length,
      occludedLogs: toOcclude,
      dryRun: false,
    );
  }

  /// Occlude logs by tags with optional dry-run
  static LogOccludeResult occludeByTags(
    LogCollection logs,
    List<String> tags, {
    bool requireAll = false,
    bool dryRun = false,
  }) {
    final toOcclude = <LogEntry>[];

    for (final log in logs.includedEntries) {
      bool shouldOcclude = false;

      if (requireAll) {
        shouldOcclude = log.hasAllTags(tags);
      } else {
        shouldOcclude = log.hasAnyTags(tags);
      }

      if (shouldOcclude) {
        toOcclude.add(log);
      }
    }

    if (dryRun) {
      return LogOccludeResult(
        occludedCount: toOcclude.length,
        occludedLogs: toOcclude,
        dryRun: true,
      );
    }

    // Actually occlude the logs
    for (final log in toOcclude) {
      final occludedLog = log.setOcclude(true);
      logs.replaceEntry(log, occludedLog);
    }

    return LogOccludeResult(
      occludedCount: toOcclude.length,
      occludedLogs: toOcclude,
      dryRun: false,
    );
  }

  /// Occlude logs by date range with optional dry-run
  static LogOccludeResult occludeByDateRange(
    LogCollection logs,
    DateTime start,
    DateTime end, {
    bool dryRun = false,
  }) {
    final toOcclude = logs.includedEntries.where((log) {
      return log.timestamp.isAfter(start) && log.timestamp.isBefore(end);
    }).toList();

    if (dryRun) {
      return LogOccludeResult(
        occludedCount: toOcclude.length,
        occludedLogs: toOcclude,
        dryRun: true,
      );
    }

    // Actually occlude the logs
    for (final log in toOcclude) {
      final occludedLog = log.setOcclude(true);
      logs.replaceEntry(log, occludedLog);
    }

    return LogOccludeResult(
      occludedCount: toOcclude.length,
      occludedLogs: toOcclude,
      dryRun: false,
    );
  }

  /// Include (un-occlude) logs by session tags with optional dry-run
  static LogOccludeResult includeBySession(
    LogCollection logs,
    List<String> sessionTags, {
    bool dryRun = false,
  }) {
    final toInclude = <LogEntry>[];

    for (final log in logs.occludedEntries) {
      if (sessionTags.contains(log.session)) {
        toInclude.add(log);
      }
    }

    if (dryRun) {
      return LogOccludeResult(
        occludedCount: toInclude.length,
        occludedLogs: toInclude,
        dryRun: true,
      );
    }

    // Actually include the logs
    for (final log in toInclude) {
      final includedLog = log.setOcclude(false);
      logs.replaceEntry(log, includedLog);
    }

    return LogOccludeResult(
      occludedCount: toInclude.length,
      occludedLogs: toInclude,
      dryRun: false,
    );
  }

  /// Include logs by tags with optional dry-run
  static LogOccludeResult includeByTags(
    LogCollection logs,
    List<String> tags, {
    bool requireAll = false,
    bool dryRun = false,
  }) {
    final toInclude = <LogEntry>[];

    for (final log in logs.occludedEntries) {
      bool shouldInclude = false;

      if (requireAll) {
        shouldInclude = log.hasAllTags(tags);
      } else {
        shouldInclude = log.hasAnyTags(tags);
      }

      if (shouldInclude) {
        toInclude.add(log);
      }
    }

    if (dryRun) {
      return LogOccludeResult(
        occludedCount: toInclude.length,
        occludedLogs: toInclude,
        dryRun: true,
      );
    }

    // Actually include the logs
    for (final log in toInclude) {
      final includedLog = log.setOcclude(false);
      logs.replaceEntry(log, includedLog);
    }

    return LogOccludeResult(
      occludedCount: toInclude.length,
      occludedLogs: toInclude,
      dryRun: false,
    );
  }

  /// Get statistics about what would be occluded (dry-run analysis)
  static OcclusionAnalysis analyzeOcclusion(
    LogCollection logs, {
    List<String>? sessionTags,
    List<String>? tags,
    DateTime? startDate,
    DateTime? endDate,
    bool requireAllTags = false,
  }) {
    var candidateEntries = logs.includedEntries;

    // Apply session filter
    if (sessionTags != null && sessionTags.isNotEmpty) {
      candidateEntries = candidateEntries
        .where((log) => sessionTags.contains(log.session))
        .toList();
    }

    // Apply tag filter
    if (tags != null && tags.isNotEmpty) {
      candidateEntries = candidateEntries.where((log) {
        return requireAllTags ? log.hasAllTags(tags) : log.hasAnyTags(tags);
      }).toList();
    }

    // Apply date filter
    if (startDate != null || endDate != null) {
      candidateEntries = candidateEntries.where((log) {
        if (startDate != null && log.timestamp.isBefore(startDate)) return false;
        if (endDate != null && log.timestamp.isAfter(endDate)) return false;
        return true;
      }).toList();
    }

    // Analyze the results
    final sessionCounts = <String, int>{};
    final tagCounts = <String, int>{};
    
    for (final entry in candidateEntries) {
      sessionCounts[entry.session] = (sessionCounts[entry.session] ?? 0) + 1;
      for (final tag in entry.tags) {
        tagCounts[tag] = (tagCounts[tag] ?? 0) + 1;
      }
    }

    return OcclusionAnalysis(
      totalCandidates: candidateEntries.length,
      sessionBreakdown: sessionCounts,
      tagBreakdown: tagCounts,
      dateRange: candidateEntries.isNotEmpty ? DateRange(
        start: candidateEntries.map((e) => e.timestamp).reduce((a, b) => a.isBefore(b) ? a : b),
        end: candidateEntries.map((e) => e.timestamp).reduce((a, b) => a.isAfter(b) ? a : b),
      ) : null,
    );
  }
}

/// Result of a log occlude operation
class LogOccludeResult {
  const LogOccludeResult({
    required this.occludedCount,
    required this.occludedLogs,
    required this.dryRun,
  });

  final int occludedCount;
  final List<LogEntry> occludedLogs;
  final bool dryRun;

  bool get hasOccluded => occludedCount > 0;
  
  @override
  String toString() {
    final action = dryRun ? 'Would occlude' : 'Occluded';
    return '$action $occludedCount log entries';
  }
}

/// Analysis of what would be occluded
class OcclusionAnalysis {
  const OcclusionAnalysis({
    required this.totalCandidates,
    required this.sessionBreakdown,
    required this.tagBreakdown,
    this.dateRange,
  });

  final int totalCandidates;
  final Map<String, int> sessionBreakdown;
  final Map<String, int> tagBreakdown;
  final DateRange? dateRange;

  @override
  String toString() {
    final buffer = StringBuffer();
    buffer.writeln('Occlusion Analysis:');
    buffer.writeln('  Total candidates: $totalCandidates');
    
    if (sessionBreakdown.isNotEmpty) {
      buffer.writeln('  By session:');
      for (final entry in sessionBreakdown.entries) {
        buffer.writeln('    ${entry.key}: ${entry.value}');
      }
    }
    
    if (tagBreakdown.isNotEmpty) {
      buffer.writeln('  By tag:');
      for (final entry in tagBreakdown.entries) {
        buffer.writeln('    ${entry.key}: ${entry.value}');
      }
    }
    
    if (dateRange != null) {
      buffer.writeln('  Date range: ${dateRange!.start} to ${dateRange!.end}');
    }
    
    return buffer.toString();
  }
}

/// Represents a date range
class DateRange {
  const DateRange({required this.start, required this.end});
  
  final DateTime start;
  final DateTime end;
  
  Duration get duration => end.difference(start);
}
