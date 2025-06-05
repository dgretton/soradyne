import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
import 'dart:io';
import '../models/album.dart';
import '../models/media_item.dart';

class AlbumService extends ChangeNotifier {
  static const String baseUrl = 'http://localhost:3030/api';
  
  List<Album> _albums = [];
  Map<String, List<MediaItem>> _albumItems = {};
  bool _isLoading = false;
  String? _error;

  List<Album> get albums => _albums;
  bool get isLoading => _isLoading;
  String? get error => _error;

  List<MediaItem> getAlbumItems(String albumId) {
    return _albumItems[albumId] ?? [];
  }

  Future<void> loadAlbums() async {
    _isLoading = true;
    _error = null;
    notifyListeners();

    try {
      final response = await http.get(Uri.parse('$baseUrl/albums'));
      
      if (response.statusCode == 200) {
        final List<dynamic> albumsJson = json.decode(response.body);
        _albums = albumsJson.map((json) => Album.fromJson(json)).toList();
        _error = null;
      } else {
        _error = 'Failed to load albums: ${response.statusCode}';
      }
    } catch (e) {
      _error = 'Network error: $e';
      debugPrint('Error loading albums: $e');
    }

    _isLoading = false;
    notifyListeners();
  }

  Future<void> loadAlbumItems(String albumId) async {
    try {
      final response = await http.get(Uri.parse('$baseUrl/albums/$albumId'));
      
      if (response.statusCode == 200) {
        final List<dynamic> itemsJson = json.decode(response.body);
        _albumItems[albumId] = itemsJson.map((json) => MediaItem.fromJson(json, albumId)).toList();
        notifyListeners();
      }
    } catch (e) {
      debugPrint('Error loading album items: $e');
    }
  }

  Future<bool> createAlbum(String name) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/albums'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({'name': name}),
      );
      
      if (response.statusCode == 200) {
        await loadAlbums(); // Refresh the list
        return true;
      }
    } catch (e) {
      debugPrint('Error creating album: $e');
    }
    return false;
  }

  Future<bool> uploadMedia(String albumId, File file) async {
    try {
      var request = http.MultipartRequest('POST', Uri.parse('$baseUrl/albums/$albumId/media'));
      request.files.add(await http.MultipartFile.fromPath('file', file.path));
      
      var response = await request.send();
      
      if (response.statusCode == 200) {
        await loadAlbumItems(albumId); // Refresh the album items
        return true;
      }
    } catch (e) {
      debugPrint('Error uploading media: $e');
    }
    return false;
  }

  Future<void> rotateMedia(String albumId, String mediaId, double degrees) async {
    try {
      await http.post(
        Uri.parse('$baseUrl/albums/$albumId/media/$mediaId/rotate'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({
          'degrees': degrees,
          'author': 'flutter_user',
        }),
      );
      
      // Refresh the album items
      await loadAlbumItems(albumId);
    } catch (e) {
      debugPrint('Error rotating media: $e');
    }
  }

  Future<void> addComment(String albumId, String mediaId, String text) async {
    try {
      await http.post(
        Uri.parse('$baseUrl/albums/$albumId/media/$mediaId/comments'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({
          'text': text,
          'author': 'flutter_user',
        }),
      );
      
      // Refresh the album items
      await loadAlbumItems(albumId);
    } catch (e) {
      debugPrint('Error adding comment: $e');
    }
  }
}
