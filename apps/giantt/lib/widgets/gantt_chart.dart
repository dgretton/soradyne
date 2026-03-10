import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
import '../layout/gantt_layout.dart';

// ─── sizing constants ─────────────────────────────────────────────────────────

const double _labelWidth = 160.0;
const double _rowHeight = 48.0;
const double _barHeight = 32.0;
const double _barVerticalPad = (_rowHeight - _barHeight) / 2;
const double _priorityStripeWidth = 4.0;
const double _minBarWidth = 8.0;

// Time axis heights for the two modes.
const double _timeAxisHeightFull = 36.0;
const double _timeAxisHeightCompact = 24.0;

// Base scale: 20 logical pixels per day.
const double _basePixelsPerDay = 20.0;
const double _basePixelsPerSecond = _basePixelsPerDay / 86400.0;

// ─── colour helpers ───────────────────────────────────────────────────────────

Color _statusColor(GianttStatus status) => switch (status) {
  GianttStatus.notStarted => const Color(0xFFBDBDBD),
  GianttStatus.inProgress => const Color(0xFF42A5F5),
  GianttStatus.blocked => const Color(0xFFFFA726),
  GianttStatus.completed => const Color(0xFF66BB6A),
};

Color? _priorityStripeColor(GianttPriority priority) => switch (priority) {
  GianttPriority.critical => const Color(0xFFB71C1C),
  GianttPriority.high => const Color(0xFFE53935),
  GianttPriority.medium => const Color(0xFFFB8C00),
  _ => null,
};

// ─── GanttChart widget ────────────────────────────────────────────────────────

/// A horizontally and vertically scrollable Gantt chart.
///
/// When [compact] is true (landscape) the zoom slider is replaced by small
/// overlay ± buttons so the chart can claim nearly the full screen height.
class GanttChart extends StatefulWidget {
  const GanttChart({
    super.key,
    required this.layout,
    this.compact = false,
  });

  final GanttLayout layout;

  /// Compact mode hides the slider and shrinks the time axis.
  /// Intended for landscape orientation where vertical space is scarce.
  final bool compact;

  @override
  State<GanttChart> createState() => _GanttChartState();
}

class _GanttChartState extends State<GanttChart> {
  final _bodyHorizController = ScrollController();
  final _axisHorizController = ScrollController();

  double _zoom = 1.0;

  @override
  void initState() {
    super.initState();
    _bodyHorizController.addListener(_syncAxisToBody);
  }

  @override
  void dispose() {
    _bodyHorizController.removeListener(_syncAxisToBody);
    _bodyHorizController.dispose();
    _axisHorizController.dispose();
    super.dispose();
  }

  void _syncAxisToBody() {
    if (!_axisHorizController.hasClients) return;
    final offset = _bodyHorizController.offset;
    if ((_axisHorizController.offset - offset).abs() > 0.5) {
      _axisHorizController.jumpTo(offset);
    }
  }

  void _stepZoom(double delta) {
    setState(() => _zoom = (_zoom + delta).clamp(0.1, 3.0));
  }

  double get _pixelsPerSecond => _basePixelsPerSecond * _zoom;
  double get _totalCanvasWidth =>
      _labelWidth + widget.layout.totalSpanSeconds * _pixelsPerSecond;
  double get _totalCanvasHeight => widget.layout.rows.length * _rowHeight;
  double get _timeAxisHeight =>
      widget.compact ? _timeAxisHeightCompact : _timeAxisHeightFull;

  @override
  Widget build(BuildContext context) {
    if (widget.layout.isEmpty) {
      return const Center(
        child: Text('No items to chart', style: TextStyle(color: Colors.grey)),
      );
    }

    if (widget.compact) {
      // Landscape: time axis + chart body, with zoom overlay buttons.
      return Stack(
        children: [
          Column(
            children: [
              _buildTimeAxis(),
              const Divider(height: 1, thickness: 1),
              Expanded(child: _buildBody()),
            ],
          ),
          Positioned(
            left: 8,
            bottom: 12,
            child: _buildZoomOverlay(context),
          ),
        ],
      );
    }

    // Portrait: slider + time axis + chart body.
    return Column(
      children: [
        _buildZoomSlider(context),
        _buildTimeAxis(),
        const Divider(height: 1, thickness: 1),
        Expanded(child: _buildBody()),
      ],
    );
  }

  // ── portrait zoom slider ──────────────────────────────────────────────────

  Widget _buildZoomSlider(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      child: Row(
        children: [
          const Icon(Icons.zoom_out, size: 18, color: Colors.grey),
          Expanded(
            child: Slider(
              value: _zoom,
              min: 0.1,
              max: 3.0,
              onChanged: (v) => setState(() => _zoom = v),
            ),
          ),
          const Icon(Icons.zoom_in, size: 18, color: Colors.grey),
          const SizedBox(width: 4),
          SizedBox(
            width: 48,
            child: Text(
              '${_zoom.toStringAsFixed(1)}×',
              style: Theme.of(context).textTheme.bodySmall,
              textAlign: TextAlign.right,
            ),
          ),
        ],
      ),
    );
  }

  // ── compact zoom overlay (landscape) ─────────────────────────────────────

  Widget _buildZoomOverlay(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface.withValues(alpha: 0.85),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: Colors.grey.shade300),
      ),
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          _ZoomButton(
            icon: Icons.remove,
            onTap: () => _stepZoom(-0.25),
            enabled: _zoom > 0.1,
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 6),
            child: Text(
              '${_zoom.toStringAsFixed(1)}×',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                fontFeatures: const [FontFeature.tabularFigures()],
              ),
            ),
          ),
          _ZoomButton(
            icon: Icons.add,
            onTap: () => _stepZoom(0.25),
            enabled: _zoom < 3.0,
          ),
        ],
      ),
    );
  }

  // ── time axis ────────────────────────────────────────────────────────────

  Widget _buildTimeAxis() {
    return SizedBox(
      height: _timeAxisHeight,
      child: SingleChildScrollView(
        controller: _axisHorizController,
        scrollDirection: Axis.horizontal,
        physics: const NeverScrollableScrollPhysics(),
        child: SizedBox(
          width: _totalCanvasWidth,
          height: _timeAxisHeight,
          child: CustomPaint(
            painter: _TimeAxisPainter(
              pixelsPerSecond: _pixelsPerSecond,
              totalSpanSeconds: widget.layout.totalSpanSeconds,
              labelWidth: _labelWidth,
              height: _timeAxisHeight,
            ),
          ),
        ),
      ),
    );
  }

  // ── scrollable body ──────────────────────────────────────────────────────

  Widget _buildBody() {
    return SingleChildScrollView(
      scrollDirection: Axis.vertical,
      child: SingleChildScrollView(
        controller: _bodyHorizController,
        scrollDirection: Axis.horizontal,
        child: SizedBox(
          width: _totalCanvasWidth,
          height: _totalCanvasHeight,
          child: CustomPaint(
            painter: _GanttBodyPainter(
              layout: widget.layout,
              pixelsPerSecond: _pixelsPerSecond,
              labelWidth: _labelWidth,
              rowHeight: _rowHeight,
              barHeight: _barHeight,
              barVerticalPad: _barVerticalPad,
            ),
          ),
        ),
      ),
    );
  }
}

// ─── _ZoomButton ─────────────────────────────────────────────────────────────

class _ZoomButton extends StatelessWidget {
  const _ZoomButton({
    required this.icon,
    required this.onTap,
    required this.enabled,
  });

  final IconData icon;
  final VoidCallback onTap;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: enabled ? onTap : null,
      child: Icon(
        icon,
        size: 18,
        color: enabled ? Theme.of(context).colorScheme.onSurface : Colors.grey.shade400,
      ),
    );
  }
}

// ─── TimeAxisPainter ─────────────────────────────────────────────────────────

class _TimeAxisPainter extends CustomPainter {
  _TimeAxisPainter({
    required this.pixelsPerSecond,
    required this.totalSpanSeconds,
    required this.labelWidth,
    required this.height,
  });

  final double pixelsPerSecond;
  final double totalSpanSeconds;
  final double labelWidth;
  final double height;

  @override
  void paint(Canvas canvas, Size size) {
    final bgPaint = Paint()..color = const Color(0xFFF5F5F5);
    canvas.drawRect(Rect.fromLTWH(0, 0, size.width, size.height), bgPaint);

    final divPaint = Paint()..color = const Color(0xFFE0E0E0);
    canvas.drawLine(
      Offset(labelWidth, 0),
      Offset(labelWidth, size.height),
      divPaint,
    );

    final tickInterval = _chooseTick();
    final tickPaint = Paint()..color = const Color(0xFFBDBDBD);
    final textStyle = TextStyle(
      fontSize: height < 30 ? 9 : 10,
      color: const Color(0xFF757575),
      fontFeatures: const [FontFeature.tabularFigures()],
    );

    var t = 0.0;
    while (t <= totalSpanSeconds) {
      final x = labelWidth + t * pixelsPerSecond;
      canvas.drawLine(
        Offset(x, size.height - 5),
        Offset(x, size.height),
        tickPaint,
      );
      final label = _formatTick(t, tickInterval);
      final tp = TextPainter(
        text: TextSpan(text: label, style: textStyle),
        textDirection: TextDirection.ltr,
      )..layout();
      tp.paint(canvas, Offset(x + 2, (size.height - tp.height) / 2 - 1));
      t += tickInterval;
    }

    canvas.drawLine(
      Offset(0, size.height - 1),
      Offset(size.width, size.height - 1),
      divPaint,
    );
  }

  double _chooseTick() {
    const oneDay = 86400.0;
    const oneWeek = 7 * oneDay;
    const oneMonth = 30 * oneDay;
    const oneYear = 365 * oneDay;
    final pxPerDay = oneDay * pixelsPerSecond;
    if (pxPerDay < 3) return oneYear;
    if (pxPerDay < 12) return oneMonth;
    if (pxPerDay < 60) return oneWeek;
    if (pxPerDay < 300) return oneDay;
    return 3600.0;
  }

  String _formatTick(double seconds, double tickInterval) {
    const oneDay = 86400.0;
    const oneWeek = 7 * oneDay;
    const oneMonth = 30 * oneDay;
    const oneYear = 365 * oneDay;
    if (tickInterval >= oneYear) return 'Y${(seconds / oneYear).round() + 1}';
    if (tickInterval >= oneMonth) return 'M${(seconds / oneMonth).round() + 1}';
    if (tickInterval >= oneWeek) return 'W${(seconds / oneWeek).round() + 1}';
    return 'D${(seconds / oneDay).round() + 1}';
  }

  @override
  bool shouldRepaint(_TimeAxisPainter old) =>
      old.pixelsPerSecond != pixelsPerSecond ||
      old.totalSpanSeconds != totalSpanSeconds ||
      old.height != height;
}

// ─── GanttBodyPainter ─────────────────────────────────────────────────────────

class _GanttBodyPainter extends CustomPainter {
  _GanttBodyPainter({
    required this.layout,
    required this.pixelsPerSecond,
    required this.labelWidth,
    required this.rowHeight,
    required this.barHeight,
    required this.barVerticalPad,
  });

  final GanttLayout layout;
  final double pixelsPerSecond;
  final double labelWidth;
  final double rowHeight;
  final double barHeight;
  final double barVerticalPad;

  @override
  void paint(Canvas canvas, Size size) {
    _paintRowBackgrounds(canvas, size);
    _paintLabelDivider(canvas, size);
    _paintDependencies(canvas);
    _paintBarsAndLabels(canvas);
  }

  void _paintRowBackgrounds(Canvas canvas, Size size) {
    final evenPaint = Paint()..color = Colors.white;
    final oddPaint = Paint()..color = const Color(0xFFFAFAFA);
    final gridPaint = Paint()..color = const Color(0xFFEEEEEE);

    for (var i = 0; i < layout.rows.length; i++) {
      final y = i * rowHeight;
      canvas.drawRect(
        Rect.fromLTWH(0, y, size.width, rowHeight),
        i.isEven ? evenPaint : oddPaint,
      );
    }
    for (var i = 1; i < layout.rows.length; i++) {
      final y = i * rowHeight;
      canvas.drawLine(Offset(0, y), Offset(size.width, y), gridPaint);
    }
  }

  void _paintLabelDivider(Canvas canvas, Size size) {
    final divPaint = Paint()..color = const Color(0xFFE0E0E0);
    canvas.drawLine(
      Offset(labelWidth, 0),
      Offset(labelWidth, size.height),
      divPaint,
    );
  }

  void _paintDependencies(Canvas canvas) {
    final linePaint =
        Paint()
          ..color = const Color(0x99B0BEC5)
          ..strokeWidth = 1.5
          ..style = PaintingStyle.stroke;
    final arrowPaint =
        Paint()
          ..color = const Color(0xFF90A4AE)
          ..style = PaintingStyle.fill;

    for (final dep in layout.dependencies) {
      if (dep.fromRow >= layout.rows.length ||
          dep.toRow >= layout.rows.length) continue;

      final from = layout.rows[dep.fromRow];
      final to = layout.rows[dep.toRow];

      final x0 = labelWidth + from.endSeconds * pixelsPerSecond;
      final y0 = dep.fromRow * rowHeight + rowHeight / 2;
      final x1 = labelWidth + to.startSeconds * pixelsPerSecond;
      final y1 = dep.toRow * rowHeight + rowHeight / 2;

      final midX = math.max(x0 + 6, x1 - 6);
      final path =
          Path()
            ..moveTo(x0, y0)
            ..lineTo(midX, y0)
            ..lineTo(midX, y1)
            ..lineTo(x1, y1);
      canvas.drawPath(path, linePaint);

      const aW = 6.0;
      const aH = 4.0;
      final arrow =
          Path()
            ..moveTo(x1, y1)
            ..lineTo(x1 - aW, y1 - aH)
            ..lineTo(x1 - aW, y1 + aH)
            ..close();
      canvas.drawPath(arrow, arrowPaint);
    }
  }

  void _paintBarsAndLabels(Canvas canvas) {
    for (final row in layout.rows) {
      _paintLabel(canvas, row);
      _paintBar(canvas, row);
    }
  }

  void _paintLabel(Canvas canvas, GanttRowData row) {
    final rowTop = row.rowIndex * rowHeight;
    const hPad = 8.0;

    canvas.save();
    canvas.clipRect(Rect.fromLTWH(0, rowTop, labelWidth - 1, rowHeight));

    final dotPaint = Paint()..color = _statusColor(row.item.status);
    canvas.drawCircle(Offset(hPad + 5, rowTop + rowHeight / 2), 5, dotPaint);

    final titlePainter = TextPainter(
      text: TextSpan(
        text: row.item.title,
        style: const TextStyle(
          fontSize: 12,
          color: Color(0xFF212121),
          fontWeight: FontWeight.w500,
        ),
      ),
      textDirection: TextDirection.ltr,
      maxLines: 1,
      ellipsis: '…',
    )..layout(maxWidth: labelWidth - hPad * 2 - 16);
    titlePainter.paint(
      canvas,
      Offset(hPad + 16, rowTop + (rowHeight - titlePainter.height) / 2 - 5),
    );

    final durText =
        row.item.duration.parts.isEmpty ? '—' : row.item.duration.toString();
    final durPainter = TextPainter(
      text: TextSpan(
        text: durText,
        style: const TextStyle(fontSize: 10, color: Color(0xFF9E9E9E)),
      ),
      textDirection: TextDirection.ltr,
      maxLines: 1,
    )..layout(maxWidth: labelWidth - hPad * 2 - 16);
    durPainter.paint(
      canvas,
      Offset(hPad + 16, rowTop + (rowHeight - durPainter.height) / 2 + 8),
    );

    canvas.restore();
  }

  void _paintBar(Canvas canvas, GanttRowData row) {
    final barLeft = labelWidth + row.startSeconds * pixelsPerSecond;
    final barWidth =
        math.max(_minBarWidth, row.durationSeconds * pixelsPerSecond);
    final barTop = row.rowIndex * rowHeight + barVerticalPad;

    final rrect = RRect.fromRectAndRadius(
      Rect.fromLTWH(barLeft, barTop, barWidth, barHeight),
      const Radius.circular(4),
    );

    canvas.drawRRect(rrect, Paint()..color = _statusColor(row.item.status));

    final stripeColor = _priorityStripeColor(row.item.priority);
    if (stripeColor != null) {
      canvas.drawRRect(
        RRect.fromRectAndCorners(
          Rect.fromLTWH(barLeft, barTop, _priorityStripeWidth, barHeight),
          topLeft: const Radius.circular(4),
          bottomLeft: const Radius.circular(4),
        ),
        Paint()..color = stripeColor,
      );
    }

    canvas.drawRRect(
      rrect,
      Paint()
        ..color = _statusColor(row.item.status).withValues(alpha: 0.7)
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1,
    );

    if (barWidth > 24) {
      canvas.save();
      canvas.clipRect(Rect.fromLTWH(barLeft, barTop, barWidth, barHeight));
      final labelPainter = TextPainter(
        text: TextSpan(
          text: row.item.title,
          style: const TextStyle(fontSize: 11, color: Color(0xFF212121)),
        ),
        textDirection: TextDirection.ltr,
        maxLines: 1,
        ellipsis: '…',
      )..layout(maxWidth: barWidth - _priorityStripeWidth - 8);
      labelPainter.paint(
        canvas,
        Offset(
          barLeft + _priorityStripeWidth + 4,
          barTop + (barHeight - labelPainter.height) / 2,
        ),
      );
      canvas.restore();
    }
  }

  @override
  bool shouldRepaint(_GanttBodyPainter old) =>
      old.layout != layout || old.pixelsPerSecond != pixelsPerSecond;
}
