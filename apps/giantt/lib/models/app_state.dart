import 'package:flutter/foundation.dart';

class GianttAppState extends ChangeNotifier {
  bool _needsGraphRefresh = false;
  int _selectedTabIndex = 0;

  bool get needsGraphRefresh => _needsGraphRefresh;
  int get selectedTabIndex => _selectedTabIndex;

  void triggerGraphRefresh() {
    _needsGraphRefresh = true;
    notifyListeners();
  }

  void consumeGraphRefresh() {
    _needsGraphRefresh = false;
  }

  void selectTab(int index) {
    _selectedTabIndex = index;
    notifyListeners();
  }

  String? _pendingChart;
  String? get pendingChart => _pendingChart;

  /// Switch to the Charts tab and pre-select [chartName].
  void openChart(String chartName) {
    _pendingChart = chartName;
    _selectedTabIndex = 1; // Charts tab
    notifyListeners();
  }

  void consumePendingChart() {
    _pendingChart = null;
  }
}
