import 'dart:math' as math;
import 'dart:typed_data';

import 'package:pdf/pdf.dart';
import 'package:pdf/widgets.dart' as pw;

import '../models/status.dart';
import 'layout.dart';

/// Tunables for the PDF render. Phase 1 is intentionally dense; numbers will
/// shift as we iterate.
class RenderConfig {
  const RenderConfig({
    this.rowHeight = 11.0,
    this.fontSize = 6.5,
    this.titleFontSize = 12.0,
    this.headerHeight = 22.0,
    this.timeAxisHeight = 16.0,
    this.labelColumnWidth = 130.0,
    this.barInset = 2.0,
    this.marginPts = 18.0,
  });
  final double rowHeight;
  final double fontSize;
  final double titleFontSize;
  final double headerHeight;
  final double timeAxisHeight;
  final double labelColumnWidth;

  /// Vertical inset of the bar within its row.
  final double barInset;

  /// Page margin in points.
  final double marginPts;
}

/// Render a list of pages into a single multi-page PDF byte buffer.
Future<Uint8List> renderPdf(
  List<PageLayout> pages, {
  RenderConfig config = const RenderConfig(),
}) async {
  final doc = pw.Document();
  for (final page in pages) {
    doc.addPage(_buildPage(page, config));
  }
  return doc.save();
}

pw.Page _buildPage(PageLayout page, RenderConfig config) {
  return pw.Page(
    pageFormat: PdfPageFormat.letter.landscape,
    margin: pw.EdgeInsets.all(config.marginPts),
    build: (context) {
      return pw.LayoutBuilder(
        builder: (ctx, constraints) {
          final width = constraints!.maxWidth;
          final height = constraints.maxHeight;
          return pw.SizedBox(
            width: width,
            height: height,
            child: pw.CustomPaint(
              size: PdfPoint(width, height),
              painter: (canvas, size) =>
                  _paintPage(canvas, size, page, config),
            ),
          );
        },
      );
    },
  );
}

void _paintPage(
  PdfGraphics canvas,
  PdfPoint size,
  PageLayout page,
  RenderConfig cfg,
) {
  // PDF coordinate system has y increasing upward. We work in a "screen-like"
  // top-down coord space and flip at draw time via [y].
  double y(double topDown) => size.y - topDown;

  final width = size.x;
  final height = size.y;

  final font = PdfFont.helvetica(canvas.defaultFont!.pdfDocument);
  final boldFont = PdfFont.helveticaBold(canvas.defaultFont!.pdfDocument);

  // ---- Header ----
  final title = page.config.derivedTitle;
  canvas.setColor(PdfColors.black);
  canvas.drawString(
    boldFont,
    cfg.titleFontSize,
    _ascii(title),
    0,
    y(cfg.titleFontSize) - 2,
  );

  final dateRange =
      '${_fmtDate(page.config.from)}  ->  ${_fmtDate(page.config.to)}    rendered ${_fmtDate(page.now)}';
  canvas.setColor(PdfColors.grey700);
  canvas.drawString(
    font,
    cfg.fontSize,
    _ascii(dateRange),
    0,
    y(cfg.titleFontSize + cfg.fontSize + 2) - 2,
  );

  // ---- Geometry ----
  final chartTop = cfg.headerHeight;
  final chartLeft = cfg.labelColumnWidth;
  final chartRight = width;
  final chartWidth = chartRight - chartLeft;

  final timeAxisTop = chartTop;
  final rowsTop = chartTop + cfg.timeAxisHeight;
  final chartBottom = height;
  final usableRows = ((chartBottom - rowsTop) / cfg.rowHeight).floor();

  // X mapping from DateTime to chart pixel.
  final fromMs = page.config.from.millisecondsSinceEpoch.toDouble();
  final toMs = page.config.to.millisecondsSinceEpoch.toDouble();
  final spanMs = math.max(1.0, toMs - fromMs);
  double xForDate(DateTime d) {
    final t = (d.millisecondsSinceEpoch.toDouble() - fromMs) / spanMs;
    return chartLeft + t.clamp(0.0, 1.0) * chartWidth;
  }

  bool inWindow(DateTime d) =>
      !d.isBefore(page.config.from) && !d.isAfter(page.config.to);

  // ---- Time axis: vertical gridlines per week ----
  canvas.setLineWidth(0.25);
  canvas.setStrokeColor(PdfColors.grey400);
  final firstMonday = _nextWeekStart(page.config.from);
  for (var d = firstMonday;
      !d.isAfter(page.config.to);
      d = d.add(const Duration(days: 7))) {
    final x = xForDate(d);
    canvas.drawLine(x, y(rowsTop), x, y(chartBottom));
    canvas.strokePath();
  }

  // Month labels along the time axis.
  canvas.setColor(PdfColors.grey800);
  var monthCursor = DateTime(page.config.from.year, page.config.from.month, 1);
  while (!monthCursor.isAfter(page.config.to)) {
    final x = xForDate(monthCursor);
    if (x > chartLeft) {
      canvas.drawLine(x, y(timeAxisTop), x, y(rowsTop));
      canvas.strokePath();
    }
    canvas.drawString(
      font,
      cfg.fontSize,
      _ascii(_fmtMonth(monthCursor)),
      math.max(chartLeft + 1, x + 2),
      y(timeAxisTop + cfg.fontSize) - 1,
    );
    monthCursor = DateTime(monthCursor.year, monthCursor.month + 1, 1);
  }

  // ---- "Now" line ----
  if (inWindow(page.now)) {
    final nowX = xForDate(page.now);
    canvas.setStrokeColor(PdfColors.red300);
    canvas.setLineWidth(0.7);
    canvas.drawLine(nowX, y(rowsTop), nowX, y(chartBottom));
    canvas.strokePath();
  }

  // ---- Map rows -> y coords for edge drawing ----
  final rowCenterY = <String, double>{};
  final rowBarRect = <String, _Rect>{};

  // ---- Items ----
  final visibleItems = page.items.take(usableRows).toList();
  for (final laid in visibleItems) {
    final rowTop = rowsTop + laid.row * cfg.rowHeight;
    final rowMid = rowTop + cfg.rowHeight / 2;

    // Label column: id and title, truncated to fit.
    final label = _ascii('${laid.item.status.symbol} ${laid.item.id}  ${laid.item.title}');
    final truncated = _truncateToWidth(
        font, cfg.fontSize, label, cfg.labelColumnWidth - 4);
    canvas.setColor(_statusColor(laid.item.status));
    canvas.drawString(
      font,
      cfg.fontSize,
      truncated,
      0,
      y(rowTop + cfg.rowHeight - cfg.barInset - 1),
    );

    // Bar geometry — clipped to chart area.
    final rawStartX = xForDate(laid.start);
    final rawEndX = xForDate(laid.end);
    final startX = rawStartX.clamp(chartLeft, chartRight);
    final endX = rawEndX.clamp(chartLeft, chartRight);
    final barTop = rowTop + cfg.barInset;
    final barBottom = rowTop + cfg.rowHeight - cfg.barInset;

    if (endX > startX) {
      canvas.setColor(_barFillColor(laid));
      canvas.drawRect(
        startX,
        y(barBottom),
        endX - startX,
        barBottom - barTop,
      );
      canvas.fillPath();
    }

    rowCenterY[laid.item.id] = rowMid;
    rowBarRect[laid.item.id] = _Rect(startX, barTop, endX, barBottom);

    // Off-window arrows.
    if (rawStartX < chartLeft) {
      _drawLeftArrow(canvas, y, chartLeft, barTop, barBottom);
    }
    if (rawEndX > chartRight) {
      _drawRightArrow(canvas, y, chartRight, barTop, barBottom);
    }
  }

  // ---- Edges (straight verticals connecting predecessor right edge -> successor left edge) ----
  canvas.setStrokeColor(PdfColors.blue700);
  canvas.setLineWidth(0.4);
  for (final edge in page.edges) {
    final from = rowBarRect[edge.fromId];
    final to = rowBarRect[edge.toId];
    final fromYC = rowCenterY[edge.fromId];
    final toYC = rowCenterY[edge.toId];
    if (from == null || to == null || fromYC == null || toYC == null) continue;

    final fromX = from.right;
    final toX = to.left;
    // Right-elbow path: out, up/down, in.
    final midX = math.max(fromX + 4, toX);
    canvas.drawLine(fromX, y(fromYC), midX, y(fromYC));
    canvas.drawLine(midX, y(fromYC), midX, y(toYC));
    canvas.drawLine(midX, y(toYC), toX, y(toYC));
    canvas.strokePath();
  }

  // ---- Off-page predecessor stubs ----
  // Aggregate per-row so multiple stubs collapse to one line. Draw inside
  // the chart area at the row's left edge in small gray text, truncated to
  // the gap before the bar (or 80pt if the bar starts at chartLeft).
  final stubsByRow = <String, List<OffPageStub>>{};
  for (final stub in page.offPageStubs) {
    stubsByRow.putIfAbsent(stub.toItemId, () => []).add(stub);
  }
  canvas.setColor(PdfColors.grey600);
  final stubFontSize = cfg.fontSize - 1.0;
  for (final entry in stubsByRow.entries) {
    final yC = rowCenterY[entry.key];
    final rect = rowBarRect[entry.key];
    if (yC == null || rect == null) continue;

    // Combine predecessors. Group identical chart hints to keep it short.
    // Format: "<- pred1, pred2 (chartA) | pred3 (chartB)"
    final byChart = <String?, List<String>>{};
    for (final s in entry.value) {
      byChart.putIfAbsent(s.predecessorChart, () => []).add(s.predecessorId);
    }
    final parts = <String>[];
    byChart.forEach((chart, ids) {
      final idsStr = ids.join(', ');
      parts.add(chart != null ? '$idsStr ($chart)' : idsStr);
    });
    final combined = _ascii('<- ${parts.join(' | ')}');

    // Available width: from chartLeft+1 up to rect.left (where the bar starts).
    // If the bar starts at or near chartLeft, fall back to a fixed budget so
    // the stub still renders (it'll overlap the bar — bar is light gray, text
    // is darker gray, both readable in print).
    final preferredEnd = rect.left;
    final budget = (preferredEnd - chartLeft - 2).clamp(60.0, 220.0);
    final truncated = _truncateToWidth(font, stubFontSize, combined, budget);
    canvas.drawString(
      font,
      stubFontSize,
      truncated,
      chartLeft + 1,
      y(yC + stubFontSize / 2),
    );
  }

  // ---- Footer: overflow notice if we couldn't fit everything ----
  if (page.items.length > visibleItems.length) {
    canvas.setColor(PdfColors.red700);
    final overflowMsg =
        '... ${page.items.length - visibleItems.length} more item(s) did not fit on this page';
    canvas.drawString(
      font,
      cfg.fontSize,
      _ascii(overflowMsg),
      0,
      y(height - 1),
    );
  }
}

PdfColor _statusColor(GianttStatus status) {
  switch (status) {
    case GianttStatus.completed:
      return PdfColors.grey500;
    case GianttStatus.blocked:
      return PdfColors.red700;
    case GianttStatus.inProgress:
      return PdfColors.black;
    case GianttStatus.notStarted:
      return PdfColors.grey800;
  }
}

PdfColor _barFillColor(LaidOutItem laid) {
  if (laid.item.status == GianttStatus.completed) return PdfColors.grey300;
  if (!laid.timeAnchored) return PdfColors.grey400;
  if (laid.item.status == GianttStatus.inProgress) return PdfColors.blue300;
  return PdfColors.blueGrey300;
}

void _drawLeftArrow(
  PdfGraphics canvas,
  double Function(double) y,
  double x,
  double top,
  double bottom,
) {
  final mid = (top + bottom) / 2;
  canvas.setColor(PdfColors.grey700);
  canvas.moveTo(x, y(mid));
  canvas.lineTo(x + 3, y(top));
  canvas.lineTo(x + 3, y(bottom));
  canvas.fillPath();
}

void _drawRightArrow(
  PdfGraphics canvas,
  double Function(double) y,
  double x,
  double top,
  double bottom,
) {
  final mid = (top + bottom) / 2;
  canvas.setColor(PdfColors.grey700);
  canvas.moveTo(x, y(mid));
  canvas.lineTo(x - 3, y(top));
  canvas.lineTo(x - 3, y(bottom));
  canvas.fillPath();
}

String _truncateToWidth(PdfFont font, double size, String text, double maxWidth) {
  if (_textWidth(font, size, text) <= maxWidth) return text;
  var lo = 0;
  var hi = text.length;
  while (lo < hi) {
    final mid = (lo + hi + 1) ~/ 2;
    final candidate = '${text.substring(0, mid)}...';
    if (_textWidth(font, size, candidate) <= maxWidth) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }
  return lo > 0 ? '${text.substring(0, lo)}...' : '';
}

double _textWidth(PdfFont font, double size, String text) {
  return font.stringMetrics(text).width * size;
}

DateTime _nextWeekStart(DateTime d) {
  final dow = d.weekday; // Mon=1..Sun=7
  final daysUntilMonday = (8 - dow) % 7;
  return DateTime(d.year, d.month, d.day)
      .add(Duration(days: daysUntilMonday == 0 ? 0 : daysUntilMonday));
}

String _fmtDate(DateTime d) =>
    '${d.year.toString().padLeft(4, '0')}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}';

/// Replace characters the built-in Helvetica can't encode. We map known UI
/// glyphs to ASCII equivalents and fall back to '?' for anything else.
/// (Phase 2 should embed a TTF and drop this helper.)
String _ascii(String s) {
  const replacements = {
    '→': '->', // →
    '←': '<-', // ←
    '…': '...', // …
    '○': '[ ]', // ○ not started
    '◑': '[~]', // ◑ in progress
    '⊘': '[!]', // ⊘ blocked
    '●': '[x]', // ● completed
  };
  final buf = StringBuffer();
  for (final rune in s.runes) {
    if (rune <= 0x7E && rune >= 0x20) {
      buf.writeCharCode(rune);
      continue;
    }
    final ch = String.fromCharCode(rune);
    final repl = replacements[ch];
    if (repl != null) {
      buf.write(repl);
    } else {
      buf.write('?');
    }
  }
  return buf.toString();
}

String _fmtMonth(DateTime d) {
  const months = [
    'Jan',
    'Feb',
    'Mar',
    'Apr',
    'May',
    'Jun',
    'Jul',
    'Aug',
    'Sep',
    'Oct',
    'Nov',
    'Dec'
  ];
  return '${months[d.month - 1]} ${d.year}';
}

class _Rect {
  const _Rect(this.left, this.top, this.right, this.bottom);
  final double left;
  final double top;
  final double right;
  final double bottom;
}
