import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';
import 'package:path/path.dart' as path;

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

typedef SoradyneGetMediaDataC = Int32 Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);
typedef SoradyneGetMediaData = int Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);

typedef SoradyneGetMediaThumbnailC = Int32 Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);
typedef SoradyneGetMediaThumbnail = int Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);

typedef SoradyneGetMediaMediumC = Int32 Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);
typedef SoradyneGetMediaMedium = int Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);

typedef SoradyneGetMediaHighC = Int32 Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);
typedef SoradyneGetMediaHigh = int Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Pointer<Uint8>>, Pointer<Size>);

typedef SoradyneFreeMediaDataC = Void Function(Pointer<Uint8>, Size);
typedef SoradyneFreeMediaData = void Function(Pointer<Uint8>, int);

typedef SoradyneCleanupC = Void Function();
typedef SoradyneCleanup = void Function();

class SoradyneBindings {
  late final DynamicLibrary _lib;
  late final SoradyneInit _init;
  late final SoradyneGetAlbums _getAlbums;
  late final SoradyneCreateAlbum _createAlbum;
  late final SoradyneGetAlbumItems _getAlbumItems;
  late final SoradyneUploadMedia _uploadMedia;
  late final SoradyneGetMediaData _getMediaData;
  late final SoradyneGetMediaThumbnail _getMediaThumbnail;
  late final SoradyneGetMediaMedium _getMediaMedium;
  late final SoradyneGetMediaHigh _getMediaHigh;
  late final SoradyneFreeMediaData _freeMediaData;
  late final SoradyneFreeString _freeString;
  late final SoradyneCleanup _cleanup;

  SoradyneBindings() {
    // Load the dynamic library
    if (Platform.isMacOS) {
      // For macOS, the library should be bundled with the app
      try {
        // Try loading from the app bundle's MacOS directory first
        final executablePath = Platform.resolvedExecutable;
        final appDir = path.dirname(executablePath);
        final dylibPath = path.join(appDir, 'libsoradyne.dylib');
        print('Attempting to load dylib from: $dylibPath');
        _lib = DynamicLibrary.open(dylibPath);
        print('Successfully loaded dylib from: $dylibPath');
      } catch (e) {
        print('Failed to load from app bundle: $e');
        print('Attempting fallback to loading by name...');
        // Fallback to loading by name
        _lib = DynamicLibrary.open('libsoradyne.dylib');
        print('Successfully loaded dylib by name');
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

    // Bind the functions
    _init = _lib.lookupFunction<SoradyneInitC, SoradyneInit>('soradyne_init');
    _getAlbums = _lib.lookupFunction<SoradyneGetAlbumsC, SoradyneGetAlbums>('soradyne_get_albums');
    _createAlbum = _lib.lookupFunction<SoradyneCreateAlbumC, SoradyneCreateAlbum>('soradyne_create_album');
    _getAlbumItems = _lib.lookupFunction<SoradyneGetAlbumItemsC, SoradyneGetAlbumItems>('soradyne_get_album_items');
    _uploadMedia = _lib.lookupFunction<SoradyneUploadMediaC, SoradyneUploadMedia>('soradyne_upload_media');
    _getMediaData = _lib.lookupFunction<SoradyneGetMediaDataC, SoradyneGetMediaData>('soradyne_get_media_data');
    _getMediaThumbnail = _lib.lookupFunction<SoradyneGetMediaThumbnailC, SoradyneGetMediaThumbnail>('soradyne_get_media_thumbnail');
    _getMediaMedium = _lib.lookupFunction<SoradyneGetMediaMediumC, SoradyneGetMediaMedium>('soradyne_get_media_medium');
    _getMediaHigh = _lib.lookupFunction<SoradyneGetMediaHighC, SoradyneGetMediaHigh>('soradyne_get_media_high');
    _freeMediaData = _lib.lookupFunction<SoradyneFreeMediaDataC, SoradyneFreeMediaData>('soradyne_free_media_data');
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
    print('FFI uploadMedia called with albumId: $albumId, filePath: $filePath');
    
    final albumIdPtr = albumId.toNativeUtf8();
    final filePathPtr = filePath.toNativeUtf8();
    
    print('Calling native uploadMedia function...');
    final result = _uploadMedia(albumIdPtr, filePathPtr);
    print('Native uploadMedia returned: $result');
    
    malloc.free(albumIdPtr);
    malloc.free(filePathPtr);
    return result;
  }

  List<int>? getMediaData(String albumId, String mediaId) {
    return _getMediaAtResolution(albumId, mediaId, _getMediaData, 'getMediaData');
  }

  List<int>? getMediaThumbnail(String albumId, String mediaId) {
    return _getMediaAtResolution(albumId, mediaId, _getMediaThumbnail, 'getMediaThumbnail');
  }

  List<int>? getMediaMedium(String albumId, String mediaId) {
    return _getMediaAtResolution(albumId, mediaId, _getMediaMedium, 'getMediaMedium');
  }

  List<int>? getMediaHigh(String albumId, String mediaId) {
    return _getMediaAtResolution(albumId, mediaId, _getMediaHigh, 'getMediaHigh');
  }

  List<int>? _getMediaAtResolution(String albumId, String mediaId, Function nativeFunction, String functionName) {
    print('FFI $functionName called with albumId: $albumId, mediaId: $mediaId');
    
    final albumIdPtr = albumId.toNativeUtf8();
    final mediaIdPtr = mediaId.toNativeUtf8();
    final dataPtrPtr = malloc<Pointer<Uint8>>();
    final sizePtr = malloc<Size>();
    
    try {
      print('Calling native $functionName function...');
      final result = nativeFunction(albumIdPtr, mediaIdPtr, dataPtrPtr, sizePtr);
      print('Native $functionName returned: $result');
      
      if (result == 0) {
        final dataPtr = dataPtrPtr.value;
        final size = sizePtr.value;
        
        if (dataPtr != nullptr && size > 0) {
          print('Retrieved media data: $size bytes');
          final data = dataPtr.asTypedList(size).toList();
          _freeMediaData(dataPtr, size);
          return data;
        }
      }
      
      return null;
    } finally {
      malloc.free(albumIdPtr);
      malloc.free(mediaIdPtr);
      malloc.free(dataPtrPtr);
      malloc.free(sizePtr);
    }
  }

  void cleanup() {
    _cleanup();
  }
}
