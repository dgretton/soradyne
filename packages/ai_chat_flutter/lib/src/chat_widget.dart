import 'package:flutter/material.dart';

class AiChatWidget extends StatefulWidget {
  final ActionRegistry actionRegistry;
  final String? systemPrompt;
  final Function(String)? onUserMessage;

  const AiChatWidget({
    super.key,
    required this.actionRegistry,
    this.systemPrompt,
    this.onUserMessage,
  });

  @override
  State<AiChatWidget> createState() => _AiChatWidgetState();
}

class _AiChatWidgetState extends State<AiChatWidget> {
  final List<ChatMessage> _messages = [];
  final TextEditingController _controller = TextEditingController();

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: ListView.builder(
            itemCount: _messages.length,
            itemBuilder: (context, index) => _MessageBubble(
              message: _messages[index],
              onActionTap: _executeAction,
            ),
          ),
        ),
        _ChatInput(
          controller: _controller,
          onSend: _sendMessage,
        ),
      ],
    );
  }

  void _sendMessage(String text) async {
    if (text.trim().isEmpty) return;
    
    final userMessage = ChatMessage.user(text);
    setState(() => _messages.add(userMessage));
    
    widget.onUserMessage?.call(text);
    _controller.clear();

    // Send to AI and handle response with action calls
    await _processAiResponse(text);
  }

  Future<void> _processAiResponse(String userInput) async {
    // Implementation for AI API call and action parsing
    // This would parse responses for action calls like:
    // ```action:create_task
    // {"title": "New task", "due_date": "2024-01-01"}
    // ```
  }

  Future<void> _executeAction(ActionCall actionCall) async {
    try {
      final result = await widget.actionRegistry.executeAction(
        actionCall.actionName,
        actionCall.parameters,
      );
      
      setState(() {
        _messages.add(ChatMessage.system(
          'Action "${actionCall.actionName}" completed: $result'
        ));
      });
    } catch (e) {
      setState(() {
        _messages.add(ChatMessage.system(
          'Action "${actionCall.actionName}" failed: $e'
        ));
      });
    }
  }
}
