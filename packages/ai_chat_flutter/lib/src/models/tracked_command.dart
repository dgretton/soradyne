import 'dart:math';

enum CommandStatus { pending, executed, errored, skipped }

class TrackedCommand {
  /// Optional delegate for app-specific command summaries.
  /// Set at startup: `TrackedCommand.summarizer = processor.commandSummary;`
  static String Function(String commandName, Map<String, dynamic> args)? summarizer;

  /// Optional delegate for app-specific command previews.
  static String Function(String commandName, Map<String, dynamic> args)? previewer;

  final String id;
  final Map<String, dynamic> command;
  final DateTime createdAt;
  final String? replacesId;

  CommandStatus status;
  DateTime? executedAt;
  String? errorMessage;
  bool archived;

  TrackedCommand({
    required this.command,
    this.replacesId,
  })  : id = _generateId(),
        createdAt = DateTime.now(),
        status = CommandStatus.pending,
        archived = false;

  TrackedCommand._withId({
    required this.id,
    required this.command,
    required this.createdAt,
    required this.status,
    this.replacesId,
    this.executedAt,
    this.errorMessage,
    this.archived = false,
  });

  /// Generates a 5-character alphanumeric ID prefixed with 'c-'
  /// e.g., "c-a3f2k"
  static String _generateId() {
    const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
    final random = Random.secure();
    final id = List.generate(5, (_) => chars[random.nextInt(chars.length)]).join();
    return 'c-$id';
  }

  String get commandName => command['command'] as String? ?? 'unknown';

  String get summary {
    final args = command['arguments'] as Map<String, dynamic>? ?? {};
    if (summarizer != null) {
      return summarizer!(commandName, args);
    }
    return _defaultSummary(args);
  }

  String _defaultSummary(Map<String, dynamic> args) {
    switch (commandName) {
      case 'add':
        final desc = args['description'] ?? '?';
        final loc = args['location'] ?? '?';
        return 'add "$desc" to $loc';
      case 'delete':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        return 'delete "$search"';
      case 'move':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final loc = args['new_location'] ?? args['newLocation'] ?? '?';
        return 'move "$search" to $loc';
      case 'edit-description':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final newDesc = args['new_description'] ?? args['newDescription'] ?? '?';
        return 'edit "$search" to "$newDesc"';
      case 'put-in':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final container = args['container_id'] ?? args['containerId'] ?? '?';
        return 'put "$search" in $container';
      case 'remove-from-container':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        return 'remove "$search" from container';
      case 'create-container':
        final containerId = args['container_id'] ?? args['containerId'] ?? '?';
        final loc = args['location'] ?? '?';
        return 'create container $containerId at $loc';
      case 'add-tag':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final tag = args['tag'] ?? '?';
        return 'add tag "$tag" to "$search"';
      case 'remove-tag':
        final search = args['search_str'] ?? args['searchStr'] ?? '?';
        final tag = args['tag'] ?? '?';
        return 'remove tag "$tag" from "$search"';
      case 'group-put-in':
        final tag = args['tag'] ?? '?';
        final container = args['container_id'] ?? args['containerId'] ?? '?';
        return 'put all tagged "$tag" in $container';
      case 'group-remove-tag':
        final tag = args['tag'] ?? '?';
        return 'remove tag "$tag" from all';
      default:
        return '$commandName(...)';
    }
  }

  String get preview {
    final args = command['arguments'] as Map<String, dynamic>? ?? {};
    if (previewer != null) {
      return previewer!(commandName, args);
    }
    return _defaultPreview(args);
  }

  String _defaultPreview(Map<String, dynamic> args) {
    final desc = args['description'];
    final search = args['search_str'] ?? args['searchStr'];
    final tag = args['tag'];
    return (desc is String ? desc : null) ??
           (search is String ? search : null) ??
           (tag is String ? tag : null) ??
           '';
  }

  String get statusSymbol {
    switch (status) {
      case CommandStatus.pending:
        return '☐';
      case CommandStatus.executed:
        return '✓';
      case CommandStatus.errored:
        return '✗';
      case CommandStatus.skipped:
        return '−';
    }
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'command': command,
        'createdAt': createdAt.toIso8601String(),
        'status': status.name,
        'replacesId': replacesId,
        'executedAt': executedAt?.toIso8601String(),
        'errorMessage': errorMessage,
        'archived': archived,
      };

  factory TrackedCommand.fromJson(Map<String, dynamic> json) {
    return TrackedCommand._withId(
      id: json['id'] as String,
      command: json['command'] as Map<String, dynamic>,
      createdAt: DateTime.parse(json['createdAt'] as String),
      status: CommandStatus.values.byName(json['status'] as String),
      replacesId: json['replacesId'] as String?,
      executedAt: json['executedAt'] != null
          ? DateTime.parse(json['executedAt'] as String)
          : null,
      errorMessage: json['errorMessage'] as String?,
      archived: json['archived'] as bool? ?? false,
    );
  }
}
