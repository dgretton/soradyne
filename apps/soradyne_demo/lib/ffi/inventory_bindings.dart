import 'dart:convert';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

// ---------------------------------------------------------------------------
// C function signature typedefs — unified soradyne_flow_* symbols
// ---------------------------------------------------------------------------

typedef SoradyneFlowInitC = Int32 Function(Pointer<Utf8>);
typedef SoradyneFlowInit = int Function(Pointer<Utf8>);

typedef SoradyneFlowOpenC = Pointer<Void> Function(
    Pointer<Utf8>, Pointer<Utf8>);
typedef SoradyneFlowOpen = Pointer<Void> Function(
    Pointer<Utf8>, Pointer<Utf8>);

typedef SoradyneFlowCloseC = Void Function(Pointer<Void>);
typedef SoradyneFlowClose = void Function(Pointer<Void>);

typedef SoradyneFlowWriteOpC = Int32 Function(
    Pointer<Void>, Pointer<Utf8>);
typedef SoradyneFlowWriteOp = int Function(Pointer<Void>, Pointer<Utf8>);

typedef SoradyneFlowReadDripC = Pointer<Utf8> Function(Pointer<Void>);
typedef SoradyneFlowReadDrip = Pointer<Utf8> Function(Pointer<Void>);

typedef SoradyneFlowConnectEnsembleC = Int32 Function(
    Pointer<Void>, Pointer<Utf8>);
typedef SoradyneFlowConnectEnsemble = int Function(
    Pointer<Void>, Pointer<Utf8>);

typedef SoradyneFlowStartSyncC = Int32 Function(Pointer<Void>);
typedef SoradyneFlowStartSync = int Function(Pointer<Void>);

typedef SoradyneFlowStopSyncC = Int32 Function(Pointer<Void>);
typedef SoradyneFlowStopSync = int Function(Pointer<Void>);

typedef SoradyneFlowCleanupC = Void Function();
typedef SoradyneFlowCleanup = void Function();

typedef SoradyneFreeStringC = Void Function(Pointer<Utf8>);
typedef SoradyneFreeString = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// Dart bindings class
//
// Uses the unified soradyne_flow_* FFI symbols with schema="inventory".
// ---------------------------------------------------------------------------

class InventoryBindings {
  final DynamicLibrary _lib;

  late final SoradyneFlowInit _init;
  late final SoradyneFlowOpen _open;
  late final SoradyneFlowClose _close;
  late final SoradyneFlowWriteOp _writeOp;
  late final SoradyneFlowReadDrip _readDrip;
  late final SoradyneFlowConnectEnsemble _connectEnsemble;
  late final SoradyneFlowStartSync _startSync;
  late final SoradyneFlowStopSync _stopSync;
  late final SoradyneFlowCleanup _cleanup;
  late final SoradyneFreeString _freeString;

  InventoryBindings(this._lib) {
    _init = _lib.lookupFunction<SoradyneFlowInitC, SoradyneFlowInit>(
        'soradyne_flow_init');
    _open = _lib.lookupFunction<SoradyneFlowOpenC, SoradyneFlowOpen>(
        'soradyne_flow_open');
    _close = _lib.lookupFunction<SoradyneFlowCloseC, SoradyneFlowClose>(
        'soradyne_flow_close');
    _writeOp = _lib.lookupFunction<SoradyneFlowWriteOpC,
        SoradyneFlowWriteOp>('soradyne_flow_write_op');
    _readDrip = _lib.lookupFunction<SoradyneFlowReadDripC,
        SoradyneFlowReadDrip>('soradyne_flow_read_drip');
    _connectEnsemble = _lib.lookupFunction<SoradyneFlowConnectEnsembleC,
        SoradyneFlowConnectEnsemble>('soradyne_flow_connect_ensemble');
    _startSync = _lib.lookupFunction<SoradyneFlowStartSyncC,
        SoradyneFlowStartSync>('soradyne_flow_start_sync');
    _stopSync = _lib.lookupFunction<SoradyneFlowStopSyncC,
        SoradyneFlowStopSync>('soradyne_flow_stop_sync');
    _cleanup = _lib.lookupFunction<SoradyneFlowCleanupC,
        SoradyneFlowCleanup>('soradyne_flow_cleanup');
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
    final uuidPtr = uuid.toNativeUtf8();
    final schemaPtr = 'inventory'.toNativeUtf8();
    try {
      final handle = _open(uuidPtr, schemaPtr);
      return handle.address == 0 ? null : handle;
    } finally {
      malloc.free(uuidPtr);
      malloc.free(schemaPtr);
    }
  }

  void close(Pointer<Void> handle) => _close(handle);

  /// Write a single CRDT operation (JSON-encoded Operation enum).
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
