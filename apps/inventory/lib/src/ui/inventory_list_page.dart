import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../core/inventory_api.dart';
import '../core/models/inventory_entry.dart';
import '../models/app_state.dart';
import '../ui/widgets/filter_bar.dart';
import '../ui/widgets/chat_fab.dart';
import '../ui/settings_page.dart';
import 'history_page.dart';
import 'widgets/export_dialog.dart';

class InventoryListPage extends StatefulWidget {
  const InventoryListPage({super.key});

  @override
  State<InventoryListPage> createState() => _InventoryListPageState();
}

class _InventoryListPageState extends State<InventoryListPage> {
  late Future<List<InventoryEntry>> _inventoryFuture;

  @override
  void initState() {
    super.initState();
    _inventoryFuture = _loadInventory();
  }

  Future<List<InventoryEntry>> _loadInventory() async {
    // The API is now provided, so we just need to call it.
    return Provider.of<InventoryApi>(context, listen: false).search('');
  }

  void _reloadInventory() {
    setState(() {
      _inventoryFuture = _loadInventory();
    });
  }

  List<InventoryEntry> _filterItems(List<InventoryEntry> items, String filterText) {
    if (filterText.isEmpty) return items;

    final searchTerm = filterText.toLowerCase();
    return items.where((item) {
      return item.description.toLowerCase().contains(searchTerm) ||
          item.location.toLowerCase().contains(searchTerm) ||
          item.category.toLowerCase().contains(searchTerm) ||
          item.tags.any((tag) => tag.toLowerCase().contains(searchTerm));
    }).toList();
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, child) {
        if (appState.needsInventoryRefresh) {
          appState.consumeInventoryRefresh();
          WidgetsBinding.instance.addPostFrameCallback((_) {
            _reloadInventory();
          });
        }
        return Scaffold(
          appBar: AppBar(
            title: const Text('Inventory'),
            actions: [
              if (appState.selectedItems.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(right: 8.0),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        '${appState.selectedItems.length} selected',
                        style: const TextStyle(fontSize: 14),
                      ),
                      const SizedBox(width: 4),
                      GestureDetector(
                        onTap: () => appState.clearSelection(),
                        child: Container(
                          padding: const EdgeInsets.all(2),
                          decoration: BoxDecoration(
                            color: Theme.of(context).colorScheme.onPrimary.withValues(alpha: 0.2),
                            borderRadius: BorderRadius.circular(10),
                          ),
                          child: Icon(
                            Icons.close,
                            size: 14,
                            color: Theme.of(context).colorScheme.onPrimary,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              IconButton(
                icon: const Icon(Icons.refresh),
                onPressed: _reloadInventory,
              ),
              PopupMenuButton<String>(
                icon: const Icon(Icons.more_vert),
                onSelected: (value) {
                  switch (value) {
                    case 'export':
                      final api = Provider.of<InventoryApi>(context, listen: false);
                      ExportDialog.show(context, api);
                      break;
                    case 'history':
                      Navigator.of(context).push(
                        MaterialPageRoute(
                          builder: (context) => const HistoryPage(),
                        ),
                      );
                      break;
                    case 'settings':
                      Navigator.of(context).push(
                        MaterialPageRoute(
                          builder: (context) => const SettingsPage(),
                        ),
                      );
                      break;
                  }
                },
                itemBuilder: (context) => [
                  const PopupMenuItem(
                    value: 'export',
                    child: Row(
                      children: [
                        Icon(Icons.ios_share),
                        SizedBox(width: 8),
                        Text('Export'),
                      ],
                    ),
                  ),
                  const PopupMenuDivider(),
                  const PopupMenuItem(
                    value: 'history',
                    child: Row(
                      children: [
                        Icon(Icons.history),
                        SizedBox(width: 8),
                        Text('History'),
                      ],
                    ),
                  ),
                  const PopupMenuItem(
                    value: 'settings',
                    child: Row(
                      children: [
                        Icon(Icons.settings),
                        SizedBox(width: 8),
                        Text('Settings'),
                      ],
                    ),
                  ),
                ],
              ),
            ],
          ),
          body: Column(
            children: [
              FilterBar(
                onFilterChanged: (text) {
                  appState.setFilterText(text);
                },
              ),
              Expanded(
                child: FutureBuilder<List<InventoryEntry>>(
                  future: _inventoryFuture,
                  builder: (context, snapshot) {
                    if (snapshot.connectionState == ConnectionState.waiting) {
                      return const Center(child: CircularProgressIndicator());
                    } else if (snapshot.hasError) {
                      return Center(child: Text('Error: ${snapshot.error}'));
                    } else if (!snapshot.hasData || snapshot.data!.isEmpty) {
                      return const Center(child: Text('No inventory items found.'));
                    }

                    final filteredItems = _filterItems(snapshot.data!, appState.filterText);

                    if (filteredItems.isEmpty && appState.filterText.isNotEmpty) {
                      return const Center(child: Text('No items match the current filter.'));
                    }

                    return ListView.builder(
                      itemCount: filteredItems.length,
                      itemBuilder: (context, index) {
                        final item = filteredItems[index];
                        final isSelected = appState.selectedItems.contains(item);

                        return Card(
                          margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                          child: ListTile(
                            leading: Checkbox(
                              value: isSelected,
                              onChanged: (bool? value) {
                                appState.toggleItemSelection(item);
                              },
                            ),
                            title: Text(
                              item.description,
                              style: TextStyle(
                                fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
                              ),
                            ),
                            subtitle: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(item.location),
                                if (item.tags.isNotEmpty)
                                  Padding(
                                    padding: const EdgeInsets.only(top: 2),
                                    child: Wrap(
                                      spacing: 2,
                                      runSpacing: 1,
                                      children: item.tags.take(3).map((tag) =>
                                        Container(
                                          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                                          decoration: BoxDecoration(
                                            color: Theme.of(context).colorScheme.surfaceContainerHighest.withValues(alpha: 0.3),
                                            borderRadius: BorderRadius.circular(8),
                                          ),
                                          child: Text(
                                            tag,
                                            style: TextStyle(
                                              fontSize: 8,
                                              color: Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.6),
                                            ),
                                          ),
                                        )
                                      ).toList(),
                                    ),
                                  ),
                              ],
                            ),
                            onTap: () {
                              appState.toggleItemSelection(item);
                            },
                            selected: isSelected,
                          ),
                        );
                      },
                    );
                  },
                ),
              ),
            ],
          ),
          floatingActionButton: const ChatFab(),
        );
      },
    );
  }
}
