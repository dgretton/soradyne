class ChatMessage {
  final String content;
  final bool isUser;
  final DateTime timestamp;
  final bool isSessionStart;
  final bool isError;
  final bool isToolResult;
  final List<String> commandIds;

  ChatMessage({
    required this.content,
    required this.isUser,
    required this.timestamp,
    this.isSessionStart = false,
    this.isError = false,
    this.isToolResult = false,
    this.commandIds = const [],
  });

  Map<String, dynamic> toJson() => {
        'content': content,
        'isUser': isUser,
        'timestamp': timestamp.toIso8601String(),
        'isSessionStart': isSessionStart,
        'isError': isError,
        if (isToolResult) 'isToolResult': isToolResult,
        if (commandIds.isNotEmpty) 'commandIds': commandIds,
      };

  factory ChatMessage.fromJson(Map<String, dynamic> json) => ChatMessage(
        content: json['content'] as String,
        isUser: json['isUser'] as bool,
        timestamp: DateTime.parse(json['timestamp'] as String),
        isSessionStart: json['isSessionStart'] as bool? ?? false,
        isError: json['isError'] as bool? ?? false,
        isToolResult: json['isToolResult'] as bool? ?? false,
        commandIds: (json['commandIds'] as List<dynamic>?)
                ?.cast<String>()
                .toList() ??
            const [],
      );
}
