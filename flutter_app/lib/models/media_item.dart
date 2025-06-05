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

  // For now, return placeholder URLs since FFI doesn't serve HTTP endpoints
  // In a full implementation, you'd implement image loading via FFI as well
  String thumbnailUrl(String albumId) => 'https://via.placeholder.com/150x150.png?text=Media';
  String mediumResUrl(String albumId) => 'https://via.placeholder.com/600x600.png?text=Media';
  String highResUrl(String albumId) => 'https://via.placeholder.com/1200x1200.png?text=Media';

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
