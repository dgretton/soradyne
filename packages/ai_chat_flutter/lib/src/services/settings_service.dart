import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';
import '../models/llm_settings.dart';

class SettingsService {
  static const String _settingsKey = 'llm_settings';

  static Future<void> saveSettings(LLMSettings settings) async {
    final prefs = await SharedPreferences.getInstance();
    final json = jsonEncode(settings.toJson());
    await prefs.setString(_settingsKey, json);
  }

  static Future<LLMSettings> loadSettings() async {
    final prefs = await SharedPreferences.getInstance();
    final jsonString = prefs.getString(_settingsKey);

    final settings = LLMSettings();
    if (jsonString != null) {
      try {
        final json = jsonDecode(jsonString) as Map<String, dynamic>;
        settings.fromJson(json);
      } catch (e) {
        // If there's an error loading settings, use defaults
        print('Error loading settings: $e');
      }
    }

    return settings;
  }
}
