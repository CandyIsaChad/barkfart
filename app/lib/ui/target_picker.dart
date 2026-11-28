// Searchable, filterable Target Picker for Modifiers, Scripts, and Curves.
import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

import 'kit.dart' as kit;
import 'pictograms.dart';
import 'tokens.dart';

/// One selectable target (a figure or an animatable property).
class TargetOption {
  const TargetOption({
    required this.id,
    required this.label,
    required this.group,
    this.layerKind = 'figure',
    this.category = '',
    this.valueText,
    this.animated = false,
    this.modifierCount = 0,
  });

  final String id;
  final String label;
  final String group;
  /// Layer kind for pictogram & category chip ('figure', 'camera', 'adjustment', 'picture', 'text', 'movieclip', 'group', 'joint').
  final String layerKind;
  final String category;
  final String? valueText;
  final bool animated;
  final int modifierCount;

  /// Parses engine JSON maps from `sc['figures']`, `sc['values']`, `availableTargets`, or `curves['addable']`.
  factory TargetOption.fromMap(Map m, {bool isFigure = false}) {
    final id = '${m['id'] ?? ''}';
    final rawName = '${m['label'] ?? m['name'] ?? ''}';
    final rawGroup = '${m['layer'] ?? m['group'] ?? (isFigure ? 'Figures' : '')}';
    final propLabel = m['property'] is String && (m['property'] as String).isNotEmpty
        ? '${m['property']}'
        : (rawGroup.isNotEmpty && rawName.startsWith('$rawGroup ')
            ? rawName.substring(rawGroup.length + 1)
            : rawName);
    final inferredKind = _inferKind(m, isFigure: isFigure, group: rawGroup, id: id);
    final rawVal = m['value'];
    String? valStr;
    if (rawVal is num) {
      valStr = rawVal == rawVal.roundToDouble() ? '${rawVal.round()}' : rawVal.toStringAsFixed(2);
    }
    final cat = m['category'] is String && (m['category'] as String).isNotEmpty
        ? '${m['category']}'
        : _categoryForKind(inferredKind);
    return TargetOption(
      id: id,
      label: propLabel.isEmpty ? rawName : propLabel,
      group: rawGroup,
      layerKind: inferredKind,
      category: cat,
      valueText: valStr,
      animated: m['animated'] == true || ((m['keys'] as num?)?.toInt() ?? 0) > 1,
      modifierCount: (m['mods'] as num?)?.toInt() ?? ((m['mods'] is List) ? (m['mods'] as List).length : 0),
    );
  }

  static String _inferKind(Map m, {required bool isFigure, required String group, required String id}) {
    if (m['layerKind'] is String && (m['layerKind'] as String).isNotEmpty) return '${m['layerKind']}';
    if (m['kind'] is String && (m['kind'] as String).isNotEmpty) return '${m['kind']}';
    if (isFigure) return 'figure';
    final lowId = id.toLowerCase();
    final lowGroup = group.toLowerCase();
    if (lowId.startsWith('camera') || lowGroup == 'camera') return 'camera';
    if (lowId.contains('.fx.') || lowGroup.contains('effect')) return 'adjustment';
    if (lowId.startsWith('pic') || lowGroup.contains('picture')) return 'picture';
    if (lowId.startsWith('text') || lowGroup.contains('text')) return 'text';
    if (lowId.startsWith('clip') || lowGroup.contains('movieclip')) return 'movieclip';
    if (lowId.startsWith('group') || lowGroup.contains('group')) return 'group';
    return 'figure';
  }

  static String _categoryForKind(String kind) => switch (kind) {
        'camera' => 'Camera',
        'adjustment' => 'Effects',
        'picture' => 'Pictures',
        'text' => 'Text',
        'movieclip' => 'Clips',
        'group' => 'Groups',
        'joint' => 'Joints',
        _ => 'Figures',
      };

  static String iconForKind(String kind) => switch (kind) {
        'camera' => 'camera',
        'adjustment' => 'fx',
        'picture' => 'picture',
        'text' => 'text',
        'movieclip' => 'clip',
        'group' => 'folder',
        'joint' => 'pose',
        _ => 'figure',
      };
}

/// Opens a crisp, searchable, filterable target picker dialog.
/// Returns the selected target id, `''` if cleared, or `null` if cancelled.
Future<String?> showTargetPickerSheet(
  BuildContext context, {
  required String title,
  required List<TargetOption> options,
  String? currentId,
  bool allowClear = false,
  String emptyText = 'No matching targets.',
  String searchHint = 'Search layers or properties…',
}) {
  return showGeneralDialog<String>(
    context: context,
    barrierDismissible: true,
    barrierLabel: title,
    barrierColor: const Color(0x6114181E),
    transitionDuration: kit.motion(context, 150),
    transitionBuilder: (c, a, _, child) {
      final e = CurvedAnimation(parent: a, curve: kit.kEase, reverseCurve: Curves.easeInCubic);
      return FadeTransition(
        opacity: e,
        child: ScaleTransition(scale: Tween(begin: 0.96, end: 1.0).animate(e), child: child),
      );
    },
    pageBuilder: (c, a1, a2) => _TargetPickerDialog(
      title: title,
      options: options,
      currentId: currentId,
      allowClear: allowClear,
      emptyText: emptyText,
      searchHint: searchHint,
    ),
  );
}

class _TargetPickerDialog extends StatefulWidget {
  const _TargetPickerDialog({
    required this.title,
    required this.options,
    required this.currentId,
    required this.allowClear,
    required this.emptyText,
    required this.searchHint,
  });

  final String title;
  final List<TargetOption> options;
  final String? currentId;
  final bool allowClear;
  final String emptyText;
  final String searchHint;

  @override
  State<_TargetPickerDialog> createState() => _TargetPickerDialogState();
}

class _TargetPickerDialogState extends State<_TargetPickerDialog> {
  final TextEditingController _searchCtrl = TextEditingController();
  String _query = '';
  String _category = 'All';
  bool _onlyAnimated = false;

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  List<String> get _categories {
    final seen = <String>[];
    for (final o in widget.options) {
      if (o.category.isNotEmpty && !seen.contains(o.category)) seen.add(o.category);
    }
    if (seen.length <= 1) return const [];
    return ['All', ...seen];
  }

  bool get _hasAnimated => widget.options.any((o) => o.animated);

  List<TargetOption> get _filtered {
    final q = _query.trim().toLowerCase();
    return [
      for (final o in widget.options)
        if ((_category == 'All' || o.category == _category) &&
            (!_onlyAnimated || o.animated) &&
            (q.isEmpty ||
                o.label.toLowerCase().contains(q) ||
                o.group.toLowerCase().contains(q) ||
                o.category.toLowerCase().contains(q) ||
                o.id.toLowerCase().contains(q)))
          o,
    ];
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    final pad = MediaQuery.paddingOf(context);
    final compact = size.height < 480;
    final cats = _categories;
    final hasAnim = _hasAnimated;
    final items = _filtered;

    // Group items cleanly while preserving order
    final groups = <String, List<TargetOption>>{};
    for (final o in items) {
      groups.putIfAbsent(o.group, () => []).add(o);
    }

    return Center(
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth: math.min(440.0, size.width - pad.left - pad.right - 24),
          maxHeight: math.min(540.0, size.height - pad.top - pad.bottom - 24),
        ),
        child: Container(
          decoration: BoxDecoration(
            color: cCard,
            borderRadius: BorderRadius.circular(18),
            boxShadow: const [BoxShadow(color: Color(0x52000000), blurRadius: 42, offset: Offset(0, 16))],
          ),
          clipBehavior: Clip.antiAlias,
          child: Material(
            type: MaterialType.transparency,
            child: DefaultTextStyle(
              style: kit.ink(),
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                // Header
                Padding(
                  padding: EdgeInsets.fromLTRB(16, compact ? 10 : 12, 10, 6),
                  child: Row(children: [
                    const Pictogram('modifier', size: 18, color: cAccent),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        widget.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: kit.ink(16, FontWeight.w700, cTitle),
                      ),
                    ),
                    kit.Tap(
                      label: 'Close',
                      minSize: 34,
                      onTap: () => Navigator.pop(context),
                      child: const Pictogram('x', size: 17, color: cSecondary),
                    ),
                  ]),
                ),
                // Instant Search bar
                Padding(
                  padding: const EdgeInsets.fromLTRB(14, 0, 14, 8),
                  child: Container(
                    height: 36,
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    decoration: BoxDecoration(
                      color: cControl,
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Row(children: [
                      const Pictogram('search', size: 16, color: cSecondary),
                      const SizedBox(width: 8),
                      Expanded(
                        child: TextField(
                          controller: _searchCtrl,
                          onChanged: (v) => setState(() => _query = v),
                          style: kit.ink(13.5),
                          decoration: InputDecoration(
                            isDense: true,
                            border: InputBorder.none,
                            hintText: widget.searchHint,
                            hintStyle: kit.ink(13, FontWeight.w400, cSecondary),
                          ),
                        ),
                      ),
                      if (_query.isNotEmpty)
                        kit.Tap(
                          label: 'Clear search',
                          minSize: 28,
                          onTap: () {
                            _searchCtrl.clear();
                            setState(() => _query = '');
                          },
                          child: const Pictogram('x', size: 14, color: cSecondary),
                        ),
                    ]),
                  ),
                ),
                // Category & Animated filter chips
                if (cats.isNotEmpty || hasAnim)
                  SizedBox(
                    height: 34,
                    child: ListView(
                      scrollDirection: Axis.horizontal,
                      padding: const EdgeInsets.symmetric(horizontal: 14),
                      children: [
                        for (final cat in cats)
                          Padding(
                            padding: const EdgeInsets.only(right: 6),
                            child: _FilterChipPill(
                              label: cat,
                              selected: _category == cat,
                              onTap: () => setState(() => _category = cat),
                            ),
                          ),
                        if (hasAnim)
                          _FilterChipPill(
                            label: 'Animated',
                            icon: 'key',
                            selected: _onlyAnimated,
                            onTap: () => setState(() => _onlyAnimated = !_onlyAnimated),
                          ),
                      ],
                    ),
                  ),
                if (cats.isNotEmpty || hasAnim) const SizedBox(height: 6),
                const Divider(height: 1, color: Color(0xFFE6E8EB)),
                // Grouped list
                Flexible(
                  child: items.isEmpty
                      ? Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 28),
                          child: Text(
                            _query.isNotEmpty ? 'No results for "$_query".' : widget.emptyText,
                            textAlign: TextAlign.center,
                            style: kit.ink(13.5, FontWeight.w400, cSecondary),
                          ),
                        )
                      : ListView(
                          shrinkWrap: true,
                          padding: const EdgeInsets.fromLTRB(10, 6, 10, 10),
                          children: [
                            for (final entry in groups.entries) ...[
                              if (entry.key.isNotEmpty)
                                Padding(
                                  padding: const EdgeInsets.fromLTRB(8, 8, 8, 4),
                                  child: Row(children: [
                                    Pictogram(
                                      TargetOption.iconForKind(entry.value.first.layerKind),
                                      size: 14,
                                      color: cSecondary,
                                    ),
                                    const SizedBox(width: 6),
                                    Expanded(
                                      child: Text(
                                        entry.key,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: kit.ink(12, FontWeight.w700, cSecondary),
                                      ),
                                    ),
                                  ]),
                                ),
                              for (final o in entry.value) _buildOptionRow(context, o),
                            ],
                          ],
                        ),
                ),
                // Optional Clear footer
                if (widget.allowClear && widget.currentId != null)
                  Container(
                    padding: const EdgeInsets.fromLTRB(14, 8, 14, 10),
                    decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
                    child: kit.WordButton(
                      'Unassign',
                      filled: true,
                      expand: true,
                      height: 36,
                      size: 13.5,
                      onTap: () => Navigator.pop(context, ''),
                    ),
                  ),
              ]),
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildOptionRow(BuildContext context, TargetOption o) {
    final selected = o.id == widget.currentId;
    return kit.Tap(
      minSize: 0,
      radius: 9,
      onTap: () => Navigator.pop(context, o.id),
      child: Container(
        height: 38,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: selected ? cSelected : null,
          borderRadius: BorderRadius.circular(9),
        ),
        child: Row(children: [
          if (o.group.isEmpty) ...[
            Pictogram(TargetOption.iconForKind(o.layerKind), size: 16, color: selected ? cAccent : cInk2b),
            const SizedBox(width: 8),
          ],
          Expanded(
            child: Text(
              o.label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: kit.ink(14, selected ? FontWeight.w700 : FontWeight.w500, selected ? cAccent : cText),
            ),
          ),
          if (o.animated) ...[
            const SizedBox(width: 6),
            const Pictogram('key', size: 12, color: cAccent, stroke: 2.2),
          ],
          if (o.modifierCount > 0) ...[
            const SizedBox(width: 5),
            const Pictogram('modifier', size: 13, color: cAccent, stroke: 2.0),
          ],
          if (o.valueText != null) ...[
            const SizedBox(width: 8),
            Text(
              o.valueText!,
              style: kit.ink(12, FontWeight.w500, cSecondary).copyWith(fontFeatures: const [FontFeature.tabularFigures()]),
            ),
          ],
          if (selected) ...[
            const SizedBox(width: 8),
            const Pictogram('check', size: 15, color: cAccent, stroke: 2.3),
          ],
        ]),
      ),
    );
  }
}

class _FilterChipPill extends StatelessWidget {
  const _FilterChipPill({required this.label, required this.selected, required this.onTap, this.icon});
  final String label;
  final bool selected;
  final VoidCallback onTap;
  final String? icon;

  @override
  Widget build(BuildContext context) {
    return kit.Tap(
      minSize: 0,
      radius: 15,
      onTap: onTap,
      child: Container(
        height: 28,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: selected ? cSelected : cControl,
          borderRadius: BorderRadius.circular(14),
          border: selected ? Border.all(color: cAccent.withValues(alpha: 0.4)) : null,
        ),
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          if (icon != null) ...[
            Pictogram(icon!, size: 12, color: selected ? cAccent : cSecondary, stroke: 2.1),
            const SizedBox(width: 5),
          ],
          Text(
            label,
            style: kit.ink(12, selected ? FontWeight.w700 : FontWeight.w500, selected ? cAccent : cText),
          ),
        ]),
      ),
    );
  }
}
