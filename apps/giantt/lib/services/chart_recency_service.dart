import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';

/// Tracks which charts the user has interacted with recently.
/// Persisted in SharedPreferences as a JSON list ordered by last interaction.
class ChartRecencyService {
  static const _prefsKey = 'chart_recency';
  static const _maxTracked = 20;

  static ChartRecencyService? _instance;
  static ChartRecencyService get instance {
    _instance ??= ChartRecencyService._();
    return _instance!;
  }
  ChartRecencyService._();

  List<String> _recent = [];
  bool _loaded = false;

  Future<void> _ensureLoaded() async {
    if (_loaded) return;
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_prefsKey);
    if (raw != null) {
      _recent = List<String>.from(jsonDecode(raw) as List);
    }
    _loaded = true;
  }

  /// Record that the user interacted with [chart].
  Future<void> touch(String chart) async {
    await _ensureLoaded();
    _recent.remove(chart);
    _recent.insert(0, chart);
    if (_recent.length > _maxTracked) _recent = _recent.sublist(0, _maxTracked);
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_prefsKey, jsonEncode(_recent));
  }

  /// Returns charts ordered by most recently interacted, filtered to those
  /// that actually exist in [allCharts]. Falls back to alphabetical for new
  /// charts that haven't been touched yet.
  Future<List<String>> ordered(Set<String> allCharts, {int limit = 8}) async {
    await _ensureLoaded();
    final seen = <String>{};
    final result = <String>[];

    // First: charts we've seen before, in recency order.
    for (final chart in _recent) {
      if (allCharts.contains(chart) && seen.add(chart)) {
        result.add(chart);
        if (result.length == limit) return result;
      }
    }

    // Fill remainder with unseen charts alphabetically.
    final unseen = allCharts.where((c) => !seen.contains(c)).toList()..sort();
    for (final chart in unseen) {
      if (seen.add(chart)) {
        result.add(chart);
        if (result.length == limit) break;
      }
    }

    return result;
  }
}
