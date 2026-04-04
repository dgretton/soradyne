/// High-level client for Giantt Flow operations.
///
/// Wraps the low-level FFI bindings to provide a convenient Dart API
/// for interacting with Soradyne flows.
library;

import 'dart:convert';
import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'soradyne_ffi.dart';

/// Exception thrown when a flow operation fails.
class FlowException implements Exception {
  final String message;
  final String? operation;

  FlowException(this.message, [this.operation]);

  @override
  String toString() {
    if (operation != null) {
      return 'FlowException: $message (during $operation)';
    }
    return 'FlowException: $message';
  }
}

/// Client for interacting with a Soradyne convergent flow.
///
/// The FlowClient manages a connection to a Soradyne flow identified by UUID.
/// Operations are written to the flow and the current state can be read in
/// the schema's native format (.giantt text for giantt, JSON for inventory).
///
/// Usage:
/// ```dart
/// // Initialize the flow system (once per application)
/// FlowClient.initialize('device-uuid');
///
/// // Open a giantt flow (default schema)
/// final client = FlowClient.open('flow-uuid');
///
/// // Open an inventory flow
/// final invClient = FlowClient.open('flow-uuid', schema: 'inventory');
/// ```
class FlowClient {
  static bool _initialized = false;
  static final _ffi = SoradyneFFI.instance;

  final String uuid;
  final String schema;
  Pointer<Void> _handle;
  bool _closed = false;

  FlowClient._(this.uuid, this.schema, this._handle);

  /// Initialize the flow system with a device ID.
  ///
  /// Must be called once before any flows can be opened.
  /// The device ID should be unique per device.
  static void initialize(String deviceId) {
    if (_initialized) return;

    final deviceIdPtr = deviceId.toNativeUtf8();
    try {
      final result = _ffi.flowInit(deviceIdPtr);
      if (result != 0) {
        throw FlowException('Failed to initialize flow system', 'initialize');
      }
      _initialized = true;
    } finally {
      malloc.free(deviceIdPtr);
    }
  }

  /// Open a flow by UUID and schema.
  ///
  /// [schema] selects the convergent document schema: `"giantt"` (default)
  /// or `"inventory"`. The flow will load its edit history from disk (or
  /// create a new one).
  ///
  /// Returns a [FlowClient] that must be closed when done.
  ///
  /// Throws [FlowException] if the flow cannot be opened.
  static FlowClient open(String uuid, {String schema = 'giantt'}) {
    if (!_initialized) {
      throw FlowException(
        'Flow system not initialized. Call FlowClient.initialize() first.',
        'open',
      );
    }

    final uuidPtr = uuid.toNativeUtf8();
    final schemaPtr = schema.toNativeUtf8();
    try {
      final handle = _ffi.flowOpen(uuidPtr, schemaPtr);
      if (handle == nullptr) {
        throw FlowException('Failed to open flow: $uuid (schema: $schema)', 'open');
      }
      return FlowClient._(uuid, schema, handle);
    } finally {
      malloc.free(uuidPtr);
      malloc.free(schemaPtr);
    }
  }

  /// Write an operation to the flow.
  ///
  /// The operation should be a map matching the Rust Operation enum format:
  /// ```dart
  /// // AddItem
  /// {'AddItem': {'item_id': 'task_1', 'item_type': 'GianttItem'}}
  ///
  /// // SetField
  /// {'SetField': {'item_id': 'task_1', 'field': 'title', 'value': {'String': 'My Task'}}}
  ///
  /// // AddToSet
  /// {'AddToSet': {'item_id': 'task_1', 'set_name': 'tags', 'element': {'String': 'important'}}}
  ///
  /// // RemoveFromSet
  /// {'RemoveFromSet': {
  ///   'item_id': 'task_1',
  ///   'set_name': 'tags',
  ///   'element': {'String': 'old-tag'},
  ///   'observed_add_ids': ['uuid1', 'uuid2']
  /// }}
  ///
  /// // RemoveItem
  /// {'RemoveItem': {'item_id': 'task_1'}}
  /// ```
  void writeOperation(Map<String, dynamic> operation) {
    _checkNotClosed();

    final opJson = jsonEncode(operation);
    final opJsonPtr = opJson.toNativeUtf8();
    try {
      final result = _ffi.flowWriteOp(_handle, opJsonPtr);
      if (result != 0) {
        throw FlowException('Failed to write operation', 'writeOperation');
      }
    } finally {
      malloc.free(opJsonPtr);
    }
  }

  /// Write the materialized .giantt state to [filePath] on disk.
  ///
  /// Call this explicitly when you want a human-readable snapshot.
  /// Nothing writes to [filePath] automatically.
  void writeSnapshot(String filePath) {
    _checkNotClosed();

    final fn = _ffi.flowWriteSnapshot;
    final pathPtr = filePath.toNativeUtf8();
    try {
      final result = fn(_handle, pathPtr);
      if (result != 0) {
        throw FlowException('Failed to write snapshot to $filePath', 'writeSnapshot');
      }
    } finally {
      malloc.free(pathPtr);
    }
  }

  /// Read the current state as .giantt text format.
  ///
  /// This materializes the state from all operations (local and remote)
  /// and serializes it to the standard .giantt text format that can be
  /// parsed by [GianttParser].
  String readDrip() {
    _checkNotClosed();

    final resultPtr = _ffi.flowReadDrip(_handle);
    if (resultPtr == nullptr) {
      throw FlowException('Failed to read drip', 'readDrip');
    }

    try {
      return resultPtr.toDartString();
    } finally {
      _ffi.freeString(resultPtr);
    }
  }

  /// Get all operations as a JSON-encoded list.
  ///
  /// This is useful for syncing operations to other devices.
  String getOperationsJson() {
    _checkNotClosed();

    final resultPtr = _ffi.flowGetOperations(_handle);
    if (resultPtr == nullptr) {
      throw FlowException('Failed to get operations', 'getOperationsJson');
    }

    try {
      return resultPtr.toDartString();
    } finally {
      _ffi.freeString(resultPtr);
    }
  }

  /// Apply remote operations received from another device.
  ///
  /// The operations should be a JSON-encoded list of OpEnvelope objects
  /// (typically obtained from another device's [getOperationsJson]).
  void applyRemoteOperations(String operationsJson) {
    _checkNotClosed();

    final opsJsonPtr = operationsJson.toNativeUtf8();
    try {
      final result = _ffi.flowApplyRemote(_handle, opsJsonPtr);
      if (result != 0) {
        throw FlowException('Failed to apply remote operations', 'applyRemoteOperations');
      }
    } finally {
      malloc.free(opsJsonPtr);
    }
  }

  /// Connect this flow to a TCP peer for direct CRDT sync (no capsule/pairing needed).
  ///
  /// [peerAddr] — the peer's "ip:port" (e.g. a Tailscale address like "100.64.1.2:7979").
  /// [listen]   — if true, bind and wait for the peer to connect (server role);
  ///               if false, connect to the peer (client role).
  ///
  /// Both sides must call [connectTcp]. The server should call first (or concurrently).
  /// A UUID handshake identifies each peer; sync begins immediately after.
  ///
  /// Throws [FlowException] if the native library was not compiled with
  /// `tcp-transport`, or if the connection fails.
  void connectTcp(String peerAddr, {bool listen = false}) {
    _checkNotClosed();

    final fn = _ffi.flowConnectTcp;
    if (fn == null) {
      throw FlowException(
        'TCP transport not available — rebuild with --features tcp-transport',
        'connectTcp',
      );
    }

    final peerAddrPtr = peerAddr.toNativeUtf8();
    try {
      final result = fn(_handle, peerAddrPtr, listen ? 1 : 0);
      if (result != 0) {
        throw FlowException(
          'TCP connect failed (code $result)',
          'connectTcp',
        );
      }
    } finally {
      malloc.free(peerAddrPtr);
    }
  }

  /// Close the flow client and release resources.
  ///
  /// After closing, the client cannot be used for further operations.
  void close() {
    if (_closed) return;
    _ffi.flowClose(_handle);
    _closed = true;
  }

  /// Check if the client has been closed.
  bool get isClosed => _closed;

  void _checkNotClosed() {
    if (_closed) {
      throw FlowException('FlowClient has been closed');
    }
  }

  /// Clean up the flow system.
  ///
  /// Call this when the application is shutting down to release all resources.
  static void cleanup() {
    if (!_initialized) return;
    _ffi.flowCleanup();
    _initialized = false;
  }
}
