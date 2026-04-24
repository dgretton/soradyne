import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

/// Dart FFI bindings for Soradyne convergent-document flows.
///
/// Wraps the `soradyne_flow_*` C FFI functions exported by soradyne_core.
/// Schema knowledge lives entirely in the calling app; this client is
/// schema-agnostic. `read_drip` returns generic DocumentState JSON:
///   `{"items": {"<id>": {"item_type": "…", "fields": {…}, "sets": {…}}}}`
class FlowClient {
  static FlowClient? _instance;
  late final DynamicLibrary _lib;
  bool _initialized = false;

  FlowClient._() {
    _lib = _loadLibrary();
  }

  /// Singleton instance — shares the same native library handle.
  static FlowClient get instance {
    _instance ??= FlowClient._();
    return _instance!;
  }

  DynamicLibrary _loadLibrary() {
    if (Platform.isMacOS) return DynamicLibrary.open('libsoradyne.dylib');
    if (Platform.isLinux) return DynamicLibrary.open('libsoradyne.so');
    if (Platform.isWindows) return DynamicLibrary.open('soradyne.dll');
    if (Platform.isAndroid) return DynamicLibrary.open('libsoradyne.so');
    if (Platform.isIOS) return DynamicLibrary.process();
    throw UnsupportedError('Platform not supported');
  }

  // ===========================================================================
  // Lifecycle
  // ===========================================================================

  /// Initialize the flow system with a device ID.
  void init(String deviceId) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Utf8>),
        int Function(Pointer<Utf8>)>('soradyne_flow_init');

    final ptr = deviceId.toNativeUtf8();
    try {
      if (func(ptr) != 0) throw Exception('Failed to initialize flow system');
      _initialized = true;
    } finally {
      calloc.free(ptr);
    }
  }

  /// Open a flow by UUID. [schema] is passed through for compatibility but
  /// currently unused — all flows are schema-agnostic on the Rust side.
  Pointer<Void> open(String uuid, {String schema = ''}) {
    final func = _lib.lookupFunction<
        Pointer<Void> Function(Pointer<Utf8>, Pointer<Utf8>),
        Pointer<Void> Function(Pointer<Utf8>, Pointer<Utf8>)>('soradyne_flow_open');

    final uuidPtr = uuid.toNativeUtf8();
    final schemaPtr = schema.toNativeUtf8();
    try {
      final handle = func(uuidPtr, schemaPtr);
      if (handle == nullptr) throw Exception('Failed to open flow "$uuid"');
      return handle;
    } finally {
      calloc.free(uuidPtr);
      calloc.free(schemaPtr);
    }
  }

  /// Close a flow handle.
  void close(Pointer<Void> handle) {
    _lib.lookupFunction<
        Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>('soradyne_flow_close')(handle);
  }

  /// Tear down the flow system.
  void cleanup() {
    _lib.lookupFunction<Void Function(), void Function()>('soradyne_flow_cleanup')();
    _initialized = false;
  }

  // ===========================================================================
  // Data operations
  // ===========================================================================

  /// Write a convergent operation to a flow.
  /// [opJson]: JSON-encoded Operation, e.g. `{"AddItem":{"item_id":"x","item_type":"T"}}`
  void writeOp(Pointer<Void> handle, String opJson) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_flow_write_op');

    final ptr = opJson.toNativeUtf8();
    try {
      if (func(handle, ptr) != 0) throw Exception('Failed to write operation');
    } finally {
      calloc.free(ptr);
    }
  }

  /// Read the current materialized state as generic DocumentState JSON.
  /// Format: `{"items": {"<id>": {"item_type":"…","fields":{…},"sets":{…}}}}`
  String readDrip(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_flow_read_drip');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) throw Exception('Failed to read drip');
    try {
      return resultPtr.toDartString();
    } finally {
      _freeString(resultPtr);
    }
  }

  /// Get all operations as a JSON array (for manual syncing if needed).
  String getOperations(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_flow_get_operations');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) throw Exception('Failed to get operations');
    try {
      return resultPtr.toDartString();
    } finally {
      _freeString(resultPtr);
    }
  }

  /// Apply remote operations received from another device.
  void applyRemote(Pointer<Void> handle, String opsJson) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_flow_apply_remote');

    final ptr = opsJson.toNativeUtf8();
    try {
      if (func(handle, ptr) != 0) throw Exception('Failed to apply remote operations');
    } finally {
      calloc.free(ptr);
    }
  }

  // ===========================================================================
  // Sync
  // ===========================================================================

  /// Connect a flow to a capsule ensemble for sync.
  void connectEnsemble(Pointer<Void> handle, String capsuleId) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_flow_connect_ensemble');

    final ptr = capsuleId.toNativeUtf8();
    try {
      if (func(handle, ptr) != 0) throw Exception('Failed to connect flow to ensemble');
    } finally {
      calloc.free(ptr);
    }
  }

  /// Start background sync for a flow.
  void startSync(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>),
        int Function(Pointer<Void>)>('soradyne_flow_start_sync');
    if (func(handle) != 0) throw Exception('Failed to start sync');
  }

  /// Stop background sync for a flow.
  void stopSync(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>),
        int Function(Pointer<Void>)>('soradyne_flow_stop_sync');
    if (func(handle) != 0) throw Exception('Failed to stop sync');
  }

  /// Get the sync status of a flow as a JSON string.
  String getSyncStatus(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_flow_get_sync_status');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) throw Exception('Failed to get sync status');
    try {
      return resultPtr.toDartString();
    } finally {
      _freeString(resultPtr);
    }
  }

  void _freeString(Pointer<Utf8> ptr) {
    _lib.lookupFunction<
        Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('soradyne_free_string')(ptr);
  }
}
