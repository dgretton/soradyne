import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
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

    // Initialize pairing bridge BEFORE FlowRepository so that when
    // soradyne_flow_init creates the FlowRegistry it picks up the correct
    // data_dir from the bridge (via bridge_data_dir()). Without this, the
    // registry falls back to "./soradyne/flows" (wrong on Android) and flows
    // open empty even when journals exist on disk.
    final soradyneDir = _soradyneDataDir();
    if (soradyneDir != null) {
      try {
        FlowClient.initPairingBridge(soradyneDir);
      } catch (_) {
        // Non-fatal: flow reads may fall back to wrong path, but sync will
        // deliver ops once the bridge is initialized later in startSyncWhenReady.
      }
    }

    final deviceId = await _getOrCreateDeviceId(gianttDir.path);
    FlowRepository.initialize(deviceId);

    _initialized = true;
  }

  /// Start background sync. Call this after the UI has rendered — it makes
  /// synchronous FFI calls into Rust that may take a moment on first run.
  void startSyncWhenReady() {
    debugPrint('[giantt] startSyncWhenReady: beginning sync init');
    final sw = Stopwatch()..start();
    _startSync();
    debugPrint('[giantt] startSyncWhenReady: done in ${sw.elapsedMilliseconds}ms');
  }

  /// Start background sync for all flows. Non-fatal — if there's no capsule
  /// yet (device not paired) this is a no-op until pairing happens.
  void _startSync() {
    final dataDir = _soradyneDataDir();
    debugPrint('[giantt] _startSync: dataDir=$dataDir flows=$_flowUuids');
    for (final uuid in _flowUuids) {
      try {
        final sw = Stopwatch()..start();
        debugPrint('[giantt] _startSync: enabling sync for $uuid');
        FlowRepository.enableSync(uuid, dataDir: dataDir);
        debugPrint('[giantt] _startSync: $uuid done in ${sw.elapsedMilliseconds}ms');
      } catch (e) {
        debugPrint('[giantt] _startSync: $uuid failed: $e');
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
    // Desktop: share $HOME/.soradyne with soradyne-cli so both see the same
    // capsule store and flows.
    final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
    return home != null ? '$home/.soradyne' : null;
  }

  String? _appSupportPath;

  /// Writes the dev-default flow UUID to app storage on first launch.
  ///
  /// TODO: replace with a proper "create new flow" call once soradyne
  /// exposes flow creation and app-level sharing is designed.
  Future<void> _installDevFlows(File flowsFile) async {
    flowsFile.writeAsStringSync(jsonEncode([_devDefaultFlowUuid]));
  }

  /// Returns the device ID to use for CRDT op authorship.
  ///
  /// Reads from soradyne's device_identity.json so the CRDT author UUID matches
  /// the soradyne pairing identity — ensures journal files are named consistently
  /// and that op authors are unambiguous across devices.
  Future<String> _getOrCreateDeviceId(String gianttDirPath) async {
    final soradyneDir = _soradyneDataDir();
    if (soradyneDir != null) {
      final identityFile = File('$soradyneDir/device_identity.json');
      if (identityFile.existsSync()) {
        try {
          final json = jsonDecode(identityFile.readAsStringSync()) as Map<String, dynamic>;
          final id = json['device_id'] as String?;
          if (id != null && id.isNotEmpty) return id;
        } catch (_) {}
      }
    }
    // Fallback: stable UUID in giantt app storage (generated once, then reused).
    final file = File('$gianttDirPath/device_id');
    if (file.existsSync()) return file.readAsStringSync().trim();
    // Last resort: generate a new UUID via Dart's UUID library if available,
    // or use a time-based unique ID.
    final id = DateTime.now().microsecondsSinceEpoch.toRadixString(16).padLeft(16, '0');
    final uuid = '${id.substring(0,8)}-${id.substring(8,12)}-4${id.substring(13,16)}-8${id.substring(13,16)}-${id.padLeft(12,'0').substring(4,16)}';
    file.writeAsStringSync(uuid);
    return uuid;
  }

  // ── Public accessors for home screen ────────────────────────────────────────

  /// The soradyne data directory path, or null if not yet initialized.
  String? get soradyneDataDir => _soradyneDataDir();

  /// The local device ID used as CRDT op author, or null if not initialized.
  Future<String?> get localDeviceId async {
    if (!_initialized) return null;
    final soradyneDir = _soradyneDataDir();
    if (soradyneDir == null) return null;
    try {
      final f = File('$soradyneDir/device_identity.json');
      if (!f.existsSync()) return null;
      final json = jsonDecode(f.readAsStringSync()) as Map<String, dynamic>;
      return json['device_id'] as String?;
    } catch (_) {
      return null;
    }
  }

  /// The primary flow UUID (first in the configured list), or null.
  String? get primaryFlowUuid =>
      _initialized && _flowUuids.isNotEmpty ? _flowUuids.first : null;

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
    // userComment maps to the 'comment' CRDT field
    if (updatedItem.userComment != old.userComment) {
      ops.add(GianttOp.setComment(itemId, updatedItem.userComment));
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

  /// Permanently remove an item from the flow.
  Future<CommandResult<String>> removeItem(String itemId) async {
    await initialize();
    final graph = await getGraph();
    if (!graph.items.containsKey(itemId)) {
      return CommandResult.failure('Item "$itemId" not found');
    }
    FlowRepository.saveOperation(_flowFor(itemId), GianttOp.removeItem(itemId));
    _itemToFlow.remove(itemId);
    return CommandResult.success(itemId, 'Removed item "$itemId"');
  }

  /// Add a relation between two items.
  Future<CommandResult<String>> addRelation(
      String fromId, String relationType, String toId) async {
    await initialize();
    final graph = await getGraph();
    if (!graph.items.containsKey(fromId)) {
      return CommandResult.failure('Item "$fromId" not found');
    }
    final setName = relationType.toLowerCase();
    FlowRepository.saveOperation(
        _flowFor(fromId), GianttOp.addToSet(fromId, setName, toId));
    return CommandResult.success(fromId, 'Added $relationType relation');
  }

  /// Remove a relation between two items.
  Future<CommandResult<String>> removeRelation(
      String fromId, String relationType, String toId) async {
    await initialize();
    final setName = relationType.toLowerCase();
    FlowRepository.saveOperation(
        _flowFor(fromId),
        GianttOp.removeFromSet(fromId, setName, toId, []));
    return CommandResult.success(fromId, 'Removed $relationType relation');
  }

  /// Insert a new item between [beforeId] and [afterId] in the dependency chain.
  Future<CommandResult<GianttItem>> insertBetween(
      GianttItem newItem, String beforeId, String afterId) async {
    await initialize();
    final graph = await getGraph();
    if (!graph.items.containsKey(beforeId)) {
      return CommandResult.failure('Item "$beforeId" not found');
    }
    if (!graph.items.containsKey(afterId)) {
      return CommandResult.failure('Item "$afterId" not found');
    }

    final flow = _defaultFlow;
    final ops = <GianttOp>[
      ...GianttOp.fromItem(newItem),
      // new item REQUIRES before
      GianttOp.addRequires(newItem.id, beforeId),
      // new item is required by after (after REQUIRES new)
      GianttOp.addToSet(afterId, 'requires', newItem.id),
      // remove any direct before→after blocks/requires edges
      GianttOp.removeFromSet(beforeId, 'blocks', afterId, []),
      GianttOp.removeFromSet(afterId, 'requires', beforeId, []),
    ];
    FlowRepository.saveOperations(flow, ops);
    _itemToFlow[newItem.id] = flow;
    return CommandResult.success(newItem, 'Inserted "${newItem.id}"');
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

  // ── Compatibility stubs for callers written against the old file-based API ──

  /// Returns the giantt documents directory. Include directives and file-based
  /// operations are no-ops in the soradyne-backed service.
  String get workspacePath {
    if (!_initialized) return '';
    final docsPath = _appSupportPath ?? (Platform.environment['HOME'] ?? '');
    return '$docsPath/giantt';
  }

  /// No-op — writes are persisted immediately when ops are applied.
  Future<void> saveGraph() async {}

  /// Re-fetches the graph from the flow (clears any stale cached state).
  Future<void> refresh() async {
    _itemToFlow = {};
  }

  /// Returns an empty log collection — logs are not yet modelled in soradyne.
  Future<LogCollection> getLogs() async => LogCollection();

  /// No-op — log persistence is not yet implemented in the soradyne-backed service.
  Future<void> saveLogs() async {}

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
