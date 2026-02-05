import 'dart:io';

/// Resolves file paths for Giantt files with mobile-friendly fallbacks
class PathResolver {
  /// Get the default Giantt directory path
  static String getDefaultGianttDirectory({bool dev = false}) {
    if (dev) {
      // Use local directory in dev mode
      return '${Directory.current.path}${Platform.pathSeparator}.giantt';
    } else {
      // Use home directory in normal mode, with mobile-friendly fallback
      try {
        final homeDir = _getHomeDirectory();
        return '$homeDir${Platform.pathSeparator}.giantt';
      } catch (e) {
        // Fallback to current directory if home directory is not accessible
        return '${Directory.current.path}${Platform.pathSeparator}.giantt';
      }
    }
  }

  /// Get the home directory with mobile platform considerations
  static String _getHomeDirectory() {
    // Try environment variables first
    final home = Platform.environment['HOME'] ?? 
                 Platform.environment['USERPROFILE'] ?? 
                 Platform.environment['HOMEPATH'];
    
    if (home != null && home.isNotEmpty) {
      return home;
    }

    // Platform-specific fallbacks
    if (Platform.isWindows) {
      final userProfile = Platform.environment['USERPROFILE'];
      if (userProfile != null) return userProfile;
      
      final homeDrive = Platform.environment['HOMEDRIVE'] ?? 'C:';
      final homePath = Platform.environment['HOMEPATH'] ?? '\\Users\\Default';
      return '$homeDrive$homePath';
    }

    if (Platform.isLinux || Platform.isMacOS) {
      // Try common Unix home directory patterns
      final user = Platform.environment['USER'] ?? Platform.environment['USERNAME'];
      if (user != null) {
        final homeDir = '/home/$user';
        if (Directory(homeDir).existsSync()) {
          return homeDir;
        }
      }
    }

    // Mobile platform fallbacks
    if (Platform.isAndroid || Platform.isIOS) {
      // On mobile, we might not have access to a traditional home directory
      // Fall back to the app's documents directory or current directory
      return Directory.current.path;
    }

    // Last resort fallback
    return Directory.current.path;
  }

  /// Get the default path for a Giantt file
  static String getDefaultGianttPath(String filename, {bool occlude = false, bool dev = false}) {
    final baseDir = getDefaultGianttDirectory(dev: dev);
    final subDir = occlude ? 'occlude' : 'include';
    return '$baseDir${Platform.pathSeparator}$subDir${Platform.pathSeparator}$filename';
  }

  /// Check if a Giantt workspace exists at the given path
  static bool gianttWorkspaceExists(String basePath) {
    final includeDir = Directory('$basePath${Platform.pathSeparator}include');
    final occludeDir = Directory('$basePath${Platform.pathSeparator}occlude');
    
    return includeDir.existsSync() && occludeDir.existsSync();
  }

  /// Find the nearest Giantt workspace by walking up the directory tree
  static String? findNearestGianttWorkspace([String? startPath]) {
    startPath ??= Directory.current.path;
    
    var currentDir = Directory(startPath);
    
    while (true) {
      final gianttDir = '${currentDir.path}${Platform.pathSeparator}.giantt';
      if (gianttWorkspaceExists(gianttDir)) {
        return gianttDir;
      }
      
      final parentDir = currentDir.parent;
      if (parentDir.path == currentDir.path) {
        // Reached root directory
        break;
      }
      currentDir = parentDir;
    }
    
    return null;
  }

  /// Get the active Giantt workspace path (local dev > nearest > home)
  static String getActiveGianttWorkspace() {
    // First check for local dev workspace
    final localDevPath = getDefaultGianttDirectory(dev: true);
    if (gianttWorkspaceExists(localDevPath)) {
      return localDevPath;
    }
    
    // Then check for nearest workspace
    final nearestPath = findNearestGianttWorkspace();
    if (nearestPath != null) {
      return nearestPath;
    }
    
    // Finally fall back to home directory
    return getDefaultGianttDirectory(dev: false);
  }

  /// Resolve a relative path against a base path
  static String resolvePath(String basePath, String relativePath) {
    if (isAbsolutePath(relativePath)) {
      return relativePath;
    }
    
    // Handle parent directory references
    final baseParts = basePath.split(Platform.pathSeparator);
    final relativeParts = relativePath.split('/'); // Always use forward slash for relative paths
    
    final resolvedParts = List<String>.from(baseParts);
    
    for (final part in relativeParts) {
      if (part == '..') {
        if (resolvedParts.isNotEmpty) {
          resolvedParts.removeLast();
        }
      } else if (part != '.' && part.isNotEmpty) {
        resolvedParts.add(part);
      }
    }
    
    return resolvedParts.join(Platform.pathSeparator);
  }

  /// Check if a path is absolute
  static bool isAbsolutePath(String path) {
    if (Platform.isWindows) {
      // Windows: C:\ or \\server\share
      return path.length >= 2 && 
             ((path[1] == ':') || (path.startsWith(r'\\')));
    } else {
      // Unix-like: starts with /
      return path.startsWith('/');
    }
  }

  /// Normalize a path for the current platform
  static String normalizePath(String path) {
    // Replace forward slashes with platform separator
    if (Platform.pathSeparator != '/') {
      path = path.replaceAll('/', Platform.pathSeparator);
    }
    
    // Remove duplicate separators
    final separator = Platform.pathSeparator;
    while (path.contains('$separator$separator')) {
      path = path.replaceAll('$separator$separator', separator);
    }
    
    return path;
  }

  /// Get the directory portion of a file path
  static String _getDirectoryPath(String filepath) {
    final lastSeparator = filepath.lastIndexOf(Platform.pathSeparator);
    if (lastSeparator == -1) {
      // Also check for forward slash in case of cross-platform paths
      final lastForwardSlash = filepath.lastIndexOf('/');
      if (lastForwardSlash == -1) {
        return '.';
      }
      return filepath.substring(0, lastForwardSlash);
    }
    return filepath.substring(0, lastSeparator);
  }

  /// Get the directory portion of a file path (public version)
  static String getDirectoryPath(String filepath) => _getDirectoryPath(filepath);

  /// Get a safe filename by removing invalid characters
  static String getSafeFilename(String filename) {
    // Remove or replace characters that are invalid on various platforms
    final invalidChars = RegExp(r'[<>:"/\\|?*\x00-\x1f]');
    return filename.replaceAll(invalidChars, '_');
  }

  /// Create directory structure if it doesn't exist
  static void ensureDirectoryExists(String dirPath) {
    final directory = Directory(dirPath);
    if (!directory.existsSync()) {
      try {
        directory.createSync(recursive: true);
      } catch (e) {
        throw Exception('Failed to create directory $dirPath: $e');
      }
    }
  }

  /// Get the relative path from one directory to another
  static String getRelativePath(String from, String to) {
    final fromParts = from.split(Platform.pathSeparator);
    final toParts = to.split(Platform.pathSeparator);
    
    // Find common prefix
    int commonLength = 0;
    final minLength = fromParts.length < toParts.length ? fromParts.length : toParts.length;
    
    for (int i = 0; i < minLength; i++) {
      if (fromParts[i] == toParts[i]) {
        commonLength++;
      } else {
        break;
      }
    }
    
    // Build relative path
    final relativeParts = <String>[];
    
    // Add .. for each remaining part in from
    for (int i = commonLength; i < fromParts.length; i++) {
      relativeParts.add('..');
    }
    
    // Add remaining parts from to
    for (int i = commonLength; i < toParts.length; i++) {
      relativeParts.add(toParts[i]);
    }
    
    return relativeParts.isEmpty ? '.' : relativeParts.join('/');
  }
}
