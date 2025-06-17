import '../models/log_entry.dart';

/// A collection of log entries with query capabilities
class LogCollection {
  final List<LogEntry> _entries = [];

  /// Create an empty log collection
  LogCollection();

  /// Create a log collection with initial entries
  LogCollection.fromEntries(List<LogEntry> entries) {
    _entries.addAll(entries);
    _sortEntries();
  }

  /// Get all entries
  List<LogEntry> get entries => List.unmodifiable(_entries);

  /// Get entries that are not occluded
  List<LogEntry> get includedEntries {
    return _entries.where((entry) => !entry.occlude).toList();
  }

  /// Get entries that are occluded
  List<LogEntry> get occludedEntries {
    return _entries.where((entry) => entry.occlude).toList();
  }

  /// Add a single entry
  void addEntry(LogEntry entry) {
    final index = _getInsertionIndex(entry.timestamp);
    _entries.insert(index, entry);
  }

  /// Add multiple entries
  void addEntries(List<LogEntry> entries) {
    _entries.addAll(entries);
    _sortEntries();
  }

  /// Replace an entry with a new one
  void replaceEntry(LogEntry oldEntry, LogEntry newEntry) {
    final index = _entries.indexOf(oldEntry);
    if (index != -1) {
      _entries[index] = newEntry;
      _sortEntries();
    }
  }

  /// Create and add a new entry
  LogEntry createEntry(
    String sessionTag,
    String message, {
    List<String>? additionalTags,
    Map<String, String>? metadata,
    bool occlude = false,
  }) {
    final entry = LogEntry.create(
      sessionTag,
      message,
      additionalTags: additionalTags,
      metadata: metadata,
      occlude: occlude,
    );
    addEntry(entry);
    return entry;
  }

  /// Get all entries with a specific session tag
  List<LogEntry> getBySession(String sessionTag) {
    return _entries.where((entry) => entry.session == sessionTag).toList();
  }

  /// Get entries with specified tags
  List<LogEntry> getByTags(List<String> tags, {bool requireAll = false}) {
    if (requireAll) {
      return _entries.where((entry) => entry.hasAllTags(tags)).toList();
    }
    return _entries.where((entry) => entry.hasAnyTags(tags)).toList();
  }

  /// Get entries within a date range
  List<LogEntry> getByDateRange(DateTime start, [DateTime? end]) {
    final endDate = end ?? DateTime.now().toUtc();
    return _entries.where((entry) {
      return entry.timestamp.isAfter(start) && entry.timestamp.isBefore(endDate);
    }).toList();
  }

  /// Get entries with a specific substring in the message
  List<LogEntry> getBySubstring(String substring) {
    final lowerSubstring = substring.toLowerCase();
    return _entries.where((entry) {
      return entry.message.toLowerCase().contains(lowerSubstring);
    }).toList();
  }

  /// Get the index where a new entry with the given timestamp should be inserted
  int _getInsertionIndex(DateTime timestamp) {
    if (_entries.isEmpty) return 0;
    if (timestamp.isAfter(_entries.last.timestamp)) return _entries.length;
    if (timestamp.isBefore(_entries.first.timestamp)) return 0;

    int low = 0;
    int high = _entries.length - 1;
    
    while (low <= high) {
      final mid = (low + high) ~/ 2;
      final midTimestamp = _entries[mid].timestamp;
      
      if (midTimestamp.isBefore(timestamp)) {
        low = mid + 1;
      } else {
        high = mid - 1;
      }
    }
    
    return low;
  }

  /// Sort entries by timestamp
  void _sortEntries() {
    _entries.sort((a, b) => a.timestamp.compareTo(b.timestamp));
  }

  /// Get the first index after a timestamp (for binary search)
  int getFirstIndexAfterTimestamp(DateTime timestamp) {
    if (_entries.isEmpty) return 0;
    if (timestamp.isAfter(_entries.last.timestamp)) return _entries.length - 1;
    if (timestamp.isBefore(_entries.first.timestamp)) return 0;

    int low = 0;
    int high = _entries.length - 1;
    
    while (low < high) {
      final mid = (low + high) ~/ 2;
      if (_entries[mid].timestamp.isBefore(timestamp)) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }
    
    return low;
  }

  /// Clear all entries
  void clear() {
    _entries.clear();
  }

  /// Get the number of entries
  int get length => _entries.length;

  /// Check if the collection is empty
  bool get isEmpty => _entries.isEmpty;

  /// Check if the collection is not empty
  bool get isNotEmpty => _entries.isNotEmpty;

  /// Iterator support
  Iterator<LogEntry> get iterator => _entries.iterator;
}
