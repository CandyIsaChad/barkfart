// The Workshop (REDESIGN K3, the Workshop board): a page of its own for
// plugins (scripts, effects, modifiers). Browse them by kind, see what each
// does and where it's used in this animation, why one is switched off, peek
// at its code; import, share, remove, switch one on again. Settings of one
// use (a script's run, an effect on a figure) stay in Properties.
import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

import 'ui/kit.dart' as kit;
import 'ui/pictograms.dart';
import 'ui/rows.dart' as rw;
import 'ui/tokens.dart';

/// What the Workshop hands back to the editor as it closes: add the plugin
/// to the animation, or show one of its uses.
sealed class WorkshopAction {
  const WorkshopAction();
}

class WorkshopAdd extends WorkshopAction {
  const WorkshopAdd(this.id, this.kind);
  final String id, kind;
}

class WorkshopShow extends WorkshopAction {
  const WorkshopShow(this.use);
  final Map use;
}

class WorkshopPage extends StatefulWidget {
  const WorkshopPage({super.key, required this.cmd, required this.import, required this.reinstall, required this.remove, required this.share, required this.files, required this.figureSelected, this.picked});

  /// An engine command; returns the new state.
  final Map<String, dynamic>? Function(Map<String, Object?> c) cmd;

  /// Import a plugin file; returns its id (null: nothing imported).
  final Future<String?> Function() import;

  /// Install a plugin again from its file (switches it back on).
  final Future<void> Function(String id) reinstall;

  /// Uninstall a plugin and delete its file.
  final Future<void> Function(String id) remove;

  /// Share a plugin's file (the system share sheet).
  final Future<void> Function(String id, Rect from) share;

  /// Plugin id → its file in Barkfart Plugins.
  final Map<String, String> files;

  /// Whether a stick figure is selected (an effect goes on it).
  final bool figureSelected;
  final String? picked;

  @override
  State<WorkshopPage> createState() => _WorkshopPageState();
}

const _kinds = [('all', 'Everything', 'all'), ('script', 'Scripts', 'script'), ('effect', 'Effects', 'fx'), ('modifier', 'Modifiers', 'wiggle')];
const _kindName = {'script': 'Script', 'effect': 'Effect', 'modifier': 'Modifier'};
const _kindIcon = {'script': 'script', 'effect': 'fx', 'modifier': 'wiggle'};
const _orange = Color(0xFFD9821E);

class _WorkshopPageState extends State<WorkshopPage> {
  String _kind = 'all';
  String? _picked;
  Map<String, dynamic> _w = const {};
  final GlobalKey _shareKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    _picked = widget.picked;
    _load();
  }

  void _load() {
    final st = widget.cmd({'op': 'workshop', 'id': ?_picked});
    setState(() => _w = (st?['workshop'] as Map?)?.cast<String, dynamic>() ?? const {});
  }

  List<Map> get _all => ((_w['plugins'] as List?) ?? const []).cast<Map>();

  List<Map> get _shown => _all.where((p) => _kind == 'all' || (_kind == 'off' ? p['off'] != null : p['kind'] == _kind)).toList();

  int _count(String k) => _all.where((p) => k == 'all' || (k == 'off' ? p['off'] != null : p['kind'] == k)).length;

  void _pick(String? id) {
    _picked = id;
    _load();
  }

  Future<void> _importOne() async {
    final id = await widget.import();
    if (!mounted) return;
    if (id != null) _kind = 'all';
    _pick(id ?? _picked);
  }

  @override
  Widget build(BuildContext context) {
    final pad = MediaQuery.paddingOf(context);
    final wide = MediaQuery.sizeOf(context).width >= 900;
    final picked = _all.where((p) => p['id'] == _picked).firstOrNull;
    final top = Container(
      height: 40 + pad.top,
      padding: EdgeInsets.only(top: pad.top, left: pad.left + 4, right: pad.right + 4),
      decoration: const BoxDecoration(color: cCard, border: Border(bottom: BorderSide(color: cLine))),
      // the bar's whole width: the title centred, Back and Import at the ends
      child: SizedBox.expand(
        child: Stack(alignment: Alignment.center, children: [
          Text('Workshop', style: kit.ink(15, FontWeight.w700)),
          Positioned(left: 0, child: kit.WordButton('‹ Back', onTap: () => Navigator.pop(context))),
          Positioned(right: 0, child: kit.WordButton('Import…', onTap: _importOne)),
        ]),
      ),
    );
    final grid = _grid(wide);
    final detail = picked == null ? null : _detail(picked);
    Widget body;
    if (wide) {
      body = Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        _shelf(),
        Expanded(child: grid),
        Padding(
          padding: EdgeInsets.fromLTRB(0, 16, 16 + pad.right, 16 + pad.bottom),
          child: SizedBox(width: 360, child: detail ?? Align(alignment: Alignment.topCenter, child: _nothingPicked())),
        ),
      ]);
    } else {
      // upright or small: the kinds as a row; the picked one floats over the cards
      body = Column(children: [
        _chips(),
        Expanded(
          child: Stack(children: [
            Positioned.fill(child: grid),
            if (detail != null) Positioned(top: 8, right: 12 + pad.right, bottom: 12 + pad.bottom, width: math.min(360, MediaQuery.sizeOf(context).width - 24), child: detail),
          ]),
        ),
      ]);
    }
    return Scaffold(backgroundColor: cPanel, body: Column(children: [top, Expanded(child: body)]));
  }

  // ---------------------------------------------------------------- the shelf

  Widget _shelf() => Container(
        width: 212,
        padding: const EdgeInsets.fromLTRB(12, 18, 12, 12),
        decoration: const BoxDecoration(color: Color(0xFFF7F8FA), border: Border(right: BorderSide(color: cLine))),
        child: ListView(children: [
          for (final (k, label, icon) in _kinds) _kindRow(k, label, icon),
          const Divider(height: 16, color: cLine),
          _kindRow('off', 'Switched off', 'x'),
          Padding(padding: const EdgeInsets.fromLTRB(6, 14, 6, 0), child: Text('A plugin is a .luau file. It runs inside Barkfart only: it can’t reach the internet or your other files.', style: kit.ink(12, FontWeight.w400, cSecondary))),
        ]),
      );

  Widget _kindRow(String k, String label, String icon) {
    final on = _kind == k;
    return kit.Tap(
      minSize: 0,
      onTap: () => setState(() => _kind = k),
      child: AnimatedContainer(
        duration: kit.motion(context, 140),
        height: 40,
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(color: on ? const Color(0xFFE4EEFA) : null, borderRadius: BorderRadius.circular(10)),
        child: Row(children: [
          Pictogram(icon, size: 20, color: on ? cAccent : k == 'off' ? _orange : const Color(0xFF4A4F57)),
          const SizedBox(width: 10),
          Expanded(child: Text(label, style: kit.ink(15, on ? FontWeight.w700 : FontWeight.w400, on ? cAccent : cText))),
          Text('${_count(k)}', style: kit.ink(13, FontWeight.w600, cSecondary)),
        ]),
      ),
    );
  }

  Widget _chips() => Container(
        height: 52,
        decoration: const BoxDecoration(color: Color(0xFFF7F8FA), border: Border(bottom: BorderSide(color: cLine))),
        child: ListView(scrollDirection: Axis.horizontal, padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6), children: [
          for (final (k, label, icon) in [..._kinds, ('off', 'Switched off', 'x')])
            Padding(padding: const EdgeInsets.only(right: 4), child: SizedBox(width: 170, child: _kindRow(k, label, icon))),
        ]),
      );

  // ---------------------------------------------------------------- the cards

  Widget _grid(bool wide) {
    final shown = _shown;
    return LayoutBuilder(builder: (context, box) {
      final cols = box.maxWidth >= 520 ? 2 : 1;
      const gap = 14.0;
      final w = (box.maxWidth - 40 - gap * (cols - 1)) / cols;
      return SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(20, 18, 20, 24),
        child: Wrap(spacing: gap, runSpacing: gap, children: [
          for (final p in shown) SizedBox(width: w, child: _card(p)),
          if (_kind == 'all' || shown.isEmpty) SizedBox(width: w, child: _importCard()),
        ]),
      );
    });
  }

  Widget _card(Map p) {
    final sel = p['id'] == _picked;
    final off = p['off'] != null;
    final uses = ((p['uses'] as List?) ?? const []).cast<Map>();
    final used = uses.isEmpty ? 'Not used in this animation' : 'In this animation: ${uses.map((u) => u['label']).take(2).join('; ')}${uses.length > 2 ? ' and ${uses.length - 2} more' : ''}';
    return kit.Tap(
      minSize: 0,
      radius: 14,
      onTap: () => _pick(sel ? null : '${p['id']}'),
      child: AnimatedContainer(
        duration: kit.motion(context, 140),
        constraints: const BoxConstraints(minHeight: 142),
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
        decoration: BoxDecoration(
          color: cCard,
          borderRadius: BorderRadius.circular(14),
          boxShadow: [BoxShadow(color: sel ? cAccent : const Color(0x0F000000), spreadRadius: sel ? 2 : 1), const BoxShadow(color: Color(0x1210141A), blurRadius: 8, offset: Offset(0, 2))],
        ),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Pictogram(_kindIcon[p['kind']] ?? 'script', size: 22, color: off ? const Color(0xFF9AA0A8) : cAccent),
            const SizedBox(width: 8),
            Expanded(child: Text('${p['name']}', maxLines: 1, overflow: TextOverflow.ellipsis, style: kit.ink(16, FontWeight.w700))),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
              decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(6)),
              child: Text(_kindName[p['kind']] ?? '${p['kind']}', style: kit.ink(12, FontWeight.w600, cSecondary)),
            ),
          ]),
          const SizedBox(height: 4),
          Text(_byline(p), style: kit.ink(12, FontWeight.w400, cSecondary)),
          if (p['description'] != null) Padding(padding: const EdgeInsets.only(top: 4), child: Text('${p['description']}', maxLines: 3, overflow: TextOverflow.ellipsis, style: kit.ink(13))),
          const SizedBox(height: 8),
          if (off)
            Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              const Padding(padding: EdgeInsets.only(top: 1), child: Pictogram('x', size: 14, color: _orange)),
              const SizedBox(width: 6),
              Expanded(child: Text('Switched off: ${p['off']}', maxLines: 3, overflow: TextOverflow.ellipsis, style: kit.ink(12, FontWeight.w600, _orange))),
            ])
          else
            Text(used, maxLines: 2, overflow: TextOverflow.ellipsis, style: kit.ink(12, uses.isEmpty ? FontWeight.w400 : FontWeight.w600, uses.isEmpty ? cSecondary : cAccent)),
        ]),
      ),
    );
  }

  String _byline(Map p) => [if (p['author'] case final a?) 'by $a', '${p['version']}'].join(' · ');

  Widget _importCard() => kit.Tap(
        minSize: 0,
        radius: 14,
        onTap: _importOne,
        child: Container(
          constraints: const BoxConstraints(minHeight: 142),
          alignment: Alignment.center,
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(borderRadius: BorderRadius.circular(14), border: Border.all(color: const Color(0xFFC3C8CF), width: 1.5)),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            const Pictogram('plus', size: 26, color: cAccent),
            const SizedBox(height: 6),
            Text('Import a plugin…', style: kit.ink(15, FontWeight.w700, cAccent)),
            Text('A .luau file: a script, an effect or a modifier', textAlign: TextAlign.center, style: kit.ink(12, FontWeight.w400, cSecondary)),
          ]),
        ),
      );

  // ---------------------------------------------------------------- the picked one

  Widget _nothingPicked() => kit.PanelCard(
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: Text(_all.isEmpty ? 'No plugins yet. Import a .luau file: a script that moves figures, an effect, or a modifier for the graph.' : 'Pick a plugin to see what it does, where it’s used and its code.', style: kit.ink(13, FontWeight.w400, cSecondary)),
        ),
      );

  Widget _detail(Map p) {
    final info = (_w['picked'] as Map?) ?? const {};
    final kind = '${p['kind']}';
    final off = p['off'];
    final uses = ((p['uses'] as List?) ?? const []).cast<Map>();
    final settings = ((info['settings'] as List?) ?? const []).cast<Map>();
    final targets = ((info['targets'] as List?) ?? const []).cast<Map>();
    String settingText(Map s) {
      final v = s['value'];
      final x = (s['extra'] as Map?) ?? const {};
      String n(Object? y) => y is num ? (y == y.roundToDouble() ? '${y.round()}' : y.toStringAsFixed(2)) : '$y';
      final range = x['min'] != null && x['max'] != null ? ' (${n(x['min'])} to ${n(x['max'])})' : '';
      return switch (s['kind']) {
        'toggle' => v == true ? 'on' : 'off',
        'colour' => 'a colour',
        'point' => 'a point',
        _ => '${n(v)}$range',
      };
    }

    final hasFile = widget.files.containsKey(p['id']);
    final rows = <rw.Desc>[
      if (p['description'] != null) rw.DHint('${p['description']}'),
      if (off != null) rw.DHint('Switched off: $off${hasFile ? ' Switch it on again to try once more.' : ' Import it again to switch it back on.'}'),
      if (targets.isNotEmpty) ...[
        const rw.DTitle('It moves'),
        for (final t in targets) rw.DInfo('${t['label']}', t['kind'] == 'figure' ? 'a stick figure' : 'a value'),
      ],
      if (settings.isNotEmpty) ...[
        const rw.DTitle('Its settings'),
        for (final s in settings) rw.DInfo('${s['label']}', settingText(s)),
      ],
      const rw.DTitle('In this animation'),
      if (uses.isEmpty) const rw.DHint('Not used in this animation.'),
      for (final u in uses) rw.DLink('${u['label']}', 'Show', onTap: () => Navigator.pop(context, WorkshopShow(u))),
      const rw.DTitle('The code'),
    ];
    final addLabel = switch (kind) { 'effect' => 'Add to figure', 'modifier' => 'Open the graph', _ => 'Add to animation' };
    final canAdd = off == null && (kind != 'effect' || widget.figureSelected);
    return kit.PanelCard(
      padding: EdgeInsets.zero,
      child: Column(children: [
        Expanded(
          child: ListView(padding: const EdgeInsets.fromLTRB(16, 12, 16, 12), children: [
            Row(children: [
              Pictogram(_kindIcon[kind] ?? 'script', size: 26, color: off != null ? const Color(0xFF9AA0A8) : cAccent),
              const SizedBox(width: 10),
              Expanded(
                child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Text('${p['name']}', style: kit.ink(17, FontWeight.w700)),
                  Text('${_kindName[kind] ?? kind} ${_byline(p)}', style: kit.ink(12, FontWeight.w400, cSecondary)),
                ]),
              ),
              kit.Tap(label: 'Close', onTap: () => _pick(null), child: const Pictogram('x', size: 18, color: cSecondary)),
            ]),
            const SizedBox(height: 4),
            for (final d in rows) rw.renderDesc(d),
            _code('${info['source'] ?? ''}'),
            const SizedBox(height: 10),
            Row(children: [
              Expanded(
                flex: 10,
                child: kit.WordButton('Share the file…', key: _shareKey, filled: true, expand: true, height: 38, onTap: !hasFile
                    ? null
                    : () {
                        final box = _shareKey.currentContext?.findRenderObject() as RenderBox?;
                        widget.share('${p['id']}', box == null ? Rect.zero : box.localToGlobal(Offset.zero) & box.size);
                      }),
              ),
              const SizedBox(width: 8),
              Expanded(
                flex: 13,
                child: off != null && hasFile
                    ? kit.WordButton('Switch it on again', primary: true, expand: true, height: 38, onTap: () async {
                        await widget.reinstall('${p['id']}');
                        if (mounted) _load();
                      })
                    : kit.WordButton(addLabel, primary: true, expand: true, height: 38, onTap: canAdd ? () => Navigator.pop(context, WorkshopAdd('${p['id']}', kind)) : null),
              ),
            ]),
            if (kind == 'effect' && !widget.figureSelected && off == null) const Padding(padding: EdgeInsets.only(top: 6), child: Text('Select a stick figure first, then add it here or from its Effects.', style: TextStyle(color: cSecondary, fontSize: 12))),
          ]),
        ),
        Container(
          decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
          padding: const EdgeInsets.fromLTRB(16, 9, 16, 9),
          child: kit.WordButton('Remove from Barkfart', filled: true, danger: true, expand: true, height: 36, onTap: () async {
            await widget.remove('${p['id']}');
            if (mounted) _pick(null);
          }),
        ),
      ]),
    );
  }

  /// A peek at the plugin's code: comments grey, Luau's words red.
  Widget _code(String src) {
    final lines = src.split('\n').take(60).toList();
    while (lines.isNotEmpty && lines.last.trim().isEmpty) {
      lines.removeLast();
    }
    if (lines.isEmpty) return const SizedBox.shrink();
    const mono = TextStyle(fontFamily: 'monospace', fontFamilyFallback: ['Menlo', 'Courier New', 'DejaVu Sans Mono'], fontSize: 12, height: 1.4, color: Color(0xFF3A3F47));
    final words = RegExp(r'\b(return|function|local|end|if|then|else|elseif|for|in|do|while|not|and|or|nil|true|false)\b');
    List<TextSpan> line(String l) {
      final c = l.indexOf('--');
      final code = c < 0 ? l : l.substring(0, c);
      final out = <TextSpan>[];
      var at = 0;
      for (final m in words.allMatches(code)) {
        if (m.start > at) out.add(TextSpan(text: code.substring(at, m.start)));
        out.add(TextSpan(text: m[0], style: const TextStyle(color: cTitle, fontWeight: FontWeight.w600)));
        at = m.end;
      }
      if (at < code.length) out.add(TextSpan(text: code.substring(at)));
      if (c >= 0) out.add(TextSpan(text: l.substring(c), style: const TextStyle(color: Color(0xFF8A9099))));
      return out;
    }

    return Container(
      constraints: const BoxConstraints(maxHeight: 170),
      decoration: BoxDecoration(color: const Color(0xFFF4F5F7), borderRadius: BorderRadius.circular(10)),
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(10),
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Text.rich(
            TextSpan(style: mono, children: [
              for (var i = 0; i < lines.length; i++) ...[...line(lines[i]), if (i < lines.length - 1) const TextSpan(text: '\n')],
            ]),
            softWrap: false,
          ),
        ),
      ),
    );
  }
}
