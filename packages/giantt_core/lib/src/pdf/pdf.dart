import 'dart:typed_data';

import '../graph/giantt_graph.dart';
import 'config.dart';
import 'layout.dart';
import 'render.dart';

export 'config.dart';

/// Top-level entry point: render a full review PDF from a graph plus options.
Future<Uint8List> renderReview({
  required GianttGraph graph,
  required PdfReviewOptions options,
}) async {
  final pages = <PageLayout>[];
  for (final spec in options.pages) {
    final window = resolvePageWindow(spec: spec, options: options);
    final config = PageConfig(
      charts: spec.charts.where((c) => c != '__none__').toList(),
      from: window.from,
      to: window.to,
      title: spec.title,
    );
    pages.add(buildPageLayout(
      config: config,
      graph: graph,
      now: options.now,
    ));
  }
  return renderPdf(pages);
}
