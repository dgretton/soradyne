import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import 'dart:io';
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
  // All known peer device IDs discovered from journal files.
  List<String> _knownDevices = [];
  String? _localDeviceId;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final service = GianttService();
    final nicknames = await DeviceNicknameService.instance.getAll();
    final localId = await service.localDeviceId;
    final discovered = await _discoverDevicesFromJournals(service, localId);
    if (mounted) {
      setState(() {
        _nicknames = Map.from(nicknames);
        _localDeviceId = localId;
        _knownDevices = discovered;
        _loading = false;
      });
    }
  }

  /// Reads journal filenames from the flow directory — each file is named
  /// <device-uuid>.jsonl — to discover which peer devices have ever synced.
  Future<List<String>> _discoverDevicesFromJournals(
      GianttService service, String? localId) async {
    final soradyneDir = service.soradyneDataDir;
    final flowUuid = service.primaryFlowUuid;
    if (soradyneDir == null || flowUuid == null) return [];

    final journalsDir = Directory('$soradyneDir/flows/$flowUuid/journals');
    if (!journalsDir.existsSync()) return [];

    final ids = <String>[];
    for (final f in journalsDir.listSync().whereType<File>()) {
      if (!f.path.endsWith('.jsonl')) continue;
      final id = f.path.split('/').last.replaceAll('.jsonl', '');
      ids.add(id);
    }
    return ids;
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
    if (_loading) {
      return const SizedBox(
          height: 40, child: Center(child: CircularProgressIndicator(strokeWidth: 2)));
    }

    // Show all devices found in journals — local + peers.
    final allDevices = {
      if (_localDeviceId != null) _localDeviceId!,
      ..._knownDevices,
    }.toList();

    if (allDevices.isEmpty) {
      return Text(
        'No journal files found. Try refreshing after sync runs.',
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
      );
    }

    return Column(
      children: [
        for (final id in allDevices)
          ListTile(
            dense: true,
            leading: Icon(
              id == _localDeviceId ? Icons.phone_android : Icons.devices_other,
              size: 20,
            ),
            title: Text(
              _nicknames[id]?.isNotEmpty == true
                  ? _nicknames[id]!
                  : '${id.substring(0, id.length.clamp(0, 8))}…',
            ),
            subtitle: Text(
              id == _localDeviceId ? 'this device · $id' : id,
              style: Theme.of(context).textTheme.bodySmall,
              overflow: TextOverflow.ellipsis,
            ),
            trailing: id == _localDeviceId
                ? null
                : IconButton(
                    icon: const Icon(Icons.edit, size: 18),
                    onPressed: () => _edit(id),
                  ),
          ),
      ],
    );
  }
}
