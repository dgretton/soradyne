import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
import 'package:provider/provider.dart';
import '../models/app_state.dart';
import '../services/giantt_service.dart';
import '../widgets/item_card.dart';
import 'add_item_screen.dart';

class ItemsScreen extends StatefulWidget {
  const ItemsScreen({super.key});

  @override
  State<ItemsScreen> createState() => _ItemsScreenState();
}

class _ItemsScreenState extends State<ItemsScreen> {
  final GianttService _service = GianttService();
  final TextEditingController _searchController = TextEditingController();

  List<GianttItem> _items = [];
  bool _loading = true;
  String _searchTerm = '';
  bool _includeOccluded = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final appState = context.read<GianttAppState>();
    if (appState.needsGraphRefresh) {
      appState.consumeGraphRefresh();
      _load();
    }
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final items = await _service.searchItems(_searchTerm,
          includeOccluded: _includeOccluded);
      if (mounted) setState(() { _items = items; _loading = false; });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _onSearch(String term) {
    setState(() => _searchTerm = term);
    _load();
  }

  void _openAddItem() {
    Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => const AddItemScreen()),
    ).then((added) {
      if (added == true) _load();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Items'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            tooltip: 'Add item',
            onPressed: _openAddItem,
          ),
          IconButton(
            icon: Icon(_includeOccluded
                ? Icons.visibility
                : Icons.visibility_off_outlined),
            tooltip: _includeOccluded ? 'Hide occluded' : 'Show occluded',
            onPressed: () {
              setState(() => _includeOccluded = !_includeOccluded);
              _load();
            },
          ),
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: _load,
          ),
        ],
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 6),
            child: TextField(
              controller: _searchController,
              autofocus: false,
              decoration: InputDecoration(
                hintText: 'Search items…',
                prefixIcon: const Icon(Icons.search, size: 20),
                suffixIcon: _searchTerm.isNotEmpty
                    ? IconButton(
                        icon: const Icon(Icons.clear, size: 18),
                        onPressed: () {
                          _searchController.clear();
                          _onSearch('');
                        },
                      )
                    : null,
                isDense: true,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(24),
                  borderSide: BorderSide.none,
                ),
                filled: true,
                fillColor: Theme.of(context)
                    .colorScheme
                    .surfaceContainerHighest,
              ),
              onChanged: _onSearch,
            ),
          ),
          Expanded(
            child: _loading
                ? const Center(child: CircularProgressIndicator())
                : _items.isEmpty
                    ? Center(
                        child: Text(
                          _searchTerm.isEmpty
                              ? 'No items yet.'
                              : 'No items matching "$_searchTerm".',
                          style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                                color: Theme.of(context)
                                    .colorScheme
                                    .onSurfaceVariant,
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
                            onTap: () => Navigator.pushNamed(
                                context, '/item/${item.id}'),
                          );
                        },
                      ),
          ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }
}
