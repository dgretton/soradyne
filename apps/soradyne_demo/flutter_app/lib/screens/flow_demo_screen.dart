import 'dart:async';
import 'dart:ffi';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../ffi/inventory_bindings.dart';
import '../services/pairing_service.dart';

/// A shared note stored in the inventory CRDT.
class _Note {
  final String id;
  final String text;
  _Note({required this.id, required this.text});
}

/// Extract notes from the inventory `read_drip` JSON state.
///
/// The inventory state has shape: `{"items": {"id": {"category": "Note", "description": "...", ...}}}`
/// We only show items where `category == "Note"`.
List<_Note> _parseNotes(Map<String, dynamic> state) {
  final items = state['items'] as Map<String, dynamic>? ?? {};
  return items.entries
      .where((e) {
        final item = e.value as Map<String, dynamic>? ?? {};
        return item['category'] == 'Note';
      })
      .map((e) {
        final item = e.value as Map<String, dynamic>? ?? {};
        return _Note(
          id: e.key,
          text: (item['description'] as String?) ?? '',
        );
      })
      .toList()
    ..sort((a, b) => a.id.compareTo(b.id));
}

/// Flow Demo screen — shows a shared notes list backed by the inventory CRDT.
///
/// Opens an inventory flow for the selected capsule, connects it to the
/// capsule ensemble (which makes it ensemble-sync-aware), and polls
/// the local CRDT state every 2 seconds to keep the UI fresh.
///
/// Cross-device sync (via BLE) activates in Phase 7 when the
/// EnsembleManager is started with a real transport after pairing.
class FlowDemoScreen extends StatefulWidget {
  final String capsuleId;
  final String capsuleName;

  const FlowDemoScreen({
    super.key,
    required this.capsuleId,
    required this.capsuleName,
  });

  @override
  State<FlowDemoScreen> createState() => _FlowDemoScreenState();
}

class _FlowDemoScreenState extends State<FlowDemoScreen> {
  InventoryBindings? _inv;
  Pointer<Void>? _handle;
  List<_Note> _notes = [];
  bool _ready = false;
  String? _error;
  Timer? _pollTimer;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _init());
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    final h = _handle;
    final inv = _inv;
    if (h != null && inv != null) {
      inv.stopSync(h);
      inv.close(h);
    }
    super.dispose();
  }

  void _init() {
    final service = context.read<PairingService>();
    final bindings = service.bindings;
    final deviceId = service.deviceId;

    if (bindings == null || deviceId == null) {
      setState(() => _error = 'Pairing not initialized');
      return;
    }

    final inv = InventoryBindings(bindings.lib);
    final initResult = inv.init(deviceId);
    if (initResult != 0) {
      setState(() => _error = 'Inventory init failed ($initResult)');
      return;
    }

    final handle = inv.open(widget.capsuleId);
    if (handle == null) {
      setState(() => _error = 'Could not open inventory flow');
      return;
    }

    // Connect to capsule ensemble so the flow is ensemble-sync-aware.
    // (Cross-device BLE sync activates in Phase 7.)
    inv.connectEnsemble(handle, widget.capsuleId);
    inv.startSync(handle);

    _inv = inv;
    _handle = handle;

    setState(() => _ready = true);
    _refresh();
    _pollTimer = Timer.periodic(const Duration(seconds: 2), (_) => _refresh());
  }

  void _refresh() {
    final h = _handle;
    final inv = _inv;
    if (h == null || inv == null) return;
    final state = inv.readDrip(h);
    if (state != null) {
      setState(() => _notes = _parseNotes(state));
    }
  }

  void _addNote(String text) {
    final h = _handle;
    final inv = _inv;
    if (h == null || inv == null || text.isEmpty) return;

    // Use a timestamp-based ID — simple and sortable.
    final id = 'note-${DateTime.now().millisecondsSinceEpoch}';
    inv.writeOp(h, {
      'AddItem': {'item_id': id, 'item_type': 'InventoryItem'}
    });
    inv.writeOp(h, {
      'SetField': {'item_id': id, 'field': 'category', 'value': 'Note'}
    });
    inv.writeOp(h, {
      'SetField': {'item_id': id, 'field': 'description', 'value': text}
    });
    _refresh();
  }

  void _removeNote(String id) {
    final h = _handle;
    final inv = _inv;
    if (h == null || inv == null) return;
    inv.writeOp(h, {
      'RemoveItem': {'item_id': id}
    });
    _refresh();
  }

  void _showAddDialog() {
    final controller = TextEditingController();
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Add Note'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(hintText: 'Enter note text'),
          textCapitalization: TextCapitalization.sentences,
          onSubmitted: (value) {
            _addNote(value.trim());
            Navigator.pop(ctx);
          },
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              _addNote(controller.text.trim());
              Navigator.pop(ctx);
            },
            child: const Text('Add'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('Shared Notes'),
            Text(
              widget.capsuleName,
              style: Theme.of(context)
                  .textTheme
                  .bodySmall
                  ?.copyWith(color: Colors.white70),
            ),
          ],
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Refresh',
            onPressed: _ready ? _refresh : null,
          ),
        ],
      ),
      body: _buildBody(),
      floatingActionButton: _ready
          ? FloatingActionButton.extended(
              onPressed: _showAddDialog,
              icon: const Icon(Icons.add),
              label: const Text('Add Note'),
            )
          : null,
    );
  }

  Widget _buildBody() {
    if (_error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.error_outline, size: 48, color: Colors.red),
              const SizedBox(height: 16),
              Text(_error!, textAlign: TextAlign.center),
            ],
          ),
        ),
      );
    }

    if (!_ready) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_notes.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.note_outlined, size: 64, color: Colors.grey[400]),
            const SizedBox(height: 16),
            Text(
              'No notes yet',
              style: TextStyle(fontSize: 18, color: Colors.grey[600]),
            ),
            const SizedBox(height: 8),
            Text(
              'Tap + to add a shared note.\nNotes sync across all devices in this capsule.',
              style: TextStyle(color: Colors.grey[500]),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      );
    }

    return ListView.builder(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 88),
      itemCount: _notes.length,
      itemBuilder: (context, index) {
        final note = _notes[index];
        return Card(
          margin: const EdgeInsets.only(bottom: 8),
          child: ListTile(
            leading: CircleAvatar(
              backgroundColor:
                  Theme.of(context).colorScheme.primaryContainer,
              child: Icon(
                Icons.note,
                color: Theme.of(context).colorScheme.onPrimaryContainer,
              ),
            ),
            title: Text(note.text),
            subtitle: Text(
              'ID: ${note.id.length > 20 ? note.id.substring(note.id.length - 8) : note.id}',
              style: TextStyle(fontSize: 11, color: Colors.grey[500]),
            ),
            trailing: IconButton(
              icon: const Icon(Icons.delete_outline, color: Colors.red),
              onPressed: () => _removeNote(note.id),
            ),
          ),
        );
      },
    );
  }
}
