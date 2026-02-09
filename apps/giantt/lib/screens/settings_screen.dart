import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController _anthropicKeyController;
  late final TextEditingController _openaiKeyController;
  late final TextEditingController _projectDescriptionController;

  @override
  void initState() {
    super.initState();
    final settings = Provider.of<LLMSettings>(context, listen: false);
    _anthropicKeyController = TextEditingController(text: settings.anthropicApiKey);
    _openaiKeyController = TextEditingController(text: settings.openaiApiKey);
    _projectDescriptionController = TextEditingController(text: settings.spaceDescription);
  }

  @override
  void dispose() {
    _anthropicKeyController.dispose();
    _openaiKeyController.dispose();
    _projectDescriptionController.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final settings = Provider.of<LLMSettings>(context, listen: false);
    settings.setAnthropicApiKey(_anthropicKeyController.text.trim());
    settings.setOpenaiApiKey(_openaiKeyController.text.trim());
    settings.setSpaceDescription(_projectDescriptionController.text.trim());
    await SettingsService.saveSettings(settings);

    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Settings saved.'), backgroundColor: Colors.green),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
        actions: [
          IconButton(icon: const Icon(Icons.save), tooltip: 'Save', onPressed: _save),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text('LLM API Keys', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 12),
          TextField(
            controller: _anthropicKeyController,
            decoration: const InputDecoration(
              labelText: 'Anthropic API Key',
              border: OutlineInputBorder(),
              prefixIcon: Icon(Icons.key),
            ),
            obscureText: true,
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _openaiKeyController,
            decoration: const InputDecoration(
              labelText: 'OpenAI API Key',
              border: OutlineInputBorder(),
              prefixIcon: Icon(Icons.key),
            ),
            obscureText: true,
          ),
          const SizedBox(height: 24),
          Text('Project Description', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 4),
          Text(
            'Describe your project or workspace to give the AI context.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _projectDescriptionController,
            decoration: const InputDecoration(
              labelText: 'Description',
              border: OutlineInputBorder(),
              alignLabelWithHint: true,
            ),
            maxLines: 5,
          ),
        ],
      ),
    );
  }
}
