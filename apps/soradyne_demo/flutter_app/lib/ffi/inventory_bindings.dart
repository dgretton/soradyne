import 'dart:convert';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

// ---------------------------------------------------------------------------
// C function signature typedefs
// ---------------------------------------------------------------------------

typedef SoradyneInventoryInitC = Int32 Function(Pointer<Utf8>);
typedef SoradyneInventoryInit = int Function(Pointer<Utf8>);

typedef SoradyneInventoryOpenC = Pointer<Void> Function(Pointer<Utf8>);
typedef SoradyneInventoryOpen = Pointer<Void> Function(Pointer<Utf8>);

typedef SoradyneInventoryCloseC = Void Function(Pointer<Void>);
typedef SoradyneInventoryClose = void Function(Pointer<Void>);

typedef SoradyneInventoryWriteOpC = Int32 Function(
    Pointer<Void>, Pointer<Utf8>);
typedef SoradyneInventoryWriteOp = int Function(Pointer<Void>, Pointer<Utf8>);

typedef SoradyneInventoryReadDripC = Pointer<Utf8> Function(Pointer<Void>);
typedef SoradyneInventoryReadDrip = Pointer<Utf8> Function(Pointer<Void>);

typedef SoradyneInventoryConnectEnsembleC = Int32 Function(
    Pointer<Void>, Pointer<Utf8>);
typedef SoradyneInventoryConnectEnsemble = int Function(
    Pointer<Void>, Pointer<Utf8>);

typedef SoradyneInventoryStartSyncC = Int32 Function(Pointer<Void>);
typedef SoradyneInventoryStartSync = int Function(Pointer<Void>);

typedef SoradyneInventoryStopSyncC = Int32 Function(Pointer<Void>);
typedef SoradyneInventoryStopSync = int Function(Pointer<Void>);

typedef SoradyneInventoryCleanupC = Void Function();
typedef SoradyneInventoryCleanup = void Function();

typedef SoradyneFreeStringC = Void Function(Pointer<Utf8>);
typedef SoradyneFreeString = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// Dart bindings class
//
// Uses the same DynamicLibrary as PairingBindings — the Rust .so/.dylib
// exposes both pairing and inventory symbols.
// ---------------------------------------------------------------------------

class InventoryBindings {
  final DynamicLibrary _lib;

  late final SoradyneInventoryInit _init;
  late final SoradyneInventoryOpen _open;
  late final SoradyneInventoryClose _close;
  late final SoradyneInventoryWriteOp _writeOp;
  late final SoradyneInventoryReadDrip _readDrip;
  late final SoradyneInventoryConnectEnsemble _connectEnsemble;
  late final SoradyneInventoryStartSync _startSync;
  late final SoradyneInventoryStopSync _stopSync;
  late final SoradyneInventoryCleanup _cleanup;
  late final SoradyneFreeString _freeString;

  InventoryBindings(this._lib) {
    _init = _lib.lookupFunction<SoradyneInventoryInitC, SoradyneInventoryInit>(
        'soradyne_inventory_init');
    _open = _lib.lookupFunction<SoradyneInventoryOpenC, SoradyneInventoryOpen>(
        'soradyne_inventory_open');
    _close =
        _lib.lookupFunction<SoradyneInventoryCloseC, SoradyneInventoryClose>(
            'soradyne_inventory_close');
    _writeOp = _lib.lookupFunction<SoradyneInventoryWriteOpC,
        SoradyneInventoryWriteOp>('soradyne_inventory_write_op');
    _readDrip = _lib.lookupFunction<SoradyneInventoryReadDripC,
        SoradyneInventoryReadDrip>('soradyne_inventory_read_drip');
    _connectEnsemble = _lib.lookupFunction<SoradyneInventoryConnectEnsembleC,
        SoradyneInventoryConnectEnsemble>('soradyne_inventory_connect_ensemble');
    _startSync = _lib.lookupFunction<SoradyneInventoryStartSyncC,
        SoradyneInventoryStartSync>('soradyne_inventory_start_sync');
    _stopSync = _lib.lookupFunction<SoradyneInventoryStopSyncC,
        SoradyneInventoryStopSync>('soradyne_inventory_stop_sync');
    _cleanup = _lib.lookupFunction<SoradyneInventoryCleanupC,
        SoradyneInventoryCleanup>('soradyne_inventory_cleanup');
    _freeString =
        _lib.lookupFunction<SoradyneFreeStringC, SoradyneFreeString>(
            'soradyne_free_string');
  }

  int init(String deviceId) {
    final ptr = deviceId.toNativeUtf8();
    final result = _init(ptr);
    malloc.free(ptr);
    return result;
  }

  /// Returns an opaque handle, or null on failure.
  Pointer<Void>? open(String uuid) {
    final ptr = uuid.toNativeUtf8();
    final handle = _open(ptr);
    malloc.free(ptr);
    return handle.address == 0 ? null : handle;
  }

  void close(Pointer<Void> handle) => _close(handle);

  /// Write a single CRDT operation (JSON-encoded Operation enum).
  ///
  /// The operation must match the Rust `Operation` serde format, e.g.:
  ///   `{"AddItem": {"item_id": "id", "item_type": "InventoryItem"}}`
  ///   `{"SetField": {"item_id": "id", "field": "description", "value": "text"}}`
  ///   `{"RemoveItem": {"item_id": "id"}}`
  int writeOp(Pointer<Void> handle, Map<String, dynamic> op) {
    final json = jsonEncode(op);
    final ptr = json.toNativeUtf8();
    final result = _writeOp(handle, ptr);
    malloc.free(ptr);
    return result;
  }

  /// Read the current inventory state as a parsed JSON map, or null on error.
  Map<String, dynamic>? readDrip(Pointer<Void> handle) {
    final ptr = _readDrip(handle);
    if (ptr.address == 0) return null;
    final json = ptr.toDartString();
    _freeString(ptr);
    try {
      return jsonDecode(json) as Map<String, dynamic>;
    } catch (_) {
      return null;
    }
  }

  int connectEnsemble(Pointer<Void> handle, String capsuleId) {
    final ptr = capsuleId.toNativeUtf8();
    final result = _connectEnsemble(handle, ptr);
    malloc.free(ptr);
    return result;
  }

  int startSync(Pointer<Void> handle) => _startSync(handle);
  int stopSync(Pointer<Void> handle) => _stopSync(handle);
  void cleanup() => _cleanup();
}
