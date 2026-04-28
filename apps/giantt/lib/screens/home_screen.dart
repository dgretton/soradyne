import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
import 'package:provider/provider.dart';
import '../models/app_state.dart';
import '../services/giantt_service.dart';
import '../services/graph_intelligence.dart';
import '../services/sync_activity_service.dart';
import '../services/chart_recency_service.dart';
import '../screens/item_detail_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  final GianttService _service = GianttService();
  bool _loading = true;

  GianttGraph? _graph;
  List<ScoredItem> _availableNow = [];
  List<GianttItem> _inProgress = [];
  List<DeviceActivity> _syncActivity = [];
  List<String> _recentCharts = [];
  bool _syncExpanded = false;
  int _localRecentOpCount = 0;
  DateTime? _localLatestOpTime;

  // Live activity watcher — polls journal file sizes once per second and
  // flashes _liveBlip whenever a journal grows (incoming or outgoing op).
  Timer? _liveTimer;
  Timer? _blipTimer;
  final Map<String, int> _journalSizes = {};
  bool _liveBlip = false;

  // Clock tick — recomputes time-dependent scoring (chain-overdue) once a
  // minute against the cached graph, with no file I/O.
  Timer? _clockTimer;

  @override
  void initState() {
    super.initState();
    _load().then((_) {
      _startLiveWatcher();
      _startClockTick();
    });
  }

  @override
  void dispose() {
    _liveTimer?.cancel();
    _blipTimer?.cancel();
    _clockTimer?.cancel();
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // Refresh when GianttAppState signals a graph change.
    // (GianttAppState.triggerGraphRefresh → HomeScreen.didChangeDependencies via Consumer above.)
  }

  Future<void> _load({bool showLoading = true}) async {
    if (showLoading && mounted) setState(() => _loading = true);
    try {
      final soradyneDir = _service.soradyneDataDir;
      final localDeviceId = await _service.localDeviceId;

      final graph = await _service.getGraph();
      final available = GraphIntelligence.availableNow(graph);
      final inProgress = GraphIntelligence.inProgress(graph);
      final allCharts = GraphIntelligence.allCharts(graph);
      final recentCharts = await _chartsByActivity(
          graph, allCharts, soradyneDir, _service.primaryFlowUuid);
      List<DeviceActivity> syncActivity = [];
      if (soradyneDir != null && localDeviceId != null) {
        final flowUuid = _service.primaryFlowUuid;
        if (flowUuid != null) {
          final journalsDir = '$soradyneDir/flows/$flowUuid/journals';
          syncActivity = await SyncActivityService.recentActivity(
            journalsDir: journalsDir,
            localDeviceId: localDeviceId,
            since: const Duration(days: 7),
          );
        }
      }

      // Count recent local outgoing ops.
      int localCount = 0;
      DateTime? localLatest;
      if (soradyneDir != null && localDeviceId != null) {
        final flowUuid = _service.primaryFlowUuid;
        if (flowUuid != null) {
          final localJournal = File(
              '$soradyneDir/flows/$flowUuid/journals/$localDeviceId.jsonl');
          if (localJournal.existsSync()) {
            final cutoff = DateTime.now().subtract(const Duration(days: 7));
            for (final line in localJournal.readAsLinesSync().reversed) {
              if (line.trim().isEmpty) continue;
              try {
                final json = jsonDecode(line) as Map<String, dynamic>;
                final ts = DateTime.fromMillisecondsSinceEpoch(
                    (json['timestamp'] as num).toInt());
                if (ts.isBefore(cutoff)) break;
                localCount++;
                localLatest ??= ts;
              } catch (_) {}
            }
          }
        }
      }

      if (mounted) {
        setState(() {
          _graph = graph;
          _availableNow = available;
          _inProgress = inProgress;
          _syncActivity = syncActivity;
          _recentCharts = recentCharts;
          _localRecentOpCount = localCount;
          _localLatestOpTime = localLatest;
          _loading = false;
        });
      }
    } catch (e) {
      if (mounted) setState(() => _loading = false);
    }
  }

  /// Start a 1-second poller that watches journal file sizes and flashes the
  /// live-blip dot whenever any file grows. Cheap (one stat per file per
  /// second) and works without any FFI callback wiring.
  void _startLiveWatcher() {
    _liveTimer?.cancel();
    _liveTimer = Timer.periodic(const Duration(seconds: 1), (_) async {
      if (!mounted) return;
      final soradyneDir = _service.soradyneDataDir;
      final flowUuid = _service.primaryFlowUuid;
      if (soradyneDir == null || flowUuid == null) return;
      final journalsDir = Directory('$soradyneDir/flows/$flowUuid/journals');
      if (!journalsDir.existsSync()) return;

      bool changed = false;
      bool firstScan = _journalSizes.isEmpty;
      for (final f in journalsDir.listSync().whereType<File>()) {
        if (!f.path.endsWith('.jsonl')) continue;
        final size = f.lengthSync();
        final prev = _journalSizes[f.path];
        if (!firstScan && prev != null && size != prev) changed = true;
        _journalSizes[f.path] = size;
      }
      if (changed) {
        if (mounted) setState(() => _liveBlip = true);
        _blipTimer?.cancel();
        _blipTimer = Timer(const Duration(milliseconds: 800), () {
          if (mounted) setState(() => _liveBlip = false);
        });
        // Refresh sync counts so the strip text updates promptly.
        _load(showLoading: false);
      }
    });
  }

  /// Re-score availability against the cached graph once a minute so that
  /// items become "chain-overdue" without waiting for a sync event.
  void _startClockTick() {
    _clockTimer?.cancel();
    _clockTimer = Timer.periodic(const Duration(minutes: 1), (_) {
      if (!mounted || _graph == null) return;
      final available = GraphIntelligence.availableNow(_graph!);
      setState(() => _availableNow = available);
    });
  }

  /// Order charts by the most recent op (any device) that touched an item
  /// belonging to each chart. Falls back to alphabetical for charts with no
  /// recent ops. Manual chip taps (ChartRecencyService) act as a tiebreaker.
  static Future<List<String>> _chartsByActivity(
    GianttGraph graph,
    Set<String> allCharts,
    String? soradyneDir,
    String? flowUuid, {
    int limit = 8,
  }) async {
    if (soradyneDir == null || flowUuid == null) {
      return allCharts.toList()..sort();
    }

    final journalsDir = Directory('$soradyneDir/flows/$flowUuid/journals');
    if (!journalsDir.existsSync()) return allCharts.toList()..sort();

    // item_id → most recent timestamp across all journals
    final itemLatest = <String, DateTime>{};
    final cutoff = DateTime.now().subtract(const Duration(days: 90));

    for (final file in journalsDir.listSync().whereType<File>()) {
      if (!file.path.endsWith('.jsonl')) continue;
      for (final line in file.readAsLinesSync().reversed) {
        if (line.trim().isEmpty) continue;
        try {
          final json = jsonDecode(line) as Map<String, dynamic>;
          final ts = DateTime.fromMillisecondsSinceEpoch(
              (json['timestamp'] as num).toInt());
          if (ts.isBefore(cutoff)) break;
          final op = json['op'] as Map<String, dynamic>;
          final itemId = SyncActivityService.extractItemId(op);
          if (itemId == '?') continue;
          if (!itemLatest.containsKey(itemId) ||
              ts.isAfter(itemLatest[itemId]!)) {
            itemLatest[itemId] = ts;
          }
        } catch (_) {}
      }
    }

    // chart → most recent op timestamp for any item in that chart
    final chartLatest = <String, DateTime>{};
    for (final entry in itemLatest.entries) {
      final item = graph.items[entry.key];
      if (item == null) continue;
      for (final chart in item.charts) {
        if (!chartLatest.containsKey(chart) ||
            entry.value.isAfter(chartLatest[chart]!)) {
          chartLatest[chart] = entry.value;
        }
      }
    }

    // Blend with UI-interaction recency as tiebreaker.
    final uiOrder = await ChartRecencyService.instance
        .ordered(allCharts, limit: allCharts.length);
    final uiRank = {for (var i = 0; i < uiOrder.length; i++) uiOrder[i]: i};

    final sorted = allCharts.toList()
      ..sort((a, b) {
        final ta = chartLatest[a];
        final tb = chartLatest[b];
        if (ta != null && tb != null) {
          final cmp = tb.compareTo(ta);
          if (cmp != 0) return cmp;
        } else if (ta != null) {
          return -1;
        } else if (tb != null) {
          return 1;
        }
        final ra = uiRank[a] ?? 999;
        final rb = uiRank[b] ?? 999;
        if (ra != rb) return ra.compareTo(rb);
        return a.compareTo(b);
      });

    return sorted.take(limit).toList();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('giantt'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: _load,
            tooltip: 'Refresh',
          ),
          IconButton(
            icon: const Icon(Icons.settings_outlined),
            onPressed: () => Navigator.pushNamed(context, '/settings'),
            tooltip: 'Settings',
          ),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : RefreshIndicator(
              onRefresh: _load,
              child: ListView(
                padding: const EdgeInsets.only(bottom: 32),
                children: [
                  _buildSyncStrip(),
                  _buildAvailableNow(),
                  if (_inProgress.isNotEmpty) _buildInProgress(),
                  if (_recentCharts.isNotEmpty) _buildChartStrip(),
                ],
              ),
            ),
    );
  }

  // ── Sync strip ─────────────────────────────────────────────────────────────

  Widget _buildSyncStrip() {
    final cs = Theme.of(context).colorScheme;
    return GestureDetector(
      onTap: () => setState(() => _syncExpanded = !_syncExpanded),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 250),
        color: cs.surfaceContainerHighest,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
              child: Row(
                children: [
                  _LiveBlipDot(active: _liveBlip),
                  const SizedBox(width: 8),
                  Expanded(
                    child: _buildSyncSummaryText(),
                  ),
                  if (_syncActivity.isNotEmpty)
                    Icon(
                      _syncExpanded ? Icons.expand_less : Icons.expand_more,
                      size: 18,
                      color: cs.onSurfaceVariant,
                    ),
                ],
              ),
            ),
            if (_syncExpanded) _buildSyncDiff(),
          ],
        ),
      ),
    );
  }

  Widget _buildSyncSummaryText() {
    final parts = <String>[];

    // Incoming from peers.
    if (_syncActivity.isNotEmpty) {
      final latest = _syncActivity.first;
      final inCount = _syncActivity.fold(0, (s, a) => s + a.ops.length);
      final who = _syncActivity.length == 1
          ? latest.displayName
          : '${_syncActivity.length} peers';
      parts.add('↓ $who · $inCount op${inCount == 1 ? '' : 's'} · '
          '${SyncActivityService.timeAgo(latest.latestTimestamp)}');
    }

    // Outgoing — ops this device authored (confirms writes are persisted).
    if (_localRecentOpCount > 0) {
      parts.add('↑ $_localRecentOpCount sent · '
          '${SyncActivityService.timeAgo(_localLatestOpTime!)}');
    }

    if (parts.isEmpty) {
      return Text(
        _service.primaryFlowUuid == null
            ? 'No flow configured'
            : 'No sync activity in the last 7 days',
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
      );
    }
    return Text(
      parts.join('   '),
      style: Theme.of(context).textTheme.bodySmall,
    );
  }

  Widget _buildSyncDiff() {
    final graph = _graph ?? GianttGraph();
    return Container(
      color: Theme.of(context).colorScheme.surface,
      constraints: const BoxConstraints(maxHeight: 500),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (final device in _syncActivity) ...[
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 10, 16, 4),
                child: Text(
                  '${device.displayName}  ·  '
                  '${SyncActivityService.timeAgo(device.latestTimestamp)}',
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                        color: Theme.of(context).colorScheme.primary,
                        fontWeight: FontWeight.bold,
                      ),
                ),
              ),
              for (final op in device.ops)
                Padding(
                  padding: const EdgeInsets.fromLTRB(24, 2, 16, 2),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('·  ',
                          style: Theme.of(context).textTheme.bodySmall),
                      Expanded(
                        child: Text(
                          op.describe(graph),
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ),
                    ],
                  ),
                ),
            ],
            const SizedBox(height: 12),
          ],
        ),
      ),
    );
  }

  // ── Available Now ──────────────────────────────────────────────────────────

  Widget _buildAvailableNow() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text('Available Now',
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.bold,
                      )),
              const SizedBox(width: 6),
              Text(
                '${_availableNow.length}',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          if (_availableNow.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Text(
                'Nothing unblocked — all available items are in progress or completed.',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
              ),
            )
          else
            for (final scored in _availableNow)
              _AvailableNowCard(
                scored: scored,
                onTap: () => Navigator.pushNamed(context, '/item/${scored.item.id}'),
              ),
        ],
      ),
    );
  }

  // ── In Progress ────────────────────────────────────────────────────────────

  Widget _buildInProgress() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 20, 16, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('In Progress',
              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.bold,
                  )),
          const SizedBox(height: 8),
          for (final item in _inProgress)
            _InProgressCard(
              item: item,
              onTap: () => Navigator.pushNamed(context, '/item/${item.id}'),
            ),
        ],
      ),
    );
  }

  // ── Recent charts strip ────────────────────────────────────────────────────

  Widget _buildChartStrip() {
    final graph = _graph;
    if (graph == null) return const SizedBox.shrink();
    final byChart = GraphIntelligence.byChart(graph);
    final appState = context.read<GianttAppState>();

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 24, 0, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(right: 16),
            child: Text('Charts',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.bold,
                    )),
          ),
          const SizedBox(height: 10),
          SizedBox(
            height: 100,
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.only(right: 16),
              itemCount: _recentCharts.length,
              separatorBuilder: (_, __) => const SizedBox(width: 10),
              itemBuilder: (_, i) {
                final chart = _recentCharts[i];
                final items = byChart[chart] ?? [];
                return _ChartChip(
                  name: chart,
                  items: items,
                  onTap: () {
                    ChartRecencyService.instance.touch(chart);
                    appState.openChart(chart);
                  },
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

// ── Available Now card ─────────────────────────────────────────────────────

class _AvailableNowCard extends StatelessWidget {
  final ScoredItem scored;
  final VoidCallback onTap;

  const _AvailableNowCard({required this.scored, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final item = scored.item;
    final cs = Theme.of(context).colorScheme;

    final borderColor = scored.chainOverdue
        ? cs.error
        : scored.blockingCount > 5
            ? cs.tertiary
            : cs.outlineVariant;

    return GestureDetector(
      onTap: onTap,
      child: Container(
        margin: const EdgeInsets.only(bottom: 8),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        decoration: BoxDecoration(
          border: Border(left: BorderSide(color: borderColor, width: 3)),
          color: cs.surfaceContainerLowest,
          borderRadius: const BorderRadius.horizontal(right: Radius.circular(8)),
        ),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      if (scored.chainOverdue) ...[
                        Icon(Icons.warning_amber_rounded,
                            size: 14, color: cs.error),
                        const SizedBox(width: 4),
                      ],
                      if (item.occlude) ...[
                        Icon(Icons.visibility_off_outlined,
                            size: 12,
                            color: cs.onSurfaceVariant.withOpacity(0.5)),
                        const SizedBox(width: 4),
                      ],
                      Flexible(
                        child: Text(
                          item.title,
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                                fontWeight: FontWeight.w600,
                                color: item.occlude
                                    ? cs.onSurface.withOpacity(0.55)
                                    : null,
                              ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                  if (item.charts.isNotEmpty)
                    Text(
                      item.charts.join(', '),
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            color: cs.onSurfaceVariant,
                          ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                _PriorityBadge(priority: item.priority),
                if (scored.blockingCount > 0)
                  Padding(
                    padding: const EdgeInsets.only(top: 2),
                    child: Text(
                      '↑${scored.blockingCount}',
                      style: Theme.of(context).textTheme.labelSmall?.copyWith(
                            color: cs.onSurfaceVariant,
                          ),
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

// ── In Progress card ───────────────────────────────────────────────────────

class _InProgressCard extends StatelessWidget {
  final GianttItem item;
  final VoidCallback onTap;

  const _InProgressCard({required this.item, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return GestureDetector(
      onTap: onTap,
      child: Container(
        margin: const EdgeInsets.only(bottom: 6),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        decoration: BoxDecoration(
          border: Border(
              left: BorderSide(color: cs.primary.withOpacity(0.6), width: 3)),
          color: cs.surfaceContainerLowest,
          borderRadius: const BorderRadius.horizontal(right: Radius.circular(8)),
        ),
        child: Row(
          children: [
            Icon(Icons.timelapse_rounded, size: 16, color: cs.primary),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                item.title,
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: item.occlude
                          ? cs.onSurface.withOpacity(0.55)
                          : null,
                    ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
            _PriorityBadge(priority: item.priority),
          ],
        ),
      ),
    );
  }
}

// ── Chart chip with dot grid ────────────────────────────────────────────────

class _ChartChip extends StatelessWidget {
  final String name;
  final List<GianttItem> items;
  final VoidCallback onTap;

  const _ChartChip(
      {required this.name, required this.items, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final accent = _accentColor(name);
    return GestureDetector(
      onTap: onTap,
      child: Container(
        width: 88,
        padding: const EdgeInsets.all(8),
        decoration: BoxDecoration(
          color: accent.withOpacity(0.08),
          border: Border.all(color: accent.withOpacity(0.3)),
          borderRadius: BorderRadius.circular(12),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _DotGrid(items: items, accent: accent),
            const Spacer(),
            Text(
              name,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: accent,
                    fontWeight: FontWeight.bold,
                  ),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          ],
        ),
      ),
    );
  }

  static Color _accentColor(String name) {
    const palette = [
      Color(0xFF5C6BC0), // indigo
      Color(0xFF26A69A), // teal
      Color(0xFFEF5350), // red
      Color(0xFF7E57C2), // purple
      Color(0xFF29B6F6), // blue
      Color(0xFF66BB6A), // green
      Color(0xFFFFA726), // orange
      Color(0xFFEC407A), // pink
      Color(0xFF8D6E63), // brown
      Color(0xFF42A5F5), // blue-light
      Color(0xFFAB47BC), // purple-light
      Color(0xFF26C6DA), // cyan
    ];
    final hash = name.codeUnits.fold(0, (h, c) => h * 31 + c);
    return palette[hash.abs() % palette.length];
  }
}

class _DotGrid extends StatelessWidget {
  final List<GianttItem> items;
  final Color accent;

  const _DotGrid({required this.items, required this.accent});

  @override
  Widget build(BuildContext context) {
    const cols = 5;
    const rows = 3;
    const total = cols * rows;

    // Sample items if more than grid can show.
    final sampled = items.length <= total
        ? items
        : _sample(items, total);

    return SizedBox(
      height: 36,
      child: GridView.builder(
        physics: const NeverScrollableScrollPhysics(),
        gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
          crossAxisCount: cols,
          mainAxisSpacing: 3,
          crossAxisSpacing: 3,
        ),
        itemCount: math.min(sampled.length, total),
        itemBuilder: (_, i) => _dot(sampled[i]),
      ),
    );
  }

  Widget _dot(GianttItem item) {
    final color = _statusColor(item.status, item.occlude);
    return Container(
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(2),
      ),
    );
  }

  static Color _statusColor(GianttStatus status, bool occluded) {
    if (occluded) return const Color(0xFFBDBDBD).withOpacity(0.5);
    return switch (status) {
      GianttStatus.completed  => const Color(0xFF66BB6A),
      GianttStatus.inProgress => const Color(0xFF42A5F5),
      GianttStatus.blocked    => const Color(0xFFEF5350),
      GianttStatus.notStarted => const Color(0xFFBDBDBD),
    };
  }

  static List<GianttItem> _sample(List<GianttItem> items, int n) {
    // Stratified sample: preserve status distribution.
    final step = items.length / n;
    return List.generate(n, (i) => items[(i * step).round().clamp(0, items.length - 1)]);
  }
}

// ── Small shared widgets ───────────────────────────────────────────────────

class _PriorityBadge extends StatelessWidget {
  final GianttPriority priority;
  const _PriorityBadge({required this.priority});

  @override
  Widget build(BuildContext context) {
    if (priority == GianttPriority.neutral || priority.symbol.isEmpty) {
      return const SizedBox.shrink();
    }
    return Text(
      priority.symbol,
      style: TextStyle(
        fontSize: 11,
        color: _color(priority),
        fontWeight: FontWeight.bold,
      ),
    );
  }

  static Color _color(GianttPriority p) => switch (p) {
        GianttPriority.critical => const Color(0xFFEF5350),
        GianttPriority.high     => const Color(0xFFFFA726),
        GianttPriority.low      => const Color(0xFF78909C),
        GianttPriority.lowest   => const Color(0xFFB0BEC5),
        _                       => const Color(0xFF9E9E9E),
      };
}

/// Dim grey idle; briefly grows + glows green when [active] is true.
/// Driven by the home screen's journal-size watcher.
class _LiveBlipDot extends StatelessWidget {
  final bool active;
  const _LiveBlipDot({required this.active});

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    const liveColor = Color(0xFF66BB6A); // green
    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      width: active ? 11 : 8,
      height: active ? 11 : 8,
      decoration: BoxDecoration(
        color: active ? liveColor : cs.outlineVariant,
        shape: BoxShape.circle,
        boxShadow: active
            ? [
                BoxShadow(
                  color: liveColor.withOpacity(0.6),
                  blurRadius: 8,
                  spreadRadius: 1,
                ),
              ]
            : null,
      ),
    );
  }
}
