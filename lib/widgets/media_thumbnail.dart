import 'package:flutter/material.dart';
import 'package:cached_network_image/cached_network_image.dart';
import '../models/media_item.dart';

class MediaThumbnail extends StatelessWidget {
  final MediaItem mediaItem;
  final VoidCallback onTap;

  const MediaThumbnail({
    super.key,
    required this.mediaItem,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 4,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
      ),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(12),
        child: Stack(
          children: [
            ClipRRect(
              borderRadius: BorderRadius.circular(12),
              child: AspectRatio(
                aspectRatio: _getAspectRatio(),
                child: CachedNetworkImage(
                  imageUrl: mediaItem.thumbnailUrl,
                  fit: BoxFit.cover,
                  placeholder: (context, url) => Container(
                    color: Colors.grey[300],
                    child: const Center(
                      child: CircularProgressIndicator(),
                    ),
                  ),
                  errorWidget: (context, url, error) => Container(
                    color: Colors.grey[300],
                    child: Icon(
                      _getMediaIcon(),
                      size: 40,
                      color: Colors.grey[600],
                    ),
                  ),
                ),
              ),
            ),
            
            // Media type indicator
            Positioned(
              top: 8,
              right: 8,
              child: Container(
                padding: const EdgeInsets.all(4),
                decoration: BoxDecoration(
                  color: Colors.black.withOpacity(0.7),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Icon(
                  _getMediaIcon(),
                  size: 16,
                  color: Colors.white,
                ),
              ),
            ),
            
            // Comments indicator
            if (mediaItem.comments.isNotEmpty)
              Positioned(
                bottom: 8,
                left: 8,
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                  decoration: BoxDecoration(
                    color: Colors.black.withOpacity(0.7),
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const Icon(
                        Icons.comment,
                        size: 12,
                        color: Colors.white,
                      ),
                      const SizedBox(width: 2),
                      Text(
                        '${mediaItem.comments.length}',
                        style: const TextStyle(
                          color: Colors.white,
                          fontSize: 10,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }

  double _getAspectRatio() {
    switch (mediaItem.mediaType) {
      case MediaType.image:
        return 1.0; // Square for images
      case MediaType.video:
        return 16 / 9; // Widescreen for videos
      case MediaType.audio:
        return 1.0; // Square for audio waveforms
    }
  }

  IconData _getMediaIcon() {
    switch (mediaItem.mediaType) {
      case MediaType.image:
        return Icons.image;
      case MediaType.video:
        return Icons.play_circle_outline;
      case MediaType.audio:
        return Icons.audiotrack;
    }
  }
}
