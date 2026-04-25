import 'dart:convert';
import 'dart:io';
import 'package:path_provider/path_provider.dart';
import 'package:giantt_core/giantt_core.dart';

/// Service class for managing Giantt operations in the Flutter app.
///
/// Reads and writes via soradyne [FlowRepository]. Flow UUIDs are stored
/// in app documents under `giantt/flows.json`. On first launch,
/// [_installDevFlows] seeds a hardcoded dev UUID so all builds from this
/// monorepo share the same flow by default — see that method for removal
/// instructions once real flow sharing exists.
class GianttService {
  static final GianttService _instance = GianttService._internal();
  factory GianttService() => _instance;
  GianttService._internal();

  bool _initialized = false;

  /// Ordered list of active flow UUIDs for this app instance.
  /// The first entry is the default flow (receives new items).
  List<String> _flowUuids = [];

  /// Maps item ID → the flow UUID it was loaded from, refreshed on each read.
  Map<String, String> _itemToFlow = {};

  // ── Hardcoded dev flow UUID ─────────────────────────────────────────────────
  // TODO: remove when real flow sharing (app-level invite/join) is implemented.
  // All giantt software built from this monorepo uses this UUID by default so
  // that CLI and app instances on the same device see the same data.
  static const String _devDefaultFlowUuid = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890';

  // ── Initialization ──────────────────────────────────────────────────────────

  Future<void> initialize() async {
    if (_initialized) return;

    final supportDir = await getApplicationSupportDirectory();
    _appSupportPath = supportDir.path;

    final docsDir = await getApplicationDocumentsDirectory();
    final gianttDir = Directory('${docsDir.path}/giantt');
    if (!gianttDir.existsSync()) gianttDir.createSync(recursive: true);

    final flowsFile = File('${gianttDir.path}/flows.json');

    if (!flowsFile.existsSync()) {
      await _installDevFlows(flowsFile);
    }

    final stored = jsonDecode(flowsFile.readAsStringSync()) as List<dynamic>;
    _flowUuids = stored.map((e) => e.toString()).toList();

    final deviceId = await _getOrCreateDeviceId(gianttDir.path);
    FlowRepository.initialize(deviceId);

    _startSync();

    _initialized = true;
  }

  /// Start background sync for all flows. Non-fatal — if there's no capsule
  /// yet (device not paired) this is a no-op until pairing happens.
  void _startSync() {
    final dataDir = _soradyneDataDir();
    for (final uuid in _flowUuids) {
      try {
        FlowRepository.enableSync(uuid, dataDir: dataDir);
      } catch (e) {
        // Expected when no capsule exists yet; sync will remain off until paired.
      }
    }
  }

  /// Platform-appropriate base directory for soradyne's own storage.
  ///
  /// Desktop (macOS/Linux): `~/.soradyne` — matches the CLI so both share
  /// the same capsule store and static peers config.
  /// Mobile: app support dir + `/.soradyne` (sandboxed per app).
  String? _soradyneDataDir() {
    if (Platform.isAndroid || Platform.isIOS) {
      // _appSupportPath is populated during initialize() before _startSync().
      return _appSupportPath != null ? '$_appSupportPath/.soradyne' : null;
    }
    return null; // FlowRepository falls back to $HOME/.soradyne on desktop
  }

  String? _appSupportPath;

  /// Writes the dev-default flow UUID to app storage on first launch.
  ///
  /// TODO: replace with a proper "create new flow" call once soradyne
  /// exposes flow creation and app-level sharing is designed.
  Future<void> _installDevFlows(File flowsFile) async {
    flowsFile.writeAsStringSync(jsonEncode([_devDefaultFlowUuid]));
  }

  /// Returns (and creates if absent) a stable device ID stored in app storage.
  Future<String> _getOrCreateDeviceId(String gianttDirPath) async {
    final file = File('$gianttDirPath/device_id');
    if (file.existsSync()) return file.readAsStringSync().trim();
    // Derive from platform — use a UUID-shaped hash of hostname + pid for dev.
    final raw = '${Platform.localHostname}-${Platform.environment['USER'] ?? 'user'}';
    final bytes = raw.codeUnits;
    final hash = bytes.fold(0, (h, b) => (h * 31 + b) & 0xFFFFFFFF);
    final id =
        '00000000-0000-4000-8000-${hash.toRadixString(16).padLeft(12, '0')}';
    file.writeAsStringSync(id);
    return id;
  }

  // ── Graph access ────────────────────────────────────────────────────────────

  Future<GianttGraph> getGraph() async {
    await initialize();
    final result = FlowRepository.loadMergedGraph(_flowUuids);
    _itemToFlow = result.itemToFlow;
    return result.graph;
  }

  // ── Writes ──────────────────────────────────────────────────────────────────

  String get _defaultFlow => _flowUuids.first;

  String _flowFor(String itemId) => _itemToFlow[itemId] ?? _defaultFlow;

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
    await initialize();
    final graph = await getGraph();
    if (graph.items.containsKey(id)) {
      return CommandResult.failure('Item with ID "$id" already exists');
    }

    final item = GianttItem(
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

    FlowRepository.saveOperations(_defaultFlow, GianttOp.fromItem(item));
    _itemToFlow[id] = _defaultFlow;
    return CommandResult.success(item, 'Added item "$id" successfully');
  }

  Future<CommandResult<GianttItem>> updateItem(
      String itemId, GianttItem updatedItem) async {
    await initialize();
    final graph = await getGraph();
    if (!graph.items.containsKey(itemId)) {
      return CommandResult.failure('Item with ID "$itemId" not found');
    }
    final flow = _flowFor(itemId);
    final old = graph.items[itemId]!;

    final ops = <GianttOp>[];
    if (updatedItem.title != old.title) ops.add(GianttOp.setTitle(itemId, updatedItem.title));
    if (updatedItem.status != old.status) ops.add(GianttOp.setStatus(itemId, updatedItem.status));
    if (updatedItem.priority != old.priority) ops.add(GianttOp.setPriority(itemId, updatedItem.priority));
    if (updatedItem.duration.toString() != old.duration.toString()) {
      ops.add(GianttOp.setDuration(itemId, updatedItem.duration));
    }
    if (updatedItem.userComment != old.userComment) {
      ops.add(GianttOp.setComment(itemId, updatedItem.userComment));
    }
    if (updatedItem.occlude != old.occlude) {
      ops.add(GianttOp.setOccluded(itemId, updatedItem.occlude));
    }
    for (final tag in updatedItem.tags.where((t) => !old.tags.contains(t))) {
      ops.add(GianttOp.addTag(itemId, tag));
    }
    for (final chart in updatedItem.charts.where((c) => !old.charts.contains(c))) {
      ops.add(GianttOp.addChart(itemId, chart));
    }

    if (ops.isNotEmpty) FlowRepository.saveOperations(flow, ops);
    return CommandResult.success(updatedItem, 'Updated item "$itemId" successfully');
  }

  Future<CommandResult<String>> occludeItem(String itemId) async {
    await initialize();
    final graph = await getGraph();
    final item = graph.items[itemId];
    if (item == null) return CommandResult.failure('Item "$itemId" not found');
    if (item.occlude) return CommandResult.failure('Item "$itemId" is already occluded');
    FlowRepository.saveOperation(_flowFor(itemId), GianttOp.setOccluded(itemId, true));
    return CommandResult.success(itemId, 'Occluded item "$itemId"');
  }

  Future<CommandResult<String>> includeItem(String itemId) async {
    await initialize();
    final graph = await getGraph();
    final item = graph.items[itemId];
    if (item == null) return CommandResult.failure('Item "$itemId" not found');
    if (!item.occlude) return CommandResult.failure('Item "$itemId" is not occluded');
    FlowRepository.saveOperation(_flowFor(itemId), GianttOp.setOccluded(itemId, false));
    return CommandResult.success(itemId, 'Included item "$itemId"');
  }

  // ── Queries ─────────────────────────────────────────────────────────────────

  Future<List<GianttItem>> searchItems(String searchTerm,
      {bool includeOccluded = false}) async {
    final graph = await getGraph();
    final items = includeOccluded
        ? graph.items.values.toList()
        : graph.includedItems.values.toList();
    if (searchTerm.isEmpty) return items;
    final q = searchTerm.toLowerCase();
    return items
        .where((i) =>
            i.id.toLowerCase().contains(q) ||
            i.title.toLowerCase().contains(q) ||
            i.tags.any((t) => t.toLowerCase().contains(q)))
        .toList();
  }

  Future<List<GianttItem>> getItemsByChart(String chartName,
      {bool includeOccluded = false}) async {
    final graph = await getGraph();
    final items = includeOccluded
        ? graph.items.values.toList()
        : graph.includedItems.values.toList();
    return items.where((i) => i.charts.contains(chartName)).toList();
  }

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

  Future<Map<String, dynamic>> getWorkspaceStats() async {
    final graph = await getGraph();
    final included = graph.includedItems.values.toList();
    final occluded = graph.occludedItems.values.toList();
    return {
      'total_items': graph.items.length,
      'included_items': included.length,
      'occluded_items': occluded.length,
      'charts': await getAllCharts(),
      'tags': await getAllTags(),
      'status_breakdown': _breakdown(included, (i) => i.status.name),
      'priority_breakdown': _breakdown(included, (i) => i.priority.name),
    };
  }

  Map<String, int> _breakdown(List<GianttItem> items, String Function(GianttItem) key) {
    final m = <String, int>{};
    for (final item in items) {
      final k = key(item);
      m[k] = (m[k] ?? 0) + 1;
    }
    return m;
  }
}
