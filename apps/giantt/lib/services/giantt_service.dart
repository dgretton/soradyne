import 'dart:io';
import 'package:path_provider/path_provider.dart';
import 'package:giantt_core/giantt_core.dart';

/// Service class for managing Giantt operations in Flutter
class GianttService {
  static final GianttService _instance = GianttService._internal();
  factory GianttService() => _instance;
  GianttService._internal();

  String? _workspacePath;
  GianttGraph? _graph;
  LogCollection? _logs;

  /// Initialize the service with workspace path
  Future<void> initialize() async {
    if (_workspacePath == null) {
      final documentsDir = await getApplicationDocumentsDirectory();
      _workspacePath = '${documentsDir.path}/giantt';
      
      // Ensure workspace exists
      if (!FileRepository.isWorkspaceInitialized(_workspacePath!)) {
        FileRepository.initializeWorkspace(_workspacePath!);
      }
    }
  }

  /// Get the current workspace path
  String get workspacePath {
    if (_workspacePath == null) {
      throw StateError('GianttService not initialized. Call initialize() first.');
    }
    return _workspacePath!;
  }

  /// Get the current graph, loading if necessary
  Future<GianttGraph> getGraph() async {
    await initialize();
    
    if (_graph == null) {
      await _loadGraph();
    }
    return _graph!;
  }

  /// Get the current logs, loading if necessary
  Future<LogCollection> getLogs() async {
    await initialize();
    
    if (_logs == null) {
      await _loadLogs();
    }
    return _logs!;
  }

  /// Load the graph from files
  Future<void> _loadGraph() async {
    final paths = FileRepository.getDefaultFilePaths(_workspacePath);
    _graph = DualFileManager.loadGraph(
      paths['items']!,
      paths['occlude_items']!,
    );
  }

  /// Load the logs from files
  Future<void> _loadLogs() async {
    final paths = FileRepository.getDefaultFilePaths(_workspacePath);
    _logs = DualFileManager.loadLogs(
      paths['logs']!,
      paths['occlude_logs']!,
    );
  }

  /// Save the current graph to files
  Future<void> saveGraph() async {
    if (_graph == null) return;
    
    final paths = FileRepository.getDefaultFilePaths(_workspacePath);
    DualFileManager.saveGraph(
      paths['items']!,
      paths['occlude_items']!,
      _graph!,
    );
  }

  /// Save the current logs to files
  Future<void> saveLogs() async {
    if (_logs == null) return;
    
    final paths = FileRepository.getDefaultFilePaths(_workspacePath);
    DualFileManager.saveLogs(
      paths['logs']!,
      paths['occlude_logs']!,
      _logs!,
    );
  }

  /// Add a new item to the graph
  Future<CommandResult<GianttItem>> addItem({
    required String id,
    required String title,
    GianttStatus status = GianttStatus.notStarted,
    GianttPriority priority = GianttPriority.neutral,
    GianttDuration? duration,
    List<String> charts = const [],
    List<String> tags = const [],
    Map<String, List<String>> relations = const {},
    List<TimeConstraint> timeConstraints = const [],
  }) async {
    final graph = await getGraph();
    
    // Check if item already exists
    if (graph.items.containsKey(id)) {
      return CommandResult.failure('Item with ID "$id" already exists');
    }

    // Create new item
    final newItem = GianttItem(
      id: id,
      title: title,
      status: status,
      priority: priority,
      duration: duration ?? GianttDuration.zero(),
      charts: charts,
      tags: tags,
      relations: relations,
      timeConstraints: timeConstraints,
    );

    // Add to graph
    graph.addItem(newItem);
    _graph = graph;

    // Save to files
    await saveGraph();

    return CommandResult.success(newItem, 'Added item "$id" successfully');
  }

  /// Get items matching a search term
  Future<List<GianttItem>> searchItems(String searchTerm, {bool includeOccluded = false}) async {
    final graph = await getGraph();
    
    final items = includeOccluded 
        ? graph.items.values.toList()
        : graph.includedItems.values.toList();
    
    if (searchTerm.isEmpty) {
      return items;
    }
    
    return items.where((item) =>
        item.id.toLowerCase().contains(searchTerm.toLowerCase()) ||
        item.title.toLowerCase().contains(searchTerm.toLowerCase()) ||
        item.tags.any((tag) => tag.toLowerCase().contains(searchTerm.toLowerCase()))
    ).toList();
  }

  /// Get items by chart
  Future<List<GianttItem>> getItemsByChart(String chartName, {bool includeOccluded = false}) async {
    final graph = await getGraph();
    
    final items = includeOccluded 
        ? graph.items.values.toList()
        : graph.includedItems.values.toList();
    
    return items.where((item) => item.charts.contains(chartName)).toList();
  }

  /// Get all unique chart names
  Future<List<String>> getAllCharts({bool includeOccluded = false}) async {
    final graph = await getGraph();
    
    final items = includeOccluded 
        ? graph.items.values.toList()
        : graph.includedItems.values.toList();
    
    final charts = <String>{};
    for (final item in items) {
      charts.addAll(item.charts);
    }
    
    return charts.toList()..sort();
  }

  /// Get all unique tags
  Future<List<String>> getAllTags({bool includeOccluded = false}) async {
    final graph = await getGraph();
    
    final items = includeOccluded 
        ? graph.items.values.toList()
        : graph.includedItems.values.toList();
    
    final tags = <String>{};
    for (final item in items) {
      tags.addAll(item.tags);
    }
    
    return tags.toList()..sort();
  }

  /// Update an existing item
  Future<CommandResult<GianttItem>> updateItem(String itemId, GianttItem updatedItem) async {
    final graph = await getGraph();
    
    if (!graph.items.containsKey(itemId)) {
      return CommandResult.failure('Item with ID "$itemId" not found');
    }

    // Update the item
    graph.addItem(updatedItem);
    _graph = graph;

    // Save to files
    await saveGraph();

    return CommandResult.success(updatedItem, 'Updated item "$itemId" successfully');
  }

  /// Occlude an item
  Future<CommandResult<String>> occludeItem(String itemId) async {
    final graph = await getGraph();
    
    final item = graph.items[itemId];
    if (item == null) {
      return CommandResult.failure('Item with ID "$itemId" not found');
    }

    if (item.occlude) {
      return CommandResult.failure('Item "$itemId" is already occluded');
    }

    // Occlude the item
    graph.occludeItem(itemId);
    _graph = graph;

    // Save to files
    await saveGraph();

    return CommandResult.success(itemId, 'Occluded item "$itemId" successfully');
  }

  /// Include (un-occlude) an item
  Future<CommandResult<String>> includeItem(String itemId) async {
    final graph = await getGraph();
    
    final item = graph.items[itemId];
    if (item == null) {
      return CommandResult.failure('Item with ID "$itemId" not found');
    }

    if (!item.occlude) {
      return CommandResult.failure('Item "$itemId" is not occluded');
    }

    // Include the item
    graph.includeItem(itemId);
    _graph = graph;

    // Save to files
    await saveGraph();

    return CommandResult.success(itemId, 'Included item "$itemId" successfully');
  }

  /// Refresh data from files (reload)
  Future<void> refresh() async {
    _graph = null;
    _logs = null;
    await _loadGraph();
    await _loadLogs();
  }

  /// Get workspace statistics
  Future<Map<String, dynamic>> getWorkspaceStats() async {
    final graph = await getGraph();
    final logs = await getLogs();
    
    final includedItems = graph.includedItems.values.toList();
    final occludedItems = graph.occludedItems.values.toList();
    
    return {
      'total_items': graph.items.length,
      'included_items': includedItems.length,
      'occluded_items': occludedItems.length,
      'total_logs': logs.length,
      'included_logs': logs.includedEntries.length,
      'occluded_logs': logs.occludedEntries.length,
      'charts': await getAllCharts(),
      'tags': await getAllTags(),
      'status_breakdown': _getStatusBreakdown(includedItems),
      'priority_breakdown': _getPriorityBreakdown(includedItems),
    };
  }

  Map<String, int> _getStatusBreakdown(List<GianttItem> items) {
    final breakdown = <String, int>{};
    for (final item in items) {
      final status = item.status.name;
      breakdown[status] = (breakdown[status] ?? 0) + 1;
    }
    return breakdown;
  }

  Map<String, int> _getPriorityBreakdown(List<GianttItem> items) {
    final breakdown = <String, int>{};
    for (final item in items) {
      final priority = item.priority.name;
      breakdown[priority] = (breakdown[priority] ?? 0) + 1;
    }
    return breakdown;
  }
}
