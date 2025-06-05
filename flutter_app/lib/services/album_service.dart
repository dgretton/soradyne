import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';
import '../models/album.dart';
import '../models/media_item.dart';

class AlbumService extends ChangeNotifier {
  static const String baseUrl = 'http://localhost:3031/api';
  
  List<Album> _albums = [];
  Map<String, List<MediaItem>> _albumItems = {};
  bool _isLoading = false;

  List<Album> get albums => _albums;
  bool get isLoading => _isLoading;

  List<MediaItem> getAlbumItems(String albumId) {
    return _albumItems[albumId] ?? [];
  }

  Future<void> loadAlbums() async {
    _isLoading = true;
    notifyListeners();

    try {
      final response = await http.get(Uri.parse('$baseUrl/albums'));
      if (response.statusCode == 200) {
        final List<dynamic> albumsJson = json.decode(response.body);
        _albums = albumsJson.map((json) => Album.fromJson(json)).toList();
      }
    } catch (e) {
      debugPrint('Error loading albums: $e');
    }

    _isLoading = false;
    notifyListeners();
  }

  Future<void> loadAlbumItems(String albumId) async {
    _isLoading = true;
    notifyListeners();

    try {
      final response = await http.get(Uri.parse('$baseUrl/albums/$albumId'));
      if (response.statusCode == 200) {
        final List<dynamic> itemsJson = json.decode(response.body);
        _albumItems[albumId] = itemsJson.map((json) => MediaItem.fromJson(json)).toList();
      }
    } catch (e) {
      debugPrint('Error loading album items: $e');
    }

    _isLoading = false;
    notifyListeners();
  }

  Future<void> createAlbum(String name) async {
    try {
      final response = await http.post(
        Uri.parse('$baseUrl/albums'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode({'name': name}),
      );
      
      if (response.statusCode == 200) {
        await loadAlbums(); // Refresh the list
      }
    } catch (e) {
      debugPrint('Error creating album: $e');
    }
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
