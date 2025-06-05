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
    if (!_initialized) return;

    try {
      final itemsJson = _bindings.getAlbumItems(albumId);
      final List<dynamic> itemsList = json.decode(itemsJson);
      _albumItems[albumId] = itemsList.map((json) => MediaItem.fromJson(json, albumId)).toList();
      notifyListeners();
      debugPrint('Loaded ${_albumItems[albumId]?.length ?? 0} items for album $albumId');
    } catch (e) {
      debugPrint('Error loading album items: $e');
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
    if (!_initialized) return false;

    try {
      final result = _bindings.uploadMedia(albumId, file.path);
      
      if (result == 0) {
        await loadAlbumItems(albumId); // Refresh the album items
        debugPrint('Uploaded media: ${file.path}');
        return true;
      } else {
        debugPrint('Failed to upload media: $result');
        return false;
      }
    } catch (e) {
      debugPrint('Error uploading media: $e');
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

  @override
  void dispose() {
    if (_initialized) {
      _bindings.cleanup();
    }
    super.dispose();
  }
}
