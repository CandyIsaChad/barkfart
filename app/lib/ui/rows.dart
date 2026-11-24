// Pages as data (REDESIGN.md D15): a page is a list of row descriptors, and
// one renderer draws every page (Stickfigure, Segment, Pose, Frame, Camera,
// Text, Sound, View, Export, Settings, ...). Built-in tools and plugins use
// the same descriptors.
import 'package:flutter/widgets.dart';

import 'kit.dart';
import 'tokens.dart';

/// One row of a page.
sealed class Desc {
  const Desc();
}

/// A red, centred section title, optionally with a + button.
class DTitle extends Desc {
  const DTitle(this.text, {this.onAdd, this.addLabel});
  final String text;
  final VoidCallback? onAdd;
  final String? addLabel;
}

/// A reset offered by swiping a row: "Reset to 32".
class DReset {
  const DReset(this.label, this.onReset);
  final String label;
  final VoidCallback onReset;
}

/// A tick box. `value == null` shows the mixed dash (D18).
class DTick extends Desc {
  const DTick(this.label, this.value, {this.onChanged, this.changed = false, this.reset});
  final String label;
  final bool? value;
  final ValueChanged<bool>? onChanged;
  final bool changed;
  final DReset? reset;
}

/// − value +.
class DStep extends Desc {
  const DStep(this.label, this.value, {this.onMinus, this.onPlus, this.changed = false, this.reset});
  final String label, value;
  final VoidCallback? onMinus, onPlus;
  final bool changed;
  final DReset? reset;
}

/// Words in equal parts, one chosen; an optional label above.
class DSeg extends Desc {
  const DSeg(this.options, this.selected, {this.label, this.onSelect});
  final String? label;
  final List<String> options;
  final int selected;
  final ValueChanged<int>? onSelect;
}

/// One or two word buttons.
class DButtons extends Desc {
  const DButtons(this.a, this.onA, {this.b, this.onB});
  final String a;
  final VoidCallback? onA;
  final String? b;
  final VoidCallback? onB;
}

/// A row of swatches. `mixed` shows "Mixed" next to the label.
class DSwatch extends Desc {
  const DSwatch(this.label, this.colors, this.current, {this.onPick, this.onMore, this.mixed = false, this.changed = false, this.reset});
  final String label;
  final List<Color> colors;
  final Color? current;
  final ValueChanged<Color>? onPick;
  final VoidCallback? onMore;
  final bool mixed, changed;
  final DReset? reset;
}

/// Label and value above a slider (0..1).
class DSlider extends Desc {
  const DSlider(this.label, this.valueText, this.value, {this.onChanged});
  final String label, valueText;
  final double value;
  final ValueChanged<double>? onChanged;
}

/// Label, value and a chevron: opens something.
class DLink extends Desc {
  const DLink(this.label, this.value, {this.onTap});
  final String label, value;
  final VoidCallback? onTap;
}

/// Quiet explanatory text.
class DHint extends Desc {
  const DHint(this.text);
  final String text;
}

/// Label and a read-only value.
class DInfo extends Desc {
  const DInfo(this.label, this.value);
  final String label, value;
}

/// A name in bold, with an optional word link on the right.
class DName extends Desc {
  const DName(this.name, {this.link, this.onLink});
  final String name;
  final String? link;
  final VoidCallback? onLink;
}

/// A red word button (destructive), full width.
class DDanger extends Desc {
  const DDanger(this.label, this.onTap);
  final String label;
  final VoidCallback? onTap;
}

/// An effect the layer has (round 10 board): a pale card with its name, a
/// small "animated" tag when part of it changes over time, and an eye.
class DFx extends Desc {
  const DFx(this.name, {this.tag, this.note, this.missing = false, this.shown = true, this.open = false, this.onTap, this.onEye, this.onRemove});
  final String name;
  /// What of it is animated (with the key diamond).
  final String? tag;
  /// A plain line beside the name ("by yeah").
  final String? note;
  /// A plugin that isn't installed (the States board's orange row).
  final bool missing;
  final bool shown, open;
  final VoidCallback? onTap, onEye, onRemove;
}

/// The Pose page's ease: four small curves, one chosen.
class DEase extends Desc {
  const DEase(this.selected, {this.onSelect, this.enabled = true});
  final int selected;
  final ValueChanged<int>? onSelect;
  final bool enabled;
  /// D34: Auto (the curve through the poses, the default) first, then the
  /// round 10 board's four
  static const names = ['Auto', 'Even', 'Slow end', 'Slow both ends', 'Bounce'];
}

/// Any widget, for rows not (yet) described by data. It can carry a
/// changed-dot and a swipe reset like the other rows.
class DWidget extends Desc {
  const DWidget(this.child, {this.changed = false, this.reset});
  final Widget child;
  final bool changed;
  final DReset? reset;
}

/// Draws one descriptor.
Widget renderDesc(Desc d) {
  Widget swipe(Widget row, DReset? r) => r == null ? row : SwipeActions(actions: [SwipeAction(r.label, r.onReset)], child: row);
  return switch (d) {
    DTitle() => SectionTitle(d.text, onAdd: d.onAdd, addLabel: d.addLabel),
    DTick() => swipe(TickRow(d.label, d.value, onChanged: d.onChanged, changed: d.changed), d.reset),
    DStep() => swipe(StepRow(d.label, d.value, onMinus: d.onMinus, onPlus: d.onPlus, changed: d.changed), d.reset),
    DSeg() => Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        if (d.label != null) PropRow(d.label!, height: 26),
        Segmented(d.options, d.selected, onSelect: d.onSelect),
      ]),
    DButtons() => ButtonPair(d.a, d.onA, d.b, d.onB),
    DSwatch() => swipe(
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: Row(children: [
            if (d.changed) Container(width: 6, height: 6, margin: const EdgeInsets.only(right: 6), decoration: const BoxDecoration(color: cAccent, shape: BoxShape.circle)),
            Text(d.label, style: ink(14)),
            if (d.mixed) Padding(padding: const EdgeInsets.only(left: 6), child: Text('Mixed', style: ink(11, FontWeight.w400, cSecondary))),
            const SizedBox(width: 10),
            Expanded(child: Align(alignment: Alignment.centerRight, child: SwatchRow(d.colors, d.current, onPick: d.onPick, onMore: d.onMore))),
          ]),
        ),
        d.reset),
    DSlider() => SliderRow(d.label, d.valueText, d.value, onChanged: d.onChanged),
    DLink() => LinkRow(d.label, d.value, onTap: d.onTap),
    DHint() => Hint(d.text),
    // a long value (a graphics card's name) gives way, never the label
    DInfo() => PropRow(d.label, trailing: Flexible(fit: FlexFit.tight, child: Text(d.value, maxLines: 1, overflow: TextOverflow.ellipsis, textAlign: TextAlign.right, style: ink(13, FontWeight.w400, cSecondary)))),
    DName() => SizedBox(
        height: 40,
        child: Row(children: [
          Expanded(child: Text(d.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: ink(15, FontWeight.w700))),
          if (d.link != null) WordButton(d.link!, onTap: d.onLink, size: 13),
        ]),
      ),
    DDanger() => Padding(padding: const EdgeInsets.only(top: 8), child: WordButton(d.label, onTap: d.onTap, filled: true, danger: true, expand: true, height: 38)),
    DFx() => Padding(
        padding: const EdgeInsets.symmetric(vertical: 3),
        child: SwipeActions(
          actions: [if (d.onRemove != null) SwipeAction('Remove', d.onRemove!, danger: true)],
          child: Container(
            height: 40,
            decoration: BoxDecoration(color: d.missing ? const Color(0xFFFFF4E5) : d.open ? cSelected : cControl, borderRadius: BorderRadius.circular(10)),
            padding: const EdgeInsets.only(left: 12, right: 2),
            child: Row(children: [
              Expanded(
                child: Tap(
                  label: d.open ? 'Close ${d.name} settings' : '${d.name} settings',
                  onTap: d.onTap,
                  minSize: 0,
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: Row(children: [
                      Text(d.name, style: ink(14, FontWeight.w600, d.shown ? cText : cSecondary)),
                      if (d.note != null) ...[
                        const SizedBox(width: 8),
                        Flexible(child: Text(d.note!, maxLines: 1, overflow: TextOverflow.ellipsis, style: ink(12, FontWeight.w400, cSecondary))),
                      ],
                      if (d.tag != null) ...[
                        const SizedBox(width: 10),
                        Transform.rotate(angle: 0.785398, child: Container(width: 7, height: 7, decoration: BoxDecoration(border: Border.all(color: cAccent, width: 1.6), borderRadius: BorderRadius.circular(1)))),
                        const SizedBox(width: 5),
                        Flexible(child: Text(d.tag!, maxLines: 1, overflow: TextOverflow.ellipsis, style: ink(12, FontWeight.w400, cSecondary))),
                      ],
                    ]),
                  ),
                ),
              ),
              Tap(label: d.shown ? 'Hide ${d.name}' : 'Show ${d.name}', onTap: d.onEye, minSize: 34, child: _Eye(d.shown)),
            ]),
          ),
        ),
      ),
    DEase() => SizedBox(
        height: 46,
        child: Row(children: [
          Expanded(child: Text('Ease', style: ink(14))),
          for (var i = 0; i < DEase.names.length; i++)
            Padding(
              padding: const EdgeInsets.only(left: 6),
              child: Tap(
                label: DEase.names[i],
                onTap: d.enabled && d.onSelect != null ? () => d.onSelect!(i) : null,
                minSize: 0,
                child: Opacity(
                  opacity: d.enabled ? 1 : 0.4,
                  child: Container(
                    width: 38,
                    height: 36,
                    decoration: BoxDecoration(
                      color: i == d.selected ? cCard : cControl,
                      borderRadius: BorderRadius.circular(9),
                      border: i == d.selected ? Border.all(color: cAccent, width: 2) : null,
                    ),
                    child: CustomPaint(painter: _EasePainter(i, i == d.selected)),
                  ),
                ),
              ),
            ),
        ]),
      ),
    DWidget() => swipe(
        d.changed
            ? Row(children: [
                Container(width: 6, height: 6, margin: const EdgeInsets.only(right: 6), decoration: const BoxDecoration(color: cAccent, shape: BoxShape.circle)),
                Expanded(child: d.child),
              ])
            : d.child,
        d.reset),
  };
}

/// A page: optional tabs, the rows (scrolling), an optional pinned red
/// button at the bottom.
class PropertyPage extends StatelessWidget {
  const PropertyPage(this.rows, {super.key, this.tabs, this.tab = 0, this.onTab, this.footer, this.width, this.height, this.docked = false});
  final List<Desc> rows;
  final List<String>? tabs;
  final int tab;
  final ValueChanged<int>? onTab;
  final DDanger? footer;
  final double? width, height;
  final bool docked;

  @override
  Widget build(BuildContext context) => PanelCard(
        width: width,
        height: height,
        docked: docked,
        padding: EdgeInsets.zero,
        child: Column(children: [
          Expanded(
            child: PanelScroll(padding: const EdgeInsets.fromLTRB(14, 10, 14, 16), children: [
              if (tabs != null) Segmented(tabs!, tab, onSelect: onTab),
              for (final d in rows) renderDesc(d),
            ]),
          ),
          if (footer != null)
            Container(
              decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
              padding: const EdgeInsets.fromLTRB(14, 8, 14, 8),
              child: WordButton(footer!.label, onTap: footer!.onTap, filled: true, danger: true, expand: true, height: 38),
            ),
        ]),
      );
}

/// A row that slides aside to show actions (Duplicate, Delete, Reset to N).
/// The same gesture everywhere: layers, effects, keys, properties.
class SwipeAction {
  const SwipeAction(this.label, this.onTap, {this.danger = false});
  final String label;
  final VoidCallback onTap;
  final bool danger;
}

class SwipeActions extends StatefulWidget {
  const SwipeActions({super.key, required this.child, required this.actions, this.initiallyOpen = false, this.actionWidth = 112});
  final Widget child;
  final List<SwipeAction> actions;
  final bool initiallyOpen;
  /// Each action's width (narrow places, like the timeline's names column).
  final double actionWidth;
  @override
  State<SwipeActions> createState() => _SwipeActionsState();
}

class _SwipeActionsState extends State<SwipeActions> {
  double get _w => widget.actionWidth;
  late double _dx = widget.initiallyOpen ? -_w * widget.actions.length : 0;

  double get _max => _w * widget.actions.length;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onHorizontalDragUpdate: (d) => setState(() => _dx = (_dx + d.delta.dx).clamp(-_max, 0)),
      onHorizontalDragEnd: (_) => setState(() => _dx = _dx < -_max / 2 ? -_max : 0),
      onSecondaryTap: () => setState(() => _dx = _dx == 0 ? -_max : 0), // right-click on Linux
      child: ClipRect(
        child: Stack(children: [
          Positioned.fill(
            child: Row(mainAxisAlignment: MainAxisAlignment.end, children: [
              for (final a in widget.actions)
                GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onTap: () {
                    setState(() => _dx = 0);
                    a.onTap();
                  },
                  child: Container(
                    width: _w,
                    alignment: Alignment.center,
                    color: a.danger ? const Color(0xFFE5484D) : cControl,
                    child: Text(a.label, maxLines: 1, overflow: TextOverflow.fade, softWrap: false, style: ink(_w < 90 ? 12 : 13, FontWeight.w700, a.danger ? cCard : cAccent)),
                  ),
                ),
            ]),
          ),
          AnimatedContainer(
            duration: const Duration(milliseconds: 120),
            transform: Matrix4.translationValues(_dx, 0, 0),
            color: cCard,
            child: widget.child,
          ),
        ]),
      ),
    );
  }
}

class _Eye extends StatelessWidget {
  const _Eye(this.open);
  final bool open;
  @override
  Widget build(BuildContext context) => CustomPaint(size: const Size(18, 18), painter: _EyePainter(open ? const Color(0xFF9AA0A8) : const Color(0xFFD5D8DD)));
}

class _EyePainter extends CustomPainter {
  _EyePainter(this.c);
  final Color c;
  @override
  void paint(Canvas canvas, Size size) {
    final k = size.width / 24;
    canvas.scale(k);
    final pen = Paint()
      ..color = c
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.8;
    final p = Path()
      ..moveTo(2.5, 12)
      ..quadraticBezierTo(12, 2, 21.5, 12)
      ..quadraticBezierTo(12, 22, 2.5, 12);
    canvas.drawPath(p, pen);
    canvas.drawCircle(const Offset(12, 12), 2.8, pen);
  }

  @override
  bool shouldRepaint(_EyePainter o) => o.c != c;
}

/// The board's ease curves (viewBox 0 0 32 32, drawn at 24 in a 38 x 36 cell).
class _EasePainter extends CustomPainter {
  _EasePainter(this.i, this.on);
  final int i;
  final bool on;
  @override
  void paint(Canvas canvas, Size size) {
    canvas.translate((size.width - 24) / 2, (size.height - 24) / 2);
    canvas.scale(24 / 32);
    final p = Path()..moveTo(5, 27);
    final colour = on ? cAccent : cSecondary;
    switch (i) {
      case 0:
        // Auto: a curve that flows through the middle pose without turning on it
        p
          ..cubicTo(10, 27, 12, 18, 16, 16)
          ..cubicTo(20, 14, 22, 5, 27, 5);
        for (final o in const [Offset(5, 27), Offset(16, 16), Offset(27, 5)]) {
          canvas.drawCircle(o, 2.4, Paint()..color = colour);
        }
      case 1:
        p.lineTo(27, 5);
      case 2:
        p.cubicTo(10, 12, 16, 5, 27, 5);
      case 3:
        p.cubicTo(17, 27, 15, 5, 27, 5);
      default:
        p
          ..cubicTo(8, 14, 12, 6, 17, 5)
          ..cubicTo(18.5, 9, 19.5, 9, 21, 5)
          ..cubicTo(22, 7, 22.8, 7, 23.6, 5)
          ..lineTo(27, 5);
    }
    canvas.drawPath(
        p,
        Paint()
          ..color = colour
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2.2
          ..strokeCap = StrokeCap.round
          ..strokeJoin = StrokeJoin.round);
  }

  @override
  bool shouldRepaint(_EasePainter o) => o.i != i || o.on != on;
}
