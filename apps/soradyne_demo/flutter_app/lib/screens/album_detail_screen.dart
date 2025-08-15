import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:photo_view/photo_view.dart';
import 'package:photo_view/photo_view_gallery.dart';
import 'package:image_picker/image_picker.dart';
import 'dart:io';
import 'dart:typed_data';
import '../services/album_service.dart';
import '../models/album.dart';
import '../models/media_item.dart';
import '../widgets/progressive_image.dart';

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
              child: DragTarget<Object>(
                onWillAcceptWithDetails: (details) {
                  print('Drag will accept check: ${details.data} (type: ${details.data.runtimeType})');
                  // Accept any data - we'll handle conversion in onAccept
                  return details.data != null;
                },
                onAcceptWithDetails: (details) {
                  print('Data dropped: ${details.data} (type: ${details.data.runtimeType})');
                  _handleDroppedData(details.data);
                },
                builder: (context, candidateData, rejectedData) {
                  final isDragOver = candidateData.isNotEmpty;
                  return AnimatedContainer(
                    duration: const Duration(milliseconds: 200),
                    decoration: BoxDecoration(
                      color: isDragOver ? Colors.blue.withOpacity(0.1) : Colors.transparent,
                      border: Border.all(
                        color: isDragOver ? Colors.blue : Colors.grey.shade300,
                        width: 2,
                        style: BorderStyle.solid,
                      ),
                      borderRadius: BorderRadius.circular(12),
                    ),
                    padding: const EdgeInsets.all(40),
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(
                          isDragOver ? Icons.cloud_upload : Icons.photo_outlined,
                          size: 64,
                          color: isDragOver ? Colors.blue : Colors.grey,
                        ),
                        const SizedBox(height: 16),
                        Text(
                          isDragOver ? 'Drop files here!' : 'No media yet',
                          style: TextStyle(
                            fontSize: 18,
                            color: isDragOver ? Colors.blue : Colors.grey,
                            fontWeight: isDragOver ? FontWeight.bold : FontWeight.normal,
                          ),
                        ),
                        const SizedBox(height: 8),
                        Text(
                          isDragOver 
                            ? 'Release to upload photos, videos, or audio'
                            : 'Supports JPG, PNG, MP4, MOV, MP3, WAV, and more',
                          style: TextStyle(
                            color: isDragOver ? Colors.blue : Colors.grey,
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
                },
              ),
            );
          }

          return DragTarget<Object>(
            onWillAcceptWithDetails: (details) {
              print('Grid drag will accept check: ${details.data} (type: ${details.data.runtimeType})');
              return details.data != null;
            },
            onAcceptWithDetails: (details) {
              print('Data dropped on grid: ${details.data} (type: ${details.data.runtimeType})');
              _handleDroppedData(details.data);
            },
            builder: (context, candidateData, rejectedData) {
              final isDragOver = candidateData.isNotEmpty;
              return Container(
                decoration: isDragOver ? BoxDecoration(
                  color: Colors.blue.withOpacity(0.1),
                  border: Border.all(color: Colors.blue, width: 2),
                  borderRadius: BorderRadius.circular(8),
                ) : null,
                child: Padding(
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
                ),
              );
            },
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
    _showMediaPickerDialog();
  }

  void _showMediaPickerDialog() {
    showModalBottomSheet(
      context: context,
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.photo_camera),
              title: const Text('Take Photo'),
              onTap: () {
                Navigator.pop(context);
                _pickMedia(MediaType.image, ImageSource.camera);
              },
            ),
            ListTile(
              leading: const Icon(Icons.photo_library),
              title: const Text('Choose Photos'),
              onTap: () {
                Navigator.pop(context);
                _pickMedia(MediaType.image, ImageSource.gallery);
              },
            ),
            ListTile(
              leading: const Icon(Icons.videocam),
              title: const Text('Record Video'),
              onTap: () {
                Navigator.pop(context);
                _pickMedia(MediaType.video, ImageSource.camera);
              },
            ),
            ListTile(
              leading: const Icon(Icons.video_library),
              title: const Text('Choose Videos'),
              onTap: () {
                Navigator.pop(context);
                _pickMedia(MediaType.video, ImageSource.gallery);
              },
            ),
            ListTile(
              leading: const Icon(Icons.audiotrack),
              title: const Text('Choose Audio'),
              onTap: () {
                Navigator.pop(context);
                _pickAudioFile();
              },
            ),
          ],
        ),
      ),
    );
  }

  void _pickMedia(MediaType mediaType, ImageSource source) async {
    print('_pickMedia called with type: $mediaType, source: $source');
    
    try {
      final picker = ImagePicker();
      XFile? pickedFile;
      
      switch (mediaType) {
        case MediaType.image:
          print('Picking image...');
          pickedFile = await picker.pickImage(
            source: source,
            imageQuality: 85,
          );
          break;
        case MediaType.video:
          print('Picking video...');
          pickedFile = await picker.pickVideo(
            source: source,
            maxDuration: const Duration(minutes: 10), // Reasonable limit
          );
          break;
        case MediaType.audio:
          // Audio picking will be handled separately
          return;
      }
      
      print('Media picker returned: ${pickedFile?.path ?? 'null'}');
      
      if (pickedFile != null && mounted) {
        print('Media picked successfully: ${pickedFile.path}');
        await _uploadPickedFile(pickedFile);
      } else {
        print('No media was picked or widget not mounted');
      }
    } catch (e) {
      print('Error in _pickMedia: $e');
      print('Error type: ${e.runtimeType}');
      print('Stack trace: ${StackTrace.current}');
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error picking media: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  void _pickAudioFile() async {
    print('_pickAudioFile called');
    
    try {
      // For audio files, we'll need to use file_picker package
      // For now, show a message that audio support is coming
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Audio file support coming soon! For now, you can drag and drop audio files.'),
            duration: Duration(seconds: 3),
          ),
        );
      }
    } catch (e) {
      print('Error in _pickAudioFile: $e');
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error picking audio: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  Future<void> _uploadPickedFile(XFile pickedFile) async {
    // Show loading indicator
    if (mounted) {
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
    }
    
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
  }

  void _handleDroppedData(Object data) async {
    print('Handling dropped data: $data (type: ${data.runtimeType})');
    
    List<File> files = [];
    
    // Convert different data types to File objects
    if (data is List<File>) {
      files = data;
    } else if (data is List<String>) {
      // File paths as strings
      files = data.map((path) => File(path)).toList();
    } else if (data is String) {
      // Single file path
      files = [File(data)];
    } else if (data is List) {
      // Try to convert list items to files
      for (final item in data) {
        if (item is String) {
          files.add(File(item));
        } else if (item is File) {
          files.add(item);
        } else {
          print('Unknown item type in dropped list: ${item.runtimeType}');
        }
      }
    } else {
      print('Unsupported drop data type: ${data.runtimeType}');
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Unsupported file drop format'),
            backgroundColor: Colors.red,
          ),
        );
      }
      return;
    }
    
    if (files.isEmpty) {
      print('No valid files found in dropped data');
      return;
    }
    
    _handleDroppedFiles(files);
  }

  void _handleDroppedFiles(List<File> files) async {
    print('Handling ${files.length} dropped files');
    
    for (final file in files) {
      print('Processing dropped file: ${file.path}');
      
      // Show loading indicator
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Row(
              children: [
                const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 16),
                Expanded(child: Text('Uploading ${file.path.split('/').last}...')),
              ],
            ),
            duration: const Duration(seconds: 10),
          ),
        );
      }
      
      final success = await context.read<AlbumService>().uploadMedia(widget.album.id, file);
      
      if (mounted) {
        ScaffoldMessenger.of(context).hideCurrentSnackBar();
        
        if (success) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text('${file.path.split('/').last} uploaded successfully!'),
              backgroundColor: Colors.green,
            ),
          );
        } else {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text('Failed to upload ${file.path.split('/').last}'),
              backgroundColor: Colors.red,
            ),
          );
        }
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
              // Progressive image loading
              ProgressiveImage(
                mediaItem: item,
                albumId: albumId,
                fit: BoxFit.cover,
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
            imageProvider: item.hasDisplayData
                ? MemoryImage(Uint8List.fromList(item.displayData!))
                : MemoryImage(Uint8List.fromList([0])), // Empty placeholder
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
