/// Low-level FFI bindings to the Soradyne Rust library.
///
/// These bindings provide direct access to the C FFI functions exposed by
/// soradyne_core. Users should prefer [FlowClient] for a higher-level API.
library;

import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';
import 'package:path/path.dart' as path;

/// Native function signatures for Soradyne FFI
typedef SoradyneFlowInitNative = Int32 Function(Pointer<Utf8> deviceId);
typedef SoradyneFlowInit = int Function(Pointer<Utf8> deviceId);

typedef SoradyneFlowOpenNative = Pointer<Void> Function(
    Pointer<Utf8> uuid, Pointer<Utf8> schema);
typedef SoradyneFlowOpen = Pointer<Void> Function(
    Pointer<Utf8> uuid, Pointer<Utf8> schema);

typedef SoradyneFlowCloseNative = Void Function(Pointer<Void> handle);
typedef SoradyneFlowClose = void Function(Pointer<Void> handle);

typedef SoradyneFlowWriteOpNative = Int32 Function(
    Pointer<Void> handle, Pointer<Utf8> opJson);
typedef SoradyneFlowWriteOp = int Function(
    Pointer<Void> handle, Pointer<Utf8> opJson);

typedef SoradyneFlowReadDripNative = Pointer<Utf8> Function(Pointer<Void> handle);
typedef SoradyneFlowReadDrip = Pointer<Utf8> Function(Pointer<Void> handle);

typedef SoradyneFlowGetOperationsNative = Pointer<Utf8> Function(Pointer<Void> handle);
typedef SoradyneFlowGetOperations = Pointer<Utf8> Function(Pointer<Void> handle);

typedef SoradyneFlowApplyRemoteNative = Int32 Function(
    Pointer<Void> handle, Pointer<Utf8> opsJson);
typedef SoradyneFlowApplyRemote = int Function(
    Pointer<Void> handle, Pointer<Utf8> opsJson);

typedef SoradyneFreeStringNative = Void Function(Pointer<Utf8> ptr);
typedef SoradyneFreeString = void Function(Pointer<Utf8> ptr);

typedef SoradyneFlowCleanupNative = Void Function();
typedef SoradyneFlowCleanup = void Function();

typedef SoradyneFlowWriteSnapshotNative = Int32 Function(
    Pointer<Void> handle, Pointer<Utf8> path);
typedef SoradyneFlowWriteSnapshot = int Function(
    Pointer<Void> handle, Pointer<Utf8> path);

typedef SoradynePairingInitNative = Int32 Function(Pointer<Utf8> dataDir);
typedef SoradynePairingInit = int Function(Pointer<Utf8> dataDir);

typedef SoradyneFlowConnectEnsembleNative = Int32 Function(
    Pointer<Void> handle, Pointer<Utf8> capsuleId);
typedef SoradyneFlowConnectEnsemble = int Function(
    Pointer<Void> handle, Pointer<Utf8> capsuleId);

typedef SoradyneFlowStartSyncNative = Int32 Function(Pointer<Void> handle);
typedef SoradyneFlowStartSync = int Function(Pointer<Void> handle);

typedef SoradyneFlowEnableSyncNative = Int32 Function(Pointer<Void> handle);
typedef SoradyneFlowEnableSync = int Function(Pointer<Void> handle);

/// Soradyne FFI bindings singleton.
///
/// Provides access to native Soradyne functions through Dart FFI.
class SoradyneFFI {
  static SoradyneFFI? _instance;
  late final DynamicLibrary _lib;

  // FFI functions
  late final SoradyneFlowInit flowInit;
  late final SoradyneFlowOpen flowOpen;
  late final SoradyneFlowClose flowClose;
  late final SoradyneFlowWriteOp flowWriteOp;
  late final SoradyneFlowReadDrip flowReadDrip;
  late final SoradyneFlowGetOperations flowGetOperations;
  late final SoradyneFlowApplyRemote flowApplyRemote;
  late final SoradyneFreeString freeString;
  late final SoradyneFlowCleanup flowCleanup;
  late final SoradyneFlowWriteSnapshot flowWriteSnapshot;
  late final SoradynePairingInit pairingInit;
  late final SoradyneFlowConnectEnsemble flowConnectEnsemble;
  late final SoradyneFlowStartSync flowStartSync;
  late final SoradyneFlowEnableSync flowEnableSync;

  SoradyneFFI._internal() {
    _lib = _loadLibrary();
    _bindFunctions();
  }

  /// Get the singleton instance.
  static SoradyneFFI get instance {
    _instance ??= SoradyneFFI._internal();
    return _instance!;
  }

  /// Load the native library based on the platform.
  DynamicLibrary _loadLibrary() {
    final String libraryName;
    final List<String> searchPaths = [];

    if (Platform.isMacOS) {
      libraryName = 'libsoradyne.dylib';
      // Search paths for macOS
      searchPaths.addAll([
        // Development: relative to package
        path.join(Directory.current.path, 'target', 'release', libraryName),
        path.join(Directory.current.path, 'target', 'debug', libraryName),
        // Monorepo structure
        path.join(Directory.current.path, '..', 'soradyne_core', 'target', 'release', libraryName),
        path.join(Directory.current.path, '..', 'soradyne_core', 'target', 'debug', libraryName),
        // Installed location
        '/usr/local/lib/$libraryName',
        // Home directory
        path.join(Platform.environment['HOME'] ?? '', '.soradyne', 'lib', libraryName),
      ]);
    } else if (Platform.isLinux) {
      libraryName = 'libsoradyne.so';
      searchPaths.addAll([
        path.join(Directory.current.path, 'target', 'release', libraryName),
        path.join(Directory.current.path, 'target', 'debug', libraryName),
        path.join(Directory.current.path, '..', 'soradyne_core', 'target', 'release', libraryName),
        path.join(Directory.current.path, '..', 'soradyne_core', 'target', 'debug', libraryName),
        '/usr/local/lib/$libraryName',
        '/usr/lib/$libraryName',
        // Home directory
        path.join(Platform.environment['HOME'] ?? '', '.soradyne', 'lib', libraryName),
      ]);
    } else if (Platform.isWindows) {
      libraryName = 'soradyne.dll';
      searchPaths.addAll([
        path.join(Directory.current.path, 'target', 'release', libraryName),
        path.join(Directory.current.path, 'target', 'debug', libraryName),
      ]);
    } else if (Platform.isAndroid) {
      // On Android the .so is bundled in the APK by soradyne_flutter (ffiPlugin).
      // The dynamic linker finds it by name from the app's lib directory.
      return DynamicLibrary.open('libsoradyne.so');
    } else if (Platform.isIOS) {
      return DynamicLibrary.process();
    } else {
      throw UnsupportedError('Platform not supported: ${Platform.operatingSystem}');
    }

    // Try each search path
    for (final searchPath in searchPaths) {
      if (File(searchPath).existsSync()) {
        return DynamicLibrary.open(searchPath);
      }
    }

    // Fall back to system search
    try {
      return DynamicLibrary.open(libraryName);
    } catch (e) {
      throw StateError(
        'Could not find Soradyne native library. '
        'Searched paths:\n${searchPaths.join('\n')}\n'
        'Build the Rust library with: cargo build --release',
      );
    }
  }

  /// Bind all FFI functions.
  void _bindFunctions() {
    flowInit = _lib
        .lookup<NativeFunction<SoradyneFlowInitNative>>('soradyne_flow_init')
        .asFunction();

    flowOpen = _lib
        .lookup<NativeFunction<SoradyneFlowOpenNative>>('soradyne_flow_open')
        .asFunction();

    flowClose = _lib
        .lookup<NativeFunction<SoradyneFlowCloseNative>>('soradyne_flow_close')
        .asFunction();

    flowWriteOp = _lib
        .lookup<NativeFunction<SoradyneFlowWriteOpNative>>('soradyne_flow_write_op')
        .asFunction();

    flowReadDrip = _lib
        .lookup<NativeFunction<SoradyneFlowReadDripNative>>('soradyne_flow_read_drip')
        .asFunction();

    flowGetOperations = _lib
        .lookup<NativeFunction<SoradyneFlowGetOperationsNative>>('soradyne_flow_get_operations')
        .asFunction();

    flowApplyRemote = _lib
        .lookup<NativeFunction<SoradyneFlowApplyRemoteNative>>('soradyne_flow_apply_remote')
        .asFunction();

    freeString = _lib
        .lookup<NativeFunction<SoradyneFreeStringNative>>('soradyne_free_string')
        .asFunction();

    flowCleanup = _lib
        .lookup<NativeFunction<SoradyneFlowCleanupNative>>('soradyne_flow_cleanup')
        .asFunction();

    flowWriteSnapshot = _lib
        .lookup<NativeFunction<SoradyneFlowWriteSnapshotNative>>(
            'soradyne_flow_write_snapshot')
        .asFunction();

    pairingInit = _lib
        .lookup<NativeFunction<SoradynePairingInitNative>>(
            'soradyne_pairing_init')
        .asFunction();

    flowConnectEnsemble = _lib
        .lookup<NativeFunction<SoradyneFlowConnectEnsembleNative>>(
            'soradyne_flow_connect_ensemble')
        .asFunction();

    flowStartSync = _lib
        .lookup<NativeFunction<SoradyneFlowStartSyncNative>>(
            'soradyne_flow_start_sync')
        .asFunction();

    flowEnableSync = _lib
        .lookup<NativeFunction<SoradyneFlowEnableSyncNative>>(
            'soradyne_flow_enable_sync')
        .asFunction();

  }
}
