import 'package:flutter/foundation.dart';

class GianttAppState extends ChangeNotifier {
  bool _needsGraphRefresh = false;

  bool get needsGraphRefresh => _needsGraphRefresh;

  void triggerGraphRefresh() {
    _needsGraphRefresh = true;
    notifyListeners();
  }

  void consumeGraphRefresh() {
    _needsGraphRefresh = false;
  }
}
