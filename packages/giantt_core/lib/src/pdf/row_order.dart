import '../graph/giantt_graph.dart';

/// DFS ordering that pulls single-predecessor chains adjacent.
///
/// Algorithm: traverse from items with no visible predecessors. For each
/// item, emit its (so-far unvisited) predecessors first, then itself, then
/// recurse into its successors immediately if all of *their* predecessors
/// are also already emitted. This yields:
///
///   * In a linear chain A -> B -> C -> D, rows print A,B,C,D adjacently.
///   * When B has multiple predecessors {A1, A2}, the chain reaching B first
///     gets adjacency; the other lands wherever DFS reaches it (relaxed —
///     the user is OK with this for multi-pred cases).
///   * When A has multiple successors {B1, B2}, B1 lands adjacent to A; B2
///     follows after B1's full subtree is emitted.
///
/// Cycles or unreachable items are appended at the end in input order.
List<String> orderRows({
  required List<String> ids,
  required GianttGraph graph,
}) {
  if (ids.isEmpty) return ids;
  final visible = ids.toSet();

  // Build predecessor / successor lists restricted to visible items.
  final preds = <String, List<String>>{};
  final succs = <String, List<String>>{};
  for (final id in ids) {
    preds[id] = const [];
    succs[id] = const [];
  }
  for (final id in ids) {
    final item = graph.items[id];
    if (item == null) continue;
    final p = (item.relations['REQUIRES'] ?? const <String>[])
        .where(visible.contains)
        .toList();
    preds[id] = p;
    for (final pred in p) {
      succs[pred] = [...(succs[pred] ?? const []), id];
    }
  }

  final visited = <String>{};
  final order = <String>[];

  void visit(String id) {
    if (visited.contains(id)) return;
    visited.add(id);
    // Make sure every predecessor is emitted first (defensive — should be
    // true already when called from a source, but cycles or weird entry
    // orders make this safer).
    for (final p in preds[id] ?? const []) {
      if (!visited.contains(p)) visit(p);
    }
    order.add(id);
    // Recurse into successors whose other predecessors are also done, so
    // chains stay adjacent.
    for (final s in succs[id] ?? const []) {
      if (visited.contains(s)) continue;
      final sPreds = preds[s] ?? const [];
      if (sPreds.every(visited.contains)) {
        visit(s);
      }
    }
  }

  // Sources first (items with no visible predecessors), in input order.
  for (final id in ids) {
    if ((preds[id] ?? const []).isEmpty) visit(id);
  }
  // Anything left (multi-predecessor items not yet reached, cycles).
  for (final id in ids) {
    visit(id);
  }

  return order;
}
