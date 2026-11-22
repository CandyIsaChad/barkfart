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

/// Any widget, for rows not (yet) described by data.
class DWidget extends Desc {
  const DWidget(this.child);
  final Widget child;
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
    DInfo() => PropRow(d.label, trailing: Text(d.value, style: ink(13, FontWeight.w400, cSecondary))),
    DName() => PropRow(d.name, trailing: d.link == null ? null : WordButton(d.link!, onTap: d.onLink, size: 13)),
    DDanger() => Padding(padding: const EdgeInsets.only(top: 8), child: WordButton(d.label, onTap: d.onTap, filled: true, danger: true, expand: true, height: 38)),
    DWidget() => d.child,
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
  const SwipeActions({super.key, required this.child, required this.actions, this.initiallyOpen = false});
  final Widget child;
  final List<SwipeAction> actions;
  final bool initiallyOpen;
  @override
  State<SwipeActions> createState() => _SwipeActionsState();
}

class _SwipeActionsState extends State<SwipeActions> {
  static const _w = 112.0;
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
                    child: Text(a.label, style: ink(13, FontWeight.w700, a.danger ? cCard : cAccent)),
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
