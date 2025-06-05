import 'package:flutter/material.dart';
import 'dart:typed_data';
import '../models/media_item.dart';

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
    // Simulate progressive loading: placeholder → thumbnail → full
    _loadProgressively();
  }

  void _loadProgressively() async {
    // Start with placeholder
    setState(() {
      _currentResolution = 'placeholder';
      _isLoading = true;
    });

    // Simulate loading delay for thumbnail
    await Future.delayed(const Duration(milliseconds: 100));
    
    if (mounted) {
      setState(() {
        _currentResolution = 'thumbnail';
      });
    }

    // Load the actual image data after a short delay
    await Future.delayed(const Duration(milliseconds: 300));
    
    if (mounted && widget.mediaItem.hasImageData) {
      setState(() {
        _currentImageData = Uint8List.fromList(widget.mediaItem.imageData!);
        _currentResolution = 'full';
        _isLoading = false;
      });
    } else if (mounted) {
      setState(() {
        _isLoading = false;
        _currentResolution = 'error';
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
      case 'full':
        if (_currentImageData != null) {
          return ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: Image.memory(
              _currentImageData!,
              width: widget.width,
              height: widget.height,
              fit: widget.fit,
              errorBuilder: (context, error, stackTrace) {
                return _buildErrorWidget();
              },
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
=======
import 'package:flutter/material.dart';
import 'dart:typed_data';
import '../models/media_item.dart';

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
    // Simulate progressive loading: placeholder → thumbnail → full
    _loadProgressively();
  }

  void _loadProgressively() async {
    // Start with placeholder
    setState(() {
      _currentResolution = 'placeholder';
      _isLoading = true;
    });

    // Simulate loading delay for thumbnail
    await Future.delayed(const Duration(milliseconds: 100));
    
    if (mounted) {
      setState(() {
        _currentResolution = 'thumbnail';
      });
    }

    // Load the actual image data after a short delay
    await Future.delayed(const Duration(milliseconds: 300));
    
    if (mounted && widget.mediaItem.hasImageData) {
      setState(() {
        _currentImageData = Uint8List.fromList(widget.mediaItem.imageData!);
        _currentResolution = 'full';
        _isLoading = false;
      });
    } else if (mounted) {
      setState(() {
        _isLoading = false;
        _currentResolution = 'error';
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
      case 'full':
        if (_currentImageData != null) {
          return ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: Image.memory(
              _currentImageData!,
              width: widget.width,
              height: widget.height,
              fit: widget.fit,
              errorBuilder: (context, error, stackTrace) {
                return _buildErrorWidget();
              },
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
}
