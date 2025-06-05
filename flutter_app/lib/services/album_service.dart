import 'package:flutter/foundation.dart';
import 'dart:convert';
import 'dart:io';
import '../models/album.dart';
import '../models/media_item.dart';
import '../ffi/soradyne_bindings.dart';

class AlbumService extends ChangeNotifier {
  late final SoradyneBindings _bindings;
  
  List<Album> _albums = [];
  Map<String, List<MediaItem>> _albumItems = {};
  bool _isLoading = false;
  String? _error;
  bool _initialized = false;

  List<Album> get albums => _albums;
  bool get isLoading => _isLoading;
  String? get error => _error;

  AlbumService() {
    _initializeBindings();
  }

  void _initializeBindings() {
    try {
      _bindings = SoradyneBindings();
      final result = _bindings.init();
      if (result == 0) {
        _initialized = true;
        debugPrint('Soradyne FFI initialized successfully');
      } else {
        _error = 'Failed to initialize Soradyne FFI';
        debugPrint('Failed to initialize Soradyne FFI: $result');
      }
    } catch (e) {
      _error = 'FFI initialization error: $e';
      debugPrint('FFI initialization error: $e');
    }
  }

  List<MediaItem> getAlbumItems(String albumId) {
    return _albumItems[albumId] ?? [];
  }

  Future<void> loadAlbums() async {
    if (!_initialized) {
      _error = 'Soradyne not initialized';
      notifyListeners();
      return;
    }

    _isLoading = true;
    _error = null;
    notifyListeners();

    try {
      final albumsJson = _bindings.getAlbums();
      final List<dynamic> albumsList = json.decode(albumsJson);
      _albums = albumsList.map((json) => Album.fromJson(json)).toList();
      _error = null;
      debugPrint('Loaded ${_albums.length} albums via FFI');
    } catch (e) {
      _error = 'Error loading albums: $e';
      debugPrint('Error loading albums: $e');
    }

    _isLoading = false;
    notifyListeners();
  }

  Future<void> loadAlbumItems(String albumId) async {
    if (!_initialized) {
      debugPrint('Cannot load album items: FFI not initialized');
      return;
    }

    try {
      debugPrint('Loading items for album: $albumId');
      final itemsJson = _bindings.getAlbumItems(albumId);
      debugPrint('Raw items JSON: $itemsJson');
      
      final List<dynamic> itemsList = json.decode(itemsJson);
      debugPrint('Parsed items list: $itemsList');
      
      final items = itemsList.map((json) => MediaItem.fromJson(json, albumId)).toList();
      
      // Load thumbnail data for each item (for immediate display)
      for (final item in items) {
        debugPrint('Loading thumbnail data for media: ${item.id}');
        final thumbnailData = _bindings.getMediaThumbnail(albumId, item.id);
        if (thumbnailData != null) {
          item.setThumbnailData(thumbnailData);
          debugPrint('Loaded ${thumbnailData.length} bytes thumbnail for media: ${item.id}');
        } else {
          debugPrint('Failed to load thumbnail data for media: ${item.id}');
        }
      }
      
      _albumItems[albumId] = items;
      notifyListeners();
      debugPrint('Loaded ${_albumItems[albumId]?.length ?? 0} items for album $albumId');
    } catch (e) {
      debugPrint('Error loading album items: $e');
      debugPrint('Stack trace: ${StackTrace.current}');
    }
  }

  Future<bool> createAlbum(String name) async {
    if (!_initialized) return false;

    try {
      final resultJson = _bindings.createAlbum(name);
      final result = json.decode(resultJson);
      
      if (result.containsKey('error')) {
        debugPrint('Error creating album: ${result['error']}');
        return false;
      } else {
        await loadAlbums(); // Refresh the list
        debugPrint('Created album: ${result['name']}');
        return true;
      }
    } catch (e) {
      debugPrint('Error creating album: $e');
      return false;
    }
  }

  Future<bool> uploadMedia(String albumId, File file) async {
    if (!_initialized) {
      debugPrint('Cannot upload media: FFI not initialized');
      return false;
    }

    try {
      // Verify file exists and is readable
      if (!await file.exists()) {
        debugPrint('File does not exist: ${file.path}');
        return false;
      }
      
      final fileSize = await file.length();
      debugPrint('Uploading file: ${file.path} (${fileSize} bytes) to album: $albumId');
      
      final result = _bindings.uploadMedia(albumId, file.path);
      debugPrint('FFI upload result: $result');
      
      if (result == 0) {
        debugPrint('Upload successful, refreshing album items...');
        await loadAlbumItems(albumId); // Refresh the album items
        debugPrint('Album items refreshed');
        return true;
      } else {
        debugPrint('FFI upload failed with code: $result');
        return false;
      }
    } catch (e) {
      debugPrint('Exception during media upload: $e');
      return false;
    }
  }

  Future<void> rotateMedia(String albumId, String mediaId, double degrees) async {
    // TODO: Implement rotation via FFI
    debugPrint('Rotation not yet implemented via FFI');
  }

  Future<void> addComment(String albumId, String mediaId, String text) async {
    // TODO: Implement comments via FFI
    debugPrint('Comments not yet implemented via FFI');
  }

  Future<List<int>?> loadMediaAtResolution(String albumId, String mediaId, String resolution) async {
    if (!_initialized) {
      debugPrint('Cannot load media: FFI not initialized');
      return null;
    }

    try {
      debugPrint('Loading $resolution resolution for media: $mediaId');
      
      List<int>? data;
      switch (resolution) {
        case 'thumbnail':
          data = _bindings.getMediaThumbnail(albumId, mediaId);
          break;
        case 'medium':
          data = _bindings.getMediaMedium(albumId, mediaId);
          break;
        case 'high':
          data = _bindings.getMediaHigh(albumId, mediaId);
          break;
        default:
          data = _bindings.getMediaData(albumId, mediaId);
      }
      
      if (data != null) {
        debugPrint('Loaded ${data.length} bytes at $resolution resolution for media: $mediaId');
        
        // Update the media item in the album items cache
        final items = _albumItems[albumId];
        if (items != null) {
          final item = items.firstWhere((item) => item.id == mediaId, orElse: () => items.first);
          switch (resolution) {
            case 'thumbnail':
              item.setThumbnailData(data);
              break;
            case 'medium':
              item.setMediumData(data);
              break;
            case 'high':
              item.setHighData(data);
              break;
          }
          notifyListeners();
        }
        
        return data;
      } else {
        debugPrint('Failed to load $resolution resolution for media: $mediaId');
        return null;
      }
    } catch (e) {
      debugPrint('Error loading $resolution resolution for media $mediaId: $e');
      return null;
    }
  }

  @override
  void dispose() {
    if (_initialized) {
      _bindings.cleanup();
    }
    super.dispose();
  }
}
