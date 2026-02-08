import 'package:flutter/foundation.dart';

enum LLMProvider {
  openai('OpenAI'),
  anthropic('Anthropic'),
  ollama('Ollama (Local)'),
  personal('Personal Server');

  const LLMProvider(this.displayName);
  final String displayName;
}

class LLMSettings extends ChangeNotifier {
  String _openaiApiKey = '';
  String _anthropicApiKey = '';
  String _ollamaUrl = 'http://localhost:11434';
  String _personalServerUrl = '';
  LLMProvider _defaultProvider = LLMProvider.anthropic;
  String _defaultModel = 'claude-sonnet-4-0';
  String _spaceDescription = '';

  // Getters
  String get openaiApiKey => _openaiApiKey;
  String get anthropicApiKey => _anthropicApiKey;
  String get ollamaUrl => _ollamaUrl;
  String get personalServerUrl => _personalServerUrl;
  LLMProvider get defaultProvider => _defaultProvider;
  String get defaultModel => _defaultModel;
  String get spaceDescription => _spaceDescription;

  // Setters
  void setOpenaiApiKey(String key) {
    _openaiApiKey = key;
    notifyListeners();
  }

  void setAnthropicApiKey(String key) {
    _anthropicApiKey = key;
    notifyListeners();
  }

  void setOllamaUrl(String url) {
    _ollamaUrl = url;
    notifyListeners();
  }

  void setPersonalServerUrl(String url) {
    _personalServerUrl = url;
    notifyListeners();
  }

  void setDefaultProvider(LLMProvider provider) {
    _defaultProvider = provider;
    notifyListeners();
  }

  void setDefaultModel(String model) {
    _defaultModel = model;
    notifyListeners();
  }

  void setSpaceDescription(String description) {
    _spaceDescription = description;
    notifyListeners();
  }

  Map<String, dynamic> toJson() {
    return {
      'openaiApiKey': _openaiApiKey,
      'anthropicApiKey': _anthropicApiKey,
      'ollamaUrl': _ollamaUrl,
      'personalServerUrl': _personalServerUrl,
      'defaultProvider': _defaultProvider.name,
      'defaultModel': _defaultModel,
      'spaceDescription': _spaceDescription,
    };
  }

  void fromJson(Map<String, dynamic> json) {
    _openaiApiKey = json['openaiApiKey'] ?? '';
    _anthropicApiKey = json['anthropicApiKey'] ?? '';
    _ollamaUrl = json['ollamaUrl'] ?? 'http://localhost:11434';
    _personalServerUrl = json['personalServerUrl'] ?? '';
    _defaultProvider = LLMProvider.values.firstWhere(
      (p) => p.name == json['defaultProvider'],
      orElse: () => LLMProvider.anthropic,
    );
    _defaultModel = json['defaultModel'] ?? 'claude-sonnet-4-0';
    _spaceDescription = json['spaceDescription'] ?? '';
    notifyListeners();
  }
}
