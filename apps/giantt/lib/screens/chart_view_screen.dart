import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../models/app_state.dart';
import '../services/giantt_service.dart';
import '../layout/gantt_layout.dart';
import '../widgets/gantt_chart.dart';

class ChartViewScreen extends StatefulWidget {
  const ChartViewScreen({super.key});

  @override
  State<ChartViewScreen> createState() => _ChartViewScreenState();
}

class _ChartViewScreenState extends State<ChartViewScreen> {
  final GianttService _gianttService = GianttService();

  List<String> _charts = [];
  String? _selectedChart;
  GanttLayout _layout = const GanttLayout(
    rows: [],
    dependencies: [],
    totalSpanSeconds: 0,
  );
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();
    _loadCharts();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final appState = context.read<GianttAppState>();
    final pending = appState.pendingChart;
    if (pending != null && _charts.contains(pending)) {
      appState.consumePendingChart();
      setState(() => _selectedChart = pending);
      _computeLayout();
    } else if (pending != null && _charts.isEmpty) {
      // Charts not loaded yet — will be applied once _loadCharts finishes.
      appState.consumePendingChart();
      _pendingChartFromNav = pending;
    }
  }

  String? _pendingChartFromNav;

  Future<void> _loadCharts() async {
    setState(() => _isLoading = true);
    try {
      final charts = await _gianttService.getAllCharts();
      setState(() {
        _charts = charts;
        _isLoading = false;
        if (_pendingChartFromNav != null && charts.contains(_pendingChartFromNav)) {
          _selectedChart = _pendingChartFromNav;
          _pendingChartFromNav = null;
        } else if (charts.isNotEmpty && _selectedChart == null) {
          _selectedChart = charts.first;
        }
      });
      await _computeLayout();
    } catch (e) {
      setState(() => _isLoading = false);
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('Error loading charts: $e')));
      }
    }
  }

  Future<void> _computeLayout() async {
    final chart = _selectedChart;
    if (chart == null) return;
    try {
      final items = await _gianttService.getItemsByChart(chart);
      setState(() {
        _layout = GanttLayout.compute(items);
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('Error computing layout: $e')));
      }
    }
  }

  Future<void> _computeLayoutAllCharts() async {
    try {
      final graph = await _gianttService.getGraph();
      final items = graph.includedItems.values.toList();
      setState(() {
        _layout = GanttLayout.compute(items);
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('Error computing layout: $e')));
      }
    }
  }

  Future<void> _reload() async {
    await _gianttService.refresh();
    await _loadCharts();
  }

  @override
  Widget build(BuildContext context) {
    return OrientationBuilder(
      builder: (context, orientation) {
        final isLandscape = orientation == Orientation.landscape;
        return Scaffold(
          // In landscape, drop the AppBar entirely so the chart fills the screen.
          appBar: isLandscape
              ? null
              : AppBar(
                  title: Text(_selectedChart ?? 'All Charts'),
                  actions: [
                    IconButton(
                      icon: const Icon(Icons.refresh),
                      tooltip: 'Reload',
                      onPressed: _reload,
                    ),
                  ],
                ),
          body: isLandscape
              ? SafeArea(
                  bottom: false,
                  child: _isLoading
                      ? const Center(child: CircularProgressIndicator())
                      : _buildContent(isLandscape),
                )
              : (_isLoading
                  ? const Center(child: CircularProgressIndicator())
                  : _buildContent(isLandscape)),
        );
      },
    );
  }

  Widget _buildContent(bool isLandscape) {
    if (_charts.isEmpty) return _buildEmptyState();

    return Column(
      children: [
        _buildChartChips(isLandscape),
        const Divider(height: 1, thickness: 1),
        Expanded(
          child: GanttChart(
            key: ValueKey(_selectedChart),
            layout: _layout,
            compact: isLandscape,
          ),
        ),
      ],
    );
  }

  Widget _buildChartChips(bool isLandscape) {
    // In landscape: very compact strip (32 dp) with the refresh icon folded in.
    final rowHeight = isLandscape ? 40.0 : 52.0;
    final vertPad = isLandscape ? 2.0 : 8.0;
    final horizPad = isLandscape ? 6.0 : 12.0;
    final chipPadding = isLandscape
        ? const EdgeInsets.symmetric(horizontal: 6, vertical: 0)
        : const EdgeInsets.symmetric(horizontal: 8, vertical: 4);

    Widget chip(String label, bool selected, VoidCallback onTap) {
      return Padding(
        padding: EdgeInsets.only(right: isLandscape ? 4 : 8),
        child: FilterChip(
          label: Text(label, style: TextStyle(fontSize: isLandscape ? 11 : 14)),
          labelPadding: chipPadding,
          padding: isLandscape ? EdgeInsets.zero : null,
          materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
          selected: selected,
          onSelected: (_) => onTap(),
          visualDensity: isLandscape ? VisualDensity.compact : null,
        ),
      );
    }

    return SizedBox(
      height: rowHeight,
      child: Row(
        children: [
          Expanded(
            child: ListView(
              scrollDirection: Axis.horizontal,
              padding: EdgeInsets.symmetric(
                horizontal: horizPad,
                vertical: vertPad,
              ),
              children: [
                chip('All', _selectedChart == null, () {
                  setState(() => _selectedChart = null);
                  _computeLayoutAllCharts();
                }),
                ..._charts.map((chart) => chip(
                      chart,
                      chart == _selectedChart,
                      () {
                        setState(() => _selectedChart = chart);
                        _computeLayout();
                      },
                    )),
              ],
            ),
          ),
          // In landscape the AppBar is gone, so tuck refresh here.
          if (isLandscape)
            IconButton(
              icon: const Icon(Icons.refresh, size: 18),
              padding: const EdgeInsets.symmetric(horizontal: 8),
              constraints: const BoxConstraints(),
              tooltip: 'Reload',
              onPressed: _reload,
            ),
        ],
      ),
    );
  }

  Widget _buildEmptyState() {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.timeline, size: 64, color: Colors.grey[400]),
          const SizedBox(height: 16),
          Text(
            'No charts yet',
            style: Theme.of(context).textTheme.titleMedium?.copyWith(
              color: Colors.grey[600],
            ),
          ),
          const SizedBox(height: 8),
          Text(
            'Add items with charts to see them here',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Colors.grey[500],
            ),
          ),
        ],
      ),
    );
  }
}
