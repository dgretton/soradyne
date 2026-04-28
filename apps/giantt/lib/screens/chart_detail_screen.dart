import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
import '../services/giantt_service.dart';
import '../services/chart_recency_service.dart';
import '../widgets/item_card.dart';

/// Shows all items belonging to a single chart, sorted by priority then status.
class ChartDetailScreen extends StatefulWidget {
  final String chartName;

  const ChartDetailScreen({super.key, required this.chartName});

  @override
  State<ChartDetailScreen> createState() => _ChartDetailScreenState();
}

class _ChartDetailScreenState extends State<ChartDetailScreen> {
  final GianttService _service = GianttService();
  List<GianttItem> _items = [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    ChartRecencyService.instance.touch(widget.chartName);
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final items = await _service.getItemsByChart(widget.chartName,
          includeOccluded: true);
      items.sort((a, b) {
        // Sort: not-completed first, then by priority desc.
        final aDone = a.status == GianttStatus.completed;
        final bDone = b.status == GianttStatus.completed;
        if (aDone != bDone) return aDone ? 1 : -1;
        return b.priority.index.compareTo(a.priority.index);
      });
      if (mounted) setState(() { _items = items; _loading = false; });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.chartName),
        actions: [
          IconButton(icon: const Icon(Icons.refresh), onPressed: _load),
        ],
      ),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : _items.isEmpty
              ? Center(
                  child: Text(
                    'No items in "${widget.chartName}".',
                    style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: Theme.of(context).colorScheme.onSurfaceVariant,
                        ),
                  ),
                )
              : ListView.builder(
                  padding: const EdgeInsets.only(bottom: 24),
                  itemCount: _items.length,
                  itemBuilder: (context, i) {
                    final item = _items[i];
                    return ItemCard(
                      item: item,
                      onTap: () =>
                          Navigator.pushNamed(context, '/item/${item.id}'),
                    );
                  },
                ),
    );
  }
}
