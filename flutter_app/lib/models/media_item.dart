enum MediaType { image, video, audio }

class MediaItem {
  final String id;
  final String filename;
  final MediaType mediaType;
  final int size;
  final double rotation;
  final bool hasCrop;
  final int markupCount;
  final List<Comment> comments;

  MediaItem({
    required this.id,
    required this.filename,
    required this.mediaType,
    required this.size,
    this.rotation = 0.0,
    this.hasCrop = false,
    this.markupCount = 0,
    this.comments = const [],
  });

  // Image data will be loaded via FFI instead of URLs
  List<int>? _imageData;
  List<int>? _thumbnailData;
  
  void setImageData(List<int> data) {
    _imageData = data;
  }
  
  void setThumbnailData(List<int> data) {
    _thumbnailData = data;
  }
  
  List<int>? get imageData => _imageData;
  List<int>? get thumbnailData => _thumbnailData;
  
  bool get hasImageData => _imageData != null;
  bool get hasThumbnailData => _thumbnailData != null;
  
  // For videos, we should use thumbnail data for display
  List<int>? get displayData {
    if (mediaType == MediaType.video) {
      return _thumbnailData ?? _imageData;
    }
    return _imageData;
  }
  
  bool get hasDisplayData {
    if (mediaType == MediaType.video) {
      return _thumbnailData != null || _imageData != null;
    }
    return _imageData != null;
  }

  factory MediaItem.fromJson(Map<String, dynamic> json, String albumId) {
    MediaType type;
    switch (json['media_type']) {
      case 'video':
        type = MediaType.video;
        break;
      case 'audio':
        type = MediaType.audio;
        break;
      default:
        type = MediaType.image;
    }

    return MediaItem(
      id: json['id'],
      filename: json['filename'],
      mediaType: type,
      size: json['size'],
      rotation: (json['rotation'] ?? 0.0).toDouble(),
      hasCrop: json['has_crop'] ?? false,
      markupCount: json['markup_count'] ?? 0,
      comments: (json['comments'] as List?)
          ?.map((c) => Comment.fromJson(c))
          .toList() ?? [],
    );
  }
}

class Comment {
  final String author;
  final String text;
  final DateTime timestamp;

  Comment({
    required this.author,
    required this.text,
    required this.timestamp,
  });

  factory Comment.fromJson(Map<String, dynamic> json) {
    return Comment(
      author: json['author'],
      text: json['text'],
      timestamp: DateTime.fromMillisecondsSinceEpoch(json['timestamp'] * 1000),
    );
  }
}
