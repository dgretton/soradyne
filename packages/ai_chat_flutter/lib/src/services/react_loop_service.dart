import '../models/chat_message.dart';
import 'chat_command_processor.dart';
import 'command_manager.dart';
import 'llm_service.dart';

enum ReactPhase { sendingToLLM, executingQueries, complete }

/// Stateless service that orchestrates the ReAct loop for a single user turn.
///
/// When [processor.isQuery] returns false for all commands, the loop
/// degrades to a single LLM call — backward compatible with apps that
/// have no query commands.
class ReactLoopService {
  final int maxIterations;

  const ReactLoopService({this.maxIterations = 3});

  /// Run the ReAct loop: send → parse → auto-execute queries → feed back → repeat.
  ///
  /// - [getSessionMessages] is a callback (not a fixed list) so each iteration
  ///   gets the fresh list after intermediate messages are appended.
  /// - [onIntermediateMessage] lets the caller append messages to state so
  ///   the UI updates incrementally.
  /// - [onPhaseChange] optional callback for UI phase indicators.
  ///
  /// Returns the final assistant [ChatMessage].
  Future<ChatMessage> run({
    required LLMService llmService,
    required CommandManager commandManager,
    required ChatCommandProcessor processor,
    required List<ChatMessage> Function() getSessionMessages,
    required Future<String> Function() buildContext,
    required Future<void> Function(ChatMessage message) onIntermediateMessage,
    void Function(ReactPhase phase, int iteration)? onPhaseChange,
  }) async {
    for (int iteration = 0; iteration <= maxIterations; iteration++) {
      onPhaseChange?.call(ReactPhase.sendingToLLM, iteration);

      final llmContext = await buildContext();
      final response = await llmService.sendMessage(
        getSessionMessages(),
        llmContext,
      );

      // Parse commands from the response
      final newCommands = commandManager.parseAndAddCommands(response);
      if (newCommands.isNotEmpty) {
        await commandManager.save();
      }

      // Separate query commands from action commands
      onPhaseChange?.call(ReactPhase.executingQueries, iteration);
      final queryResults = <String>[];

      for (final cmd in newCommands) {
        if (processor.isQuery(cmd.commandName)) {
          final result = await processor.executeQuery(cmd.command);
          if (result != null) {
            queryResults.add('Result of ${cmd.commandName}: $result');
            commandManager.markExecuted(cmd.id);
          }
        }
      }

      if (queryResults.isNotEmpty && iteration < maxIterations) {
        // Show the LLM's intermediate response (with query cards already marked executed)
        final intermediateMessage = ChatMessage(
          content: response,
          isUser: false,
          timestamp: DateTime.now(),
          commandIds: newCommands.map((c) => c.id).toList(),
        );
        await onIntermediateMessage(intermediateMessage);

        // Add query results as a tool-result message
        final resultsText = queryResults.join('\n\n');
        final toolResultMessage = ChatMessage(
          content: resultsText,
          isUser: true, // Sent as user so the LLM sees it
          timestamp: DateTime.now(),
          isToolResult: true,
        );
        await onIntermediateMessage(toolResultMessage);

        await commandManager.save();
        continue;
      }

      // No queries (or max iterations reached) — return final response
      onPhaseChange?.call(ReactPhase.complete, iteration);
      return ChatMessage(
        content: response,
        isUser: false,
        timestamp: DateTime.now(),
        commandIds: newCommands.map((c) => c.id).toList(),
      );
    }

    // Should not reach here, but safety fallback
    onPhaseChange?.call(ReactPhase.complete, maxIterations);
    return ChatMessage(
      content: 'ReAct loop reached maximum iterations without a final response.',
      isUser: false,
      timestamp: DateTime.now(),
      isError: true,
    );
  }
}
