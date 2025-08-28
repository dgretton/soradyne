import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
import '../services/giantt_service.dart';
import '../widgets/item_card.dart';
import '../widgets/stats_card.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  final GianttService _gianttService = GianttService();
  final TextEditingController _searchController = TextEditingController();
  
  List<GianttItem> _items = [];
  Map<String, dynamic> _stats = {};
  bool _isLoading = true;
  String _searchTerm = '';

  @override
  void initState() {
    super.initState();
    _loadData();
  }

  Future<void> _loadData() async {
    setState(() {
      _isLoading = true;
    });

    try {
      final items = await _gianttService.searchItems(_searchTerm);
      final stats = await _gianttService.getWorkspaceStats();
      
      setState(() {
        _items = items;
        _stats = stats;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _isLoading = false;
      });
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error loading data: $e')),
        );
      }
    }
  }

  Future<void> _onSearch(String searchTerm) async {
    setState(() {
      _searchTerm = searchTerm;
    });
    await _loadData();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Giantt'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () async {
              await _gianttService.refresh();
              await _loadData();
            },
          ),
        ],
      ),
      body: Column(
        children: [
          // Search bar
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: TextField(
              controller: _searchController,
              decoration: const InputDecoration(
                hintText: 'Search items...',
                prefixIcon: Icon(Icons.search),
                border: OutlineInputBorder(),
              ),
              onChanged: _onSearch,
            ),
          ),
          
          // Stats cards
          if (_stats.isNotEmpty) ...[
            SizedBox(
              height: 120,
              child: ListView(
                scrollDirection: Axis.horizontal,
                padding: const EdgeInsets.symmetric(horizontal: 16.0),
                children: [
                  StatsCard(
                    title: 'Total Items',
                    value: _stats['total_items']?.toString() ?? '0',
                    icon: Icons.list,
                  ),
                  StatsCard(
                    title: 'Active Items',
                    value: _stats['included_items']?.toString() ?? '0',
                    icon: Icons.check_circle_outline,
                  ),
                  StatsCard(
                    title: 'Charts',
                    value: (_stats['charts'] as List?)?.length.toString() ?? '0',
                    icon: Icons.timeline,
                  ),
                  StatsCard(
                    title: 'Tags',
                    value: (_stats['tags'] as List?)?.length.toString() ?? '0',
                    icon: Icons.label,
                  ),
                ],
              ),
            ),
            const SizedBox(height: 16),
          ],
          
          // Items list
          Expanded(
            child: _isLoading
                ? const Center(child: CircularProgressIndicator())
                : _items.isEmpty
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
                              _searchTerm.isEmpty 
                                  ? 'No items yet'
                                  : 'No items found for "$_searchTerm"',
                              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                                color: Colors.grey[600],
                              ),
                            ),
                            const SizedBox(height: 8),
                            Text(
                              _searchTerm.isEmpty
                                  ? 'Add your first item to get started'
                                  : 'Try a different search term',
                              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                                color: Colors.grey[500],
                              ),
                            ),
                          ],
                        ),
                      )
                    : ListView.builder(
                        padding: const EdgeInsets.symmetric(horizontal: 16.0),
                        itemCount: _items.length,
                        itemBuilder: (context, index) {
                          final item = _items[index];
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
      floatingActionButton: FloatingActionButton(
        onPressed: () async {
          final result = await Navigator.pushNamed(context, '/add-item');
          if (result == true) {
            await _loadData();
          }
        },
        child: const Icon(Icons.add),
      ),
    );
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }
}
