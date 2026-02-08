import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

/// Dart FFI bindings for Soradyne flow operations.
///
/// Wraps the C FFI functions exported by soradyne_core for inventory flows.
/// Uses the same native library as [SoradyneClient].
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
  // Inventory Flow FFI
  // ===========================================================================

  /// Initialize the inventory flow system with a device ID.
  ///
  /// Must be called before [inventoryOpen]. Safe to call multiple times
  /// (reinitializes the registry).
  void inventoryInit(String deviceId) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Utf8>),
        int Function(Pointer<Utf8>)>('soradyne_inventory_init');

    final deviceIdPtr = deviceId.toNativeUtf8();
    try {
      final result = func(deviceIdPtr);
      if (result != 0) {
        throw Exception('Failed to initialize inventory flow system');
      }
      _initialized = true;
    } finally {
      calloc.free(deviceIdPtr);
    }
  }

  /// Open an inventory flow by UUID.
  ///
  /// Returns an opaque handle. The flow will load its operation history
  /// from disk (or create a new empty flow). Call [inventoryClose] when done.
  Pointer<Void> inventoryOpen(String uuid) {
    final func = _lib.lookupFunction<
        Pointer<Void> Function(Pointer<Utf8>),
        Pointer<Void> Function(Pointer<Utf8>)>('soradyne_inventory_open');

    final uuidPtr = uuid.toNativeUtf8();
    try {
      final handle = func(uuidPtr);
      if (handle == nullptr) {
        throw Exception('Failed to open inventory flow "$uuid"');
      }
      return handle;
    } finally {
      calloc.free(uuidPtr);
    }
  }

  /// Close an inventory flow handle.
  void inventoryClose(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>('soradyne_inventory_close');
    func(handle);
  }

  /// Write a convergent operation to an inventory flow.
  ///
  /// [opJson] is a JSON-encoded Operation matching the Rust Operation enum:
  /// ```json
  /// {"AddItem": {"item_id": "uuid", "item_type": "InventoryItem"}}
  /// {"SetField": {"item_id": "uuid", "field": "description", "value": {"String": "Hammer"}}}
  /// {"RemoveItem": {"item_id": "uuid"}}
  /// {"AddToSet": {"item_id": "uuid", "set_name": "tags", "element": {"String": "workshop"}}}
  /// ```
  void inventoryWriteOp(Pointer<Void> handle, String opJson) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_inventory_write_op');

    final opPtr = opJson.toNativeUtf8();
    try {
      final result = func(handle, opPtr);
      if (result != 0) {
        throw Exception('Failed to write inventory operation');
      }
    } finally {
      calloc.free(opPtr);
    }
  }

  /// Read the current materialized inventory state as JSON.
  ///
  /// Returns a JSON string with structure:
  /// ```json
  /// {
  ///   "items": {
  ///     "uuid-1": {"id": "uuid-1", "category": "Tools", "description": "Hammer", "location": "Toolbox", "tags": ["workshop"]},
  ///     ...
  ///   }
  /// }
  /// ```
  String inventoryReadDrip(Pointer<Void> handle) {
    final func = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>),
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_inventory_read_drip');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) {
      throw Exception('Failed to read inventory drip');
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
        Pointer<Utf8> Function(Pointer<Void>)>('soradyne_inventory_get_operations');

    final resultPtr = func(handle);
    if (resultPtr == nullptr) {
      throw Exception('Failed to get inventory operations');
    }

    try {
      return resultPtr.toDartString();
    } finally {
      _freeString(resultPtr);
    }
  }

  /// Apply remote operations received from another device.
  ///
  /// [opsJson] is a JSON array of OpEnvelope objects.
  void inventoryApplyRemote(Pointer<Void> handle, String opsJson) {
    final func = _lib.lookupFunction<
        Int32 Function(Pointer<Void>, Pointer<Utf8>),
        int Function(Pointer<Void>, Pointer<Utf8>)>('soradyne_inventory_apply_remote');

    final opsPtr = opsJson.toNativeUtf8();
    try {
      final result = func(handle, opsPtr);
      if (result != 0) {
        throw Exception('Failed to apply remote inventory operations');
      }
    } finally {
      calloc.free(opsPtr);
    }
  }

  /// Tear down the inventory flow system.
  void inventoryCleanup() {
    final func = _lib.lookupFunction<
        Void Function(),
        void Function()>('soradyne_inventory_cleanup');
    func();
    _initialized = false;
  }

  /// Free a string allocated by the Rust side.
  void _freeString(Pointer<Utf8> ptr) {
    final func = _lib.lookupFunction<
        Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('soradyne_free_string');
    func(ptr);
  }
}
