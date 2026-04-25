import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';

/// Stores human-readable nicknames for peer devices, keyed by device UUID.
/// Lives entirely in app storage — nothing soradyne-specific.
class DeviceNicknameService {
  static const _prefsKey = 'device_nicknames';

  static DeviceNicknameService? _instance;
  static DeviceNicknameService get instance {
    _instance ??= DeviceNicknameService._();
    return _instance!;
  }
  DeviceNicknameService._();

  Map<String, String> _nicknames = {};
  bool _loaded = false;

  Future<void> _ensureLoaded() async {
    if (_loaded) return;
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_prefsKey);
    if (raw != null) {
      final decoded = jsonDecode(raw) as Map<String, dynamic>;
      _nicknames = decoded.map((k, v) => MapEntry(k, v as String));
    }
    _loaded = true;
  }

  Future<Map<String, String>> getAll() async {
    await _ensureLoaded();
    return Map.unmodifiable(_nicknames);
  }

  /// Returns the nickname for [deviceId], or the first 8 chars of the UUID.
  Future<String> displayName(String deviceId) async {
    await _ensureLoaded();
    return _nicknames[deviceId] ?? deviceId.substring(0, deviceId.length.clamp(0, 8));
  }

  /// Synchronous version — call after [getAll()] has been awaited.
  String displayNameSync(String deviceId) {
    return _nicknames[deviceId] ?? deviceId.substring(0, deviceId.length.clamp(0, 8));
  }

  Future<void> setNickname(String deviceId, String nickname) async {
    await _ensureLoaded();
    if (nickname.isEmpty) {
      _nicknames.remove(deviceId);
    } else {
      _nicknames[deviceId] = nickname;
    }
    await _persist();
  }

  Future<void> _persist() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_prefsKey, jsonEncode(_nicknames));
  }
}
