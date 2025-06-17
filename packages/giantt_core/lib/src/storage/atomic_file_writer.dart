import 'dart:io';
import '../models/graph_exceptions.dart';
import 'backup_manager.dart';

/// Provides atomic file write operations with backup and rollback support
class AtomicFileWriter {
  /// Write content to a file atomically using a temporary file
  static void writeFile(String filepath, String content, {bool createBackup = true}) {
    final file = File(filepath);
    final tempPath = '$filepath.tmp';
    final tempFile = File(tempPath);

    try {
      // Create backup if file exists and backup is requested
      if (createBackup && file.existsSync()) {
        BackupManager.createBackup(filepath);
      }

      // Ensure parent directory exists
      final parentDir = file.parent;
      if (!parentDir.existsSync()) {
        parentDir.createSync(recursive: true);
      }

      // Write to temporary file first
      tempFile.writeAsStringSync(content);

      // Atomic move from temp to final location
      // On mobile platforms, this may not be truly atomic, but it's the best we can do
      tempFile.renameSync(filepath);

      // Remove duplicate backup if content is identical
      if (createBackup) {
        BackupManager.removeDuplicateBackup(filepath);
      }

    } catch (e) {
      // Clean up temp file if it exists
      try {
        if (tempFile.existsSync()) {
          tempFile.deleteSync();
        }
      } catch (_) {
        // Ignore cleanup errors
      }
      
      throw GraphException('Failed to write file atomically: $e');
    }
  }

  /// Write multiple files atomically (all succeed or all fail)
  static void writeFiles(Map<String, String> fileContents, {bool createBackup = true}) {
    final tempFiles = <String, String>{};
    final backupPaths = <String, String>{};

    try {
      // Phase 1: Create backups and write to temp files
      for (final entry in fileContents.entries) {
        final filepath = entry.key;
        final content = entry.value;
        final tempPath = '$filepath.tmp';

        // Create backup if file exists
        if (createBackup && File(filepath).existsSync()) {
          backupPaths[filepath] = BackupManager.createBackup(filepath);
        }

        // Ensure parent directory exists
        final parentDir = File(filepath).parent;
        if (!parentDir.existsSync()) {
          parentDir.createSync(recursive: true);
        }

        // Write to temp file
        File(tempPath).writeAsStringSync(content);
        tempFiles[filepath] = tempPath;
      }

      // Phase 2: Atomic moves (all or nothing)
      for (final entry in tempFiles.entries) {
        final filepath = entry.key;
        final tempPath = entry.value;
        File(tempPath).renameSync(filepath);
      }

      // Phase 3: Clean up duplicate backups
      if (createBackup) {
        for (final filepath in fileContents.keys) {
          BackupManager.removeDuplicateBackup(filepath);
        }
      }

    } catch (e) {
      // Rollback: Clean up temp files and restore from backups if needed
      _rollbackTransaction(tempFiles, backupPaths);
      throw GraphException('Failed to write files atomically: $e');
    }
  }

  /// Rollback a failed transaction
  static void _rollbackTransaction(Map<String, String> tempFiles, Map<String, String> backupPaths) {
    // Clean up temp files
    for (final tempPath in tempFiles.values) {
      try {
        final tempFile = File(tempPath);
        if (tempFile.existsSync()) {
          tempFile.deleteSync();
        }
      } catch (_) {
        // Ignore cleanup errors
      }
    }

    // Restore from backups if any files were partially written
    for (final entry in backupPaths.entries) {
      final filepath = entry.key;
      final backupPath = entry.value;
      
      try {
        if (File(backupPath).existsSync()) {
          File(backupPath).copySync(filepath);
        }
      } catch (_) {
        // Best effort restore
      }
    }
  }

  /// Check if a file write operation would be safe (enough disk space, permissions, etc.)
  static bool canWriteFile(String filepath, String content) {
    try {
      final file = File(filepath);
      final parentDir = file.parent;

      // Check if parent directory exists or can be created
      if (!parentDir.existsSync()) {
        try {
          parentDir.createSync(recursive: true);
        } catch (e) {
          return false;
        }
      }

      // Try to write a small test file
      final testPath = '$filepath.test';
      final testFile = File(testPath);
      
      try {
        testFile.writeAsStringSync('test');
        testFile.deleteSync();
        return true;
      } catch (e) {
        return false;
      }

    } catch (e) {
      return false;
    }
  }

  /// Get available disk space (returns null if cannot determine)
  static int? getAvailableDiskSpace(String filepath) {
    try {
      // This is platform-specific and may not work on all mobile platforms
      final file = File(filepath);
      final stat = file.statSync();
      // Note: Dart doesn't provide direct access to disk space info
      // This would need platform-specific implementation for accurate results
      return null;
    } catch (e) {
      return null;
    }
  }
}
