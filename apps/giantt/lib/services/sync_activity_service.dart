import 'dart:convert';
import 'dart:io';
import 'package:giantt_core/giantt_core.dart';
import 'device_nickname_service.dart';

/// A single op received from a remote device.
class RemoteOp {
  final String deviceId;
  final DateTime timestamp;
  final Map<String, dynamic> op;
  final String itemId;

  const RemoteOp({
    required this.deviceId,
    required this.timestamp,
    required this.op,
    required this.itemId,
  });

  /// Human-readable description of the op.
  String describe(GianttGraph graph) {
    final title = graph.items[itemId]?.title ?? itemId;
    final opKey = op.keys.first;
    switch (opKey) {
      case 'AddItem':
        return 'Added "$title"';
      case 'RemoveItem':
        return 'Removed "$itemId"';
      case 'SetField':
        final field = op['SetField']?['field'] as String? ?? '';
        final value = op['SetField']?['value'];
        return switch (field) {
          'title'    => 'Renamed to "$value"',
          'status'   => 'Set "$title" → ${_statusLabel(value?.toString())}',
          'priority' => 'Set "$title" priority → ${value?.toString().toLowerCase() ?? '?'}',
          'occluded' => value == true ? 'Occluded "$title"' : 'Restored "$title"',
          'duration' => 'Set "$title" duration to $value',
          'comment'  => 'Updated comment on "$title"',
          _          => 'Updated $field on "$title"',
        };
      case 'AddToSet':
        final set_ = op['AddToSet']?['set_name'] as String? ?? '';
        final el = op['AddToSet']?['element'];
        return switch (set_) {
          'tags'    => 'Tagged "$title" with "$el"',
          'charts'  => 'Added "$title" to chart "$el"',
          'requires'=> '"$title" now requires "$el"',
          'blocks'  => '"$title" now blocks "$el"',
          _         => 'Added $el to $set_ on "$title"',
        };
      case 'RemoveFromSet':
        final set_ = op['RemoveFromSet']?['set_name'] as String? ?? '';
        final el = op['RemoveFromSet']?['element'];
        return switch (set_) {
          'tags'    => 'Removed tag "$el" from "$title"',
          'charts'  => 'Removed "$title" from chart "$el"',
          'requires'=> '"$title" no longer requires "$el"',
          _         => 'Removed $el from $set_ on "$title"',
        };
      default:
        return opKey;
    }
  }

  static String _statusLabel(String? s) => switch (s) {
    'NOT_STARTED' => 'not started',
    'IN_PROGRESS' => 'in progress',
    'COMPLETED'   => 'completed',
    'BLOCKED'     => 'blocked',
    _             => s ?? '?',
  };
}

/// Activity from one device in a time window.
class DeviceActivity {
  final String deviceId;
  final String displayName;
  final DateTime latestTimestamp;
  final List<RemoteOp> ops;

  const DeviceActivity({
    required this.deviceId,
    required this.displayName,
    required this.latestTimestamp,
    required this.ops,
  });
}

class SyncActivityService {
  /// Read recent remote ops from the flow journals on disk.
  ///
  /// [journalsDir] — path to the flow's journals/ directory.
  /// [localDeviceId] — ops authored by this device are skipped.
  /// [since] — only return ops after this time (default: last 24 hours).
  static Future<List<DeviceActivity>> recentActivity({
    required String journalsDir,
    required String localDeviceId,
    Duration since = const Duration(hours: 24),
  }) async {
    final dir = Directory(journalsDir);
    if (!dir.existsSync()) return [];

    final cutoff = DateTime.now().subtract(since);
    final nicknames = DeviceNicknameService.instance;
    await nicknames.getAll();

    final activityByDevice = <String, List<RemoteOp>>{};

    for (final file in dir.listSync().whereType<File>()) {
      if (!file.path.endsWith('.jsonl')) continue;

      // Journal file name is the device UUID.
      final deviceId = file.path.split('/').last.replaceAll('.jsonl', '');
      if (deviceId == localDeviceId) continue;

      final lines = file.readAsLinesSync();
      for (final line in lines.reversed) {
        if (line.trim().isEmpty) continue;
        try {
          final json = jsonDecode(line) as Map<String, dynamic>;
          final ts = DateTime.fromMillisecondsSinceEpoch(
              (json['timestamp'] as num).toInt());
          if (ts.isBefore(cutoff)) break; // journals are chronological
          final op = json['op'] as Map<String, dynamic>;
          final itemId = extractItemId(op);
          activityByDevice.putIfAbsent(deviceId, () => []).add(RemoteOp(
            deviceId: deviceId,
            timestamp: ts,
            op: op,
            itemId: itemId,
          ));
        } catch (_) {
          continue;
        }
      }
    }

    final result = <DeviceActivity>[];
    for (final entry in activityByDevice.entries) {
      final ops = entry.value..sort((a, b) => b.timestamp.compareTo(a.timestamp));
      result.add(DeviceActivity(
        deviceId: entry.key,
        displayName: nicknames.displayNameSync(entry.key),
        latestTimestamp: ops.first.timestamp,
        ops: ops,
      ));
    }
    result.sort((a, b) => b.latestTimestamp.compareTo(a.latestTimestamp));
    return result;
  }

  static String extractItemId(Map<String, dynamic> op) {
    for (final v in op.values) {
      if (v is Map && v.containsKey('item_id')) return v['item_id'] as String;
    }
    return '?';
  }

  static String timeAgo(DateTime t) {
    final diff = DateTime.now().difference(t);
    if (diff.inSeconds < 60) return 'just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    return '${diff.inDays}d ago';
  }
}
