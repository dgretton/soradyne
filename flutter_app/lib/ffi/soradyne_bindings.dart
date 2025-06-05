import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

// Define the C function signatures
typedef SoradyneInitC = Int32 Function();
typedef SoradyneInit = int Function();

typedef SoradyneGetAlbumsC = Pointer<Utf8> Function();
typedef SoradyneGetAlbums = Pointer<Utf8> Function();

typedef SoradyneCreateAlbumC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef SoradyneCreateAlbum = Pointer<Utf8> Function(Pointer<Utf8>);

typedef SoradyneGetAlbumItemsC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef SoradyneGetAlbumItems = Pointer<Utf8> Function(Pointer<Utf8>);

typedef SoradyneUploadMediaC = Int32 Function(Pointer<Utf8>, Pointer<Utf8>);
typedef SoradyneUploadMedia = int Function(Pointer<Utf8>, Pointer<Utf8>);

typedef SoradyneFreeStringC = Void Function(Pointer<Utf8>);
typedef SoradyneFreeString = void Function(Pointer<Utf8>);

typedef SoradyneCleanupC = Void Function();
typedef SoradyneCleanup = void Function();

class SoradyneBindings {
  late final DynamicLibrary _lib;
  late final SoradyneInit _init;
  late final SoradyneGetAlbums _getAlbums;
  late final SoradyneCreateAlbum _createAlbum;
  late final SoradyneGetAlbumItems _getAlbumItems;
  late final SoradyneUploadMedia _uploadMedia;
  late final SoradyneFreeString _freeString;
  late final SoradyneCleanup _cleanup;

  SoradyneBindings() {
    // Load the dynamic library
    if (Platform.isMacOS) {
      // For macOS, the library should be bundled with the app
      _lib = DynamicLibrary.open('libsoradyne.dylib');
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

    // Bind the functions
    _init = _lib.lookupFunction<SoradyneInitC, SoradyneInit>('soradyne_init');
    _getAlbums = _lib.lookupFunction<SoradyneGetAlbumsC, SoradyneGetAlbums>('soradyne_get_albums');
    _createAlbum = _lib.lookupFunction<SoradyneCreateAlbumC, SoradyneCreateAlbum>('soradyne_create_album');
    _getAlbumItems = _lib.lookupFunction<SoradyneGetAlbumItemsC, SoradyneGetAlbumItems>('soradyne_get_album_items');
    _uploadMedia = _lib.lookupFunction<SoradyneUploadMediaC, SoradyneUploadMedia>('soradyne_upload_media');
    _freeString = _lib.lookupFunction<SoradyneFreeStringC, SoradyneFreeString>('soradyne_free_string');
    _cleanup = _lib.lookupFunction<SoradyneCleanupC, SoradyneCleanup>('soradyne_cleanup');
  }

  int init() {
    return _init();
  }

  String getAlbums() {
    final ptr = _getAlbums();
    final result = ptr.toDartString();
    _freeString(ptr);
    return result;
  }

  String createAlbum(String name) {
    final namePtr = name.toNativeUtf8();
    final resultPtr = _createAlbum(namePtr);
    final result = resultPtr.toDartString();
    _freeString(resultPtr);
    malloc.free(namePtr);
    return result;
  }

  String getAlbumItems(String albumId) {
    final albumIdPtr = albumId.toNativeUtf8();
    final resultPtr = _getAlbumItems(albumIdPtr);
    final result = resultPtr.toDartString();
    _freeString(resultPtr);
    malloc.free(albumIdPtr);
    return result;
  }

  int uploadMedia(String albumId, String filePath) {
    final albumIdPtr = albumId.toNativeUtf8();
    final filePathPtr = filePath.toNativeUtf8();
    final result = _uploadMedia(albumIdPtr, filePathPtr);
    malloc.free(albumIdPtr);
    malloc.free(filePathPtr);
    return result;
  }

  void cleanup() {
    _cleanup();
  }
}
