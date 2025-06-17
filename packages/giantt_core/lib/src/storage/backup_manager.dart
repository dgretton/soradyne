import 'dart:io';
import '../models/graph_exceptions.dart';

/// Manages backup files with configurable retention and cleanup
class BackupManager {
  /// Default number of backups to keep
  static const int defaultRetentionCount = 3;

  /// Create a backup of a file with incremental naming
  static String createBackup(String filepath, {int? retentionCount}) {
    retentionCount ??= defaultRetentionCount;
    
    final file = File(filepath);
    if (!file.existsSync()) {
      throw GraphException('Cannot backup non-existent file: $filepath');
    }

    final backupPath = _getNextBackupPath(filepath);
    
    try {
      file.copySync(backupPath);
      _cleanupOldBackups(filepath, retentionCount);
      return backupPath;
    } catch (e) {
      throw GraphException('Failed to create backup: $e');
    }
  }

  /// Get the next available backup path with incremental numbering
  static String _getNextBackupPath(String filepath) {
    int backupNum = 1;
    String backupPath;
    
    do {
      backupPath = '$filepath.$backupNum.backup';
      backupNum++;
    } while (File(backupPath).existsSync());
    
    return backupPath;
  }

  /// Get the most recent backup path for a file
  static String? getMostRecentBackup(String filepath) {
    final directory = Directory(_getDirectoryPath(filepath));
    if (!directory.existsSync()) return null;

    final filename = _getFileName(filepath);
    final backupPattern = RegExp(r'^' + RegExp.escape(filename) + r'\.(\d+)\.backup$');
    
    int highestNum = 0;
    String? mostRecentBackup;
    
    try {
      for (final entity in directory.listSync()) {
        if (entity is File) {
          final match = backupPattern.firstMatch(_getFileName(entity.path));
          if (match != null) {
            final num = int.parse(match.group(1)!);
            if (num > highestNum) {
              highestNum = num;
              mostRecentBackup = entity.path;
            }
          }
        }
      }
    } catch (e) {
      // Ignore permission errors on mobile platforms
      return null;
    }
    
    return mostRecentBackup;
  }

  /// Clean up old backups, keeping only the most recent ones
  static void _cleanupOldBackups(String filepath, int retentionCount) {
    final directory = Directory(_getDirectoryPath(filepath));
    if (!directory.existsSync()) return;

    final filename = _getFileName(filepath);
    final backupPattern = RegExp(r'^' + RegExp.escape(filename) + r'\.(\d+)\.backup$');
    
    final backups = <int, String>{};
    
    try {
      for (final entity in directory.listSync()) {
        if (entity is File) {
          final match = backupPattern.firstMatch(_getFileName(entity.path));
          if (match != null) {
            final num = int.parse(match.group(1)!);
            backups[num] = entity.path;
          }
        }
      }
    } catch (e) {
      // Ignore permission errors on mobile platforms
      return;
    }

    // Sort backup numbers in descending order (newest first)
    final sortedNums = backups.keys.toList()..sort((a, b) => b.compareTo(a));
    
    // Delete backups beyond retention count
    for (int i = retentionCount; i < sortedNums.length; i++) {
      final backupPath = backups[sortedNums[i]]!;
      try {
        File(backupPath).deleteSync();
      } catch (e) {
        // Ignore deletion errors (may be permission issues on mobile)
      }
    }
  }

  /// Check if the most recent backup is identical to the current file
  static bool isIdenticalToMostRecentBackup(String filepath) {
    final mostRecentBackup = getMostRecentBackup(filepath);
    if (mostRecentBackup == null) return false;

    try {
      final currentContent = File(filepath).readAsStringSync();
      final backupContent = File(mostRecentBackup).readAsStringSync();
      return currentContent == backupContent;
    } catch (e) {
      return false;
    }
  }

  /// Remove the most recent backup if it's identical to the current file
  static void removeDuplicateBackup(String filepath) {
    if (isIdenticalToMostRecentBackup(filepath)) {
      final mostRecentBackup = getMostRecentBackup(filepath);
      if (mostRecentBackup != null) {
        try {
          File(mostRecentBackup).deleteSync();
        } catch (e) {
          // Ignore deletion errors
        }
      }
    }
  }

  /// Get all backup files for a given file
  static List<String> getAllBackups(String filepath) {
    final directory = Directory(_getDirectoryPath(filepath));
    if (!directory.existsSync()) return [];

    final filename = _getFileName(filepath);
    final backupPattern = RegExp(r'^' + RegExp.escape(filename) + r'\.(\d+)\.backup$');
    
    final backups = <int, String>{};
    
    try {
      for (final entity in directory.listSync()) {
        if (entity is File) {
          final match = backupPattern.firstMatch(_getFileName(entity.path));
          if (match != null) {
            final num = int.parse(match.group(1)!);
            backups[num] = entity.path;
          }
        }
      }
    } catch (e) {
      return [];
    }

    // Sort by backup number (oldest first)
    final sortedNums = backups.keys.toList()..sort();
    return sortedNums.map((num) => backups[num]!).toList();
  }

  /// Clean up all backups for multiple files, keeping specified retention
  static void cleanupAllBackups(List<String> filepaths, {int? retentionCount}) {
    retentionCount ??= defaultRetentionCount;
    
    for (final filepath in filepaths) {
      try {
        _cleanupOldBackups(filepath, retentionCount);
      } catch (e) {
        // Continue with other files if one fails
      }
    }
  }

  /// Get directory path from file path
  static String _getDirectoryPath(String filepath) {
    final lastSeparator = filepath.lastIndexOf(Platform.pathSeparator);
    if (lastSeparator == -1) {
      return '.'; // Current directory
    }
    return filepath.substring(0, lastSeparator);
  }

  /// Get filename from file path
  static String _getFileName(String filepath) {
    final lastSeparator = filepath.lastIndexOf(Platform.pathSeparator);
    if (lastSeparator == -1) {
      return filepath;
    }
    return filepath.substring(lastSeparator + 1);
  }
}
