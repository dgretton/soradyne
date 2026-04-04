import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

/// Dart FFI bindings for Soradyne flow operations.
///
/// Wraps the unified soradyne_flow_* C FFI functions exported by soradyne_core.
/// Passes schema="inventory" for inventory flows.
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
  // Inventory Flow FFI (using unified soradyne_flow_* symbols)
  // ===========================================================================

  /// Initialize the flow system with a device ID.
  void inventoryInit(String deviceId) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Utf8>),
        int Function(Pointer<Utf8>)>('soradyne_flow_init');

    final deviceIdPtr = deviceId.toNativeUtf8();
    try {
      final result = func(deviceIdPtr);
      if (result != 0) {
        throw Exception('Failed to initialize flow system');
      }
      _initialized = true;
    } finally {
      calloc.free(deviceIdPtr);
    }
  }

  /// Open an inventory flow by UUID.
  Pointer<Void> inventoryOpen(String uuid) {
    final func = _lib.lookupFunction<
        Pointer<Void> Function(Pointer<Utf8>, Pointer<Utf8>),
        Pointer<Void> Function(Pointer<Utf8>, Pointer<Utf8>)>('soradyne_flow_open');

    final uuidPtr = uuid.toNativeUtf8();
    final schemaPtr = 'inventory'.toNativeUtf8();
    try {
      final handle = func(uuidPtr, schemaPtr);
      if (handle == nullptr) {
        throw Exception('Failed to open inventory flow "$uuid"');
      }
      return handle;
    } finally {
      calloc.free(uuidPtr);
      calloc.free(schemaPtr);
    }
  }

  /// Close a flow handle.
  void inventoryClose(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>('soradyne_flow_close');
    func(handle);
  }

  /// Write a convergent operation to a flow.
  void inventoryWriteOp(Pointer<Void> handle, String opJson) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_flow_write_op');

    final opPtr = opJson.toNativeUtf8();
    try {
      final result = func(handle, opPtr);
      if (result != 0) {
        throw Exception('Failed to write operation');
      }
    } finally {
      calloc.free(opPtr);
    }
  }

  /// Read the current materialized inventory state as JSON.
  String inventoryReadDrip(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_flow_read_drip');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) {
      throw Exception('Failed to read drip');
    }

    try {
      return resultPtr.toDartString();
    } finally {
      _freeString(resultPtr);
    }
  }

  /// Get all operations as a JSON array (for syncing).
  String inventoryGetOperations(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_flow_get_operations');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) {
      throw Exception('Failed to get operations');
    }

    try {
      return resultPtr.toDartString();
    } finally {
      _freeString(resultPtr);
    }
  }

  /// Apply remote operations received from another device.
  void inventoryApplyRemote(Pointer<Void> handle, String opsJson) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_flow_apply_remote');

    final opsPtr = opsJson.toNativeUtf8();
    try {
      final result = func(handle, opsPtr);
      if (result != 0) {
        throw Exception('Failed to apply remote operations');
      }
    } finally {
      calloc.free(opsPtr);
    }
  }

  /// Tear down the flow system.
  void inventoryCleanup() {
    final func = _lib.lookupFunction<
        Void Function(),
        void Function()>('soradyne_flow_cleanup');
    func();
    _initialized = false;
  }

  // ===========================================================================
  // Flow Sync FFI
  // ===========================================================================

  /// Connect a flow to an ensemble for sync.
  void inventoryConnectEnsemble(Pointer<Void> handle, String capsuleId) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_flow_connect_ensemble');

    final capsuleIdPtr = capsuleId.toNativeUtf8();
    try {
      final result = func(handle, capsuleIdPtr);
      if (result != 0) {
        throw Exception('Failed to connect flow to ensemble');
      }
    } finally {
      calloc.free(capsuleIdPtr);
    }
  }

  /// Start background sync for a flow.
  void inventoryStartSync(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>),
        int Function(Pointer<Void>)>('soradyne_flow_start_sync');

    final result = func(handle);
    if (result != 0) {
      throw Exception('Failed to start sync');
    }
  }

  /// Stop background sync for a flow.
  void inventoryStopSync(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>),
        int Function(Pointer<Void>)>('soradyne_flow_stop_sync');

    final result = func(handle);
    if (result != 0) {
      throw Exception('Failed to stop sync');
    }
  }

  /// Get the sync status of a flow.
  String inventoryGetSyncStatus(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_flow_get_sync_status');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) {
      throw Exception('Failed to get sync status');
    }

    try {
      return resultPtr.toDartString();
    } finally {
      _freeString(resultPtr);
    }
  }

  /// Free a string allocated by the Rust side.
  void _freeString(Pointer<Utf8> ptr) {
    final func = _lib.lookupFunction<
        Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('soradyne_free_string');
    func(ptr);
  }
}
