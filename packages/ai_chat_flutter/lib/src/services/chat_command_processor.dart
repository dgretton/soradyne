/// Contract that app-specific command processors implement.
///
/// Keeps the shared package agnostic to domain commands while
/// enabling the [ReactLoopService] to classify and execute queries.
abstract class ChatCommandProcessor {
  /// Returns true if [commandName] is a query (auto-executed in the ReAct loop).
  bool isQuery(String commandName);

  /// Execute a query command and return formatted results.
  /// Returns null if the command is not a query or execution fails.
  Future<String?> executeQuery(Map<String, dynamic> command);

  /// One-line summary for display in the command panel (e.g., 'add "Setup database"').
  String commandSummary(String commandName, Map<String, dynamic> arguments);

  /// Short preview string for compact display (e.g., the item description).
  String commandPreview(String commandName, Map<String, dynamic> arguments);
}
