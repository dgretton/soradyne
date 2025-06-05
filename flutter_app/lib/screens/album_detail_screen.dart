import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:cached_network_image/cached_network_image.dart';
import 'package:photo_view/photo_view.dart';
import 'package:photo_view/photo_view_gallery.dart';
import 'package:image_picker/image_picker.dart';
import 'dart:io';
import '../services/album_service.dart';
import '../models/album.dart';
import '../models/media_item.dart';

class AlbumDetailScreen extends StatefulWidget {
  final Album album;

  const AlbumDetailScreen({super.key, required this.album});

  @override
  State<AlbumDetailScreen> createState() => _AlbumDetailScreenState();
}

class _AlbumDetailScreenState extends State<AlbumDetailScreen> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<AlbumService>().loadAlbumItems(widget.album.id);
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.album.name),
        actions: [
          IconButton(
            icon: const Icon(Icons.add_a_photo),
            onPressed: () {
              print('Add photo button pressed');
              _pickAndUploadImage();
            },
          ),
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () => context.read<AlbumService>().loadAlbumItems(widget.album.id),
          ),
        ],
      ),
      body: Consumer<AlbumService>(
        builder: (context, albumService, child) {
          final items = albumService.getAlbumItems(widget.album.id);
          
          print('Album ${widget.album.id} has ${items.length} items');

          if (albumService.isLoading) {
            return const Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  CircularProgressIndicator(),
                  SizedBox(height: 16),
                  Text('Loading album contents...'),
                ],
              ),
            );
          }

          if (items.isEmpty) {
            return Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  const Icon(
                    Icons.photo_outlined,
                    size: 64,
                    color: Colors.grey,
                  ),
                  const SizedBox(height: 16),
                  const Text(
                    'No media yet',
                    style: TextStyle(
                      fontSize: 18,
                      color: Colors.grey,
                    ),
                  ),
                  const SizedBox(height: 8),
                  const Text(
                    'Add photos, videos, or audio files',
                    style: TextStyle(
                      color: Colors.grey,
                    ),
                  ),
                  const SizedBox(height: 24),
                  ElevatedButton.icon(
                    onPressed: () {
                      print('Add Media button pressed');
                      _pickAndUploadImage();
                    },
                    icon: const Icon(Icons.add_a_photo),
                    label: const Text('Add Media'),
                  ),
                ],
              ),
            );
          }

          return Padding(
            padding: const EdgeInsets.all(8.0),
            child: GridView.builder(
              gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                crossAxisCount: 3,
                crossAxisSpacing: 4,
                mainAxisSpacing: 4,
              ),
              itemCount: items.length,
              itemBuilder: (context, index) {
                final item = items[index];
                return _MediaThumbnail(
                  item: item,
                  albumId: widget.album.id,
                  onTap: () => _openMediaViewer(context, items, index),
                );
              },
            ),
          );
        },
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: () {
          print('Floating action button pressed');
          _pickAndUploadImage();
        },
        child: const Icon(Icons.add),
      ),
    );
  }

  void _pickAndUploadImage() async {
    print('_pickAndUploadImage called');
    
    try {
      final picker = ImagePicker();
      print('ImagePicker created, attempting to pick image...');
      
      // Try multiple sources and show a dialog to let user choose
      final result = await showDialog<ImageSource>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Select Image Source'),
          content: const Text('Choose where to get your image from:'),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, ImageSource.camera),
              child: const Text('Camera'),
            ),
            TextButton(
              onPressed: () => Navigator.pop(context, ImageSource.gallery),
              child: const Text('Gallery'),
            ),
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel'),
            ),
          ],
        ),
      );
      
      if (result == null) {
        print('User cancelled image source selection');
        return;
      }
      
      print('User selected source: $result');
      
      final pickedFile = await picker.pickImage(
        source: result,
        imageQuality: 85,
      );
      
      print('Image picker returned: ${pickedFile?.path ?? 'null'}');
      
      if (pickedFile != null && mounted) {
        print('Image picked successfully: ${pickedFile.path}');
        
        // Show loading indicator
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Row(
              children: [
                SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                SizedBox(width: 16),
                Text('Uploading media...'),
              ],
            ),
            duration: Duration(seconds: 10),
          ),
        );
        
        final file = File(pickedFile.path);
        print('Attempting to upload file: ${file.path}');
        print('File exists: ${await file.exists()}');
        print('File size: ${await file.length()} bytes');
        
        final success = await context.read<AlbumService>().uploadMedia(widget.album.id, file);
        
        if (mounted) {
          ScaffoldMessenger.of(context).hideCurrentSnackBar();
          
          if (success) {
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(
                content: Text('Media uploaded successfully!'),
                backgroundColor: Colors.green,
              ),
            );
          } else {
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(
                content: Text('Failed to upload media'),
                backgroundColor: Colors.red,
              ),
            );
          }
        }
      } else {
        print('No image was picked or widget not mounted');
      }
    } catch (e) {
      print('Error in _pickAndUploadImage: $e');
      print('Error type: ${e.runtimeType}');
      print('Stack trace: ${StackTrace.current}');
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error picking image: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  void _openMediaViewer(BuildContext context, List<MediaItem> items, int initialIndex) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (context) => MediaViewerScreen(
          items: items,
          initialIndex: initialIndex,
          albumId: widget.album.id,
        ),
      ),
    );
  }
}

class _MediaThumbnail extends StatelessWidget {
  final MediaItem item;
  final String albumId;
  final VoidCallback onTap;

  const _MediaThumbnail({
    required this.item,
    required this.albumId,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: Colors.grey.shade300),
        ),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(8),
          child: Stack(
            fit: StackFit.expand,
            children: [
              CachedNetworkImage(
                imageUrl: item.thumbnailUrl(albumId),
                fit: BoxFit.cover,
                placeholder: (context, url) => Container(
                  color: Colors.grey.shade200,
                  child: const Center(
                    child: CircularProgressIndicator(),
                  ),
                ),
                errorWidget: (context, url, error) => Container(
                  color: Colors.grey.shade200,
                  child: Icon(
                    _getMediaIcon(item.mediaType),
                    color: Colors.grey.shade600,
                    size: 32,
                  ),
                ),
              ),
              if (item.mediaType == MediaType.video)
                const Center(
                  child: Icon(
                    Icons.play_circle_outline,
                    color: Colors.white,
                    size: 32,
                  ),
                ),
              if (item.mediaType == MediaType.audio)
                const Center(
                  child: Icon(
                    Icons.audiotrack,
                    color: Colors.white,
                    size: 32,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }

  IconData _getMediaIcon(MediaType type) {
    switch (type) {
      case MediaType.video:
        return Icons.videocam;
      case MediaType.audio:
        return Icons.audiotrack;
      case MediaType.image:
      default:
        return Icons.image;
    }
  }
}

class MediaViewerScreen extends StatefulWidget {
  final List<MediaItem> items;
  final int initialIndex;
  final String albumId;

  const MediaViewerScreen({
    super.key,
    required this.items,
    required this.initialIndex,
    required this.albumId,
  });

  @override
  State<MediaViewerScreen> createState() => _MediaViewerScreenState();
}

class _MediaViewerScreenState extends State<MediaViewerScreen> {
  late PageController _pageController;
  int _currentIndex = 0;

  @override
  void initState() {
    super.initState();
    _currentIndex = widget.initialIndex;
    _pageController = PageController(initialPage: widget.initialIndex);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        backgroundColor: Colors.black,
        iconTheme: const IconThemeData(color: Colors.white),
        title: Text(
          '${_currentIndex + 1} of ${widget.items.length}',
          style: const TextStyle(color: Colors.white),
        ),
      ),
      body: PhotoViewGallery.builder(
        pageController: _pageController,
        itemCount: widget.items.length,
        builder: (context, index) {
          final item = widget.items[index];
          return PhotoViewGalleryPageOptions(
            imageProvider: CachedNetworkImageProvider(
              item.highResUrl(widget.albumId),
            ),
            minScale: PhotoViewComputedScale.contained,
            maxScale: PhotoViewComputedScale.covered * 2,
            heroAttributes: PhotoViewHeroAttributes(tag: item.id),
          );
        },
        onPageChanged: (index) {
          setState(() {
            _currentIndex = index;
          });
        },
        scrollPhysics: const BouncingScrollPhysics(),
        backgroundDecoration: const BoxDecoration(color: Colors.black),
      ),
    );
  }

  @override
  void dispose() {
    _pageController.dispose();
    super.dispose();
  }
}
