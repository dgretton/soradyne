import 'package:flutter/foundation.dart';
import '../core/models/inventory_entry.dart';

class AppState extends ChangeNotifier {
  List<InventoryEntry> _selectedItems = [];
  String _filterText = '';
  bool _isChatOpen = false;
  bool _needsInventoryRefresh = false;

  List<InventoryEntry> get selectedItems => List.unmodifiable(_selectedItems);
  String get filterText => _filterText;
  bool get isChatOpen => _isChatOpen;
  bool get needsInventoryRefresh => _needsInventoryRefresh;

  AppState();

  void selectItem(InventoryEntry item) {
    if (!_selectedItems.contains(item)) {
      _selectedItems.add(item);
      notifyListeners();
    }
  }

  void deselectItem(InventoryEntry item) {
    _selectedItems.remove(item);
    notifyListeners();
  }

  void clearSelection() {
    _selectedItems.clear();
    notifyListeners();
  }

  void toggleItemSelection(InventoryEntry item) {
    if (_selectedItems.contains(item)) {
      deselectItem(item);
    } else {
      selectItem(item);
    }
  }

  void setFilterText(String text) {
    _filterText = text;
    notifyListeners();
  }

  void openChat() {
    _isChatOpen = true;
    notifyListeners();
  }

  void closeChat() {
    _isChatOpen = false;
    notifyListeners();
  }

  void toggleChat() {
    _isChatOpen = !_isChatOpen;
    notifyListeners();
  }

  void triggerInventoryRefresh() {
    _needsInventoryRefresh = true;
    notifyListeners();
  }

  void consumeInventoryRefresh() {
    _needsInventoryRefresh = false;
    // No notification, this is consumed by a listener that is already rebuilding.
  }
}
