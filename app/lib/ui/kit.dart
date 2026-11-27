// The redesign's component kit (REDESIGN.md §3, §4; D15). One of each
// control, drawn from the tokens. Every tappable thing has a 44 pt hit area,
// even where the drawn control is smaller.
import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/rendering.dart';
import 'package:material_ui/material_ui.dart' show Material, MaterialType, Tooltip;
import 'package:flutter/widgets.dart';

import 'pictograms.dart';
import 'tokens.dart';

const kFont = 'NotoSans';
const _shadowCard = [BoxShadow(color: Color(0x14000000), spreadRadius: 1), BoxShadow(color: Color(0x4D10141A), blurRadius: 28, offset: Offset(0, 10))];

/// Motion (REDESIGN D37): short, and only in answer to something done.
/// None at all when the system asks for less motion.
Duration motion(BuildContext context, int ms) => MediaQuery.maybeDisableAnimationsOf(context) == true ? Duration.zero : Duration(milliseconds: ms);

/// Things arriving ease out; the standard curve for the app.
const Curve kEase = Curves.easeOutCubic;

TextStyle ink([double size = 14, FontWeight w = FontWeight.w400, Color c = cText]) => TextStyle(fontFamily: kFont, fontSize: size, fontWeight: w, color: c, decoration: TextDecoration.none, height: 1.25);

/// A tap target with a pressed look; at least 44 x 44 for touch.
class Tap extends StatefulWidget {
  const Tap({super.key, required this.child, this.onTap, this.onLongPress, this.minSize = 44, this.radius = 10, this.label, this.pressedColor = cPressed});
  final Widget child;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final double minSize;
  final double radius;
  final String? label;
  final Color pressedColor;
  @override
  State<Tap> createState() => _TapState();
}

class _TapState extends State<Tap> {
  bool _down = false;
  @override
  Widget build(BuildContext context) {
    final on = widget.onTap != null || widget.onLongPress != null;
    Widget w = GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapDown: on ? (_) => setState(() => _down = true) : null,
      onTapUp: on ? (_) => setState(() => _down = false) : null,
      onTapCancel: on ? () => setState(() => _down = false) : null,
      onTap: widget.onTap,
      onLongPress: widget.onLongPress,
      child: ConstrainedBox(
        constraints: BoxConstraints(minWidth: widget.minSize, minHeight: widget.minSize),
        child: Center(
          widthFactor: 1,
          heightFactor: 1,
          child: AnimatedScale(
            // buttons dip a little under the finger (menu rows only shade)
            scale: _down && widget.minSize >= 44 ? 0.95 : 1,
            duration: motion(context, 90),
            curve: kEase,
            child: AnimatedContainer(
              duration: motion(context, 80),
              decoration: BoxDecoration(color: _down ? widget.pressedColor : null, borderRadius: BorderRadius.circular(widget.radius)),
              child: widget.child,
            ),
          ),
        ),
      ),
    );
    if (widget.label != null) w = Semantics(button: true, label: widget.label, child: Tooltip(message: widget.label!, waitDuration: const Duration(milliseconds: 600), child: w));
    return MouseRegion(cursor: on ? SystemMouseCursors.click : MouseCursor.defer, child: w);
  }
}

/// Touches up to `dy` above or below the child still land on it: a control
/// drawn shorter than 44 pt keeps a 44 pt touch area without taking more
/// room. (Its parent must hit-test it, as rows and columns do.)
class HitPad extends SingleChildRenderObjectWidget {
  const HitPad({super.key, required this.dy, super.child});
  final double dy;
  @override
  RenderObject createRenderObject(BuildContext context) => RenderHitPad(dy);
  @override
  void updateRenderObject(BuildContext context, RenderHitPad renderObject) => renderObject.dy = dy;
}

class RenderHitPad extends RenderProxyBox {
  RenderHitPad(this.dy);
  double dy;
  @override
  bool hitTest(BoxHitTestResult result, {required Offset position}) {
    if (!Rect.fromLTRB(0, -dy, size.width, size.height + dy).contains(position)) return false;
    final inside = Offset(position.dx, position.dy.clamp(0.0, math.max(0.0, size.height - 0.01)));
    if (hitTestChildren(result, position: inside)) {
      result.add(BoxHitTestEntry(this, position));
      return true;
    }
    return false;
  }
}

/// Blue words that do something. `filled`: a field-grey button; `primary`:
/// the one blue button of a sheet; `danger`: red words.
class WordButton extends StatelessWidget {
  const WordButton(this.text, {super.key, this.onTap, this.filled = false, this.primary = false, this.danger = false, this.strong = false, this.height = 36, this.expand = false, this.size = 15});
  final String text;
  final VoidCallback? onTap;
  final bool filled, primary, danger, strong, expand;
  final double height, size;
  @override
  Widget build(BuildContext context) {
    final enabled = onTap != null;
    final fg = !enabled ? cTag : primary ? cCard : danger ? cDanger : cAccent;
    final bg = primary ? (enabled ? cAccent : cControl) : filled ? (danger ? const Color(0xFFFDECEE) : cControl) : null;
    final box = Container(
      height: height,
      alignment: Alignment.center,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(color: bg, borderRadius: BorderRadius.circular(10)),
      child: Text(text, maxLines: 1, overflow: TextOverflow.ellipsis, style: ink(size, strong || filled || primary ? FontWeight.w700 : FontWeight.w600, fg)),
    );
    return Tap(onTap: onTap, minSize: expand ? 0 : 44, child: expand ? SizedBox(width: double.infinity, child: box) : Row(mainAxisSize: MainAxisSize.min, children: [box]));
  }
}

/// A rail button: a pictogram on a 44 pt tile. `active`: the current mode
/// (red pictogram, white tile); `pressed`: a toggled panel (white tile).
class RailButton extends StatelessWidget {
  const RailButton(this.icon, {super.key, required this.label, this.onTap, this.active = false, this.pressed = false, this.size = 28, this.swatch});
  final String icon;
  final String label;
  final VoidCallback? onTap;
  final bool active, pressed;
  final double size;
  final Color? swatch;
  @override
  Widget build(BuildContext context) {
    final on = active || pressed;
    return Tap(
      label: label,
      onTap: onTap,
      radius: 11,
      child: Container(
        width: 44,
        height: 44,
        alignment: Alignment.center,
        decoration: on ? BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(11), boxShadow: const [BoxShadow(color: cLine, spreadRadius: 1), BoxShadow(color: Color(0x0F000000), blurRadius: 2, offset: Offset(0, 1))]) : null,
        child: Opacity(opacity: onTap == null ? 0.35 : 1, child: Pictogram(icon, size: size, color: active ? cTitle : cAccent)),
      ),
    );
  }
}

/// An icon-only toolbar button (timeline toolbar, bottom bar).
class IconTool extends StatelessWidget {
  const IconTool(this.icon, {super.key, required this.label, this.onTap, this.on = false, this.size = 20, this.color});
  final String icon;
  final String label;
  final VoidCallback? onTap;
  final bool on;
  final double size;
  final Color? color;
  @override
  Widget build(BuildContext context) => Tap(
        label: label,
        onTap: onTap,
        radius: 8,
        minSize: 36,
        child: Container(
          width: 34,
          height: 28,
          alignment: Alignment.center,
          decoration: on ? BoxDecoration(color: const Color(0xFFE8F0FA), borderRadius: BorderRadius.circular(8)) : null,
          child: Pictogram(icon, size: size, color: color ?? (on ? cAccent : cInk2b)),
        ),
      );
}

/// The big square tick box. `value == null` is the mixed state (a dash).
class TickBox extends StatelessWidget {
  const TickBox(this.value, {super.key, this.onChanged, this.size = 26});
  final bool? value;
  final ValueChanged<bool>? onChanged;
  final double size;
  @override
  Widget build(BuildContext context) => Tap(
        onTap: onChanged == null ? null : () => onChanged!(!(value ?? false)),
        child: Opacity(
          opacity: onChanged == null ? 0.45 : 1,
          // the tick draws itself in when it comes on
          child: TweenAnimationBuilder<double>(
            tween: Tween(end: value == false ? 0 : 1),
            duration: motion(context, 160),
            curve: kEase,
            builder: (c, t, _) => CustomPaint(size: Size.square(size), painter: _TickPainter(value, t)),
          ),
        ),
      );
}

class _TickPainter extends CustomPainter {
  _TickPainter(this.value, [this.t = 1]);
  final bool? value;
  /// How far the tick is drawn in (0: off, 1: on).
  final double t;
  @override
  void paint(Canvas canvas, Size size) {
    final k = size.width / 30;
    canvas.scale(k);
    final on = value != false;
    canvas.drawRRect(RRect.fromLTRBR(1, 1, 29, 29, const Radius.circular(7)), Paint()..color = cCard);
    canvas.drawRRect(RRect.fromLTRBR(1, 1, 29, 29, const Radius.circular(7)), Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5 + 0.5 * t
      ..color = Color.lerp(const Color(0xFFC9CED5), cAccent, t)!);
    final pen = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3.2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    if (value == null) {
      canvas.drawLine(const Offset(9, 15), const Offset(21, 15), pen..color = cAccent);
      return;
    }
    final tick = Path()..moveTo(8.5, 15.5)..lineTo(12.9, 19.9)..lineTo(21.5, 10.5);
    // the faint tick of an off box, and the blue one drawn along it
    if (t < 1) canvas.drawPath(tick, pen..color = const Color(0xFFD9DDE2));
    if (on && t > 0) {
      final m = tick.computeMetrics().first;
      canvas.drawPath(m.extractPath(0, m.length * t), pen..color = cAccent);
    }
  }

  @override
  bool shouldRepaint(_TickPainter old) => old.value != value || old.t != t;
}

/// One row of a page: label on the left, a control on the right. A small blue
/// dot before the label marks a value that differs from the figure file.
class PropRow extends StatelessWidget {
  const PropRow(this.label, {super.key, this.trailing, this.changed = false, this.height = 38, this.secondary});
  final String label;
  final Widget? trailing;
  final bool changed;
  final double height;
  final String? secondary;
  @override
  Widget build(BuildContext context) => SizedBox(
        height: height,
        child: Row(children: [
          if (changed) Container(width: 6, height: 6, margin: const EdgeInsets.only(right: 6), decoration: const BoxDecoration(color: cAccent, shape: BoxShape.circle)),
          Expanded(child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: ink(14))),
          if (secondary != null) Padding(padding: const EdgeInsets.only(left: 8), child: Text(secondary!, style: ink(13, FontWeight.w400, cSecondary))),
          ?trailing,
        ]),
      );
}

/// A tick-box row.
class TickRow extends StatelessWidget {
  const TickRow(this.label, this.value, {super.key, this.onChanged, this.changed = false});
  final String label;
  final bool? value;
  final ValueChanged<bool>? onChanged;
  final bool changed;
  @override
  Widget build(BuildContext context) => PropRow(label, changed: changed, trailing: TickBox(value, onChanged: onChanged));
}

/// Words in a row of equal parts, one of them chosen.
class Segmented extends StatelessWidget {
  const Segmented(this.options, this.selected, {super.key, this.onSelect});
  final List<String> options;
  final int selected;
  final ValueChanged<int>? onSelect;
  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.all(3),
        margin: const EdgeInsets.only(bottom: 6),
        decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(11)),
        child: Stack(children: [
          // the chosen part's card slides to it
          if (selected >= 0 && selected < options.length)
            Positioned.fill(
              child: AnimatedAlign(
                alignment: Alignment(options.length < 2 ? 0 : -1 + 2 * selected / (options.length - 1), 0),
                duration: motion(context, 180),
                curve: kEase,
                child: FractionallySizedBox(
                  widthFactor: 1 / options.length,
                  child: Container(decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(8), boxShadow: const [BoxShadow(color: Color(0x24000000), blurRadius: 3, offset: Offset(0, 1))])),
                ),
              ),
            ),
          Row(children: [
            for (var i = 0; i < options.length; i++)
              Expanded(
                child: GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTap: onSelect == null ? null : () => onSelect!(i),
                  child: Container(
                    height: 32,
                    alignment: Alignment.center,
                    child: Text(options[i], maxLines: 1, overflow: TextOverflow.ellipsis, style: ink(13, FontWeight.w700, onSelect == null ? cTag : cAccent)),
                  ),
                ),
              ),
          ]),
        ]),
      );
}

/// − value + on field-grey squares.
class StepperControl extends StatelessWidget {
  const StepperControl(this.value, {super.key, this.onMinus, this.onPlus});
  final String value;
  final VoidCallback? onMinus, onPlus;
  @override
  Widget build(BuildContext context) {
    Widget sq(String sign, VoidCallback? f) => Tap(
          onTap: f,
          minSize: 36,
          child: Container(
            width: 32,
            height: 32,
            decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(9)),
            child: CustomPaint(painter: _SignPainter(sign == '+', f == null ? cTag : cAccent)),
          ),
        );
    return Row(mainAxisSize: MainAxisSize.min, children: [
      sq('-', onMinus),
      SizedBox(width: 58, child: Text(value, maxLines: 1, textAlign: TextAlign.center, style: ink(15, FontWeight.w700).copyWith(fontFeatures: const [FontFeature.tabularFigures()]))),
      sq('+', onPlus),
    ]);
  }
}

class _SignPainter extends CustomPainter {
  _SignPainter(this.plus, this.color);
  final bool plus;
  final Color color;
  @override
  void paint(Canvas canvas, Size s) {
    final p = Paint()
      ..color = color
      ..strokeWidth = 2.2
      ..strokeCap = StrokeCap.round;
    final c = s.center(Offset.zero);
    canvas.drawLine(c - const Offset(6, 0), c + const Offset(6, 0), p);
    if (plus) canvas.drawLine(c - const Offset(0, 6), c + const Offset(0, 6), p);
  }

  @override
  bool shouldRepaint(_SignPainter o) => o.plus != plus || o.color != color;
}

/// A stepper row.
class StepRow extends StatelessWidget {
  const StepRow(this.label, this.value, {super.key, this.onMinus, this.onPlus, this.changed = false});
  final String label, value;
  final VoidCallback? onMinus, onPlus;
  final bool changed;
  @override
  Widget build(BuildContext context) => PropRow(label, changed: changed, trailing: StepperControl(value, onMinus: onMinus, onPlus: onPlus));
}

/// A row of colour swatches; the last one opens the full picker.
class SwatchRow extends StatelessWidget {
  const SwatchRow(this.colors, this.current, {super.key, this.onPick, this.onMore, this.moreOn = false});
  final List<Color> colors;
  final Color? current;
  final ValueChanged<Color>? onPick;
  final VoidCallback? onMore;
  /// The colour is none of the presets: the rainbow "more" is ringed.
  final bool moreOn;
  @override
  Widget build(BuildContext context) => Wrap(spacing: 6, runSpacing: 6, children: [
        for (final c in colors)
          GestureDetector(
            onTap: onPick == null ? null : () => onPick!(c),
            child: Container(
              width: 22,
              height: 22,
              decoration: BoxDecoration(
                color: c,
                shape: BoxShape.circle,
                boxShadow: current != null && c.toARGB32() == current!.toARGB32()
                    ? const [BoxShadow(color: cCard, spreadRadius: 2), BoxShadow(color: cAccent, spreadRadius: 4)]
                    : const [BoxShadow(color: Color(0x2E000000), spreadRadius: 1)],
              ),
            ),
          ),
        if (onMore != null)
          GestureDetector(
            onTap: onMore,
            child: Container(
              width: 22,
              height: 22,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                gradient: const SweepGradient(colors: [Color(0xFFE5484D), Color(0xFFF2A93B), Color(0xFFFFD54A), Color(0xFF1E9E55), Color(0xFF2D63E0), Color(0xFFA534D0), Color(0xFFE5484D)]),
                boxShadow: moreOn ? const [BoxShadow(color: cCard, spreadRadius: 2), BoxShadow(color: cAccent, spreadRadius: 4)] : null,
              ),
            ),
          ),
      ]);
}

/// A slider: blue fill, white knob. `value` is 0..1.
class SliderBar extends StatelessWidget {
  const SliderBar(this.value, {super.key, this.onChanged});
  final double value;
  final ValueChanged<double>? onChanged;
  @override
  Widget build(BuildContext context) => LayoutBuilder(builder: (context, c) {
        void at(Offset p) => onChanged?.call((p.dx / c.maxWidth).clamp(0.0, 1.0));
        return GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (d) => at(d.localPosition),
          onHorizontalDragUpdate: (d) => at(d.localPosition),
          child: SizedBox(height: 28, child: CustomPaint(size: Size(c.maxWidth, 28), painter: _SliderPainter(value.clamp(0.0, 1.0)))),
        );
      });
}

class _SliderPainter extends CustomPainter {
  _SliderPainter(this.v);
  final double v;
  @override
  void paint(Canvas canvas, Size s) {
    const r = 9.0;
    final y = s.height / 2, x0 = r, x1 = s.width - r, x = x0 + (x1 - x0) * v;
    canvas.drawRRect(RRect.fromLTRBR(x0, y - 2, x1, y + 2, const Radius.circular(2)), Paint()..color = const Color(0xFFE1E4E8));
    canvas.drawRRect(RRect.fromLTRBR(x0, y - 2, x, y + 2, const Radius.circular(2)), Paint()..color = cAccent);
    canvas.drawCircle(Offset(x, y + 1), r, Paint()..color = const Color(0x2E000000)..maskFilter = const MaskFilter.blur(BlurStyle.normal, 2));
    canvas.drawCircle(Offset(x, y), r, Paint()..color = cCard);
    canvas.drawCircle(Offset(x, y), r, Paint()..color = const Color(0xFFC9CED5)..style = PaintingStyle.stroke..strokeWidth = 1);
  }

  @override
  bool shouldRepaint(_SliderPainter o) => o.v != v;
}

/// A slider row: label and value above the bar.
class SliderRow extends StatelessWidget {
  const SliderRow(this.label, this.valueText, this.value, {super.key, this.onChanged});
  final String label, valueText;
  final double value;
  final ValueChanged<double>? onChanged;
  @override
  Widget build(BuildContext context) => Column(children: [
        PropRow(label, height: 30, trailing: Text(valueText, style: ink(14, FontWeight.w700))),
        SliderBar(value, onChanged: onChanged),
        const SizedBox(height: 4),
      ]);
}

/// A row that opens something else: label, value and a chevron.
class LinkRow extends StatelessWidget {
  const LinkRow(this.label, this.value, {super.key, this.onTap});
  final String label, value;
  final VoidCallback? onTap;
  @override
  Widget build(BuildContext context) => GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: onTap,
        child: PropRow(label, trailing: Row(mainAxisSize: MainAxisSize.min, children: [
          Text(value, style: ink(14, FontWeight.w700)),
          const SizedBox(width: 4),
          const Pictogram('next', size: 14, color: cSecondary),
        ])),
      );
}

/// Red, centred section title, optionally with a + on the right.
class SectionTitle extends StatelessWidget {
  const SectionTitle(this.text, {super.key, this.onAdd, this.addLabel});
  final String text;
  final VoidCallback? onAdd;
  final String? addLabel;
  @override
  Widget build(BuildContext context) => SizedBox(
        height: 36,
        child: Stack(alignment: Alignment.center, children: [
          Text(text, textAlign: TextAlign.center, style: ink(15, FontWeight.w700, cTitle)),
          if (onAdd != null) Positioned(right: -6, child: Tap(label: addLabel ?? 'Add', onTap: onAdd, child: const Pictogram('plus', size: 20, color: cAccent, stroke: 2.2))),
        ]),
      );
}

/// Quiet explanatory text.
class Hint extends StatelessWidget {
  const Hint(this.text, {super.key});
  final String text;
  @override
  Widget build(BuildContext context) => Padding(padding: const EdgeInsets.only(top: 2, bottom: 8), child: Text(text, style: ink(12, FontWeight.w400, cSecondary).copyWith(height: 1.4)));
}

/// Two word buttons side by side (or one, full width).
class ButtonPair extends StatelessWidget {
  const ButtonPair(this.a, this.onA, [this.b, this.onB, this.primaryB = false]) : super(key: null);
  final String a;
  final VoidCallback? onA;
  final String? b;
  final VoidCallback? onB;
  final bool primaryB;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(top: 2, bottom: 6),
        // touches in the padding reach the buttons
        child: HitPad(
          dy: 6,
          child: Row(children: [
            // the boards' word buttons: 36 tall, 14 pt
            Expanded(child: WordButton(a, onTap: onA, filled: true, expand: true, height: 36, size: 14)),
            if (b != null) ...[const SizedBox(width: 8), Expanded(child: WordButton(b!, onTap: onB, filled: !primaryB, primary: primaryB, expand: true, height: 36, size: 14))],
          ]),
        ),
      );
}

/// A floating card: panels, popovers, the View card.
class PanelCard extends StatelessWidget {
  const PanelCard({super.key, required this.child, this.width, this.height, this.padding = const EdgeInsets.fromLTRB(14, 10, 14, 10), this.docked = false});
  final Widget child;
  final double? width, height;
  final EdgeInsets padding;
  final bool docked;
  @override
  Widget build(BuildContext context) => Container(
        width: width,
        height: height,
        padding: padding,
        clipBehavior: Clip.antiAlias,
        decoration: docked
            ? const BoxDecoration(color: cCard, border: Border(left: BorderSide(color: cLine)))
            : BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(16), boxShadow: _shadowCard),
        child: Material(type: MaterialType.transparency, child: DefaultTextStyle(style: ink(), child: child)),
      );
}

/// A popover card by its anchor (a rect on screen, e.g. the button that
/// opened it): on the side with more room (above unless `below`), never past
/// the screen's edges or its safe area, scrolling when taller than the room.
/// Use it as a child of the popover's Stack. `card` gets the width to use.
class Anchored extends StatelessWidget {
  const Anchored({super.key, required this.anchor, required this.width, required this.card, this.alignRight = false, this.below = false, this.gap = 6, this.topMin = 0});
  final Rect anchor;
  final double width, gap, topMin;
  final bool alignRight, below;
  final Widget Function(double width) card;

  /// Where the widget with this key is, in the popover's own space (the
  /// root overlay's: Settings › UI size scales everything inside it, so
  /// window pixels would put the popover off its button). Null: not laid out.
  static Rect? rectOf(GlobalKey key) {
    final ctx = key.currentContext;
    final box = ctx?.findRenderObject() as RenderBox?;
    if (ctx == null || box == null || !box.hasSize) return null;
    final overlay = Overlay.maybeOf(ctx, rootOverlay: true)?.context.findRenderObject() as RenderBox?;
    return box.localToGlobal(Offset.zero, ancestor: overlay) & box.size;
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final screen = mq.size, pad = mq.padding;
    const margin = 8.0;
    final top = math.max(pad.top + margin, topMin);
    final bottom = screen.height - pad.bottom - margin;
    final roomAbove = anchor.top - gap - top, roomBelow = bottom - anchor.bottom - gap;
    final tightBoth = roomAbove < 235 && roomBelow < 200 && bottom - top > math.max(roomAbove, roomBelow) + 50;
    final goAbove = below ? roomBelow < 160 && roomAbove > roomBelow : roomAbove >= 120 || roomAbove >= roomBelow;
    final room = math.max(96.0, tightBoth ? bottom - top : (goAbove ? roomAbove : roomBelow));
    final w = math.min(width, screen.width - pad.left - pad.right - 2 * margin);
    final left = (alignRight ? anchor.right - w : anchor.left).clamp(pad.left + margin, math.max(pad.left + margin, screen.width - pad.right - margin - w)).toDouble();
    Widget body = ConstrainedBox(
      constraints: BoxConstraints(maxHeight: room, maxWidth: w),
      child: _AnchoredScroll(child: card(w)),
    );
    // it grows out of what opened it (the popover's route animates it)
    final a = ModalRoute.of(context)?.animation;
    if (a != null) {
      final c = CurvedAnimation(parent: a, curve: kEase, reverseCurve: Curves.easeInCubic);
      final ax = ((anchor.center.dx - left) / w * 2 - 1).clamp(-1.0, 1.0);
      body = FadeTransition(
        opacity: c,
        child: ScaleTransition(scale: Tween(begin: 0.94, end: 1.0).animate(c), alignment: Alignment(ax, tightBoth ? 0 : (goAbove ? 1 : -1)), child: body),
      );
    }
    return Positioned(
      left: left,
      top: tightBoth ? top : (goAbove ? null : math.min(anchor.bottom + gap, bottom - 96)),
      bottom: tightBoth ? null : (goAbove ? screen.height - math.max(anchor.top - gap, top + 96) : null),
      child: body,
    );
  }
}

/// Scrollable popover body with a subtle bottom cue when more rows sit below the fold.
class _AnchoredScroll extends StatefulWidget {
  const _AnchoredScroll({required this.child});
  final Widget child;
  @override
  State<_AnchoredScroll> createState() => _AnchoredScrollState();
}

class _AnchoredScrollState extends State<_AnchoredScroll> {
  final ScrollController _sc = ScrollController();
  bool _moreBelow = false;

  @override
  void initState() {
    super.initState();
    _sc.addListener(_sync);
    WidgetsBinding.instance.addPostFrameCallback((_) => _sync());
  }

  void _sync() {
    if (!mounted || !_sc.hasClients) return;
    final more = _sc.position.maxScrollExtent > 4 && _sc.offset < _sc.position.maxScrollExtent - 4;
    if (more != _moreBelow) setState(() => _moreBelow = more);
  }

  @override
  void dispose() {
    _sc.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return NotificationListener<ScrollMetricsNotification>(
      onNotification: (_) {
        _sync();
        return false;
      },
      child: Stack(children: [
        ScrollConfiguration(
          behavior: const _AnyPointerScroll(),
          child: RawScrollbar(
            controller: _sc,
            thumbVisibility: _moreBelow,
            thickness: 3.5,
            radius: const Radius.circular(2),
            thumbColor: const Color(0x666B7078),
            child: SingleChildScrollView(controller: _sc, child: widget.child),
          ),
        ),
        if (_moreBelow)
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            height: 24,
            child: IgnorePointer(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  borderRadius: const BorderRadius.vertical(bottom: Radius.circular(16)),
                  gradient: LinearGradient(begin: Alignment.topCenter, end: Alignment.bottomCenter, colors: [cCard.withValues(alpha: 0), cCard.withValues(alpha: 0.95)]),
                ),
                child: const Align(alignment: Alignment.bottomCenter, child: Padding(padding: EdgeInsets.only(bottom: 2), child: Pictogram('chevron', size: 12, color: cSecondary))),
              ),
            ),
          ),
      ]),
    );
  }
}

/// Yellow hint bubble, the original's friendly voice.
class HintBubble extends StatelessWidget {
  const HintBubble(this.text, {super.key, this.width});
  final String text;
  final double? width;
  @override
  Widget build(BuildContext context) => Container(
        width: width,
        padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 8),
        decoration: BoxDecoration(color: cHint, borderRadius: BorderRadius.circular(10), boxShadow: const [BoxShadow(color: Color(0x2E000000), blurRadius: 6, offset: Offset(0, 2))]),
        child: Text(text, style: ink(13).copyWith(height: 1.35)),
      );
}

/// A centred sheet over a dimmed screen: red title, rows, buttons.
Future<T?> showSheet<T>(BuildContext context, {required String title, required List<Widget> Function(BuildContext) children, double width = 440}) {
  return showGeneralDialog<T>(
    context: context,
    barrierDismissible: true,
    barrierLabel: title,
    barrierColor: const Color(0x6114181E),
    transitionDuration: motion(context, 160),
    // it settles in from a touch smaller (D37)
    transitionBuilder: (c, a, _, child) {
      final e = CurvedAnimation(parent: a, curve: kEase, reverseCurve: Curves.easeInCubic);
      return FadeTransition(opacity: e, child: ScaleTransition(scale: Tween(begin: 0.96, end: 1.0).animate(e), child: child));
    },
    pageBuilder: (c, a1, a2) => Center(
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: width, maxHeight: MediaQuery.sizeOf(c).height - 24),
        child: Container(
          padding: const EdgeInsets.fromLTRB(20, 14, 20, 16),
          decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(18), boxShadow: const [BoxShadow(color: Color(0x59000000), blurRadius: 48, offset: Offset(0, 18))]),
          child: Material(
            type: MaterialType.transparency,
            child: DefaultTextStyle(
              style: ink(),
              child: SingleChildScrollView(
                child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                  Padding(padding: const EdgeInsets.only(bottom: 6), child: Text(title, textAlign: TextAlign.center, style: ink(17, FontWeight.w700, cTitle))),
                  ...children(c),
                ]),
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

/// A short message at the bottom of the screen. Dark for errors and
/// warnings, light for undoable events (D19).
VoidCallback? _closeToast;

void showToast(BuildContext context, String text, {String? action, VoidCallback? onAction, bool light = false, String? icon, Color? iconColor, Duration duration = const Duration(seconds: 4)}) {
  final overlay = Overlay.maybeOf(context, rootOverlay: true);
  if (overlay == null) return;
  _closeToast?.call(); // one toast at a time: the newest replaces the last
  late OverlayEntry entry;
  var gone = false;
  void close() {
    if (gone) return;
    gone = true;
    entry.remove();
    if (_closeToast == close) _closeToast = null;
  }

  _closeToast = close;

  entry = OverlayEntry(
    builder: (c) => Positioned(
      left: 0,
      right: 0,
      bottom: 24 + MediaQuery.paddingOf(c).bottom,
      child: Center(
        // it rises into place
        child: TweenAnimationBuilder<double>(
          tween: Tween(begin: 0, end: 1),
          duration: motion(c, 200),
          curve: kEase,
          builder: (c, t, child) => Opacity(opacity: t, child: Transform.translate(offset: Offset(0, 14 * (1 - t)), child: child)),
          child: Container(
          constraints: const BoxConstraints(maxWidth: 560),
          padding: EdgeInsets.fromLTRB(16, 6, action == null ? 16 : 6, 6),
          decoration: BoxDecoration(
            color: light ? cCard : const Color(0xFF2A2D33),
            borderRadius: BorderRadius.circular(12),
            boxShadow: const [BoxShadow(color: Color(0x40000000), blurRadius: 18, offset: Offset(0, 6))],
          ),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            if (icon != null) Padding(padding: const EdgeInsets.only(right: 8), child: Pictogram(icon, size: 16, color: iconColor ?? (light ? cInk2b : cCard))),
            Flexible(child: Padding(padding: const EdgeInsets.symmetric(vertical: 8), child: Text(text, style: ink(14, FontWeight.w400, light ? cText : cCard)))),
            if (action != null)
              Tap(
                onTap: () {
                  close();
                  onAction?.call();
                },
                child: Padding(padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6), child: Text(action, style: ink(14, FontWeight.w700, light ? cAccent : const Color(0xFF9CC8F5)))),
              ),
          ]),
        ),
        ),
      ),
    ),
  );
  overlay.insert(entry);
  Timer(duration, close);
}

/// The checkerboard drawn around the stage.
class CheckerPainter extends CustomPainter {
  const CheckerPainter({this.cell = 16});
  final double cell;
  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(Offset.zero & size, Paint()..color = cSurround);
    final p = Paint()..color = cChecker;
    for (var y = 0.0, row = 0; y < size.height; y += cell / 2, row++) {
      for (var x = (row.isOdd ? cell / 2 : 0.0); x < size.width; x += cell) {
        canvas.drawRect(Rect.fromLTWH(x, y, cell / 2, cell / 2), p);
      }
    }
  }

  @override
  bool shouldRepaint(CheckerPainter o) => o.cell != cell;
}

/// Keeps gestures from a scrollable panel off the stage underneath.
class PanelScroll extends StatelessWidget {
  const PanelScroll({super.key, required this.children, this.padding = EdgeInsets.zero});
  final List<Widget> children;
  final EdgeInsets padding;
  @override
  Widget build(BuildContext context) => ScrollConfiguration(
        behavior: const _AnyPointerScroll(),
        child: ListView(padding: padding, children: children),
      );
}

class _AnyPointerScroll extends ScrollBehavior {
  const _AnyPointerScroll();
  @override
  Set<PointerDeviceKind> get dragDevices => PointerDeviceKind.values.toSet();
}
