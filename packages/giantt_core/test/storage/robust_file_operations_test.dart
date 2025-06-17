import 'dart:io';
import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

void main() {
  group('Robust File Operations Tests', () {
    late Directory tempDir;
    
    setUp(() async {
      tempDir = await Directory.systemTemp.createTemp('giantt_robust_test_');
    });
    
    tearDown(() async {
      if (tempDir.existsSync()) {
        await tempDir.delete(recursive: true);
      }
    });

    group('BackupManager', () {
      test('should create backup with incremental naming', () async {
        final testFile = File('${tempDir.path}/test.txt');
        await testFile.writeAsString('original content');

        final backupPath = BackupManager.createBackup(testFile.path);
        
        expect(File(backupPath).existsSync(), isTrue);
        expect(backupPath, endsWith('.1.backup'));
        expect(await File(backupPath).readAsString(), equals('original content'));
      });

      test('should create multiple backups with different numbers', () async {
        final testFile = File('${tempDir.path}/test.txt');
        await testFile.writeAsString('content 1');

        final backup1 = BackupManager.createBackup(testFile.path);
        
        await testFile.writeAsString('content 2');
        final backup2 = BackupManager.createBackup(testFile.path);

        expect(backup1, endsWith('.1.backup'));
        expect(backup2, endsWith('.2.backup'));
        expect(await File(backup1).readAsString(), equals('content 1'));
        expect(await File(backup2).readAsString(), equals('content 2'));
      });

      test('should clean up old backups beyond retention count', () async {
        final testFile = File('${tempDir.path}/test.txt');
        
        // Create 5 backups
        for (int i = 1; i <= 5; i++) {
          await testFile.writeAsString('content $i');
          BackupManager.createBackup(testFile.path, retentionCount: 3);
        }

        final allBackups = BackupManager.getAllBackups(testFile.path);
        expect(allBackups.length, equals(3));
      });

      test('should find most recent backup', () async {
        final testFile = File('${tempDir.path}/test.txt');
        await testFile.writeAsString('content 1');
        BackupManager.createBackup(testFile.path);
        
        await testFile.writeAsString('content 2');
        BackupManager.createBackup(testFile.path);

        final mostRecent = BackupManager.getMostRecentBackup(testFile.path);
        expect(mostRecent, isNotNull);
        expect(mostRecent, endsWith('.2.backup'));
      });

      test('should detect identical backup and remove it', () async {
        final testFile = File('${tempDir.path}/test.txt');
        await testFile.writeAsString('same content');
        
        BackupManager.createBackup(testFile.path);
        
        // Write same content again
        await testFile.writeAsString('same content');
        
        expect(BackupManager.isIdenticalToMostRecentBackup(testFile.path), isTrue);
        
        BackupManager.removeDuplicateBackup(testFile.path);
        expect(BackupManager.getMostRecentBackup(testFile.path), isNull);
      });
    });

    group('AtomicFileWriter', () {
      test('should write file atomically', () async {
        final testFile = '${tempDir.path}/atomic_test.txt';
        const content = 'atomic content';

        AtomicFileWriter.writeFile(testFile, content);
        
        expect(File(testFile).existsSync(), isTrue);
        expect(await File(testFile).readAsString(), equals(content));
      });

      test('should create backup before writing', () async {
        final testFile = File('${tempDir.path}/backup_test.txt');
        await testFile.writeAsString('original');

        AtomicFileWriter.writeFile(testFile.path, 'updated');
        
        final backup = BackupManager.getMostRecentBackup(testFile.path);
        expect(backup, isNotNull);
        expect(await File(backup!).readAsString(), equals('original'));
        expect(await testFile.readAsString(), equals('updated'));
      });

      test('should write multiple files atomically', () async {
        final file1 = '${tempDir.path}/file1.txt';
        final file2 = '${tempDir.path}/file2.txt';
        
        final contents = {
          file1: 'content 1',
          file2: 'content 2',
        };

        AtomicFileWriter.writeFiles(contents);
        
        expect(await File(file1).readAsString(), equals('content 1'));
        expect(await File(file2).readAsString(), equals('content 2'));
      });

      test('should rollback on failure in multi-file write', () async {
        final file1 = File('${tempDir.path}/file1.txt');
        await file1.writeAsString('original 1');
        
        final file2 = File('${tempDir.path}/file2.txt');
        await file2.writeAsString('original 2');

        // Create a scenario that might fail (invalid path)
        final contents = {
          file1.path: 'new content 1',
          '${tempDir.path}/invalid\x00path/file.txt': 'should fail',
        };

        expect(() => AtomicFileWriter.writeFiles(contents), throwsA(isA<GraphException>()));
        
        // Original files should be unchanged
        expect(await file1.readAsString(), equals('original 1'));
        expect(await file2.readAsString(), equals('original 2'));
      });

      test('should check if file write is safe', () {
        final testFile = '${tempDir.path}/safe_test.txt';
        expect(AtomicFileWriter.canWriteFile(testFile, 'test content'), isTrue);
        
        // Test with invalid path (if possible on current platform)
        expect(AtomicFileWriter.canWriteFile('/invalid/path/file.txt', 'test'), isFalse);
      });
    });

    group('FileHeaderGenerator', () {
      test('should create banner with proper formatting', () {
        final banner = FileHeaderGenerator.createBanner('Test\nBanner');
        
        expect(banner, contains('Test'));
        expect(banner, contains('Banner'));
        expect(banner, startsWith('#'));
        expect(banner, endsWith('#\n'));
      });

      test('should generate items file header', () {
        final header = FileHeaderGenerator.generateItemsFileHeader();
        
        expect(header, contains('Giantt Items'));
        expect(header, contains('topological'));
        expect(header, contains('#include'));
      });

      test('should generate occluded items header', () {
        final header = FileHeaderGenerator.generateOccludedItemsFileHeader();
        
        expect(header, contains('Occluded Items'));
        expect(header, contains('topological'));
      });

      test('should generate custom header', () {
        final header = FileHeaderGenerator.generateCustomHeader('Custom Title', 'Custom description');
        
        expect(header, contains('Custom Title'));
        expect(header, contains('Custom description'));
      });

      test('should generate include directive', () {
        final directive = FileHeaderGenerator.generateIncludeDirective('path/to/file.txt');
        expect(directive, equals('#include path/to/file.txt\n'));
      });
    });

    group('PathResolver', () {
      test('should resolve relative paths correctly', () {
        final basePath = '/base/path';
        final relativePath = '../other/file.txt';
        
        final resolved = PathResolver.resolvePath(basePath, relativePath);
        expect(resolved, equals('/base/other/file.txt'));
      });

      test('should handle absolute paths', () {
        expect(PathResolver.isAbsolutePath('/absolute/path'), isTrue);
        expect(PathResolver.isAbsolutePath('relative/path'), isFalse);
        
        if (Platform.isWindows) {
          expect(PathResolver.isAbsolutePath('C:\\Windows'), isTrue);
          expect(PathResolver.isAbsolutePath('\\\\server\\share'), isTrue);
        }
      });

      test('should normalize paths for current platform', () {
        final path = 'some/path/with/forward/slashes';
        final normalized = PathResolver.normalizePath(path);
        
        if (Platform.isWindows) {
          expect(normalized, contains('\\'));
        } else {
          expect(normalized, contains('/'));
        }
      });

      test('should create safe filenames', () {
        final unsafeName = 'file<>:"/\\|?*name.txt';
        final safeName = PathResolver.getSafeFilename(unsafeName);
        
        expect(safeName, isNot(contains('<')));
        expect(safeName, isNot(contains('>')));
        expect(safeName, isNot(contains(':')));
        expect(safeName, contains('file'));
        expect(safeName, contains('name.txt'));
      });

      test('should ensure directory exists', () {
        final testDir = '${tempDir.path}/new/nested/directory';
        
        PathResolver.ensureDirectoryExists(testDir);
        expect(Directory(testDir).existsSync(), isTrue);
      });

      test('should get relative path between directories', () {
        final from = '/base/current/dir';
        final to = '/base/other/target';
        
        final relative = PathResolver.getRelativePath(from, to);
        expect(relative, equals('../../other/target'));
      });
    });

    group('FileRepository Integration', () {
      test('should initialize workspace with proper structure', () {
        final workspacePath = '${tempDir.path}/workspace';
        
        FileRepository.initializeWorkspace(workspacePath);
        
        expect(Directory('$workspacePath/include').existsSync(), isTrue);
        expect(Directory('$workspacePath/occlude').existsSync(), isTrue);
        expect(File('$workspacePath/include/items.txt').existsSync(), isTrue);
        expect(File('$workspacePath/occlude/items.txt').existsSync(), isTrue);
      });

      test('should validate workspace correctly', () {
        final workspacePath = '${tempDir.path}/workspace';
        FileRepository.initializeWorkspace(workspacePath);
        
        expect(() => FileRepository.validateWorkspace(workspacePath), returnsNormally);
      });

      test('should save and load graph with atomic operations', () async {
        final workspacePath = '${tempDir.path}/workspace';
        FileRepository.initializeWorkspace(workspacePath);
        
        final paths = FileRepository.getDefaultFilePaths(workspacePath);
        
        // Create a test graph
        final graph = GianttGraph();
        final item = GianttItem(
          id: 'test1',
          title: 'Test Item',
          duration: GianttDuration.parse('1d'),
        );
        graph.addItem(item);
        
        // Save the graph
        FileRepository.saveGraph(paths['items']!, paths['occlude_items']!, graph);
        
        // Load it back
        final loadedGraph = FileRepository.loadGraph(paths['items']!, paths['occlude_items']!);
        
        expect(loadedGraph.items.length, equals(1));
        expect(loadedGraph.items['test1'], isNotNull);
        expect(loadedGraph.items['test1']!.title, equals('Test Item'));
      });

      test('should handle file headers in saved files', () async {
        final workspacePath = '${tempDir.path}/workspace';
        FileRepository.initializeWorkspace(workspacePath);
        
        final paths = FileRepository.getDefaultFilePaths(workspacePath);
        
        final graph = GianttGraph();
        FileRepository.saveGraph(paths['items']!, paths['occlude_items']!, graph);
        
        final itemsContent = await File(paths['items']!).readAsString();
        final occludeContent = await File(paths['occlude_items']!).readAsString();
        
        expect(itemsContent, contains('Giantt Items'));
        expect(occludeContent, contains('Occluded Items'));
      });
    });
  });
}
