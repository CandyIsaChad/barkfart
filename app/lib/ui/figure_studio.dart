// Stick Figure Creator Studio UI: top bar, icon-driven Shape & Node Tool Shelf,
// and clean isolated canvas guides (origin crosshair, grid, symmetry mirror axis).
import 'package:flutter/widgets.dart';

import 'kit.dart' as kit;
import 'pictograms.dart';
import 'tokens.dart';

/// Crystal-clear top bar for the Stick Figure Creator Studio:
/// `Cancel`, inline-editable figure name, `Save to Library`, and primary
/// `Add to Scene` / `Apply`.
class FigureStudioHeader extends StatelessWidget {
  const FigureStudioHeader({
    super.key,
    required this.pad,
    required this.name,
    required this.primaryLabel,
    required this.onCancel,
    required this.onRename,
    required this.onSaveLibrary,
    required this.onPrimary,
    this.showGrid = true,
    this.onToggleGrid,
    this.showCrosshair = true,
    this.onToggleCrosshair,
    this.compact = false,
  });

  final EdgeInsets pad;
  final String name;
  final String primaryLabel;
  final VoidCallback onCancel;
  final VoidCallback onRename;
  final VoidCallback onSaveLibrary;
  final VoidCallback onPrimary;
  final bool showGrid;
  final VoidCallback? onToggleGrid;
  final bool showCrosshair;
  final VoidCallback? onToggleCrosshair;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final wide = MediaQuery.sizeOf(context).width >= 680;
    return Container(
      height: 44 + pad.top,
      padding: EdgeInsets.only(top: pad.top, left: pad.left + 6, right: pad.right + 8),
      decoration: const BoxDecoration(
        color: cCard,
        border: Border(bottom: BorderSide(color: cLine)),
      ),
      child: Row(children: [
        kit.WordButton('Cancel', size: 13.5, height: 34, onTap: onCancel),
        const SizedBox(width: 4),
        // Inline-editable figure name pill
        Expanded(
          child: Center(
            child: kit.Tap(
              label: 'Rename figure',
              minSize: 34,
              radius: 9,
              onTap: onRename,
              child: Container(
                constraints: const BoxConstraints(maxWidth: 260),
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
                decoration: BoxDecoration(
                  color: cControl,
                  borderRadius: BorderRadius.circular(9),
                ),
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  const Pictogram('figure', size: 15, color: cAccent),
                  const SizedBox(width: 6),
                  Flexible(
                    child: Text(
                      name.isEmpty ? 'Untitled figure' : name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: kit.ink(13.5, FontWeight.w700, cText),
                    ),
                  ),
                  const SizedBox(width: 5),
                  const Pictogram('edit', size: 13, color: cSecondary),
                ]),
              ),
            ),
          ),
        ),
        if (wide && onToggleGrid != null)
          kit.IconTool('grid', label: 'Studio grid', on: showGrid, size: 18, onTap: onToggleGrid),
        if (wide && onToggleCrosshair != null)
          kit.IconTool('crosshair', label: 'Origin crosshair', on: showCrosshair, size: 18, onTap: onToggleCrosshair),
        if (wide && (onToggleGrid != null || onToggleCrosshair != null))
          Container(width: 1, height: 20, margin: const EdgeInsets.symmetric(horizontal: 6), color: cLine),
        // Save to Library (.nodes)
        if (wide)
          kit.WordButton('Save to Library', filled: true, height: 34, size: 13, onTap: onSaveLibrary)
        else
          kit.IconTool('save', label: 'Save to Library', color: cAccent, size: 19, onTap: onSaveLibrary),
        const SizedBox(width: 6),
        kit.WordButton(primaryLabel, primary: true, height: 34, size: 13.5, onTap: onPrimary),
      ]),
    );
  }
}

/// Polished bottom Shape & Node Tool Shelf for the Stick Figure Creator:
/// drawing shapes + quick node/branch actions (`Mirror`, `Grid`, `Split`,
/// `Duplicate`, `Flip`, `Delete`).
class FigureStudioShelf extends StatelessWidget {
  const FigureStudioShelf({
    super.key,
    required this.drawTool,
    required this.polyfillActive,
    required this.mirrorOn,
    required this.gridOn,
    required this.hasSelectedJoint,
    required this.canEditNonRoot,
    required this.shapePainterBuilder,
    required this.onPickTool,
    required this.onPickPolyfill,
    required this.onToggleMirror,
    required this.onToggleGrid,
    required this.onSplit,
    required this.onDuplicateBranch,
    required this.onFlipBranch,
    required this.onDeleteJoint,
  });

  final String drawTool;
  final bool polyfillActive;
  final bool mirrorOn;
  final bool gridOn;
  final bool hasSelectedJoint;
  final bool canEditNonRoot;
  final CustomPainter Function(String glyph, Color color) shapePainterBuilder;
  final ValueChanged<String> onPickTool;
  final VoidCallback onPickPolyfill;
  final VoidCallback onToggleMirror;
  final VoidCallback onToggleGrid;
  final VoidCallback onSplit;
  final VoidCallback onDuplicateBranch;
  final VoidCallback onFlipBranch;
  final VoidCallback onDeleteJoint;

  static const _tools = [
    ('none', 'Select', 'select'),
    ('square', 'Segment', 'segment'),
    ('line', 'Rounded', 'rounded'),
    ('circle', 'Circle', 'circle'),
    ('ellipse', 'Ellipse', 'ellipse'),
    ('triangle', 'Triangle', 'triangle'),
    ('trapezoid', 'Trapezoid', 'trapezoid'),
    ('polygon', 'Polygon', 'polygon'),
    ('polyfill', 'Polyfill', 'polyfill'),
  ];

  @override
  Widget build(BuildContext context) {
    final compact = MediaQuery.sizeOf(context).height < 460;
    final shelfH = compact ? 64.0 : 74.0;
    final tileW = compact ? 60.0 : 66.0;
    final tileH = compact ? 52.0 : 60.0;

    Widget shapeTile(String id, String label, String glyph) {
      final on = id == 'polyfill' ? polyfillActive : (drawTool == id && !polyfillActive);
      return kit.Tap(
        label: label,
        minSize: 0,
        radius: 11,
        onTap: id == 'polyfill' ? onPickPolyfill : () => onPickTool(id),
        child: Container(
          width: tileW,
          height: tileH,
          decoration: BoxDecoration(
            color: on ? cSelected : null,
            borderRadius: BorderRadius.circular(11),
            border: on ? Border.all(color: cAccent.withValues(alpha: 0.35)) : null,
          ),
          child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
            CustomPaint(
              size: Size(compact ? 32 : 36, compact ? 20 : 24),
              painter: shapePainterBuilder(glyph, on ? cAccent : cInk2b),
            ),
            const SizedBox(height: 3),
            Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: kit.ink(compact ? 10.5 : 11.5, on ? FontWeight.w700 : FontWeight.w500, on ? cAccent : cSecondary),
            ),
          ]),
        ),
      );
    }

    Widget actionTile(String icon, String label, VoidCallback? onTap, {bool active = false, bool danger = false}) {
      final enabled = onTap != null;
      final fg = !enabled ? cTag : danger ? cDanger : active ? cAccent : cInk2b;
      final txt = !enabled ? cTag : danger ? cDanger : active ? cAccent : cSecondary;
      return kit.Tap(
        label: label,
        minSize: 0,
        radius: 11,
        onTap: onTap,
        child: Container(
          width: compact ? 54 : 60,
          height: tileH,
          decoration: BoxDecoration(
            color: active ? cSelected : null,
            borderRadius: BorderRadius.circular(11),
            border: active ? Border.all(color: cAccent.withValues(alpha: 0.35)) : null,
          ),
          child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
            Pictogram(icon, size: compact ? 18 : 20, color: fg, stroke: 1.9),
            const SizedBox(height: 4),
            Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: kit.ink(compact ? 10.5 : 11, active || danger ? FontWeight.w600 : FontWeight.w500, txt),
            ),
          ]),
        ),
      );
    }

    return Container(
      height: shelfH,
      decoration: const BoxDecoration(
        color: cCard,
        border: Border(top: BorderSide(color: cLine)),
      ),
      child: Center(
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            for (final (id, label, glyph) in _tools)
              Padding(padding: const EdgeInsets.symmetric(horizontal: 2), child: shapeTile(id, label, glyph)),
            Container(width: 1, height: compact ? 34 : 40, margin: const EdgeInsets.symmetric(horizontal: 8), color: cLine),
            actionTile('mirror', 'Mirror', onToggleMirror, active: mirrorOn),
            actionTile('grid', 'Grid', onToggleGrid, active: gridOn),
            Container(width: 1, height: compact ? 34 : 40, margin: const EdgeInsets.symmetric(horizontal: 6), color: cLine),
            actionTile('split', 'Split', canEditNonRoot ? onSplit : null),
            actionTile('duplicate', 'Duplicate', canEditNonRoot ? onDuplicateBranch : null),
            actionTile('flipH', 'Flip', canEditNonRoot ? onFlipBranch : null),
            actionTile('bin', 'Delete', canEditNonRoot ? onDeleteJoint : null, danger: canEditNonRoot),
          ]),
        ),
      ),
    );
  }
}

/// Draws clean studio canvas guides when editing or creating a stick figure:
/// subtle engineering grid, origin crosshair at (0, 0), and dashed symmetry
/// mirror axis when Mirror mode is active.
void paintFigureStudioGuides(
  Canvas canvas,
  Size size, {
  required Offset Function(double sx, double sy) toScreen,
  required double scale,
  required bool showGrid,
  required bool showCrosshair,
  required bool showMirrorAxis,
}) {
  final origin = toScreen(0, 0);

  if (showGrid && scale > 0.05) {
    // Pick a clean grid spacing in stage units so screen lines stay ~36-72px apart
    double step = 50.0;
    while (step * scale < 32) {
      step *= 2;
    }
    while (step * scale > 96) {
      step /= 2;
    }
    final screenStep = step * scale;
    final minorPaint = Paint()
      ..color = const Color(0x14222428)
      ..strokeWidth = 1;
    final majorPaint = Paint()
      ..color = const Color(0x24222428)
      ..strokeWidth = 1;

    final startX = origin.dx % screenStep;
    var gx = startX;
    var ix = ((gx - origin.dx) / screenStep).round();
    while (gx < size.width) {
      canvas.drawLine(Offset(gx, 0), Offset(gx, size.height), ix % 4 == 0 ? majorPaint : minorPaint);
      gx += screenStep;
      ix++;
    }
    final startY = origin.dy % screenStep;
    var gy = startY;
    var iy = ((gy - origin.dy) / screenStep).round();
    while (gy < size.height) {
      canvas.drawLine(Offset(0, gy), Offset(size.width, gy), iy % 4 == 0 ? majorPaint : minorPaint);
      gy += screenStep;
      iy++;
    }
  }

  if (showCrosshair) {
    final axisPaint = Paint()
      ..color = const Color(0x38222428)
      ..strokeWidth = 1.2;
    canvas.drawLine(Offset(origin.dx, 0), Offset(origin.dx, size.height), axisPaint);
    canvas.drawLine(Offset(0, origin.dy), Offset(size.width, origin.dy), axisPaint);
    // Subtle origin target ring
    final ringPaint = Paint()
      ..color = const Color(0x521577D6)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.4;
    canvas.drawCircle(origin, 7, ringPaint);
  }

  if (showMirrorAxis) {
    final mirrorPaint = Paint()
      ..color = cAccent.withValues(alpha: 0.72)
      ..strokeWidth = 1.6;
    const dash = 7.0, gap = 5.0;
    for (var y = 0.0; y < size.height; y += dash + gap) {
      canvas.drawLine(Offset(origin.dx, y), Offset(origin.dx, (y + dash).clamp(0, size.height)), mirrorPaint);
    }
  }
}
