import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';
import '../models/chat_message.dart';

class HistoryService {
  static const _chatHistoryKey = 'chat_history';
  static const _commandHistoryKey = 'command_history';
  static const _chatDraftKey = 'chat_message_draft';

  Future<void> saveChatHistory(List<ChatMessage> messages) async {
    final prefs = await SharedPreferences.getInstance();
    final historyJson = messages.map((m) => m.toJson()).toList();
    await prefs.setString(_chatHistoryKey, jsonEncode(historyJson));
  }

  Future<List<ChatMessage>> loadChatHistory() async {
    final prefs = await SharedPreferences.getInstance();
    final historyString = prefs.getString(_chatHistoryKey);
    if (historyString == null) {
      return [];
    }
    try {
      final List<dynamic> historyJson = jsonDecode(historyString);
      return historyJson
          .map((json) => ChatMessage.fromJson(json as Map<String, dynamic>))
          .toList();
    } catch (e) {
      print('Error loading chat history: $e');
      return [];
    }
  }

  Future<void> clearChatHistory() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_chatHistoryKey);
  }

  Future<void> logExecutedCommand(Map<String, dynamic> command) async {
    final prefs = await SharedPreferences.getInstance();
    final history = await loadCommandHistory();

    final newEntry = {
      'timestamp': DateTime.now().toIso8601String(),
      'command': command,
    };
    history.add(newEntry);

    await prefs.setString(_commandHistoryKey, jsonEncode(history));
  }

  Future<List<Map<String, dynamic>>> loadCommandHistory() async {
    final prefs = await SharedPreferences.getInstance();
    final historyString = prefs.getString(_commandHistoryKey);
    if (historyString == null) {
      return [];
    }
    try {
      final List<dynamic> historyJson = jsonDecode(historyString);
      return historyJson.cast<Map<String, dynamic>>();
    } catch (e) {
      print('Error loading command history: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> getCommandsSince(
      DateTime timestamp) async {
    final history = await loadCommandHistory();
    return history.where((entry) {
      try {
        final entryTimestamp = DateTime.parse(entry['timestamp'] as String);
        return entryTimestamp.isAfter(timestamp);
      } catch (e) {
        return false;
      }
    }).toList();
  }

  Future<void> clearCommandHistory() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_commandHistoryKey);
  }

  Future<void> saveChatDraft(String text) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_chatDraftKey, text);
  }

  Future<String?> loadChatDraft() async {
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString(_chatDraftKey);
  }

  Future<void> clearChatDraft() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_chatDraftKey);
  }
}
