import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/album_service.dart';

class StorageStatusWidget extends StatefulWidget {
  @override
  _StorageStatusWidgetState createState() => _StorageStatusWidgetState();
}

class _StorageStatusWidgetState extends State<StorageStatusWidget> {
  Timer? _statusTimer;
  Map<String, dynamic> _status = {};

  @override
  void initState() {
    super.initState();
    _startStatusPolling();
  }

  void _startStatusPolling() {
    _updateStatus();
    _statusTimer = Timer.periodic(Duration(seconds: 2), (_) => _updateStatus());
  }

  void _updateStatus() {
    try {
      final albumService = context.read<AlbumService>();
      if (albumService.bindings != null) {
        final statusJson = albumService.bindings.getStorageStatus();
        setState(() {
          _status = json.decode(statusJson);
        });
      } else {
        print('AlbumService bindings not initialized yet');
      }
    } catch (e) {
      print('Error getting storage status: $e');
      // Set default status on error
      setState(() {
        _status = {
          'available_devices': 0,
          'required_threshold': 3,
          'can_read_data': false,
          'missing_devices': 3,
          'device_paths': [],
        };
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final canRead = _status['can_read_data'] ?? false;
    final available = _status['available_devices'] ?? 0;
    final required = _status['required_threshold'] ?? 3;
    final missing = _status['missing_devices'] ?? 3;

    return Card(
      color: canRead ? Colors.green[100] : Colors.orange[100],
      child: Padding(
        padding: EdgeInsets.all(16),
        child: Column(
          children: [
            Row(
              children: [
                Icon(
                  canRead ? Icons.sd_card : Icons.warning,
                  color: canRead ? Colors.green : Colors.orange,
                ),
                SizedBox(width: 8),
                Text(
                  canRead 
                    ? 'Storage Ready' 
                    : 'Insert $missing more SD card${missing > 1 ? 's' : ''}',
                  style: TextStyle(fontWeight: FontWeight.bold),
                ),
              ],
            ),
            SizedBox(height: 8),
            Text('$available of $required SD cards detected'),
            if (!canRead)
              ElevatedButton(
                onPressed: () {
                  try {
                    final albumService = context.read<AlbumService>();
                    if (albumService.bindings != null) {
                      albumService.bindings.refreshStorage();
                      _updateStatus();
                    }
                  } catch (e) {
                    print('Error refreshing storage: $e');
                  }
                },
                child: Text('Refresh'),
              ),
          ],
        ),
      ),
    );
  }

  @override
  void dispose() {
    _statusTimer?.cancel();
    super.dispose();
  }
}
