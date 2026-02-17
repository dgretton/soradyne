import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';
import 'package:path/path.dart' as path;

// ---------------------------------------------------------------------------
// C function signature typedefs
// ---------------------------------------------------------------------------

typedef SoradynePairingInitC = Int32 Function(Pointer<Utf8>);
typedef SoradynePairingInit = int Function(Pointer<Utf8>);

typedef SoradynePairingCleanupC = Void Function();
typedef SoradynePairingCleanup = void Function();

typedef SoradynePairingCreateCapsuleC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef SoradynePairingCreateCapsule = Pointer<Utf8> Function(Pointer<Utf8>);

typedef SoradynePairingListCapsulesC = Pointer<Utf8> Function();
typedef SoradynePairingListCapsules = Pointer<Utf8> Function();

typedef SoradynePairingGetCapsuleC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef SoradynePairingGetCapsule = Pointer<Utf8> Function(Pointer<Utf8>);

typedef SoradynePairingStartInviteC = Int32 Function(Pointer<Utf8>);
typedef SoradynePairingStartInvite = int Function(Pointer<Utf8>);

typedef SoradynePairingStartJoinC = Int32 Function(Pointer<Utf8>);
typedef SoradynePairingStartJoin = int Function(Pointer<Utf8>);

typedef SoradynePairingGetStateC = Pointer<Utf8> Function();
typedef SoradynePairingGetState = Pointer<Utf8> Function();

typedef SoradynePairingConfirmPinC = Int32 Function();
typedef SoradynePairingConfirmPin = int Function();

typedef SoradynePairingSubmitPinC = Int32 Function(Pointer<Utf8>);
typedef SoradynePairingSubmitPin = int Function(Pointer<Utf8>);

typedef SoradynePairingCancelC = Int32 Function();
typedef SoradynePairingCancel = int Function();

typedef SoradynePairingAddSimAccessoryC = Pointer<Utf8> Function(
    Pointer<Utf8>, Pointer<Utf8>);
typedef SoradynePairingAddSimAccessory = Pointer<Utf8> Function(
    Pointer<Utf8>, Pointer<Utf8>);

typedef SoradyneFreeStringC = Void Function(Pointer<Utf8>);
typedef SoradyneFreeString = void Function(Pointer<Utf8>);

// ---------------------------------------------------------------------------
// Dart bindings class
// ---------------------------------------------------------------------------

class PairingBindings {
  late final DynamicLibrary _lib;
  late final SoradynePairingInit _init;
  late final SoradynePairingCleanup _cleanup;
  late final SoradynePairingCreateCapsule _createCapsule;
  late final SoradynePairingListCapsules _listCapsules;
  late final SoradynePairingGetCapsule _getCapsule;
  late final SoradynePairingStartInvite _startInvite;
  late final SoradynePairingStartJoin _startJoin;
  late final SoradynePairingGetState _getState;
  late final SoradynePairingConfirmPin _confirmPin;
  late final SoradynePairingSubmitPin _submitPin;
  late final SoradynePairingCancel _cancel;
  late final SoradynePairingAddSimAccessory _addSimAccessory;
  late final SoradyneFreeString _freeString;

  PairingBindings() {
    if (Platform.isMacOS) {
      try {
        final executablePath = Platform.resolvedExecutable;
        final appDir = path.dirname(executablePath);
        final dylibPath = path.join(appDir, 'libsoradyne.dylib');
        _lib = DynamicLibrary.open(dylibPath);
      } catch (e) {
        _lib = DynamicLibrary.open('libsoradyne.dylib');
      }
    } else if (Platform.isLinux) {
      _lib = DynamicLibrary.open('libsoradyne.so');
    } else if (Platform.isWindows) {
      _lib = DynamicLibrary.open('soradyne.dll');
    } else if (Platform.isAndroid) {
      _lib = DynamicLibrary.open('libsoradyne.so');
    } else if (Platform.isIOS) {
      _lib = DynamicLibrary.process();
    } else {
      throw UnsupportedError('Platform not supported');
    }

    _init = _lib.lookupFunction<SoradynePairingInitC, SoradynePairingInit>(
        'soradyne_pairing_init');
    _cleanup =
        _lib.lookupFunction<SoradynePairingCleanupC, SoradynePairingCleanup>(
            'soradyne_pairing_cleanup');
    _createCapsule = _lib.lookupFunction<SoradynePairingCreateCapsuleC,
        SoradynePairingCreateCapsule>('soradyne_pairing_create_capsule');
    _listCapsules = _lib.lookupFunction<SoradynePairingListCapsulesC,
        SoradynePairingListCapsules>('soradyne_pairing_list_capsules');
    _getCapsule = _lib.lookupFunction<SoradynePairingGetCapsuleC,
        SoradynePairingGetCapsule>('soradyne_pairing_get_capsule');
    _startInvite = _lib.lookupFunction<SoradynePairingStartInviteC,
        SoradynePairingStartInvite>('soradyne_pairing_start_invite');
    _startJoin = _lib.lookupFunction<SoradynePairingStartJoinC,
        SoradynePairingStartJoin>('soradyne_pairing_start_join');
    _getState = _lib.lookupFunction<SoradynePairingGetStateC,
        SoradynePairingGetState>('soradyne_pairing_get_state');
    _confirmPin = _lib.lookupFunction<SoradynePairingConfirmPinC,
        SoradynePairingConfirmPin>('soradyne_pairing_confirm_pin');
    _submitPin = _lib.lookupFunction<SoradynePairingSubmitPinC,
        SoradynePairingSubmitPin>('soradyne_pairing_submit_pin');
    _cancel = _lib.lookupFunction<SoradynePairingCancelC,
        SoradynePairingCancel>('soradyne_pairing_cancel');
    _addSimAccessory = _lib.lookupFunction<SoradynePairingAddSimAccessoryC,
        SoradynePairingAddSimAccessory>('soradyne_pairing_add_sim_accessory');
    _freeString =
        _lib.lookupFunction<SoradyneFreeStringC, SoradyneFreeString>(
            'soradyne_free_string');
  }

  int init({String? dataDir}) {
    final dirPtr = dataDir?.toNativeUtf8() ?? nullptr.cast<Utf8>();
    final result = _init(dirPtr);
    if (dataDir != null) malloc.free(dirPtr);
    return result;
  }

  void cleanup() {
    _cleanup();
  }

  String createCapsule(String name) {
    final namePtr = name.toNativeUtf8();
    final resultPtr = _createCapsule(namePtr);
    final result = resultPtr.toDartString();
    _freeString(resultPtr);
    malloc.free(namePtr);
    return result;
  }

  String listCapsules() {
    final resultPtr = _listCapsules();
    final result = resultPtr.toDartString();
    _freeString(resultPtr);
    return result;
  }

  String getCapsule(String capsuleId) {
    final idPtr = capsuleId.toNativeUtf8();
    final resultPtr = _getCapsule(idPtr);
    final result = resultPtr.toDartString();
    _freeString(resultPtr);
    malloc.free(idPtr);
    return result;
  }

  int startInvite(String capsuleId) {
    final idPtr = capsuleId.toNativeUtf8();
    final result = _startInvite(idPtr);
    malloc.free(idPtr);
    return result;
  }

  int startJoin(String pieceName) {
    final namePtr = pieceName.toNativeUtf8();
    final result = _startJoin(namePtr);
    malloc.free(namePtr);
    return result;
  }

  String getState() {
    final resultPtr = _getState();
    final result = resultPtr.toDartString();
    _freeString(resultPtr);
    return result;
  }

  int confirmPin() {
    return _confirmPin();
  }

  int submitPin(String pin) {
    final pinPtr = pin.toNativeUtf8();
    final result = _submitPin(pinPtr);
    malloc.free(pinPtr);
    return result;
  }

  int cancel() {
    return _cancel();
  }

  String addSimAccessory(String capsuleId, String name) {
    final idPtr = capsuleId.toNativeUtf8();
    final namePtr = name.toNativeUtf8();
    final resultPtr = _addSimAccessory(idPtr, namePtr);
    final result = resultPtr.toDartString();
    _freeString(resultPtr);
    malloc.free(idPtr);
    malloc.free(namePtr);
    return result;
  }
}
