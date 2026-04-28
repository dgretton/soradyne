import '../graph/giantt_graph.dart';

/// Phase 1: include any item whose `charts` list intersects the requested
/// chart names. No window clipping, no done-collapse — those land in phase 2.
///
/// If [chartNames] is empty, returns all included (non-occluded) items.
Set<String> filterItemsForPage({
  required GianttGraph graph,
  required List<String> chartNames,
}) {
  final visible = <String>{};
  final wanted = chartNames.toSet();
  graph.includedItems.forEach((id, item) {
    if (wanted.isEmpty || item.charts.any(wanted.contains)) {
      visible.add(id);
    }
  });
  return visible;
}
