import 'dart:io';

import 'package:meta/meta.dart';
import 'package:yaml/yaml.dart';

import '../graph/giantt_graph.dart';

/// Sidecar entry: one printed page.
@immutable
class PageSpec {
  const PageSpec({
    required this.charts,
    this.from,
    this.to,
    this.window,
    this.title,
  });

  final List<String> charts;

  /// Inclusive left edge; `null` means "use the global default".
  final DateTime? from;

  /// Inclusive right edge; `null` means "use the global default" or compute
  /// from `from + window`.
  final DateTime? to;

  /// Window length in days (used when `to` isn't given).
  final int? window;

  final String? title;
}

/// Options for one full review render.
@immutable
class PdfReviewOptions {
  const PdfReviewOptions({
    required this.pages,
    required this.defaultFrom,
    required this.defaultWindowDays,
    required this.now,
  });

  /// Pages to render, in order.
  final List<PageSpec> pages;

  /// Default left edge of the time axis when a page doesn't specify its own.
  final DateTime defaultFrom;

  /// Default window length in days.
  final int defaultWindowDays;

  /// "Now" reference for the rendered "now" line and time inferences.
  final DateTime now;
}

/// Parse `~/.giantt/charts.yaml` (or whichever path) into PageSpec list.
///
/// Schema:
///   pages:
///     - charts: [thesis, papers]
///       from: 2026-03-01
///       window: 12w           # or:  to: 2026-06-01
///       title: "thesis & papers"   # optional
///     - charts: [home_renovation]
///       window: 6w
List<PageSpec>? loadChartsYaml(String path) {
  final f = File(path);
  if (!f.existsSync()) return null;
  final raw = f.readAsStringSync();
  final doc = loadYaml(raw);
  if (doc is! YamlMap) return null;
  final pages = doc['pages'];
  if (pages is! YamlList) return null;
  final result = <PageSpec>[];
  for (final entry in pages) {
    if (entry is! YamlMap) continue;
    final charts =
        (entry['charts'] as YamlList?)?.map((c) => c.toString()).toList() ??
            const <String>[];
    if (charts.isEmpty) continue;
    result.add(PageSpec(
      charts: charts,
      from: _parseDate(entry['from']),
      to: _parseDate(entry['to']),
      window: _parseDurationToDays(entry['window']),
      title: entry['title']?.toString(),
    ));
  }
  return result;
}

/// Resolve one page's actual `[from, to]` window using the default fallbacks.
({DateTime from, DateTime to}) resolvePageWindow({
  required PageSpec spec,
  required PdfReviewOptions options,
}) {
  final from = spec.from ?? options.defaultFrom;
  final to = spec.to ??
      from.add(
        Duration(days: spec.window ?? options.defaultWindowDays),
      );
  return (from: from, to: to);
}

/// Build the default page list when no CLI flags or sidecar config is given:
/// one page per chart, in alphabetical order, plus a final page for items
/// that aren't in any chart (if any exist).
List<PageSpec> defaultPagesFromGraph(GianttGraph graph) {
  final chartNames = <String>{};
  var hasOrphans = false;
  graph.includedItems.forEach((_, item) {
    if (item.charts.isEmpty) {
      hasOrphans = true;
    } else {
      chartNames.addAll(item.charts);
    }
  });
  final sorted = chartNames.toList()..sort();
  final pages =
      sorted.map((c) => PageSpec(charts: [c], title: c)).toList();
  if (hasOrphans) {
    pages.add(const PageSpec(charts: ['__none__'], title: '(no chart)'));
  }
  return pages;
}

DateTime? _parseDate(dynamic v) {
  if (v == null) return null;
  if (v is DateTime) return v;
  try {
    return DateTime.parse(v.toString());
  } catch (_) {
    return null;
  }
}

/// Parse `8w`, `60d`, `2mo` into days. Returns null if unparseable.
int? _parseDurationToDays(dynamic v) {
  if (v == null) return null;
  if (v is int) return v;
  final s = v.toString().trim();
  final m = RegExp(r'^(\d+)\s*([dwmy]|mo)?$', caseSensitive: false).firstMatch(s);
  if (m == null) return null;
  final n = int.parse(m.group(1)!);
  final unit = (m.group(2) ?? 'd').toLowerCase();
  switch (unit) {
    case 'd':
      return n;
    case 'w':
      return n * 7;
    case 'mo':
      return n * 30;
    case 'm':
      return n * 30;
    case 'y':
      return n * 365;
    default:
      return n;
  }
}
