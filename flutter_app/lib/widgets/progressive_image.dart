import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'dart:typed_data';
import '../models/media_item.dart';
import '../services/album_service.dart';

class ProgressiveImage extends StatefulWidget {
  final MediaItem mediaItem;
  final String albumId;
  final double? width;
  final double? height;
  final BoxFit fit;

  const ProgressiveImage({
    super.key,
    required this.mediaItem,
    required this.albumId,
    this.width,
    this.height,
    this.fit = BoxFit.cover,
  });

  @override
  State<ProgressiveImage> createState() => _ProgressiveImageState();
}

class _ProgressiveImageState extends State<ProgressiveImage> {
  Uint8List? _currentImageData;
  String _currentResolution = 'loading';
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();
    _loadImage();
  }

  void _loadImage() {
    // Real progressive loading: placeholder → thumbnail → medium → high
    _loadProgressively();
  }

  void _loadProgressively() async {
    // Start with placeholder
    setState(() {
      _currentResolution = 'placeholder';
      _isLoading = true;
    });

    // Check if we already have thumbnail data
    if (widget.mediaItem.hasThumbnailData) {
      setState(() {
        _currentImageData = Uint8List.fromList(widget.mediaItem.thumbnailData!);
        _currentResolution = 'thumbnail';
      });
    }

    // Load medium resolution
    await _loadResolution('medium');
    
    // Load high resolution for images (not for videos to save bandwidth)
    if (widget.mediaItem.mediaType != MediaType.video) {
      await _loadResolution('high');
    }
    
    setState(() {
      _isLoading = false;
    });
  }

  Future<void> _loadResolution(String resolution) async {
    if (!mounted) return;
    
    final albumService = context.read<AlbumService>();
    final data = await albumService.loadMediaAtResolution(
      widget.albumId, 
      widget.mediaItem.id, 
      resolution
    );
    
    if (mounted && data != null) {
      setState(() {
        _currentImageData = Uint8List.fromList(data);
        _currentResolution = resolution;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        // Main image with smooth transitions
        AnimatedSwitcher(
          duration: const Duration(milliseconds: 300),
          child: Container(
            key: ValueKey(_currentResolution),
            width: widget.width,
            height: widget.height,
            decoration: BoxDecoration(
              color: Colors.grey[300],
              borderRadius: BorderRadius.circular(8),
            ),
            child: _buildImageContent(),
          ),
        ),
        
        // Loading indicator for higher resolutions
        if (_isLoading && _currentResolution != 'placeholder')
          Positioned(
            bottom: 4,
            right: 4,
            child: Container(
              padding: const EdgeInsets.all(4),
              decoration: BoxDecoration(
                color: Colors.black54,
                borderRadius: BorderRadius.circular(4),
              ),
              child: const SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  valueColor: AlwaysStoppedAnimation<Color>(Colors.white),
                ),
              ),
            ),
          ),
        
        // Resolution indicator
        Positioned(
          bottom: 4,
          left: 4,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
            decoration: BoxDecoration(
              color: Colors.black54,
              borderRadius: BorderRadius.circular(4),
            ),
            child: Text(
              _currentResolution.toUpperCase(),
              style: const TextStyle(
                color: Colors.white,
                fontSize: 10,
                fontWeight: FontWeight.bold,
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildImageContent() {
    switch (_currentResolution) {
      case 'placeholder':
        return _buildPlaceholder();
      case 'thumbnail':
        return _buildThumbnailPlaceholder();
      case 'thumbnail':
      case 'medium':
      case 'high':
        if (_currentImageData != null) {
          return ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: Stack(
              fit: StackFit.expand,
              children: [
                Image.memory(
                  _currentImageData!,
                  width: widget.width,
                  height: widget.height,
                  fit: widget.fit,
                  errorBuilder: (context, error, stackTrace) {
                    print('Error loading image data: $error');
                    return _buildErrorWidget();
                  },
                ),
                // Add video play icon overlay for videos
                if (widget.mediaItem.mediaType == MediaType.video)
                  const Center(
                    child: Icon(
                      Icons.play_circle_outline,
                      color: Colors.white,
                      size: 32,
                      shadows: [
                        Shadow(
                          offset: Offset(1, 1),
                          blurRadius: 3,
                          color: Colors.black54,
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          );
        } else {
          return _buildErrorWidget();
        }
      case 'error':
        return _buildErrorWidget();
      default:
        return _buildPlaceholder();
    }
  }

  Widget _buildErrorWidget() {
    return Container(
      width: widget.width,
      height: widget.height,
      decoration: BoxDecoration(
        color: Colors.grey[300],
        borderRadius: BorderRadius.circular(8),
      ),
      child: const Icon(
        Icons.error,
        color: Colors.red,
        size: 32,
      ),
    );
  }

  Widget _buildPlaceholder() {
    return Container(
      width: widget.width,
      height: widget.height,
      decoration: BoxDecoration(
        color: Colors.grey[300],
        borderRadius: BorderRadius.circular(8),
      ),
      child: const Icon(
        Icons.image,
        color: Colors.grey,
        size: 32,
      ),
    );
  }

  Widget _buildThumbnailPlaceholder() {
    return Container(
      width: widget.width,
      height: widget.height,
      decoration: BoxDecoration(
        color: Colors.grey[200],
        borderRadius: BorderRadius.circular(8),
      ),
      child: Stack(
        children: [
          // Simulated low-res blur effect
          Container(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  Colors.grey[400]!,
                  Colors.grey[300]!,
                  Colors.grey[400]!,
                ],
              ),
              borderRadius: BorderRadius.circular(8),
            ),
          ),
          const Center(
            child: Icon(
              Icons.image,
              color: Colors.white70,
              size: 24,
            ),
          ),
        ],
      ),
    );
  }
}
