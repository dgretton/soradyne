import 'dart:ffi';
import 'package:ffi/ffi.dart';

class SoradyneClient {
  static SoradyneClient? _instance;
  late final DynamicLibrary _lib;
  bool _initialized = false;

  SoradyneClient._();

  static SoradyneClient get instance {
    _instance ??= SoradyneClient._();
    return _instance!;
  }

  Future<void> initialize({String? dataDirectory}) async {
    if (_initialized) return;
    
    _lib = _loadLibrary();
    final result = _lib.lookupFunction<Int32 Function(), int Function()>('soradyne_init')();
    
    if (result != 0) {
      throw Exception('Failed to initialize Soradyne core');
    }
    
    _initialized = true;
  }

  AlbumService get albums => AlbumService._(this);
  RealtimeMessaging get messaging => RealtimeMessaging._(this);

  DynamicLibrary _loadLibrary() {
    if (Platform.isMacOS) return DynamicLibrary.open('libsoradyne.dylib');
    if (Platform.isLinux) return DynamicLibrary.open('libsoradyne.so');
    if (Platform.isWindows) return DynamicLibrary.open('soradyne.dll');
    if (Platform.isAndroid) return DynamicLibrary.open('libsoradyne.so');
    if (Platform.isIOS) return DynamicLibrary.process();
    throw UnsupportedError('Platform not supported');
  }

  void dispose() {
    if (_initialized) {
      _lib.lookupFunction<Void Function(), void Function()>('soradyne_cleanup')();
      _initialized = false;
    }
  }
}
