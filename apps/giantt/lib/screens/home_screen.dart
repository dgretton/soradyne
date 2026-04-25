import 'dart:io';
import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
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

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // Refresh when GianttAppState signals a graph change.
    // (GianttAppState.triggerGraphRefresh → HomeScreen.didChangeDependencies via Consumer above.)
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final graph = await _service.getGraph();
      final available = GraphIntelligence.availableNow(graph);
      final inProgress = GraphIntelligence.inProgress(graph);
      final allCharts = GraphIntelligence.allCharts(graph);
      final recentCharts = await ChartRecencyService.instance.ordered(allCharts);

      final soradyneDir = _service.soradyneDataDir;
      final localDeviceId = await _service.localDeviceId;
      List<DeviceActivity> syncActivity = [];
      if (soradyneDir != null && localDeviceId != null) {
        final flowUuid = _service.primaryFlowUuid;
        if (flowUuid != null) {
          final journalsDir = '$soradyneDir/flows/$flowUuid/journals';
          syncActivity = await SyncActivityService.recentActivity(
            journalsDir: journalsDir,
            localDeviceId: localDeviceId,
          );
        }
      }

      if (mounted) {
        setState(() {
          _graph = graph;
          _availableNow = available;
          _inProgress = inProgress;
          _syncActivity = syncActivity;
          _recentCharts = recentCharts;
          _loading = false;
        });
      }
    } catch (e) {
      if (mounted) setState(() => _loading = false);
    }
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
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : RefreshIndicator(
              onRefresh: _load,
              child: ListView(
                padding: const EdgeInsets.only(bottom: 32),
                children: [
                  if (_syncActivity.isNotEmpty) _buildSyncStrip(),
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
    final latest = _syncActivity.first;
    final count = _syncActivity.fold(0, (s, a) => s + a.ops.length);
    final deviceSummary = _syncActivity.length == 1
        ? latest.displayName
        : '${_syncActivity.length} devices';

    return GestureDetector(
      onTap: () => setState(() => _syncExpanded = !_syncExpanded),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 250),
        color: Theme.of(context).colorScheme.surfaceContainerHighest,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
              child: Row(
                children: [
                  _PulseDot(color: Theme.of(context).colorScheme.primary),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      '$deviceSummary · $count op${count == 1 ? '' : 's'} · '
                      '${SyncActivityService.timeAgo(latest.latestTimestamp)}',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ),
                  Icon(
                    _syncExpanded ? Icons.expand_less : Icons.expand_more,
                    size: 18,
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
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
              itemBuilder: (context, i) {
                final chart = _recentCharts[i];
                final items = byChart[chart] ?? [];
                return _ChartChip(
                  name: chart,
                  items: items,
                  onTap: () async {
                    await ChartRecencyService.instance.touch(chart);
                    if (mounted) {
                      // Navigate to chart view — for now show filtered home.
                      // TODO: dedicated chart screen.
                    }
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

class _PulseDot extends StatefulWidget {
  final Color color;
  const _PulseDot({required this.color});

  @override
  State<_PulseDot> createState() => _PulseDotState();
}

class _PulseDotState extends State<_PulseDot>
    with SingleTickerProviderStateMixin {
  late final AnimationController _ctrl;
  late final Animation<double> _anim;

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
        vsync: this, duration: const Duration(milliseconds: 1200))
      ..repeat(reverse: true);
    _anim = Tween(begin: 0.4, end: 1.0).animate(_ctrl);
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return FadeTransition(
      opacity: _anim,
      child: Container(
        width: 8,
        height: 8,
        decoration:
            BoxDecoration(color: widget.color, shape: BoxShape.circle),
      ),
    );
  }
}
