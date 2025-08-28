import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
import '../services/giantt_service.dart';
import '../widgets/item_card.dart';

class ChartViewScreen extends StatefulWidget {
  const ChartViewScreen({super.key});

  @override
  State<ChartViewScreen> createState() => _ChartViewScreenState();
}

class _ChartViewScreenState extends State<ChartViewScreen> {
  final GianttService _gianttService = GianttService();
  
  List<String> _charts = [];
  String? _selectedChart;
  List<GianttItem> _chartItems = [];
  bool _isLoading = true;

  @override
  void initState() {
    super.initState();
    _loadCharts();
  }

  Future<void> _loadCharts() async {
    setState(() {
      _isLoading = true;
    });

    try {
      final charts = await _gianttService.getAllCharts();
      setState(() {
        _charts = charts;
        _isLoading = false;
        if (charts.isNotEmpty && _selectedChart == null) {
          _selectedChart = charts.first;
          _loadChartItems();
        }
      });
    } catch (e) {
      setState(() {
        _isLoading = false;
      });
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error loading charts: $e')),
        );
      }
    }
  }

  Future<void> _loadChartItems() async {
    if (_selectedChart == null) return;

    try {
      final items = await _gianttService.getItemsByChart(_selectedChart!);
      setState(() {
        _chartItems = items;
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error loading chart items: $e')),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Charts'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () async {
              await _gianttService.refresh();
              await _loadCharts();
            },
          ),
        ],
      ),
      body: _isLoading
          ? const Center(child: CircularProgressIndicator())
          : _charts.isEmpty
              ? Center(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Icon(
                        Icons.timeline,
                        size: 64,
                        color: Colors.grey[400],
                      ),
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
                )
              : Column(
                  children: [
                    // Chart selector
                    Container(
                      height: 60,
                      padding: const EdgeInsets.symmetric(horizontal: 16.0),
                      child: ListView.builder(
                        scrollDirection: Axis.horizontal,
                        itemCount: _charts.length,
                        itemBuilder: (context, index) {
                          final chart = _charts[index];
                          final isSelected = chart == _selectedChart;
                          
                          return Padding(
                            padding: const EdgeInsets.only(right: 8.0),
                            child: FilterChip(
                              label: Text(chart),
                              selected: isSelected,
                              onSelected: (selected) {
                                if (selected) {
                                  setState(() {
                                    _selectedChart = chart;
                                  });
                                  _loadChartItems();
                                }
                              },
                            ),
                          );
                        },
                      ),
                    ),
                    
                    const Divider(),
                    
                    // Chart items
                    Expanded(
                      child: _chartItems.isEmpty
                          ? Center(
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                children: [
                                  Icon(
                                    Icons.inbox,
                                    size: 64,
                                    color: Colors.grey[400],
                                  ),
                                  const SizedBox(height: 16),
                                  Text(
                                    'No items in "$_selectedChart"',
                                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                                      color: Colors.grey[600],
                                    ),
                                  ),
                                ],
                              ),
                            )
                          : ListView.builder(
                              padding: const EdgeInsets.symmetric(horizontal: 16.0),
                              itemCount: _chartItems.length,
                              itemBuilder: (context, index) {
                                final item = _chartItems[index];
                                return ItemCard(
                                  item: item,
                                  onTap: () {
                                    Navigator.pushNamed(context, '/item/${item.id}');
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
