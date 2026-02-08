import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';

class HistoryPage extends StatefulWidget {
  const HistoryPage({super.key});

  @override
  State<HistoryPage> createState() => _HistoryPageState();
}

class _HistoryPageState extends State<HistoryPage> {
  final HistoryService _historyService = HistoryService();
  Future<List<ChatMessage>>? _chatHistoryFuture;
  Future<List<Map<String, dynamic>>>? _commandHistoryFuture;
  Future<String?>? _chatDraftFuture;

  @override
  void initState() {
    super.initState();
    _loadHistories();
  }

  void _loadHistories() {
    setState(() {
      _chatHistoryFuture = _historyService.loadChatHistory();
      _commandHistoryFuture = _historyService.loadCommandHistory();
      _chatDraftFuture = _historyService.loadChatDraft();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('History'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: _loadHistories,
            tooltip: 'Refresh History',
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          _buildSection(
            context,
            title: 'Chat Draft',
            future: _chatDraftFuture,
            builder: (context, snapshot) {
              final draft = snapshot.data;
              if (draft == null || draft.isEmpty) {
                return const Text('(No draft saved)');
              }
              return SelectableText(draft);
            },
          ),
          const SizedBox(height: 24),
          _buildSection(
            context,
            title: 'Command History',
            future: _commandHistoryFuture,
            builder: (context, snapshot) {
              final commands = snapshot.data;
              if (commands == null || commands.isEmpty) {
                return const Text('(No commands executed)');
              }
              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: commands.reversed.map((cmd) {
                  return Card(
                    margin: const EdgeInsets.only(bottom: 8),
                    child: Padding(
                      padding: const EdgeInsets.all(12.0),
                      child: SelectableText(
                        const JsonEncoder.withIndent('  ').convert(cmd),
                        style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
                      ),
                    ),
                  );
                }).toList(),
              );
            },
          ),
          const SizedBox(height: 24),
          _buildSection(
            context,
            title: 'Chat History',
            future: _chatHistoryFuture,
            builder: (context, snapshot) {
              final messages = snapshot.data;
              if (messages == null || messages.isEmpty) {
                return const Text('(No chat history)');
              }
              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: messages.map((msg) {
                  return Card(
                    margin: const EdgeInsets.only(bottom: 8),
                    child: ListTile(
                      title: SelectableText(msg.content),
                      subtitle: Text(
                          '${msg.isUser ? "User" : (msg.isSessionStart ? "System" : "Assistant")} - ${msg.timestamp.toLocal()}'),
                    ),
                  );
                }).toList(),
              );
            },
          ),
        ],
      ),
    );
  }

  Widget _buildSection<T>(
    BuildContext context, {
    required String title,
    required Future<T>? future,
    required Widget Function(BuildContext, AsyncSnapshot<T>) builder,
  }) {
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
        FutureBuilder<T>(
          future: future,
          builder: (context, snapshot) {
            if (snapshot.connectionState == ConnectionState.waiting) {
              return const Center(child: CircularProgressIndicator());
            } else if (snapshot.hasError) {
              return SelectableText('Error: ${snapshot.error}');
            } else if (!snapshot.hasData) {
              return const Text('(No data)');
            }
            return builder(context, snapshot);
          },
        ),
      ],
    );
  }
}
