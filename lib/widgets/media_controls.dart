import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../models/media_item.dart';
import '../services/album_service.dart';

class MediaControls extends StatelessWidget {
  final MediaItem mediaItem;
  final String albumId;

  const MediaControls({
    super.key,
    required this.mediaItem,
    required this.albumId,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      child: Column(
        children: [
          // Rotation controls
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
            children: [
              _ControlButton(
                icon: Icons.rotate_left,
                label: 'Rotate Left',
                onPressed: () {
                  context.read<AlbumService>().rotateMedia(
                    albumId,
                    mediaItem.id,
                    -90,
                  );
                },
              ),
              _ControlButton(
                icon: Icons.rotate_right,
                label: 'Rotate Right',
                onPressed: () {
                  context.read<AlbumService>().rotateMedia(
                    albumId,
                    mediaItem.id,
                    90,
                  );
                },
              ),
            ],
          ),
          
          const SizedBox(height: 16),
          
          // Additional controls based on media type
          if (mediaItem.mediaType == MediaType.image) ...[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceEvenly,
              children: [
                _ControlButton(
                  icon: Icons.crop,
                  label: 'Crop',
                  onPressed: () {
                    // TODO: Implement crop
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Crop coming soon!')),
                    );
                  },
                ),
                _ControlButton(
                  icon: Icons.tune,
                  label: 'Adjust',
                  onPressed: () {
                    // TODO: Implement adjustments
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Adjustments coming soon!')),
                    );
                  },
                ),
              ],
            ),
          ] else if (mediaItem.mediaType == MediaType.video) ...[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceEvenly,
              children: [
                _ControlButton(
                  icon: Icons.content_cut,
                  label: 'Trim',
                  onPressed: () {
                    // TODO: Implement trim
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Trim coming soon!')),
                    );
                  },
                ),
                _ControlButton(
                  icon: Icons.speed,
                  label: 'Speed',
                  onPressed: () {
                    // TODO: Implement speed adjustment
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Speed adjustment coming soon!')),
                    );
                  },
                ),
              ],
            ),
          ] else if (mediaItem.mediaType == MediaType.audio) ...[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceEvenly,
              children: [
                _ControlButton(
                  icon: Icons.content_cut,
                  label: 'Trim',
                  onPressed: () {
                    // TODO: Implement audio trim
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Audio trim coming soon!')),
                    );
                  },
                ),
                _ControlButton(
                  icon: Icons.volume_up,
                  label: 'Volume',
                  onPressed: () {
                    // TODO: Implement volume adjustment
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Volume adjustment coming soon!')),
                    );
                  },
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class _ControlButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final VoidCallback onPressed;

  const _ControlButton({
    required this.icon,
    required this.label,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton(
          onPressed: onPressed,
          icon: Icon(icon, color: Colors.white),
          style: IconButton.styleFrom(
            backgroundColor: Colors.white.withOpacity(0.2),
            padding: const EdgeInsets.all(12),
          ),
        ),
        const SizedBox(height: 4),
        Text(
          label,
          style: const TextStyle(
            color: Colors.white,
            fontSize: 12,
          ),
        ),
      ],
    );
  }
}
