import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import '../ui/widgets/space_description_dialog.dart';

class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  late LLMSettings _settings;

  final _openaiController = TextEditingController();
  final _anthropicController = TextEditingController();
  final _ollamaController = TextEditingController();
  final _personalController = TextEditingController();
  final _modelController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _settings = Provider.of<LLMSettings>(context, listen: false);
    _openaiController.text = _settings.openaiApiKey;
    _anthropicController.text = _settings.anthropicApiKey;
    _ollamaController.text = _settings.ollamaUrl;
    _personalController.text = _settings.personalServerUrl;
    _modelController.text = _settings.defaultModel;
  }

  @override
  void dispose() {
    _saveSettings();
    _openaiController.dispose();
    _anthropicController.dispose();
    _ollamaController.dispose();
    _personalController.dispose();
    _modelController.dispose();
    super.dispose();
  }


  Future<void> _saveSettings() async {
    _settings.setOpenaiApiKey(_openaiController.text);
    _settings.setAnthropicApiKey(_anthropicController.text);
    _settings.setOllamaUrl(_ollamaController.text);
    _settings.setPersonalServerUrl(_personalController.text);
    _settings.setDefaultModel(_modelController.text);

    await SettingsService.saveSettings(_settings);

    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Settings saved')),
      );
    }
  }

  Future<bool?> _showConfirmationDialog(String title, String content) {
    return showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(title),
        content: Text(content),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Confirm'),
            style: TextButton.styleFrom(
              foregroundColor: Theme.of(context).colorScheme.error,
            ),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    _settings = Provider.of<LLMSettings>(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
        actions: [
          IconButton(
            icon: const Icon(Icons.save),
            onPressed: _saveSettings,
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _buildSection(
              'API Keys',
              [
                _buildTextField(
                  controller: _openaiController,
                  label: 'OpenAI API Key',
                  obscureText: true,
                  hint: 'sk-...',
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _anthropicController,
                  label: 'Anthropic API Key',
                  obscureText: true,
                  hint: 'sk-ant-...',
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _ollamaController,
                  label: 'Ollama URL',
                  hint: 'http://localhost:11434',
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _personalController,
                  label: 'Personal Server URL',
                  hint: 'https://your-server.com:8080',
                ),
              ],
            ),
            const SizedBox(height: 24),
            _buildSection(
              'Model Preferences',
              [
                DropdownButtonFormField<LLMProvider>(
                  value: _settings.defaultProvider,
                  decoration: const InputDecoration(
                    labelText: 'Default Provider',
                    border: OutlineInputBorder(),
                  ),
                  items: LLMProvider.values.map((provider) {
                    return DropdownMenuItem(
                      value: provider,
                      child: Text(provider.displayName),
                    );
                  }).toList(),
                  onChanged: (provider) {
                    if (provider != null) {
                      setState(() {
                        _settings.setDefaultProvider(provider);
                      });
                    }
                  },
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _modelController,
                  label: 'Default Model',
                  hint: 'gpt-4, claude-3-sonnet, llama2, etc.',
                ),
              ],
            ),
            const SizedBox(height: 24),
            _buildSection(
              'Space Description',
              [
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(16),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Living Space Description',
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        const SizedBox(height: 8),
                        Text(
                          _settings.spaceDescription.isEmpty
                              ? 'No description set. Tap to add a detailed description of your living spaces.'
                              : _settings.spaceDescription.length > 100
                                  ? '${_settings.spaceDescription.substring(0, 100)}...'
                                  : _settings.spaceDescription,
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                            color: _settings.spaceDescription.isEmpty
                                ? Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.6)
                                : null,
                          ),
                        ),
                        const SizedBox(height: 12),
                        ElevatedButton.icon(
                          onPressed: () async {
                            final result = await showDialog<String>(
                              context: context,
                              builder: (context) => SpaceDescriptionDialog(
                                initialDescription: _settings.spaceDescription,
                              ),
                            );
                            if (result != null) {
                              setState(() {
                                _settings.setSpaceDescription(result);
                              });
                            }
                          },
                          icon: Icon(_settings.spaceDescription.isEmpty ? Icons.add : Icons.edit),
                          label: Text(_settings.spaceDescription.isEmpty ? 'Add Description' : 'Edit Description'),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 24),
            _buildSection(
              'History',
              [
                ElevatedButton.icon(
                  onPressed: () async {
                    final confirmed = await _showConfirmationDialog(
                      'Clear Chat History?',
                      'This will permanently delete all chat messages. This action cannot be undone.',
                    );
                    if (confirmed == true) {
                      await HistoryService().clearChatHistory();
                      if (!mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(content: Text('Chat history cleared')),
                      );
                    }
                  },
                  icon: const Icon(Icons.delete_sweep),
                  label: const Text('Clear Chat History'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Theme.of(context).colorScheme.error,
                    foregroundColor: Theme.of(context).colorScheme.onError,
                  ),
                ),
                const SizedBox(height: 12),
                ElevatedButton.icon(
                  onPressed: () async {
                    final confirmed = await _showConfirmationDialog(
                      'Clear Command History?',
                      'This will permanently delete all recorded command executions. This action cannot be undone.',
                    );
                    if (confirmed == true) {
                      await HistoryService().clearCommandHistory();
                      if (!mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(
                            content:
                                Text('Command execution history cleared')),
                      );
                    }
                  },
                  icon: const Icon(Icons.delete_forever),
                  label: const Text('Clear Command History'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Theme.of(context).colorScheme.error,
                    foregroundColor: Theme.of(context).colorScheme.onError,
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildSection(String title, List<Widget> children) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            color: Theme.of(context).colorScheme.primary,
          ),
        ),
        const SizedBox(height: 16),
        ...children,
      ],
    );
  }

  Widget _buildTextField({
    required TextEditingController controller,
    required String label,
    String? hint,
    bool obscureText = false,
  }) {
    return TextField(
      controller: controller,
      obscureText: obscureText,
      decoration: InputDecoration(
        labelText: label,
        hintText: hint,
        border: const OutlineInputBorder(),
        suffixIcon: obscureText
            ? IconButton(
                icon: Icon(obscureText ? Icons.visibility : Icons.visibility_off),
                onPressed: () {
                  // Toggle visibility - would need state management for this
                },
              )
            : null,
      ),
    );
  }
}
