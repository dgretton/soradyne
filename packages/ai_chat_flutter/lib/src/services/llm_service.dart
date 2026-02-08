import 'dart:convert';
import 'package:http/http.dart' as http;
import '../models/chat_message.dart';
import '../models/llm_settings.dart';

abstract class LLMService {
  Future<String> sendMessage(List<ChatMessage> messages, String context);
}

class AnthropicService implements LLMService {
  final String apiKey;
  final String model;

  AnthropicService({required this.apiKey, required this.model});

  @override
  Future<String> sendMessage(List<ChatMessage> messages, String context) async {
    final url = Uri.parse('https://api.anthropic.com/v1/messages');

    final headers = {
      'Content-Type': 'application/json',
      'x-api-key': apiKey,
      'anthropic-version': '2023-06-01',
    };

    final apiMessages = messages.map((m) {
      return {
        'role': m.isUser ? 'user' : 'assistant',
        'content': m.content,
      };
    }).toList();

    final requestBody = {
      'model': model,
      'max_tokens': 1024,
      'system': context,
      'messages': apiMessages,
    };

    final bodyJson = jsonEncode(requestBody);
    final stopwatch = Stopwatch()..start();

    try {
      final response = await http.post(
        url,
        headers: headers,
        body: bodyJson,
      );

      stopwatch.stop();

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['content'][0]['text'] as String;
      } else {
        throw Exception('Failed to get response: ${response.statusCode} ${response.body}');
      }
    } catch (e) {
      stopwatch.stop();
      throw Exception('Network request failed: $e');
    }
  }
}

class OpenAIService implements LLMService {
  final String apiKey;
  final String model;

  OpenAIService({required this.apiKey, required this.model});

  @override
  Future<String> sendMessage(List<ChatMessage> messages, String context) async {
    final url = Uri.parse('https://api.openai.com/v1/chat/completions');

    final headers = {
      'Content-Type': 'application/json',
      'Authorization': 'Bearer $apiKey',
    };

    final apiMessages = messages.map((m) {
      return {
        'role': m.isUser ? 'user' : 'assistant',
        'content': m.content,
      };
    }).toList();

    final requestBody = {
      'model': model,
      'messages': [
        {'role': 'system', 'content': context},
        ...apiMessages,
      ],
      'max_tokens': 1024,
    };

    final bodyJson = jsonEncode(requestBody);
    final stopwatch = Stopwatch()..start();

    final response = await http.post(
      url,
      headers: headers,
      body: bodyJson,
    );

    stopwatch.stop();

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body);
      return data['choices'][0]['message']['content'] as String;
    } else {
      throw Exception('Failed to get response: ${response.statusCode} ${response.body}');
    }
  }
}

class LLMServiceFactory {
  static LLMService? createService(LLMSettings settings) {
    switch (settings.defaultProvider) {
      case LLMProvider.anthropic:
        if (settings.anthropicApiKey.isNotEmpty) {
          return AnthropicService(
            apiKey: settings.anthropicApiKey,
            model: settings.defaultModel,
          );
        }
        break;
      case LLMProvider.openai:
        if (settings.openaiApiKey.isNotEmpty) {
          return OpenAIService(
            apiKey: settings.openaiApiKey,
            model: settings.defaultModel,
          );
        }
        break;
      case LLMProvider.ollama:
      case LLMProvider.personal:
        // TODO: Implement Ollama and personal server services
        break;
    }
    return null;
  }
}
