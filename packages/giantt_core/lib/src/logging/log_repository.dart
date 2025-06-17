import 'dart:io';
import '../models/log_entry.dart';
import '../models/graph_exceptions.dart';
import 'log_collection.dart';
import '../storage/file_header_generator.dart';
import '../storage/atomic_file_writer.dart';

/// Repository for log file I/O operations
class LogRepository {
  /// Load logs from a single file
  static LogCollection loadFromFile(String filepath, {bool occlude = false}) {
    final logs = <LogEntry>[];
    
    try {
      final file = File(filepath);
      if (!file.existsSync()) {
        return LogCollection.fromEntries(logs);
      }

      final lines = file.readAsLinesSync();
      for (final line in lines) {
        final trimmed = line.trim();
        if (trimmed.isNotEmpty && !trimmed.startsWith('#')) {
          try {
            final log = LogEntry.fromJsonLine(trimmed, occlude: occlude);
            logs.add(log);
          } catch (e) {
            // Skip invalid lines with warning
            print('Warning: Skipping invalid log line in $filepath: $e');
          }
        }
      }
    } catch (e) {
      throw GraphException('Error loading logs from $filepath: $e');
    }
    
    return LogCollection.fromEntries(logs);
  }

  /// Load logs from both include and occlude files
  static LogCollection loadDualFile(String includePath, String occludePath) {
    final includeCollection = loadFromFile(includePath, occlude: false);
    final occludeCollection = loadFromFile(occludePath, occlude: true);
    
    final allLogs = LogCollection();
    allLogs.addEntries(includeCollection.entries);
    allLogs.addEntries(occludeCollection.entries);
    
    return allLogs;
  }

  /// Save logs to a single file
  static void saveToFile(String filepath, LogCollection logs, {bool includeHeader = true}) {
    try {
      final content = StringBuffer();
      
      if (includeHeader) {
        content.writeln(FileHeaderGenerator.generateLogsFileHeader());
        content.writeln();
      }

      for (final log in logs.entries) {
        content.writeln(log.toJsonLine());
      }

      final fileContents = {filepath: content.toString()};
      AtomicFileWriter.writeFiles(fileContents);
    } catch (e) {
      throw GraphException('Failed to save logs to $filepath: $e');
    }
  }

  /// Save logs to both include and occlude files
  static void saveDualFile(String includePath, String occludePath, LogCollection logs) {
    try {
      final includeContent = StringBuffer();
      final occludeContent = StringBuffer();

      // Add headers
      includeContent.writeln(FileHeaderGenerator.generateLogsFileHeader());
      includeContent.writeln();
      
      occludeContent.writeln(FileHeaderGenerator.generateOccludedLogsFileHeader());
      occludeContent.writeln();

      // Add logs to appropriate files
      for (final log in logs.entries) {
        final logLine = log.toJsonLine();
        if (log.occlude) {
          occludeContent.writeln(logLine);
        } else {
          includeContent.writeln(logLine);
        }
      }

      // Write both files atomically
      final fileContents = {
        includePath: includeContent.toString(),
        occludePath: occludeContent.toString(),
      };

      AtomicFileWriter.writeFiles(fileContents);
    } catch (e) {
      throw GraphException('Failed to save logs: $e');
    }
  }

  /// Append a single log entry to a file
  static void appendToFile(String filepath, LogEntry entry) {
    try {
      final file = File(filepath);
      final logLine = entry.toJsonLine();
      
      if (file.existsSync()) {
        file.writeAsStringSync('$logLine\n', mode: FileMode.append);
      } else {
        // Create new file with header
        final content = StringBuffer();
        content.writeln(FileHeaderGenerator.generateLogsFileHeader());
        content.writeln();
        content.writeln(logLine);
        file.writeAsStringSync(content.toString());
      }
    } catch (e) {
      throw GraphException('Failed to append log to $filepath: $e');
    }
  }

  /// Check if a log file exists
  static bool fileExists(String filepath) {
    return File(filepath).existsSync();
  }

  /// Get the size of a log file in bytes
  static int getFileSize(String filepath) {
    final file = File(filepath);
    return file.existsSync() ? file.lengthSync() : 0;
  }

  /// Get the number of log entries in a file (approximate, based on line count)
  static int getEntryCount(String filepath) {
    try {
      final file = File(filepath);
      if (!file.existsSync()) return 0;
      
      final lines = file.readAsLinesSync();
      return lines.where((line) => 
        line.trim().isNotEmpty && !line.trim().startsWith('#')
      ).length;
    } catch (e) {
      return 0;
    }
  }
}
