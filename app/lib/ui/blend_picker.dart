// Grouped Blend Mode dropdown control (13 pro compositing modes across 5 groups).
import 'package:flutter/widgets.dart';

import 'kit.dart' as kit;
import 'pictograms.dart';
import 'tokens.dart';

class BlendModeEntry {
  const BlendModeEntry(this.id, this.label, this.group);
  final String id;
  final String label;
  final String group;
}

/// All 13 pro blend modes, grouped like Procreate / ToonSquid / Photoshop.
const kBlendModes = <BlendModeEntry>[
  BlendModeEntry('NORMAL', 'Normal', 'Normal'),
  BlendModeEntry('DARKEN', 'Darken', 'Darken'),
  BlendModeEntry('MULTIPLY', 'Multiply', 'Darken'),
  BlendModeEntry('COLOR_BURN', 'Color Burn', 'Darken'),
  BlendModeEntry('LIGHTEN', 'Lighten', 'Lighten'),
  BlendModeEntry('SCREEN', 'Screen', 'Lighten'),
  BlendModeEntry('COLOR_DODGE', 'Color Dodge', 'Lighten'),
  BlendModeEntry('ADD', 'Add', 'Lighten'),
  BlendModeEntry('OVERLAY', 'Overlay', 'Contrast'),
  BlendModeEntry('SOFT_LIGHT', 'Soft Light', 'Contrast'),
  BlendModeEntry('HARD_LIGHT', 'Hard Light', 'Contrast'),
  BlendModeEntry('DIFFERENCE', 'Difference', 'Invert'),
  BlendModeEntry('EXCLUSION', 'Exclusion', 'Invert'),
];

String blendModeLabel(String? raw) {
  final norm = _normalizeBlendId(raw);
  for (final m in kBlendModes) {
    if (m.id == norm) return m.label;
  }
  return 'Normal';
}

String _normalizeBlendId(String? raw) {
  if (raw == null || raw.isEmpty) return 'NORMAL';
  final upper = raw.trim().toUpperCase().replaceAll(' ', '_');
  return switch (upper) {
    'COLORBURN' => 'COLOR_BURN',
    'COLORDODGE' => 'COLOR_DODGE',
    'SOFTLIGHT' => 'SOFT_LIGHT',
    'HARDLIGHT' => 'HARD_LIGHT',
    _ => upper,
  };
}

/// Opens a crisp grouped dropdown anchored to [anchor] and returns the picked
/// wire blend mode ID (e.g. `'MULTIPLY'`, `'SOFT_LIGHT'`), or null if dismissed.
Future<String?> showBlendModeDropdown(
  BuildContext context, {
  required Rect anchor,
  required String current,
}) {
  final active = _normalizeBlendId(current);
  final topSafe = 44.0 + MediaQuery.paddingOf(context).top;
  return showGeneralDialog<String>(
    context: context,
    barrierDismissible: true,
    barrierLabel: 'Blend mode',
    barrierColor: const Color(0x00000000),
    transitionDuration: kit.motion(context, 130),
    pageBuilder: (c, a1, a2) => Stack(children: [
      kit.Anchored(
        anchor: anchor,
        width: 224,
        alignRight: true,
        below: true,
        topMin: topSafe,
        card: (w) => kit.PanelCard(
          width: w,
          padding: const EdgeInsets.fromLTRB(6, 6, 6, 6),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: _buildGroupedRows(c, active),
          ),
        ),
      ),
    ]),
  );
}

List<Widget> _buildGroupedRows(BuildContext c, String active) {
  final out = <Widget>[];
  String? lastGroup;
  for (final m in kBlendModes) {
    if (m.group != lastGroup) {
      if (lastGroup != null) {
        out.add(Container(
          height: 1,
          margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
          color: const Color(0xFFE6E8EB),
        ));
      }
      lastGroup = m.group;
      if (m.group != 'Normal') {
        out.add(Padding(
          padding: const EdgeInsets.fromLTRB(10, 3, 10, 2),
          child: Text(
            m.group.toUpperCase(),
            style: kit.ink(10.5, FontWeight.w700, cSecondary),
          ),
        ));
      }
    }
    final sel = m.id == active;
    out.add(kit.Tap(
      minSize: 0,
      radius: 8,
      onTap: () => Navigator.pop(c, m.id),
      child: Container(
        height: 34,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: sel ? cSelected : null,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(children: [
          Expanded(
            child: Text(
              m.label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: kit.ink(13.5, sel ? FontWeight.w700 : FontWeight.w500, sel ? cAccent : cText),
            ),
          ),
          if (sel) const Pictogram('check', size: 15, color: cAccent, stroke: 2.3),
        ]),
      ),
    ));
  }
  return out;
}

/// Property row for Blend mode: opens a crisp anchored dropdown menu instead of
/// tap-to-cycle.
class BlendModeRow extends StatefulWidget {
  const BlendModeRow({
    super.key,
    required this.current,
    required this.onChanged,
    this.label = 'Blend',
  });

  final String current;
  final ValueChanged<String> onChanged;
  final String label;

  @override
  State<BlendModeRow> createState() => _BlendModeRowState();
}

class _BlendModeRowState extends State<BlendModeRow> {
  final GlobalKey _pillKey = GlobalKey();

  Future<void> _open() async {
    final anchor = kit.Anchored.rectOf(_pillKey) ?? const Rect.fromLTWH(200, 200, 120, 32);
    final picked = await showBlendModeDropdown(context, anchor: anchor, current: widget.current);
    if (picked != null && mounted) widget.onChanged(picked);
  }

  @override
  Widget build(BuildContext context) {
    final activeLabel = blendModeLabel(widget.current);
    final nonNormal = _normalizeBlendId(widget.current) != 'NORMAL';
    return kit.PropRow(
      widget.label,
      trailing: kit.Tap(
        key: _pillKey,
        label: '${widget.label}: $activeLabel',
        minSize: 36,
        radius: 9,
        onTap: _open,
        child: Container(
          height: 30,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: nonNormal ? cSelected : cControl,
            borderRadius: BorderRadius.circular(9),
            border: nonNormal ? Border.all(color: cAccent.withValues(alpha: 0.35)) : null,
          ),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            Pictogram('blend', size: 14, color: nonNormal ? cAccent : cInk2b, stroke: 1.9),
            const SizedBox(width: 6),
            Text(
              activeLabel,
              style: kit.ink(13.5, FontWeight.w600, nonNormal ? cAccent : cText),
            ),
            const SizedBox(width: 4),
            Pictogram('caretDown', size: 13, color: nonNormal ? cAccent : cSecondary, stroke: 2.1),
          ]),
        ),
      ),
    );
  }
}
