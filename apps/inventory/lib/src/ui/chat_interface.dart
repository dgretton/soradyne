import 'dart:async';
import 'package:flutter/material.dart';
import 'dart:convert';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import '../chat/chat_processor.dart';
import '../models/app_state.dart';
import '../core/inventory_api.dart';
import '../core/models/inventory_entry.dart';

class ChatInterface extends StatefulWidget {
  const ChatInterface({super.key});

  @override
  State<ChatInterface> createState() => _ChatInterfaceState();
}

class _ChatInterfaceState extends State<ChatInterface> {
  final TextEditingController _messageController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  late final InventoryApi _inventoryApi;
  late final ChatProcessor _chatProcessor;
  late final HistoryService _historyService;
  final CommandManager _commandManager = CommandManager();
  List<InventoryEntry> _allItems = [];
  bool _isLoading = true;
  bool _isSending = false;
  List<ChatMessage> _messages = [];
  LLMService? _llmService;
  Timer? _debounce;
  bool _showScrollToBottom = false;
  late final ErrorWidgetBuilder _originalErrorWidgetBuilder;

  @override
  void initState() {
    super.initState();
    _originalErrorWidgetBuilder = ErrorWidget.builder;
    ErrorWidget.builder = (FlutterErrorDetails details) {
      // This prevents a single malformed markdown message from the LLM
      // from crashing the entire UI with a red error box.
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

    // The API is provided, so we can get it directly.
    // No need to listen, as the API instance itself won't change.
    _inventoryApi = Provider.of<InventoryApi>(context, listen: false);
    _chatProcessor = ChatProcessor(_inventoryApi);
    _historyService = HistoryService();
    _loadAllItems();
    _loadHistory();
    _loadDraft();
    _commandManager.load();
    _messageController.addListener(_onTextChanged);
    _scrollController.addListener(_onScroll);
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
    final isNearBottom = _scrollController.position.maxScrollExtent -
            _scrollController.offset <
        100;
    if (_showScrollToBottom == isNearBottom) {
      setState(() => _showScrollToBottom = !isNearBottom);
    }
  }

  Future<void> _startNewChat() async {
    _commandManager.archiveAll();
    await _commandManager.save();

    final newSessionMessage = ChatMessage(
      content: 'New Chat Session',
      isUser: false, // This is a system message, not from user or assistant
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
    final lastSessionStartIndex =
        _messages.lastIndexWhere((m) => m.isSessionStart);
    if (lastSessionStartIndex == -1) {
      return _messages;
    }
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
      setState(() {
        _messages = loadedMessages;
      });
      _scrollToBottom();
    }
  }

  Future<void> _loadAllItems() async {
    try {
      final items = await _inventoryApi.search('');
      if (mounted) {
        setState(() {
          _allItems = items;
          _isLoading = false;
        });
        _scrollToBottom();
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _isLoading = false;
        });
        _scrollToBottom();
      }
    }
  }

  void _initializeLLMService(LLMSettings settings) {
    _llmService = LLMServiceFactory.createService(settings);
  }

  Future<String> _buildFullContext(AppState appState, LLMSettings settings) async {
    final buffer = StringBuffer();
    buffer.writeln(_buildLLMInstructions());
    buffer.writeln('\n---\n');
    buffer.writeln(_buildSpaceDescription(settings.spaceDescription));
    buffer.writeln('\n---\n');
    buffer.writeln(_buildInventoryContext());
    buffer.writeln('\n---\n');
    buffer.writeln(_buildSelectedItemsContext(appState.selectedItems));
    buffer.writeln('\n---\n');
    buffer.writeln(_commandManager.buildContextForLLM());
    return buffer.toString();
  }

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
      final appState = Provider.of<AppState>(context, listen: false);
      final settings = Provider.of<LLMSettings>(context, listen: false);
      final llmContext = await _buildFullContext(appState, settings);

      final response = await _llmService!.sendMessage(_getCurrentSessionMessages(), llmContext);

      // Parse and track any commands in the response
      final newCommands = _commandManager.parseAndAddCommands(response);
      if (newCommands.isNotEmpty) {
        await _commandManager.save();
      }

      final assistantMessage = ChatMessage(
        content: response,
        isUser: false,
        timestamp: DateTime.now(),
        commandIds: newCommands.map((c) => c.id).toList(),
      );
      _messages.add(assistantMessage);
    } catch (e) {
      final errorMessage = ChatMessage(
        content: 'Error: ${e.toString()}',
        isUser: false,
        timestamp: DateTime.now(),
        isError: true,
      );
      _messages.add(errorMessage);
    } finally {
      await _historyService.saveChatHistory(_messages);
      if (mounted) {
        setState(() {
          _isSending = false;
        });
      }
    }

    _scrollToBottom();
  }

  Future<void> _executeCommand(Map<String, dynamic> command, {String? trackedId}) async {
    setState(() => _isSending = true);

    try {
      final commandJson = jsonEncode(command);
      await _chatProcessor.processCommand(commandJson);
      await _historyService.logExecutedCommand(command);

      // Update tracked command status if we have an ID
      if (trackedId != null) {
        _commandManager.markExecuted(trackedId);
        await _commandManager.save();
      }

      if (!mounted) return;
      Provider.of<AppState>(context, listen: false).triggerInventoryRefresh();

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
      // Update tracked command with error if we have an ID
      if (trackedId != null) {
        _commandManager.markErrored(trackedId, e.toString());
        await _commandManager.save();
      }

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error executing command: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    } finally {
      if (mounted) {
        setState(() => _isSending = false);
      }
    }
  }

  /// Execute a tracked command by its ID.
  Future<void> _executeTrackedCommand(String commandId) async {
    final tracked = _commandManager.findById(commandId);
    if (tracked == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Command $commandId not found.'),
          backgroundColor: Colors.red,
        ),
      );
      return;
    }
    await _executeCommand(tracked.command, trackedId: commandId);
  }

  /// Execute the next pending (non-skipped) command.
  Future<void> _executeNextPending() async {
    final next = _commandManager.getNextPending();
    if (next == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('No pending commands to execute.'),
          backgroundColor: Colors.orange,
        ),
      );
      return;
    }
    await _executeTrackedCommand(next.id);
  }

  /// Execute all pending (non-skipped) commands.
  Future<void> _executeAllPending() async {
    final pending = _commandManager.pendingCommands;
    if (pending.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('No pending commands to execute.'),
          backgroundColor: Colors.orange,
        ),
      );
      return;
    }

    for (final cmd in pending) {
      await _executeTrackedCommand(cmd.id);
      // Stop if we hit an error
      if (cmd.status == CommandStatus.errored) break;
    }
  }

  /// Toggle skip status of a command.
  void _toggleCommandSkip(String commandId) {
    setState(() {
      _commandManager.toggleSkipped(commandId);
    });
    _commandManager.save();
  }

  /// Send skipped commands to chat input for revision request.
  void _sendSkippedToChat() {
    final skipped = _commandManager.skippedCommands;
    if (skipped.isEmpty) return;

    final ids = skipped.map((c) => c.id).join(', ');
    _messageController.text = 'These commands need revision: $ids\n\n';
    _messageController.selection = TextSelection.fromPosition(
      TextPosition(offset: _messageController.text.length),
    );
  }

  /// Build the command panel showing tracked commands with actions.
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
            Text(
              'Commands',
              style: Theme.of(context).textTheme.titleSmall,
            ),
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
            // Status summary and action buttons
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
              child: Row(
                children: [
                  Text(
                    '$executed/$total executed',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
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
                    onExecute: () => _executeCommandWithDialog(
                      cmd.command,
                      trackedId: cmd.id,
                    ),
                    onView: () => _viewCommandDialog(cmd.command, trackedId: cmd.id),
                    onToggleSkip: () => _toggleCommandSkip(cmd.id),
                  );
                },
              ),
            ),
          ],
          // Archived commands section
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
                        isSending: true, // disables execute action
                        onExecute: null,
                        onView: () => _viewCommandDialog(cmd.command, trackedId: cmd.id),
                        onToggleSkip: null, // read-only, no skip toggle
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

  String _buildLLMInstructions() {
    return '''You are an AI assistant helping manage a personal inventory system. Your role is to help the user organize, find, and manipulate their belongings through natural conversation.

USER'S GOAL:
The user's primary goal is to create a well-organized inventory so they can always find what they own and know where to put new items. They have recently moved and want to establish a system that prevents them from buying duplicates of things they can't find. Please keep this goal of "findability" in mind with all your suggestions.

CORE CAPABILITIES:
You can suggest commands to:
- Add new items: add(category, description, location, tags?, isContainer?, containerId?)
- Create containers: create-container(containerId, location, description?)
- Move items: move(searchStr, newLocation)
- Delete items: delete(searchStr)
- Edit descriptions: edit-description(searchStr, newDescription)
- Put items in containers: put-in(searchStr, containerId)
- Remove from containers: remove-from-container(searchStr)
- Add tag to item: add-tag(searchStr, tag)
- Remove tag from item: remove-tag(searchStr, tag)
- Batch put items by tag into container: group-put-in(tag, containerId)
- Batch remove tag from all items: group-remove-tag(tag)

COMMAND FORMAT:
When suggesting actions, format them as JSON commands like:
```json
{"command": "add", "arguments": {"category": "Tools", "description": "Hammer", "location": "Toolbox"}}
```

Each command you suggest is automatically assigned a short ID (e.g., c-a3f2k). You'll see a list of commands with their IDs and statuses (pending/executed/errored/skipped) in the context below. When the user asks you to revise specific commands, reference them by ID:
```json
{"command": "move", "replaces": "c-a3f2k", "arguments": {"searchStr": "hammer", "newLocation": "Garage"}}
```

INVENTORY PRINCIPLES:
- Be specific in descriptions and locations.
- Use the established categories: Clothing, Devices, Tools, Mechanical Parts, Electrical Parts, Personal Care, Documents, Household Upkeep, Supplies, Food and Drink, Equipment, Decor, Containers.
- Please create stable location descriptions. Do not reference other movable items. For example, "Floor of bedroom in corner to left of door" is a good description, but "Floor of bedroom in corner to left of door near the large corn plant" is not, because the plant might be moved later.
- Please make use of containers whenever it makes sense. They are a key part of the user's organization strategy. If the user mentions a new container, please offer to create it for them using the `add` command with `isContainer: true`. When reorganizing, suggesting that items be placed in containers is very helpful.
- Consider the user's living space layout when suggesting storage.

ITEM IDs:
Each inventory item has a short ID (shown in brackets, e.g. [f47ac10b]). When a description matches multiple items, use the short ID as the searchStr to target a specific one.

INTERACTION STYLE:
- Be direct and businesslike.
- Do not offer extra information, speculate, or ask friendly questions.
- Only ask clarifying questions if a user's request is ambiguous and cannot be fulfilled without more information.
- If a command can be generated, provide the command and a brief, factual description of what it does. Do not add conversational filler.''';
  }

  String _buildInventoryContext() {
    if (_allItems.isEmpty) return 'CURRENT INVENTORY: (empty)';

    final buffer = StringBuffer('CURRENT INVENTORY:\n');
    for (final item in _allItems) {
      buffer.writeln('[${item.id.substring(0, 8)}] ${item.description} -> ${item.location}');
      if (item.tags.isNotEmpty) {
        buffer.writeln('  Tags: ${item.tags.join(', ')}');
      }
    }
    return buffer.toString();
  }

  String _buildSelectedItemsContext(List<InventoryEntry> selectedItems) {
    if (selectedItems.isEmpty) return 'SELECTED ITEMS: (none)';

    final buffer = StringBuffer('SELECTED ITEMS:\n');
    for (final item in selectedItems) {
      buffer.writeln('ID: ${item.id}');
      buffer.writeln('Category: ${item.category}');
      buffer.writeln('Description: ${item.description}');
      buffer.writeln('Location: ${item.location}');
      if (item.tags.isNotEmpty) {
        buffer.writeln('Tags: ${item.tags.join(', ')}');
      }
      buffer.writeln('---');
    }
    return buffer.toString();
  }

  String _buildSpaceDescription(String spaceDescription) {
    if (spaceDescription.isEmpty) return 'SPACE DESCRIPTION: (not set)';
    return 'SPACE DESCRIPTION:\n$spaceDescription';
  }

  Widget _buildModelSelector(LLMSettings settings) {
    final currentProvider = settings.defaultProvider;
    final currentModel = settings.defaultModel;
    final currentValue = '${currentProvider.name}:$currentModel';

    // Get all available dropdown values
    final availableValues = _buildModelDropdownItems().map((item) => item.value!).toList();

    // Use the current value if it exists in the dropdown, otherwise use the first available value
    final selectedValue = availableValues.contains(currentValue)
        ? currentValue
        : availableValues.first;

    return DropdownButton<String>(
      value: selectedValue,
      dropdownColor: Theme.of(context).colorScheme.primary,
      underline: Container(),
      icon: Icon(
        Icons.keyboard_arrow_down,
        color: Theme.of(context).colorScheme.onPrimary,
      ),
      style: TextStyle(
        color: Theme.of(context).colorScheme.onPrimary,
        fontSize: 18,
        fontWeight: FontWeight.w500,
      ),
      items: _buildModelDropdownItems(),
      onChanged: (String? newValue) {
        if (newValue != null) {
          final parts = newValue.split(':');
          if (parts.length == 2) {
            final providerName = parts[0];
            final modelName = parts[1];

            final provider = LLMProvider.values.firstWhere(
              (p) => p.name == providerName,
              orElse: () => LLMProvider.openai,
            );

            settings.setDefaultProvider(provider);
            settings.setDefaultModel(modelName);
            // Note: Settings will be saved when user navigates back to settings
          }
        }
      },
    );
  }

  List<DropdownMenuItem<String>> _buildModelDropdownItems() {
    return [
      // OpenAI models
      DropdownMenuItem(
        value: 'openai:gpt-4',
        child: Text('GPT-4'),
      ),
      DropdownMenuItem(
        value: 'openai:gpt-4-turbo',
        child: Text('GPT-4 Turbo'),
      ),
      DropdownMenuItem(
        value: 'openai:gpt-3.5-turbo',
        child: Text('GPT-3.5 Turbo'),
      ),

      // Anthropic models
      DropdownMenuItem(
        value: 'anthropic:claude-opus-4-1',
        child: Text('Claude Opus 4.1'),
      ),
      DropdownMenuItem(
        value: 'anthropic:claude-opus-4-0',
        child: Text('Claude Opus 4'),
      ),
      DropdownMenuItem(
        value: 'anthropic:claude-sonnet-4-0',
        child: Text('Claude Sonnet 4'),
      ),
      DropdownMenuItem(
        value: 'anthropic:claude-3-7-sonnet-latest',
        child: Text('Claude Sonnet 3.7'),
      ),
      DropdownMenuItem(
        value: 'anthropic:claude-3-5-haiku-latest',
        child: Text('Claude Haiku 3.5'),
      ),

      // Ollama models (common ones)
      DropdownMenuItem(
        value: 'ollama:llama2',
        child: Text('Llama 2 (Local)'),
      ),
      DropdownMenuItem(
        value: 'ollama:mistral',
        child: Text('Mistral (Local)'),
      ),
      DropdownMenuItem(
        value: 'ollama:codellama',
        child: Text('Code Llama (Local)'),
      ),

      // Personal server
      DropdownMenuItem(
        value: 'personal:custom',
        child: Text('Personal Server'),
      ),
    ];
  }

  @override
  Widget build(BuildContext context) {
    return Consumer2<AppState, LLMSettings>(
      builder: (context, appState, settings, child) {
        _initializeLLMService(settings);

        return Scaffold(
          appBar: AppBar(
            title: _buildModelSelector(settings),
            actions: [
              IconButton(
                icon: const Icon(Icons.add_comment_outlined),
                tooltip: 'New Chat',
                onPressed: _startNewChat,
              ),
              if (appState.selectedItems.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(right: 16.0),
                  child: Center(
                    child: Text(
                      '${appState.selectedItems.length} items loaded',
                      style: const TextStyle(fontSize: 14),
                    ),
                  ),
                ),
            ],
          ),
          body: _isLoading
              ? const Center(child: CircularProgressIndicator())
              : Column(
                  children: [
                    // Context sections
                    ExpandableSection(
                      title: 'Chat Context',
                      icon: Icons.tune,
                      child: Container(
                        constraints: BoxConstraints(
                          maxHeight: MediaQuery.of(context).size.height * 0.4,
                        ),
                        color: Theme.of(context).colorScheme.surfaceContainerHighest.withValues(alpha: 0.3),
                        child: SingleChildScrollView(
                          child: Column(
                            children: [
                              ExpandableSection(
                                title: 'LLM Instructions',
                                content: _buildLLMInstructions(),
                                icon: Icons.info_outline,
                              ),
                              ExpandableSection(
                                title: 'Space Description',
                                content: _buildSpaceDescription(settings.spaceDescription),
                                icon: Icons.home,
                              ),
                              ExpandableSection(
                                title: 'Current Inventory (${_allItems.length} items)',
                                content: _buildInventoryContext(),
                                icon: Icons.inventory,
                              ),
                              ExpandableSection(
                                title: 'Selected Items (${appState.selectedItems.length})',
                                content: _buildSelectedItemsContext(appState.selectedItems),
                                icon: Icons.check_circle_outline,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                    // Command panel
                    _buildCommandPanel(),
                    // Chat messages area
                    Expanded(
                      child: Stack(
                        children: [
                          _messages.isEmpty
                              ? Container(
                                  padding: const EdgeInsets.all(16),
                                  child: Center(
                                    child: Column(
                                      mainAxisAlignment: MainAxisAlignment.center,
                                      mainAxisSize: MainAxisSize.min,
                                      children: [
                                        Icon(
                                          Icons.chat_bubble_outline,
                                          size: 48,
                                          color: Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.3),
                                        ),
                                        const SizedBox(height: 12),
                                        Text(
                                          _llmService != null ? 'Start a conversation' : 'Configure API key in settings',
                                          style: Theme.of(context).textTheme.titleMedium?.copyWith(
                                            color: Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.6),
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                )
                              : ListView.builder(
                                  controller: _scrollController,
                                  padding: const EdgeInsets.all(16),
                                  itemCount: _messages.length,
                                  itemBuilder: (context, index) {
                                    final message = _messages[index];
                                    if (message.isSessionStart) {
                                      return _buildSessionDivider(message);
                                    }
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
                    // Message input area
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
                                  hintText: 'Type your message...',
                                  border: OutlineInputBorder(
                                    borderRadius: BorderRadius.circular(24),
                                  ),
                                  contentPadding: const EdgeInsets.symmetric(
                                    horizontal: 16,
                                    vertical: 12,
                                  ),
                                ),
                                maxLines: null,
                                textInputAction: TextInputAction.send,
                                onSubmitted: (value) {
                                  _sendMessage();
                                },
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

  Widget _buildMessageBubble(ChatMessage message) {
    if (message.isError) {
      return _buildErrorBubble(message);
    }
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
              child: Icon(
                Icons.smart_toy,
                size: 16,
                color: Theme.of(context).colorScheme.onSecondary,
              ),
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
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.onPrimary,
                      ),
                      textAlign: TextAlign.center,
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
              child: Icon(
                Icons.person,
                size: 16,
                color: Theme.of(context).colorScheme.onTertiary,
              ),
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
      // Use our custom parser instead of flutter_markdown to avoid assertion failures
      final widgets = CommandBlockParser.parseMarkdownWithCommands(
        content,
        context,
        _executeCommandWithDialog,
        _isSending,
        commandIds: commandIds,
        commandManager: _commandManager,
        onView: _viewCommandDialog,
      );

      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: widgets,
      );
    } catch (e) {
      debugPrint('Content parsing error: $e');
      // Fallback to plain text if parsing fails
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
            Row(
              children: [
                Icon(Icons.warning_amber_rounded, color: Colors.orange, size: 16),
                const SizedBox(width: 8),
                Text(
                  'Content parsing failed',
                  style: TextStyle(
                    color: Colors.orange,
                    fontWeight: FontWeight.bold,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            SelectableText(
              content,
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ],
        ),
      );
    }
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
}
