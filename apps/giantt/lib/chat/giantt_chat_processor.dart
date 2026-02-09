import 'package:giantt_core/giantt_core.dart';
import '../services/giantt_service.dart';

/// Commands that are auto-executed and return results to the LLM (ReAct queries).
const queryCommands = {
  'show',
  'list-items',
  'list-charts',
  'list-tags',
  'show-relations',
  'show-includes',
  'doctor',
};

/// Commands that produce user-visible command cards for approval.
const actionCommands = {
  'add',
  'modify',
  'remove',
  'set-status',
  'insert',
  'occlude',
  'include',
  'add-relation',
  'remove-relation',
  'log',
};

class GianttChatProcessor {
  final GianttService _service;

  GianttChatProcessor(this._service);

  /// Returns true if [commandName] is a query (auto-executed for ReAct).
  bool isQuery(String commandName) => queryCommands.contains(commandName);

  /// Execute a query command and return formatted results.
  /// Returns null if the command is not a query.
  Future<String?> executeQuery(Map<String, dynamic> command) async {
    final name = command['command'] as String?;
    if (name == null || !isQuery(name)) return null;

    final args = command['arguments'] as Map<String, dynamic>? ?? {};

    try {
      switch (name) {
        case 'show':
          return await _show(args);
        case 'list-items':
          return await _listItems(args);
        case 'list-charts':
          return await _listCharts();
        case 'list-tags':
          return await _listTags();
        case 'show-relations':
          return await _showRelations(args);
        case 'show-includes':
          return _showIncludes();
        case 'doctor':
          return await _doctor();
        default:
          return 'Unknown query command: $name';
      }
    } catch (e) {
      return 'Error executing $name: $e';
    }
  }

  /// Execute an action command (mutation). Throws on failure.
  Future<void> executeAction(Map<String, dynamic> command) async {
    final name = command['command'] as String?;
    final args = command['arguments'] as Map<String, dynamic>? ?? {};

    switch (name) {
      case 'add':
        await _add(args);
      case 'modify':
        await _modify(args);
      case 'remove':
        await _remove(args);
      case 'set-status':
        await _setStatus(args);
      case 'insert':
        await _insert(args);
      case 'occlude':
        await _occlude(args);
      case 'include':
        await _include(args);
      case 'add-relation':
        await _addRelation(args);
      case 'remove-relation':
        await _removeRelation(args);
      case 'log':
        await _log(args);
      default:
        throw StateError('Unknown action command: $name');
    }
  }

  // -- Query implementations --

  Future<String> _show(Map<String, dynamic> args) async {
    final search = _requireArg<String>(args, 'search');
    final graph = await _service.getGraph();
    final item = graph.findBySubstring(search);
    return _formatItemDetail(item);
  }

  Future<String> _listItems(Map<String, dynamic> args) async {
    final chart = args['chart'] as String?;
    final status = args['status'] as String?;
    final tag = args['tag'] as String?;

    List<GianttItem> items;
    if (chart != null) {
      items = await _service.getItemsByChart(chart);
    } else {
      items = await _service.searchItems('');
    }

    if (status != null) {
      items = items.where((i) => i.status.name == status).toList();
    }
    if (tag != null) {
      items = items.where((i) => i.tags.contains(tag)).toList();
    }

    if (items.isEmpty) return 'No items found.';

    final lines = items.map((i) =>
        '${i.status.symbol} ${i.id} "${i.title}" P:${i.priority.name} D:${i.duration}');
    return 'Items (${items.length}):\n${lines.join('\n')}';
  }

  Future<String> _listCharts() async {
    final charts = await _service.getAllCharts();
    if (charts.isEmpty) return 'No charts defined.';
    return 'Charts: ${charts.join(', ')}';
  }

  Future<String> _listTags() async {
    final tags = await _service.getAllTags();
    if (tags.isEmpty) return 'No tags defined.';
    return 'Tags: ${tags.join(', ')}';
  }

  Future<String> _showRelations(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final graph = await _service.getGraph();
    final item = graph.items[id];
    if (item == null) return 'Item "$id" not found.';

    if (item.relations.isEmpty) return 'Item "$id" has no relations.';

    final lines = <String>[];
    for (final entry in item.relations.entries) {
      lines.add('  ${entry.key}: ${entry.value.join(', ')}');
    }
    return 'Relations for "$id":\n${lines.join('\n')}';
  }

  String _showIncludes() {
    final paths = FileRepository.getDefaultFilePaths(_service.workspacePath);
    final includes = FileRepository.parseIncludeDirectives(paths['items']!);
    if (includes.isEmpty) return 'No include directives found.';
    return 'Include files:\n${includes.map((p) => '  - $p').join('\n')}';
  }

  Future<String> _doctor() async {
    final graph = await _service.getGraph();
    final doctor = GraphDoctor(graph);
    final issues = doctor.fullDiagnosis();
    if (issues.isEmpty) return 'No issues found. Graph is healthy.';

    final lines = issues.map((i) => '  [${i.type.value}] ${i.itemId}: ${i.message}');
    return 'Issues (${issues.length}):\n${lines.join('\n')}';
  }

  // -- Action implementations --

  Future<void> _add(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final title = _requireArg<String>(args, 'title');
    final statusName = args['status'] as String?;
    final priorityName = args['priority'] as String?;
    final durationStr = args['duration'] as String?;
    final charts = _parseStringList(args['charts']);
    final tags = _parseStringList(args['tags']);
    final requires = _parseStringList(args['requires']);
    final anyOf = _parseStringList(args['any_of'] ?? args['anyOf']);

    final relations = <String, List<String>>{};
    if (requires.isNotEmpty) relations['REQUIRES'] = requires;
    if (anyOf.isNotEmpty) relations['ANYOF'] = anyOf;

    final result = await _service.addItem(
      id: id,
      title: title,
      status: statusName != null ? GianttStatus.fromName(statusName) : GianttStatus.notStarted,
      priority: priorityName != null ? GianttPriority.fromName(priorityName) : GianttPriority.neutral,
      duration: durationStr != null ? GianttDuration.parse(durationStr) : null,
      charts: charts,
      tags: tags,
      relations: relations,
    );

    if (!result.success) throw StateError(result.message);
  }

  Future<void> _modify(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final graph = await _service.getGraph();
    final item = graph.items[id];
    if (item == null) throw StateError('Item "$id" not found');

    final updated = item.copyWith(
      title: args['title'] as String? ?? item.title,
      status: args['status'] != null
          ? GianttStatus.fromName(args['status'] as String)
          : null,
      priority: args['priority'] != null
          ? GianttPriority.fromName(args['priority'] as String)
          : null,
      duration: args['duration'] != null
          ? GianttDuration.parse(args['duration'] as String)
          : null,
      charts: args.containsKey('charts') ? _parseStringList(args['charts']) : null,
      tags: args.containsKey('tags') ? _parseStringList(args['tags']) : null,
    );

    final result = await _service.updateItem(id, updated);
    if (!result.success) throw StateError(result.message);
  }

  Future<void> _remove(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final graph = await _service.getGraph();
    if (!graph.items.containsKey(id)) {
      throw StateError('Item "$id" not found');
    }
    graph.removeItem(id);
    await _service.saveGraph();
  }

  Future<void> _setStatus(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final statusName = _requireArg<String>(args, 'status');
    final graph = await _service.getGraph();
    final item = graph.items[id];
    if (item == null) throw StateError('Item "$id" not found');

    final updated = item.copyWith(status: GianttStatus.fromName(statusName));
    final result = await _service.updateItem(id, updated);
    if (!result.success) throw StateError(result.message);
  }

  Future<void> _insert(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final title = _requireArg<String>(args, 'title');
    final before = _requireArg<String>(args, 'before');
    final after = _requireArg<String>(args, 'after');
    final durationStr = args['duration'] as String?;
    final priorityName = args['priority'] as String?;

    final newItem = GianttItem(
      id: id,
      title: title,
      duration: durationStr != null ? GianttDuration.parse(durationStr) : GianttDuration.zero(),
      priority: priorityName != null ? GianttPriority.fromName(priorityName) : GianttPriority.neutral,
    );

    final graph = await _service.getGraph();
    graph.insertBetween(newItem, before, after);
    await _service.saveGraph();
  }

  Future<void> _occlude(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final result = await _service.occludeItem(id);
    if (!result.success) throw StateError(result.message);
  }

  Future<void> _include(Map<String, dynamic> args) async {
    final id = _requireArg<String>(args, 'id');
    final result = await _service.includeItem(id);
    if (!result.success) throw StateError(result.message);
  }

  Future<void> _addRelation(Map<String, dynamic> args) async {
    final from = _requireArg<String>(args, 'from');
    final typeName = _requireArg<String>(args, 'type');
    final to = _requireArg<String>(args, 'to');

    final graph = await _service.getGraph();
    graph.addRelation(from, RelationType.fromName(typeName), to);
    await _service.saveGraph();
  }

  Future<void> _removeRelation(Map<String, dynamic> args) async {
    final from = _requireArg<String>(args, 'from');
    final typeName = _requireArg<String>(args, 'type');
    final to = _requireArg<String>(args, 'to');

    final graph = await _service.getGraph();
    graph.removeRelation(from, RelationType.fromName(typeName), to);
    await _service.saveGraph();
  }

  Future<void> _log(Map<String, dynamic> args) async {
    final session = _requireArg<String>(args, 'session');
    final message = _requireArg<String>(args, 'message');
    final tags = _parseStringList(args['tags']);

    final logs = await _service.getLogs();
    logs.createEntry(
      session,
      message,
      additionalTags: tags.isNotEmpty ? tags : null,
    );
    await _service.saveLogs();
  }

  // -- Helpers --

  T _requireArg<T>(Map<String, dynamic> args, String key) {
    final value = args[key];
    if (value == null) throw StateError('Missing required argument: $key');
    return value as T;
  }

  List<String> _parseStringList(dynamic value) {
    if (value == null) return [];
    if (value is List) return value.cast<String>();
    if (value is String) {
      return value.split(',').map((s) => s.trim()).where((s) => s.isNotEmpty).toList();
    }
    return [];
  }

  String _formatItemDetail(GianttItem item) {
    final lines = <String>[
      '${item.status.symbol} ${item.id} "${item.title}"',
      '  Status: ${item.status.name}',
      '  Priority: ${item.priority.name}',
      '  Duration: ${item.duration}',
    ];

    if (item.charts.isNotEmpty) {
      lines.add('  Charts: ${item.charts.join(', ')}');
    }
    if (item.tags.isNotEmpty) {
      lines.add('  Tags: ${item.tags.join(', ')}');
    }
    if (item.relations.isNotEmpty) {
      for (final entry in item.relations.entries) {
        lines.add('  ${entry.key}: ${entry.value.join(', ')}');
      }
    }
    if (item.timeConstraints.isNotEmpty) {
      lines.add('  Time constraints: ${item.timeConstraints.length}');
    }
    if (item.userComment != null) {
      lines.add('  Comment: ${item.userComment}');
    }

    return lines.join('\n');
  }

  /// Build a compact graph summary for the LLM context window.
  Future<String> buildGraphSummary() async {
    final graph = await _service.getGraph();
    final included = graph.includedItems.values.toList();
    final occluded = graph.occludedItems.values.toList();

    if (included.isEmpty && occluded.isEmpty) {
      return 'Graph is empty. No items.';
    }

    final lines = <String>[
      'Items (${included.length} active, ${occluded.length} occluded):',
    ];

    for (final item in included) {
      final parts = <String>[
        '${item.status.symbol} ${item.id} "${item.title}"',
        'P:${item.priority.name}',
        'D:${item.duration}',
      ];
      if (item.charts.isNotEmpty) {
        parts.add('charts:{${item.charts.join(",")}}');
      }
      if (item.tags.isNotEmpty) {
        parts.add('tags:[${item.tags.join(",")}]');
      }
      // Show dependency relations compactly
      final requires = item.relations['REQUIRES'] ?? [];
      final anyof = item.relations['ANYOF'] ?? [];
      if (requires.isNotEmpty) parts.add('requires:[${requires.join(",")}]');
      if (anyof.isNotEmpty) parts.add('anyof:[${anyof.join(",")}]');

      lines.add(parts.join(' '));
    }

    return lines.join('\n');
  }
}
