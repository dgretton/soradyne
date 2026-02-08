/// Repository for Flow-based Giantt storage.
///
/// Replaces file-based storage with Flow-backed edit histories,
/// enabling multi-device sync. Import/export of .giantt files
/// preserved for legacy migration.
library;

import 'dart:io';
import '../ffi/flow_client.dart';
import '../graph/giantt_graph.dart';
import '../parser/giantt_parser.dart';
import '../models/graph_exceptions.dart';
import '../operations/giantt_operations.dart';

/// Repository for Flow-based Giantt operations.
///
/// Unlike [FileRepository], this repository backs Giantt data with
/// Soradyne Flows, enabling:
/// - Multi-device sync via edit histories
/// - Informed-remove semantics for conflict resolution
/// - Eventual consistency across devices
///
/// Usage:
/// ```dart
/// // Initialize once at app startup
/// FlowRepository.initialize('device-uuid');
///
/// // Load a graph from a flow
/// final graph = FlowRepository.loadGraph('flow-uuid');
///
/// // Save operations (not whole graph)
/// FlowRepository.saveOperation('flow-uuid', GianttOp.addItem(...));
///
/// // Import legacy .giantt file (one-time migration)
/// FlowRepository.importLegacyFile('flow-uuid', '/path/to/items.txt');
///
/// // Export for backup or compaction
/// FlowRepository.exportToFile('flow-uuid', '/path/to/export.txt');
/// ```
class FlowRepository {
  static bool _initialized = false;

  /// Initialize the flow system.
  ///
  /// Must be called once before any flow operations.
  /// The device ID should be unique per device.
  static void initialize(String deviceId) {
    if (_initialized) return;
    FlowClient.initialize(deviceId);
    _initialized = true;
  }

  /// Load a graph from a flow.
  ///
  /// Opens the flow, reads the current state as .giantt text,
  /// and parses it using the existing [GianttParser].
  ///
  /// Returns an empty graph if the flow is empty.
  static GianttGraph loadGraph(String flowUuid) {
    _ensureInitialized();

    final client = FlowClient.open(flowUuid);
    try {
      final text = client.readDrip();
      return _parseText(text);
    } finally {
      client.close();
    }
  }

  /// Save an operation to a flow.
  ///
  /// The operation is persisted to the flow's edit history
  /// and will be synced to other devices.
  static void saveOperation(String flowUuid, GianttOp operation) {
    _ensureInitialized();

    final client = FlowClient.open(flowUuid);
    try {
      for (final op in operation.toOperations()) {
        client.writeOperation(op);
      }
    } finally {
      client.close();
    }
  }

  /// Save multiple operations to a flow.
  ///
  /// All operations are persisted atomically.
  static void saveOperations(String flowUuid, List<GianttOp> operations) {
    _ensureInitialized();

    final client = FlowClient.open(flowUuid);
    try {
      for (final operation in operations) {
        for (final op in operation.toOperations()) {
          client.writeOperation(op);
        }
      }
    } finally {
      client.close();
    }
  }

  /// Import a legacy .giantt file into a flow.
  ///
  /// This is a one-time migration operation. Each item in the file
  /// is converted to AddItem and SetField operations.
  ///
  /// Use cases:
  /// - Migrating existing .giantt files to flows
  /// - Importing compacted state
  static void importLegacyFile(String flowUuid, String filePath) {
    _ensureInitialized();

    final file = File(filePath);
    if (!file.existsSync()) {
      throw GraphException('File not found: $filePath');
    }

    final text = file.readAsStringSync();
    final graph = _parseText(text);

    final client = FlowClient.open(flowUuid);
    try {
      for (final item in graph.items.values) {
        // Convert item to operations
        final ops = GianttOp.fromItem(item);
        for (final op in ops) {
          for (final rawOp in op.toOperations()) {
            client.writeOperation(rawOp);
          }
        }
      }
    } finally {
      client.close();
    }
  }

  /// Export a flow to a .giantt file.
  ///
  /// The flow's current state is serialized to .giantt text format
  /// and written to the specified file.
  ///
  /// Use cases:
  /// - Creating backups
  /// - Exporting for external tools
  /// - Compaction (export then re-import to a fresh flow)
  static void exportToFile(String flowUuid, String filePath) {
    _ensureInitialized();

    final client = FlowClient.open(flowUuid);
    try {
      final text = client.readDrip();
      File(filePath).writeAsStringSync(text);
    } finally {
      client.close();
    }
  }

  /// Get operations from a flow as JSON.
  ///
  /// Useful for debugging or manual sync.
  static String getOperationsJson(String flowUuid) {
    _ensureInitialized();

    final client = FlowClient.open(flowUuid);
    try {
      return client.getOperationsJson();
    } finally {
      client.close();
    }
  }

  /// Apply remote operations to a flow.
  ///
  /// Used for syncing operations received from another device.
  static void applyRemoteOperations(String flowUuid, String operationsJson) {
    _ensureInitialized();

    final client = FlowClient.open(flowUuid);
    try {
      client.applyRemoteOperations(operationsJson);
    } finally {
      client.close();
    }
  }

  /// Clean up the flow system.
  ///
  /// Call this when the application is shutting down.
  static void cleanup() {
    if (!_initialized) return;
    FlowClient.cleanup();
    _initialized = false;
  }

  /// Check if the flow system is initialized.
  static bool get isInitialized => _initialized;

  static void _ensureInitialized() {
    if (!_initialized) {
      throw GraphException(
        'Flow system not initialized. Call FlowRepository.initialize() first.',
      );
    }
  }

  /// Parse .giantt text into a graph.
  static GianttGraph _parseText(String text) {
    final graph = GianttGraph();

    for (final line in text.split('\n')) {
      final trimmed = line.trim();
      if (trimmed.isNotEmpty && !trimmed.startsWith('#')) {
        try {
          final item = GianttParser.fromString(trimmed);
          graph.addItem(item);
        } catch (e) {
          // Skip invalid lines with warning
          print('Warning: Skipping invalid line: $e');
        }
      }
    }

    return graph;
  }
}
