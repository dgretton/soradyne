import 'dart:async';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import '../chat/giantt_chat_processor.dart';
import '../models/app_state.dart';
import '../services/giantt_service.dart';

class GianttChatScreen extends StatefulWidget {
  const GianttChatScreen({super.key});

  @override
  State<GianttChatScreen> createState() => _GianttChatScreenState();
}

class _GianttChatScreenState extends State<GianttChatScreen> {
  final TextEditingController _messageController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  late final GianttService _service;
  late final GianttChatProcessor _processor;
  late final HistoryService _historyService;
  final CommandManager _commandManager = CommandManager();
  bool _isLoading = true;
  bool _isSending = false;
  List<ChatMessage> _messages = [];
  LLMService? _llmService;
  Timer? _debounce;
  bool _showScrollToBottom = false;
  late final ErrorWidgetBuilder _originalErrorWidgetBuilder;

  static const _maxReactIterations = 3;

  @override
  void initState() {
    super.initState();
    _originalErrorWidgetBuilder = ErrorWidget.builder;
    ErrorWidget.builder = (FlutterErrorDetails details) {
      debugPrint('Caught rendering error: ${details.exception}');
      return Container(
        padding: const EdgeInsets.all(12),
        margin: const EdgeInsets.only(bottom: 16),
        decoration: BoxDecoration(
          color: Colors.red.withValues(alpha: 0.1),
          borderRadius: BorderRadius.circular(18),
        ),
        child: const Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(Icons.warning_amber_rounded, color: Colors.red, size: 20),
            SizedBox(width: 8),
            Expanded(
              child: Text(
                'Error rendering message content. The LLM may have returned malformed markdown.',
                style: TextStyle(color: Colors.red),
              ),
            ),
          ],
        ),
      );
    };

    _service = Provider.of<GianttService>(context, listen: false);
    _processor = GianttChatProcessor(_service);
    _historyService = HistoryService();
    _loadHistory();
    _loadDraft();
    _commandManager.load();
    _messageController.addListener(_onTextChanged);
    _scrollController.addListener(_onScroll);

    // Mark loading complete once we've started up
    setState(() => _isLoading = false);
  }

  @override
  void dispose() {
    ErrorWidget.builder = _originalErrorWidgetBuilder;
    _debounce?.cancel();
    _scrollController.removeListener(_onScroll);
    _messageController.removeListener(_onTextChanged);
    _messageController.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (!_scrollController.hasClients) return;
    final isNearBottom =
        _scrollController.position.maxScrollExtent - _scrollController.offset < 100;
    if (_showScrollToBottom == isNearBottom) {
      setState(() => _showScrollToBottom = !isNearBottom);
    }
  }

  Future<void> _startNewChat() async {
    _commandManager.archiveAll();
    await _commandManager.save();

    final newSessionMessage = ChatMessage(
      content: 'New Chat Session',
      isUser: false,
      timestamp: DateTime.now(),
      isSessionStart: true,
    );
    setState(() {
      _messages.add(newSessionMessage);
    });
    await _historyService.saveChatHistory(_messages);
    _scrollToBottom();
  }

  List<ChatMessage> _getCurrentSessionMessages() {
    final lastSessionStartIndex = _messages.lastIndexWhere((m) => m.isSessionStart);
    if (lastSessionStartIndex == -1) return _messages;
    return _messages.sublist(lastSessionStartIndex + 1);
  }

  void _onTextChanged() {
    if (_debounce?.isActive ?? false) _debounce!.cancel();
    _debounce = Timer(const Duration(milliseconds: 500), () {
      _historyService.saveChatDraft(_messageController.text);
    });
  }

  Future<void> _loadDraft() async {
    final draft = await _historyService.loadChatDraft();
    if (draft != null && mounted) {
      _messageController.text = draft;
    }
  }

  Future<void> _loadHistory() async {
    final loadedMessages = await _historyService.loadChatHistory();
    if (mounted) {
      setState(() => _messages = loadedMessages);
      _scrollToBottom();
    }
  }

  void _initializeLLMService(LLMSettings settings) {
    _llmService = LLMServiceFactory.createService(settings);
  }

  // -- Context building --

  Future<String> _buildFullContext(LLMSettings settings) async {
    final buffer = StringBuffer();
    buffer.writeln(_buildInstructions());
    buffer.writeln('\n---\n');
    if (settings.spaceDescription.isNotEmpty) {
      buffer.writeln('PROJECT DESCRIPTION:\n${settings.spaceDescription}');
      buffer.writeln('\n---\n');
    }
    buffer.writeln(await _processor.buildGraphSummary());
    buffer.writeln('\n---\n');
    buffer.writeln(_commandManager.buildContextForLLM());
    return buffer.toString();
  }

  String _buildInstructions() {
    return '''You are an AI assistant helping manage a task dependency graph in Giantt. You help the user create, organize, and reason about tasks and their relationships.

CONCEPTS:
- Items have: id, title, status (NOT_STARTED/IN_PROGRESS/BLOCKED/COMPLETED), priority (LOWEST/LOW/NEUTRAL/UNSURE/MEDIUM/HIGH/CRITICAL), duration (e.g. "2d", "1w", "3mo"), charts (groups), tags, and relations.
- Relations: REQUIRES (A needs B done first), ANYOF (A needs one of B,C), BLOCKS (reverse of REQUIRES), SUFFICIENT (reverse of ANYOF), SUPERCHARGES, INDICATES, TOGETHER, CONFLICTS.
- Items can be occluded (archived/hidden) or included (active).
- The graph is validated: no cycles allowed in REQUIRES/ANYOF chains.

COMMANDS:
You have two types of commands: queries and actions.

QUERY COMMANDS (I will execute these automatically and show you the results):
- show(search): Look up an item by ID or title substring
- list-items(chart?, status?, tag?): List items with optional filters
- list-charts(): List all chart names
- list-tags(): List all tag names
- show-relations(id): Show all relations for an item
- show-includes(): Show include file structure
- doctor(): Check graph health

ACTION COMMANDS (shown to the user as cards for approval before execution):
- add(id, title, status?, priority?, duration?, charts?, tags?, requires?, any_of?): Add a new task
- modify(id, title?, status?, priority?, duration?, charts?, tags?): Modify a task
- remove(id): Delete a task
- set-status(id, status): Change task status
- insert(id, title, before, after, duration?, priority?): Insert task between two existing tasks
- occlude(id): Archive a task
- include(id): Unarchive a task
- add-relation(from, type, to): Add a relation between tasks
- remove-relation(from, type, to): Remove a relation
- log(session, message, tags?): Create a log entry

COMMAND FORMAT:
Format commands as JSON code blocks:
```json
{"command": "add", "arguments": {"id": "setup_db", "title": "Setup database", "priority": "HIGH", "duration": "2d", "charts": "backend", "requires": "design_schema"}}
```

For queries, I will auto-execute them and show you the results so you can reason about the graph before proposing actions. Use queries whenever you need to check the current state — for example, before adding relations, check that the target items exist.

REVISION:
Each command gets a short ID (e.g., c-a3f2k). To revise a command:
```json
{"command": "modify", "replaces": "c-a3f2k", "arguments": {"id": "setup_db", "title": "Setup PostgreSQL database"}}
```

INTERACTION STYLE:
- Be direct and precise.
- Use queries to verify state before proposing mutations.
- When the user asks about the graph, use query commands to look things up rather than guessing.
- IDs should be lowercase with underscores (e.g., setup_db, design_api).
- For lists (charts, tags, requires), use comma-separated strings.''';
  }

  // -- ReAct send loop --

  Future<void> _sendMessage() async {
    final message = _messageController.text.trim();
    if (message.isEmpty || _isSending) return;

    if (_llmService == null) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Please configure an API key in Settings.'),
            backgroundColor: Colors.orange,
          ),
        );
      }
      return;
    }

    final userMessage = ChatMessage(
      content: message,
      isUser: true,
      timestamp: DateTime.now(),
    );

    setState(() {
      _isSending = true;
      _messages.add(userMessage);
    });

    _messageController.clear();
    await _historyService.clearChatDraft();
    _scrollToBottom();

    try {
      if (!mounted) return;
      final settings = Provider.of<LLMSettings>(context, listen: false);

      // ReAct loop: send → parse → auto-execute queries → feed back → repeat
      for (int iteration = 0; iteration <= _maxReactIterations; iteration++) {
        final llmContext = await _buildFullContext(settings);
        final response = await _llmService!.sendMessage(
          _getCurrentSessionMessages(),
          llmContext,
        );

        // Parse commands from the response
        final newCommands = _commandManager.parseAndAddCommands(response);
        if (newCommands.isNotEmpty) {
          await _commandManager.save();
        }

        // Separate query commands from action commands
        final queryResults = <String>[];
        final queryCommandIds = <String>[];

        for (final cmd in newCommands) {
          if (_processor.isQuery(cmd.commandName)) {
            final result = await _processor.executeQuery(cmd.command);
            if (result != null) {
              queryResults.add('Result of ${cmd.commandName}: $result');
              _commandManager.markExecuted(cmd.id);
            }
            queryCommandIds.add(cmd.id);
          }
        }

        if (queryResults.isNotEmpty && iteration < _maxReactIterations) {
          // Show the LLM's response (with query cards already marked executed)
          final intermediateMessage = ChatMessage(
            content: response,
            isUser: false,
            timestamp: DateTime.now(),
            commandIds: newCommands.map((c) => c.id).toList(),
          );
          setState(() => _messages.add(intermediateMessage));
          _scrollToBottom();

          // Add query results as a tool-result message
          final resultsText = queryResults.join('\n\n');
          final toolResultMessage = ChatMessage(
            content: resultsText,
            isUser: true, // Sent as user message so the LLM sees it
            timestamp: DateTime.now(),
          );
          setState(() => _messages.add(toolResultMessage));
          _scrollToBottom();

          // Continue the loop — LLM will see the results and respond again
          await _commandManager.save();
          continue;
        }

        // No queries (or max iterations reached) — show final response
        final assistantMessage = ChatMessage(
          content: response,
          isUser: false,
          timestamp: DateTime.now(),
          commandIds: newCommands.map((c) => c.id).toList(),
        );
        setState(() => _messages.add(assistantMessage));
        break;
      }
    } catch (e) {
      final errorMessage = ChatMessage(
        content: 'Error: $e',
        isUser: false,
        timestamp: DateTime.now(),
        isError: true,
      );
      setState(() => _messages.add(errorMessage));
    } finally {
      await _historyService.saveChatHistory(_messages);
      if (mounted) {
        setState(() => _isSending = false);
      }
    }

    _scrollToBottom();
  }

  // -- Command execution --

  Future<void> _executeCommand(Map<String, dynamic> command, {String? trackedId}) async {
    setState(() => _isSending = true);

    try {
      await _processor.executeAction(command);
      await _historyService.logExecutedCommand(command);

      if (trackedId != null) {
        _commandManager.markExecuted(trackedId);
        await _commandManager.save();
      }

      if (!mounted) return;
      Provider.of<GianttAppState>(context, listen: false).triggerGraphRefresh();

      if (mounted) {
        final cmdName = command['command'] ?? 'unknown';
        final displayId = trackedId != null ? ' ($trackedId)' : '';
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Command "$cmdName"$displayId executed successfully.'),
            backgroundColor: Colors.green,
          ),
        );
      }
    } catch (e) {
      if (trackedId != null) {
        _commandManager.markErrored(trackedId, e.toString());
        await _commandManager.save();
      }

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    } finally {
      if (mounted) setState(() => _isSending = false);
    }
  }

  Future<void> _executeTrackedCommand(String commandId) async {
    final tracked = _commandManager.findById(commandId);
    if (tracked == null) return;
    await _executeCommand(tracked.command, trackedId: commandId);
  }

  Future<void> _executeNextPending() async {
    final next = _commandManager.getNextPending();
    if (next == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('No pending commands.'), backgroundColor: Colors.orange),
      );
      return;
    }
    await _executeTrackedCommand(next.id);
  }

  Future<void> _executeAllPending() async {
    final pending = _commandManager.pendingCommands;
    if (pending.isEmpty) return;
    for (final cmd in pending) {
      await _executeTrackedCommand(cmd.id);
      if (cmd.status == CommandStatus.errored) break;
    }
  }

  void _toggleCommandSkip(String commandId) {
    setState(() => _commandManager.toggleSkipped(commandId));
    _commandManager.save();
  }

  void _sendSkippedToChat() {
    final skipped = _commandManager.skippedCommands;
    if (skipped.isEmpty) return;
    final ids = skipped.map((c) => c.id).join(', ');
    _messageController.text = 'These commands need revision: $ids\n\n';
    _messageController.selection = TextSelection.fromPosition(
      TextPosition(offset: _messageController.text.length),
    );
  }

  Future<void> _executeCommandWithDialog(Map<String, dynamic> command, {String? trackedId}) async {
    final editedCommand = await showDialog<Map<String, dynamic>>(
      context: context,
      builder: (context) => CommandEditDialog(command: command, trackedId: trackedId),
    );
    if (editedCommand != null) {
      await _executeCommand(editedCommand, trackedId: trackedId);
    }
  }

  Future<void> _viewCommandDialog(Map<String, dynamic> command, {String? trackedId}) async {
    await showDialog<void>(
      context: context,
      builder: (context) => CommandEditDialog(command: command, readOnly: true, trackedId: trackedId),
    );
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: const Duration(milliseconds: 300),
          curve: Curves.easeOut,
        );
      }
    });
  }

  // -- UI --

  Widget _buildCommandPanel() {
    final active = _commandManager.activeCommands;
    final archived = _commandManager.archivedCommands;
    if (active.isEmpty && archived.isEmpty) return const SizedBox.shrink();

    final executed = _commandManager.executedCount;
    final pending = _commandManager.pendingCount;
    final skipped = _commandManager.skippedCount;
    final total = _commandManager.totalCount;
    final hasCompletedActive = executed > 0 || skipped > 0;

    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surfaceContainerHigh,
        border: Border(
          bottom: BorderSide(
            color: Theme.of(context).colorScheme.outline.withValues(alpha: 0.3),
          ),
        ),
      ),
      child: ExpansionTile(
        title: Row(
          children: [
            const Icon(Icons.playlist_play, size: 20),
            const SizedBox(width: 8),
            Text('Commands', style: Theme.of(context).textTheme.titleSmall),
            if (pending > 0) ...[
              const SizedBox(width: 8),
              Text(
                '$pending pending',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.primary,
                ),
              ),
            ],
          ],
        ),
        children: [
          if (active.isNotEmpty) ...[
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
              child: Row(
                children: [
                  Text('$executed/$total executed', style: Theme.of(context).textTheme.bodySmall),
                  if (skipped > 0) ...[
                    const SizedBox(width: 8),
                    Text(
                      '($skipped skipped)',
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Theme.of(context).colorScheme.outline,
                      ),
                    ),
                  ],
                  const Spacer(),
                  if (hasCompletedActive)
                    IconButton(
                      icon: const Icon(Icons.cleaning_services, size: 18),
                      tooltip: 'Clear completed',
                      onPressed: () {
                        _commandManager.archiveCompleted();
                        _commandManager.save();
                        setState(() {});
                      },
                    ),
                  if (pending > 0) ...[
                    IconButton(
                      icon: const Icon(Icons.play_arrow, size: 18),
                      tooltip: 'Execute next',
                      onPressed: _isSending ? null : _executeNextPending,
                    ),
                    IconButton(
                      icon: const Icon(Icons.fast_forward, size: 18),
                      tooltip: 'Execute all',
                      onPressed: _isSending ? null : _executeAllPending,
                    ),
                  ],
                  if (skipped > 0)
                    IconButton(
                      icon: const Icon(Icons.send, size: 18),
                      tooltip: 'Send skipped to chat',
                      onPressed: _sendSkippedToChat,
                    ),
                ],
              ),
            ),
            Container(
              constraints: const BoxConstraints(maxHeight: 200),
              child: ListView.builder(
                shrinkWrap: true,
                padding: const EdgeInsets.symmetric(horizontal: 8),
                itemCount: active.length,
                itemBuilder: (context, index) {
                  final cmd = active[index];
                  return CommandCard(
                    tracked: cmd,
                    isSending: _isSending,
                    onExecute: () => _executeCommandWithDialog(cmd.command, trackedId: cmd.id),
                    onView: () => _viewCommandDialog(cmd.command, trackedId: cmd.id),
                    onToggleSkip: () => _toggleCommandSkip(cmd.id),
                  );
                },
              ),
            ),
          ],
          if (archived.isNotEmpty)
            ExpansionTile(
              title: Text(
                'Previously... (${archived.length})',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.outline,
                ),
              ),
              initiallyExpanded: false,
              children: [
                Container(
                  constraints: const BoxConstraints(maxHeight: 150),
                  child: ListView.builder(
                    shrinkWrap: true,
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                    itemCount: archived.length,
                    itemBuilder: (context, index) {
                      final cmd = archived[index];
                      return CommandCard(
                        tracked: cmd,
                        isSending: true,
                        onExecute: null,
                        onView: () => _viewCommandDialog(cmd.command, trackedId: cmd.id),
                        onToggleSkip: null,
                      );
                    },
                  ),
                ),
              ],
            ),
        ],
      ),
    );
  }

  Widget _buildModelSelector(LLMSettings settings) {
    final currentProvider = settings.defaultProvider;
    final currentModel = settings.defaultModel;
    final currentValue = '${currentProvider.name}:$currentModel';

    final items = _buildModelDropdownItems();
    final availableValues = items.map((item) => item.value!).toList();
    final selectedValue =
        availableValues.contains(currentValue) ? currentValue : availableValues.first;

    return DropdownButton<String>(
      value: selectedValue,
      dropdownColor: Theme.of(context).colorScheme.primary,
      underline: Container(),
      icon: Icon(Icons.keyboard_arrow_down, color: Theme.of(context).colorScheme.onPrimary),
      style: TextStyle(
        color: Theme.of(context).colorScheme.onPrimary,
        fontSize: 18,
        fontWeight: FontWeight.w500,
      ),
      items: items,
      onChanged: (String? newValue) {
        if (newValue == null) return;
        final parts = newValue.split(':');
        if (parts.length == 2) {
          final provider = LLMProvider.values.firstWhere(
            (p) => p.name == parts[0],
            orElse: () => LLMProvider.anthropic,
          );
          settings.setDefaultProvider(provider);
          settings.setDefaultModel(parts[1]);
        }
      },
    );
  }

  List<DropdownMenuItem<String>> _buildModelDropdownItems() {
    return const [
      DropdownMenuItem(value: 'anthropic:claude-opus-4-1', child: Text('Claude Opus 4.1')),
      DropdownMenuItem(value: 'anthropic:claude-sonnet-4-0', child: Text('Claude Sonnet 4')),
      DropdownMenuItem(value: 'anthropic:claude-3-7-sonnet-latest', child: Text('Claude Sonnet 3.7')),
      DropdownMenuItem(value: 'anthropic:claude-3-5-haiku-latest', child: Text('Claude Haiku 3.5')),
      DropdownMenuItem(value: 'openai:gpt-4', child: Text('GPT-4')),
      DropdownMenuItem(value: 'openai:gpt-4-turbo', child: Text('GPT-4 Turbo')),
      DropdownMenuItem(value: 'openai:gpt-3.5-turbo', child: Text('GPT-3.5 Turbo')),
    ];
  }

  Widget _buildMessageBubble(ChatMessage message) {
    if (message.isError) return _buildErrorBubble(message);

    return Padding(
      padding: const EdgeInsets.only(bottom: 16),
      child: Row(
        mainAxisAlignment: message.isUser ? MainAxisAlignment.end : MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (!message.isUser) ...[
            CircleAvatar(
              radius: 16,
              backgroundColor: Theme.of(context).colorScheme.secondary,
              child: Icon(Icons.smart_toy, size: 16, color: Theme.of(context).colorScheme.onSecondary),
            ),
            const SizedBox(width: 8),
          ],
          Flexible(
            child: message.isUser
                ? Container(
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    decoration: BoxDecoration(
                      color: Theme.of(context).colorScheme.primary,
                      borderRadius: BorderRadius.circular(18),
                    ),
                    child: Text(
                      message.content,
                      style: TextStyle(color: Theme.of(context).colorScheme.onPrimary),
                    ),
                  )
                : Container(
                    padding: const EdgeInsets.all(12),
                    child: _buildMarkdownContent(message.content, commandIds: message.commandIds),
                  ),
          ),
          if (message.isUser) ...[
            const SizedBox(width: 8),
            CircleAvatar(
              radius: 16,
              backgroundColor: Theme.of(context).colorScheme.tertiary,
              child: Icon(Icons.person, size: 16, color: Theme.of(context).colorScheme.onTertiary),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildSessionDivider(ChatMessage message) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 16.0),
      child: Row(
        children: [
          const Expanded(child: Divider()),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8.0),
            child: Text(
              DateFormat.yMMMd().add_jm().format(message.timestamp),
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
          const Expanded(child: Divider()),
        ],
      ),
    );
  }

  Widget _buildMarkdownContent(String content, {List<String> commandIds = const []}) {
    try {
      final widgets = CommandBlockParser.parseMarkdownWithCommands(
        content,
        context,
        _executeCommandWithDialog,
        _isSending,
        commandIds: commandIds,
        commandManager: _commandManager,
        onView: _viewCommandDialog,
      );
      return Column(crossAxisAlignment: CrossAxisAlignment.start, children: widgets);
    } catch (e) {
      debugPrint('Content parsing error: $e');
      return Container(
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: Colors.orange.withValues(alpha: 0.1),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: Colors.orange),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Row(
              children: [
                Icon(Icons.warning_amber_rounded, color: Colors.orange, size: 16),
                SizedBox(width: 8),
                Text('Content parsing failed',
                    style: TextStyle(color: Colors.orange, fontWeight: FontWeight.bold, fontSize: 12)),
              ],
            ),
            const SizedBox(height: 8),
            SelectableText(content, style: Theme.of(context).textTheme.bodyMedium),
          ],
        ),
      );
    }
  }

  Widget _buildErrorBubble(ChatMessage message) {
    return Container(
      padding: const EdgeInsets.all(12),
      margin: const EdgeInsets.only(bottom: 16),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.errorContainer.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: Theme.of(context).colorScheme.error),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.error_outline, color: Theme.of(context).colorScheme.error),
          const SizedBox(width: 8),
          Expanded(
            child: SelectableText(
              message.content,
              style: TextStyle(color: Theme.of(context).colorScheme.onErrorContainer),
            ),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<LLMSettings>(
      builder: (context, settings, child) {
        _initializeLLMService(settings);

        return Scaffold(
          appBar: AppBar(
            title: _buildModelSelector(settings),
            actions: [
              IconButton(
                icon: const Icon(Icons.settings),
                tooltip: 'Settings',
                onPressed: () => Navigator.of(context).pushNamed('/settings'),
              ),
              IconButton(
                icon: const Icon(Icons.add_comment_outlined),
                tooltip: 'New Chat',
                onPressed: _startNewChat,
              ),
            ],
          ),
          body: _isLoading
              ? const Center(child: CircularProgressIndicator())
              : Column(
                  children: [
                    _buildCommandPanel(),
                    Expanded(
                      child: Stack(
                        children: [
                          _messages.isEmpty
                              ? Center(
                                  child: Column(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Icon(
                                        Icons.chat_bubble_outline,
                                        size: 48,
                                        color: Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.3),
                                      ),
                                      const SizedBox(height: 12),
                                      Text(
                                        _llmService != null
                                            ? 'Start a conversation about your tasks'
                                            : 'Configure API key in settings',
                                        style: Theme.of(context).textTheme.titleMedium?.copyWith(
                                          color: Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.6),
                                        ),
                                      ),
                                    ],
                                  ),
                                )
                              : ListView.builder(
                                  controller: _scrollController,
                                  padding: const EdgeInsets.all(16),
                                  itemCount: _messages.length,
                                  itemBuilder: (context, index) {
                                    final message = _messages[index];
                                    if (message.isSessionStart) return _buildSessionDivider(message);
                                    return _buildMessageBubble(message);
                                  },
                                ),
                          if (_showScrollToBottom)
                            Positioned(
                              right: 16,
                              bottom: 8,
                              child: FloatingActionButton.small(
                                onPressed: _scrollToBottom,
                                child: const Icon(Icons.keyboard_arrow_down),
                              ),
                            ),
                        ],
                      ),
                    ),
                    Container(
                      padding: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: Theme.of(context).colorScheme.surface,
                        border: Border(
                          top: BorderSide(
                            color: Theme.of(context).colorScheme.outline.withValues(alpha: 0.2),
                          ),
                        ),
                      ),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.end,
                        children: [
                          Expanded(
                            child: ConstrainedBox(
                              constraints: const BoxConstraints(maxHeight: 150),
                              child: TextField(
                                autofocus: true,
                                controller: _messageController,
                                decoration: InputDecoration(
                                  hintText: 'Ask about your tasks...',
                                  border: OutlineInputBorder(borderRadius: BorderRadius.circular(24)),
                                  contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                                ),
                                maxLines: null,
                                textInputAction: TextInputAction.send,
                                onSubmitted: (_) => _sendMessage(),
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
                          FloatingActionButton.small(
                            onPressed: _isSending ? null : _sendMessage,
                            child: _isSending
                                ? SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                      color: Theme.of(context).colorScheme.onSecondary,
                                    ),
                                  )
                                : const Icon(Icons.send),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
        );
      },
    );
  }
}
