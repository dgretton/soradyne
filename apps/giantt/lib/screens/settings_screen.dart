import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import '../services/device_nickname_service.dart';
import '../services/giantt_service.dart';

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
          const SizedBox(height: 28),
          Text('Device Nicknames',
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 4),
          Text(
            'Give readable names to peer devices shown in the sync strip.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          const SizedBox(height: 12),
          _DeviceNicknameEditor(),
        ],
      ),
    );
  }
}

class _DeviceNicknameEditor extends StatefulWidget {
  @override
  State<_DeviceNicknameEditor> createState() => _DeviceNicknameEditorState();
}

class _DeviceNicknameEditorState extends State<_DeviceNicknameEditor> {
  Map<String, String> _nicknames = {};
  String? _localDeviceId;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final nicknames = await DeviceNicknameService.instance.getAll();
    final localId = await GianttService().localDeviceId;
    if (mounted) {
      setState(() {
        _nicknames = Map.from(nicknames);
        _localDeviceId = localId;
        _loading = false;
      });
    }
  }

  Future<void> _edit(String deviceId) async {
    final controller =
        TextEditingController(text: _nicknames[deviceId] ?? '');
    final result = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(deviceId.substring(0, 8) + '…'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(
            labelText: 'Nickname',
            hintText: 'e.g. MacBook, Linux server',
          ),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx),
              child: const Text('Cancel')),
          TextButton(
              onPressed: () => Navigator.pop(ctx, controller.text.trim()),
              child: const Text('Save')),
        ],
      ),
    );
    if (result != null) {
      await DeviceNicknameService.instance.setNickname(deviceId, result);
      await _load();
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) return const SizedBox(height: 40, child: Center(child: CircularProgressIndicator(strokeWidth: 2)));
    if (_nicknames.isEmpty) {
      return Text(
        'No peer devices seen yet. They appear here once sync runs.',
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
      );
    }
    return Column(
      children: [
        for (final entry in _nicknames.entries)
          ListTile(
            dense: true,
            leading: const Icon(Icons.devices_other, size: 20),
            title: Text(entry.value.isEmpty
                ? entry.key.substring(0, 8) + '…'
                : entry.value),
            subtitle: Text(entry.key.substring(0, 16) + '…',
                style: Theme.of(context).textTheme.bodySmall),
            trailing: entry.key == _localDeviceId
                ? const Chip(label: Text('this device'))
                : IconButton(
                    icon: const Icon(Icons.edit, size: 18),
                    onPressed: () => _edit(entry.key),
                  ),
          ),
      ],
    );
  }
}
