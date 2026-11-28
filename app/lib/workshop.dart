// The Workshop (REDESIGN K3, the Workshop board): a page of its own for
// plugins (scripts, effects, modifiers) and offline stock Effect Presets.
// Browse by kind, install stock scripts/shaders/modifiers in 1 tap offline,
// apply stacked Effect Presets, inspect code, import, share, or remove.
import 'dart:io';
import 'dart:math' as math;

import 'package:material_ui/material_ui.dart';

import 'files.dart';
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
  const WorkshopPage({
    super.key,
    required this.cmd,
    required this.import,
    required this.reinstall,
    required this.remove,
    required this.share,
    required this.files,
    required this.figureSelected,
    this.picked,
  });

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

const _kinds = [
  ('all', 'Everything', 'all'),
  ('preset', 'Effect Presets', 'fx'),
  ('script', 'Scripts', 'script'),
  ('effect', 'Shader Effects', 'fx'),
  ('modifier', 'Modifiers', 'wiggle'),
];
const _kindName = {
  'preset': 'Preset',
  'script': 'Script',
  'effect': 'Effect',
  'modifier': 'Modifier',
};
const _kindIcon = {
  'preset': 'fx',
  'script': 'script',
  'effect': 'fx',
  'modifier': 'wiggle',
};
const _orange = Color(0xFFD9821E);
const _green = Color(0xFF1E8E4E);

class _StockPreset {
  const _StockPreset({
    required this.id,
    required this.name,
    required this.category,
    required this.description,
    required this.summary,
    required this.effects,
  });
  final String id, name, category, description;
  final List<String> summary;
  final List<Map<String, Object?>> effects;
}

class _StockPlugin {
  const _StockPlugin({
    required this.id,
    required this.fileName,
    required this.name,
    required this.kind,
    required this.description,
    required this.source,
  });
  final String id, fileName, name, kind, description, source;
}

class _WorkshopPageState extends State<WorkshopPage> {
  String _kind = 'all';
  String _query = '';
  String? _picked;
  String? _banner;
  Map<String, dynamic> _w = const {};
  final GlobalKey _shareKey = GlobalKey();
  final TextEditingController _searchCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    _picked = widget.picked;
    _load();
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  void _load() {
    final pluginId = (_picked != null && !_picked!.startsWith('preset.') && _isInstalled(_picked!)) ? _picked : null;
    final st = widget.cmd({'op': 'workshop', 'id': ?pluginId});
    setState(() => _w = (st?['workshop'] as Map?)?.cast<String, dynamic>() ?? const {});
  }

  List<Map> get _installed => ((_w['plugins'] as List?) ?? const []).cast<Map>();

  Set<String> get _installedIds => {for (final p in _installed) '${p['id']}'};

  bool _isInstalled(String id) => _installedIds.contains(id);

  bool _matchesQuery(String name, String desc, [String extra = '']) {
    final q = _query.trim().toLowerCase();
    if (q.isEmpty) return true;
    return name.toLowerCase().contains(q) || desc.toLowerCase().contains(q) || extra.toLowerCase().contains(q);
  }

  List<_StockPreset> get _shownPresets {
    if (_kind != 'all' && _kind != 'preset') return const [];
    return [
      for (final p in _stockPresets)
        if (_matchesQuery(p.name, p.description, '${p.category} ${p.summary.join(' ')}')) p,
    ];
  }

  List<Map> get _shownInstalled {
    if (_kind == 'preset') return const [];
    return _installed
        .where((p) =>
            (_kind == 'all' || (_kind == 'off' ? p['off'] != null : p['kind'] == _kind)) &&
            _matchesQuery('${p['name'] ?? ''}', '${p['description'] ?? ''}', '${p['kind'] ?? ''}'))
        .toList();
  }

  List<_StockPlugin> get _shownStock {
    if (_kind == 'preset' || _kind == 'off') return const [];
    final inst = _installedIds;
    return [
      for (final s in _stockPlugins)
        if (!inst.contains(s.id) && (_kind == 'all' || s.kind == _kind) && _matchesQuery(s.name, s.description, s.kind)) s,
    ];
  }

  int _count(String k) {
    if (k == 'preset') return _stockPresets.length;
    if (k == 'off') return _installed.where((p) => p['off'] != null).length;
    final instIds = _installedIds;
    final stockExtra = _stockPlugins.where((s) => !instIds.contains(s.id) && (k == 'all' || s.kind == k)).length;
    final instMatch = _installed.where((p) => k == 'all' || p['kind'] == k).length;
    return instMatch + stockExtra + (k == 'all' ? _stockPresets.length : 0);
  }

  void _pick(String? id) {
    _picked = id;
    _banner = null;
    if (id != null && !id.startsWith('preset.') && _isInstalled(id)) {
      _load();
    } else {
      setState(() {});
    }
  }

  Future<void> _importOne() async {
    final id = await widget.import();
    if (!mounted) return;
    if (id != null) _kind = 'all';
    _picked = id ?? _picked;
    _load();
  }

  Future<bool> _installStock(_StockPlugin stock) async {
    final st = widget.cmd({'op': 'plugin_install', 'name': stock.fileName, 'source': stock.source});
    if (st?['installed'] case final String id) {
      try {
        final path = '${(await pluginsDir()).path}/${stock.fileName}';
        await File(path).writeAsString(stock.source);
        widget.files[id] = path;
      } catch (_) {}
      if (!mounted) return true;
      _picked = id;
      _banner = '${stock.name} installed.';
      _load();
      return true;
    }
    if (mounted) {
      setState(() => _banner = '${st?['error'] ?? 'Could not install ${stock.name}.'}');
    }
    return false;
  }

  void _applyPreset(_StockPreset preset, {bool asEffectLayer = false}) {
    final st = widget.cmd({
      'op': 'fx_preset',
      'name': preset.name,
      'effects': preset.effects,
      'replace': false,
      'createLayer': asEffectLayer,
    });
    if (st?['error'] case final String err) {
      setState(() => _banner = err);
      return;
    }
    if (mounted) Navigator.pop(context);
  }

  @override
  Widget build(BuildContext context) {
    final pad = MediaQuery.paddingOf(context);
    final wide = MediaQuery.sizeOf(context).width >= 900;
    final pickedInstalled = _installed.where((p) => p['id'] == _picked).firstOrNull;
    final pickedPreset = _stockPresets.where((p) => p.id == _picked).firstOrNull;
    final pickedStock = pickedInstalled == null ? _stockPlugins.where((s) => s.id == _picked).firstOrNull : null;

    final Widget? detail = pickedPreset != null
        ? _presetDetail(pickedPreset)
        : pickedInstalled != null
            ? _installedDetail(pickedInstalled)
            : pickedStock != null
                ? _stockDetail(pickedStock)
                : null;

    final top = Container(
      height: 40 + pad.top,
      padding: EdgeInsets.only(top: pad.top, left: pad.left + 4, right: pad.right + 4),
      decoration: const BoxDecoration(color: cCard, border: Border(bottom: BorderSide(color: cLine))),
      child: SizedBox.expand(
        child: Stack(alignment: Alignment.center, children: [
          Text('Workshop', style: kit.ink(15, FontWeight.w700)),
          Positioned(left: 0, child: kit.WordButton('‹ Back', onTap: () => Navigator.pop(context))),
          Positioned(right: 0, child: kit.WordButton('Import…', onTap: _importOne)),
        ]),
      ),
    );
    final grid = _grid(wide);
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
      body = Column(children: [
        _chips(),
        Expanded(
          child: Stack(children: [
            Positioned.fill(child: grid),
            if (detail != null)
              Positioned(
                top: 8,
                right: 12 + pad.right,
                bottom: 12 + pad.bottom,
                width: math.min(360, MediaQuery.sizeOf(context).width - 24),
                child: detail,
              ),
          ]),
        ),
      ]);
    }
    return Scaffold(backgroundColor: cPanel, body: Column(children: [top, Expanded(child: body)]));
  }

  // ---------------------------------------------------------------- the shelf

  Widget _shelf() => Container(
        width: 218,
        padding: const EdgeInsets.fromLTRB(12, 18, 12, 12),
        decoration: const BoxDecoration(color: Color(0xFFF7F8FA), border: Border(right: BorderSide(color: cLine))),
        child: ListView(children: [
          for (final (k, label, icon) in _kinds) _kindRow(k, label, icon),
          const Divider(height: 16, color: cLine),
          _kindRow('off', 'Switched off', 'x'),
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
          Expanded(child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: kit.ink(14, on ? FontWeight.w700 : FontWeight.w400, on ? cAccent : cText))),
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
            Padding(padding: const EdgeInsets.only(right: 4), child: SizedBox(width: 164, child: _kindRow(k, label, icon))),
        ]),
      );

  // ---------------------------------------------------------------- the cards

  Widget _searchBar() => Padding(
        padding: const EdgeInsets.only(bottom: 14),
        child: Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(10), border: Border.all(color: cLine)),
          child: Row(children: [
            Expanded(
              child: TextField(
                controller: _searchCtrl,
                onChanged: (v) => setState(() => _query = v),
                style: kit.ink(14),
                decoration: InputDecoration(
                  isDense: true,
                  border: InputBorder.none,
                  hintText: 'Search presets, scripts, effects, modifiers…',
                  hintStyle: kit.ink(13, FontWeight.w400, cSecondary),
                ),
              ),
            ),
            if (_query.isNotEmpty)
              kit.Tap(
                minSize: 0,
                label: 'Clear search',
                onTap: () {
                  _searchCtrl.clear();
                  setState(() => _query = '');
                },
                child: const Pictogram('x', size: 15, color: cSecondary),
              ),
          ]),
        ),
      );

  Widget _sectionHeading(String title, String subtitle) => Padding(
        padding: const EdgeInsets.only(top: 6, bottom: 10),
        child: Row(children: [
          Text(title, style: kit.ink(13, FontWeight.w700, cSecondary)),
          const SizedBox(width: 8),
          Expanded(child: Text(subtitle, maxLines: 1, overflow: TextOverflow.ellipsis, style: kit.ink(12, FontWeight.w400, cSecondary))),
        ]),
      );

  Widget _grid(bool wide) {
    final presets = _shownPresets;
    final installed = _shownInstalled;
    final stock = _shownStock;
    final empty = presets.isEmpty && installed.isEmpty && stock.isEmpty;
    return LayoutBuilder(builder: (context, box) {
      final cols = box.maxWidth >= 520 ? 2 : 1;
      const gap = 14.0;
      final w = math.max(180.0, (box.maxWidth - 40 - gap * (cols - 1)) / cols);
      return SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(20, 16, 20, 24),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          _searchBar(),
          if (presets.isNotEmpty) ...[
            _sectionHeading('EFFECT PRESETS', ''),
            Wrap(spacing: gap, runSpacing: gap, children: [
              for (final p in presets) SizedBox(width: w, child: _presetCard(p)),
            ]),
            const SizedBox(height: 16),
          ],
          if (installed.isNotEmpty) ...[
            _sectionHeading('INSTALLED', ''),
            Wrap(spacing: gap, runSpacing: gap, children: [
              for (final p in installed) SizedBox(width: w, child: _installedCard(p)),
            ]),
            const SizedBox(height: 16),
          ],
          if (stock.isNotEmpty) ...[
            _sectionHeading('BUILT-IN LIBRARY', ''),
            Wrap(spacing: gap, runSpacing: gap, children: [
              for (final s in stock) SizedBox(width: w, child: _stockCard(s)),
            ]),
            const SizedBox(height: 16),
          ],
          if (_kind == 'all' || empty)
            Wrap(spacing: gap, runSpacing: gap, children: [
              SizedBox(width: w, child: _importCard()),
            ]),
        ]),
      );
    });
  }

  Widget _presetCard(_StockPreset p) {
    final sel = p.id == _picked;
    return kit.Tap(
      minSize: 0,
      radius: 14,
      onTap: () => _pick(sel ? null : p.id),
      child: AnimatedContainer(
        duration: kit.motion(context, 140),
        constraints: const BoxConstraints(minHeight: 142),
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
        decoration: BoxDecoration(
          color: cCard,
          borderRadius: BorderRadius.circular(14),
          boxShadow: [
            BoxShadow(color: sel ? cAccent : const Color(0x0F000000), spreadRadius: sel ? 2 : 1),
            const BoxShadow(color: Color(0x1210141A), blurRadius: 8, offset: Offset(0, 2)),
          ],
        ),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            const Pictogram('fx', size: 22, color: cAccent),
            const SizedBox(width: 8),
            Expanded(child: Text(p.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: kit.ink(16, FontWeight.w700))),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
              decoration: BoxDecoration(color: const Color(0xFFE4EEFA), borderRadius: BorderRadius.circular(6)),
              child: Text(p.category, style: kit.ink(12, FontWeight.w600, cAccent)),
            ),
          ]),
          const SizedBox(height: 4),
          Text('${p.effects.length} stacked effects · Offline preset', style: kit.ink(12, FontWeight.w400, cSecondary)),
          Padding(padding: const EdgeInsets.only(top: 5), child: Text(p.description, maxLines: 2, overflow: TextOverflow.ellipsis, style: kit.ink(13))),
          const SizedBox(height: 8),
          Wrap(spacing: 5, runSpacing: 4, children: [
            for (final tag in p.summary)
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(5)),
                child: Text(tag, style: kit.ink(11, FontWeight.w500, cSecondary)),
              ),
          ]),
        ]),
      ),
    );
  }

  Widget _stockCard(_StockPlugin s) {
    final sel = s.id == _picked;
    return kit.Tap(
      minSize: 0,
      radius: 14,
      onTap: () => _pick(sel ? null : s.id),
      child: AnimatedContainer(
        duration: kit.motion(context, 140),
        constraints: const BoxConstraints(minHeight: 142),
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
        decoration: BoxDecoration(
          color: cCard,
          borderRadius: BorderRadius.circular(14),
          boxShadow: [
            BoxShadow(color: sel ? cAccent : const Color(0x0F000000), spreadRadius: sel ? 2 : 1),
            const BoxShadow(color: Color(0x1210141A), blurRadius: 8, offset: Offset(0, 2)),
          ],
        ),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Row(children: [
            Pictogram(_kindIcon[s.kind] ?? 'script', size: 22, color: cAccent),
            const SizedBox(width: 8),
            Expanded(child: Text(s.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: kit.ink(16, FontWeight.w700))),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
              decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(6)),
              child: Text(_kindName[s.kind] ?? s.kind, style: kit.ink(12, FontWeight.w600, cSecondary)),
            ),
          ]),
          const SizedBox(height: 4),
          Text('by Barkfart · 1.0.0', style: kit.ink(12, FontWeight.w400, cSecondary)),
          Padding(padding: const EdgeInsets.only(top: 4), child: Text(s.description, maxLines: 3, overflow: TextOverflow.ellipsis, style: kit.ink(13))),
          const SizedBox(height: 8),
          Text('Built-in · Tap to install offline', style: kit.ink(12, FontWeight.w600, _green)),
        ]),
      ),
    );
  }

  Widget _installedCard(Map p) {
    final sel = p['id'] == _picked;
    final off = p['off'] != null;
    final uses = ((p['uses'] as List?) ?? const []).cast<Map>();
    final used = uses.isEmpty
        ? 'Installed · Not used in this animation'
        : 'In this animation: ${uses.map((u) => u['label']).take(2).join('; ')}${uses.length > 2 ? ' and ${uses.length - 2} more' : ''}';
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
          boxShadow: [
            BoxShadow(color: sel ? cAccent : const Color(0x0F000000), spreadRadius: sel ? 2 : 1),
            const BoxShadow(color: Color(0x1210141A), blurRadius: 8, offset: Offset(0, 2)),
          ],
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
          if (p['description'] != null)
            Padding(padding: const EdgeInsets.only(top: 4), child: Text('${p['description']}', maxLines: 3, overflow: TextOverflow.ellipsis, style: kit.ink(13))),
          const SizedBox(height: 8),
          if (off)
            Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              const Padding(padding: EdgeInsets.only(top: 1), child: Pictogram('x', size: 14, color: _orange)),
              const SizedBox(width: 6),
              Expanded(child: Text('Switched off: ${p['off']}', maxLines: 3, overflow: TextOverflow.ellipsis, style: kit.ink(12, FontWeight.w600, _orange))),
            ])
          else
            Text(used, maxLines: 2, overflow: TextOverflow.ellipsis, style: kit.ink(12, uses.isEmpty ? FontWeight.w500 : FontWeight.w600, uses.isEmpty ? _green : cAccent)),
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

  // ---------------------------------------------------------------- detail cards

  Widget _nothingPicked() => kit.PanelCard(
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: Text(
            'Select a preset or plugin to inspect.',
            style: kit.ink(13, FontWeight.w400, cSecondary),
          ),
        ),
      );

  Widget _presetDetail(_StockPreset p) {
    final rows = <rw.Desc>[
      rw.DHint(p.description),
      const rw.DTitle('Stacked effects'),
      for (var i = 0; i < p.effects.length; i++)
        rw.DInfo(
          '${i + 1}. ${p.summary[i]}',
          _formatPresetParams(p.effects[i]),
        ),
      if (_banner != null) rw.DHint(_banner!),
    ];
    return kit.PanelCard(
      padding: EdgeInsets.zero,
      child: ListView(padding: const EdgeInsets.fromLTRB(16, 12, 16, 16), children: [
        Row(children: [
          const Pictogram('fx', size: 26, color: cAccent),
          const SizedBox(width: 10),
          Expanded(
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(p.name, style: kit.ink(17, FontWeight.w700)),
              Text('Effect Preset · ${p.category}', style: kit.ink(12, FontWeight.w400, cSecondary)),
            ]),
          ),
          kit.Tap(label: 'Close', onTap: () => _pick(null), child: const Pictogram('x', size: 18, color: cSecondary)),
        ]),
        const SizedBox(height: 4),
        for (final d in rows) rw.renderDesc(d),
        const SizedBox(height: 12),
        kit.WordButton(
          widget.figureSelected ? 'Apply to selected' : 'Apply as Effect layer',
          primary: true,
          expand: true,
          height: 40,
          onTap: () => _applyPreset(p, asEffectLayer: false),
        ),
        if (widget.figureSelected) ...[
          const SizedBox(height: 8),
          kit.WordButton(
            'Add as new Effect layer',
            filled: true,
            expand: true,
            height: 38,
            onTap: () => _applyPreset(p, asEffectLayer: true),
          ),
        ],
        const SizedBox(height: 8),
        Text(
          'Adds all ${p.effects.length} effects in one undoable step. Every parameter stays editable and keyable in Properties.',
          style: kit.ink(12, FontWeight.w400, cSecondary),
        ),
      ]),
    );
  }

  String _formatPresetParams(Map<String, Object?> fx) {
    final params = (fx['params'] as Map?) ?? const {};
    if (params.isEmpty) return 'mix ${(((fx['mix'] as num?) ?? 1) * 100).round()}%';
    final parts = <String>[];
    for (final e in params.entries) {
      if (e.key == 'color') continue;
      final v = e.value;
      if (v is num) {
        parts.add('${e.key} ${v == v.roundToDouble() ? '${v.round()}' : v.toStringAsFixed(2)}');
      }
    }
    return parts.take(3).join(' · ');
  }

  Widget _stockDetail(_StockPlugin s) {
    final canAddDirectly = s.kind != 'effect' || widget.figureSelected;
    final addLabel = switch (s.kind) {
      'effect' => 'Install & add to figure',
      'modifier' => 'Install & apply',
      _ => 'Install & add to animation',
    };
    return kit.PanelCard(
      padding: EdgeInsets.zero,
      child: ListView(padding: const EdgeInsets.fromLTRB(16, 12, 16, 16), children: [
        Row(children: [
          Pictogram(_kindIcon[s.kind] ?? 'script', size: 26, color: cAccent),
          const SizedBox(width: 10),
          Expanded(
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(s.name, style: kit.ink(17, FontWeight.w700)),
              Text('Stock ${_kindName[s.kind] ?? s.kind} · by Barkfart · 1.0.0', style: kit.ink(12, FontWeight.w400, cSecondary)),
            ]),
          ),
          kit.Tap(label: 'Close', onTap: () => _pick(null), child: const Pictogram('x', size: 18, color: cSecondary)),
        ]),
        const SizedBox(height: 4),
        rw.renderDesc(rw.DHint(s.description)),
        if (_banner != null) rw.renderDesc(rw.DHint(_banner!)),
        rw.renderDesc(const rw.DTitle('The code')),
        _code(s.source),
        const SizedBox(height: 12),
        Row(children: [
          Expanded(
            flex: 9,
            child: kit.WordButton(
              'Install',
              filled: true,
              expand: true,
              height: 38,
              onTap: () => _installStock(s),
            ),
          ),
          const SizedBox(width: 8),
          Expanded(
            flex: 14,
            child: kit.WordButton(
              addLabel,
              primary: true,
              expand: true,
              height: 38,
              onTap: canAddDirectly
                  ? () async {
                      final ok = await _installStock(s);
                      if (ok && mounted) Navigator.pop(context, WorkshopAdd(s.id, s.kind));
                    }
                  : null,
            ),
          ),
        ]),
        if (s.kind == 'effect' && !widget.figureSelected)
          Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(
              'Tap Install now, then add it from any layer’s Effects panel (including Effect layers).',
              style: kit.ink(12, FontWeight.w400, cSecondary),
            ),
          ),
      ]),
    );
  }

  Widget _installedDetail(Map p) {
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
      if (_banner != null) rw.DHint(_banner!),
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
    final addLabel = switch (kind) { 'effect' => 'Add to figure', 'modifier' => 'Apply to property', _ => 'Add to animation' };
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
                child: kit.WordButton(
                  'Share the file…',
                  key: _shareKey,
                  filled: true,
                  expand: true,
                  height: 38,
                  onTap: !hasFile
                      ? null
                      : () {
                          final box = _shareKey.currentContext?.findRenderObject() as RenderBox?;
                          widget.share('${p['id']}', box == null ? Rect.zero : box.localToGlobal(Offset.zero) & box.size);
                        },
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                flex: 13,
                child: off != null && hasFile
                    ? kit.WordButton(
                        'Switch it on again',
                        primary: true,
                        expand: true,
                        height: 38,
                        onTap: () async {
                          await widget.reinstall('${p['id']}');
                          if (mounted) _load();
                        },
                      )
                    : kit.WordButton(addLabel, primary: true, expand: true, height: 38, onTap: canAdd ? () => Navigator.pop(context, WorkshopAdd('${p['id']}', kind)) : null),
              ),
            ]),
            if (kind == 'effect' && !widget.figureSelected && off == null)
              const Padding(
                padding: EdgeInsets.only(top: 6),
                child: Text('Select a stick figure first, then add it here or from any layer’s Effects.', style: TextStyle(color: cSecondary, fontSize: 12)),
              ),
          ]),
        ),
        Container(
          decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
          padding: const EdgeInsets.fromLTRB(16, 9, 16, 9),
          child: kit.WordButton(
            'Remove from Barkfart',
            filled: true,
            danger: true,
            expand: true,
            height: 36,
            onTap: () async {
              await widget.remove('${p['id']}');
              if (mounted) _pick(null);
            },
          ),
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

// ---------------------------------------------------------------- built-in offline catalog

const _stockPresets = <_StockPreset>[
  _StockPreset(
    id: 'preset.anime_aura',
    name: 'Anime Impact Aura',
    category: 'Action',
    description: 'High-energy electric rim glow, punchy ink outline, and crisp RGB split for power-ups and hits.',
    summary: ['Glow', 'Outline', 'Chromatic aberration'],
    effects: [
      {'type': 'glow', 'mix': 0.9, 'params': {'color': 0x38B6FFFF, 'radius': 16.0, 'strength': 2.2}},
      {'type': 'outline', 'mix': 1.0, 'params': {'color': 0x101828FF, 'width': 4.0}},
      {'type': 'chromatic.aberration', 'mix': 0.85, 'params': {'amount': 6.0, 'angle': 12.0}},
    ],
  ),
  _StockPreset(
    id: 'preset.crt_retro',
    name: 'CRT Retro',
    category: 'Retro',
    description: 'Classic tube-monitor scanlines, glitch tear slices, RGB phosphor split, film grain, and dark corners.',
    summary: ['Chromatic aberration', 'Scanlines & glitch', 'Film grain', 'Vignette'],
    effects: [
      {'type': 'chromatic.aberration', 'mix': 0.8, 'params': {'amount': 5.0, 'angle': 0.0}},
      {'type': 'stylize.scanlines', 'mix': 1.0, 'params': {'amount': 0.55, 'lines': 96.0, 'offset': 5.0, 'phase': 0.0}},
      {'type': 'noise.grain', 'mix': 0.85, 'params': {'amount': 0.22, 'size': 2.0, 'seed': 1.0}},
      {'type': 'vignette', 'mix': 1.0, 'params': {'amount': 0.65, 'softness': 0.5, 'color': 0x050812FF}},
    ],
  ),
  _StockPreset(
    id: 'preset.dreamy_bloom',
    name: 'Dreamy Bloom',
    category: 'Cinematic',
    description: 'Warm sunlit halo glow with lifted exposure, soft gamma, and gentle purple-tinted vignette.',
    summary: ['Glow', 'Exposure & contrast', 'Vignette'],
    effects: [
      {'type': 'glow', 'mix': 0.75, 'params': {'color': 0xFFE4C4FF, 'radius': 22.0, 'strength': 1.4}},
      {'type': 'color.grade', 'mix': 0.85, 'params': {'exposure': 0.2, 'contrast': 0.92, 'gamma': 1.12}},
      {'type': 'vignette', 'mix': 0.8, 'params': {'amount': 0.35, 'softness': 0.75, 'color': 0x2A1635FF}},
    ],
  ),
  _StockPreset(
    id: 'preset.speed_flash',
    name: 'Speed Flash',
    category: 'Action',
    description: 'Directional motion smear, wide chromatic burst, and punchy contrast for dashes and impact frames.',
    summary: ['Motion blur', 'Chromatic aberration', 'Exposure & contrast'],
    effects: [
      {'type': 'blur.motion', 'mix': 0.85, 'params': {'distance': 42.0, 'angle': 0.0}},
      {'type': 'chromatic.aberration', 'mix': 1.0, 'params': {'amount': 12.0, 'angle': 0.0}},
      {'type': 'color.grade', 'mix': 0.9, 'params': {'exposure': 0.35, 'contrast': 1.25, 'gamma': 1.0}},
    ],
  ),
  _StockPreset(
    id: 'preset.underwater_heatwave',
    name: 'Underwater Heatwave',
    category: 'Distort',
    description: 'Animated ripple distortion with deep aqua tint and ocean vignette. Keyframe Wave Phase for motion!',
    summary: ['Wave distort', 'Tint', 'Vignette'],
    effects: [
      {'type': 'distort.wave', 'mix': 1.0, 'params': {'amplitude': 10.0, 'frequency': 4.5, 'phase': 0.0, 'angle': 0.0}},
      {'type': 'tint', 'mix': 0.85, 'params': {'color': 0x1FA2D6FF, 'amount': 0.32}},
      {'type': 'vignette', 'mix': 0.9, 'params': {'amount': 0.5, 'softness': 0.65, 'color': 0x041E2EFF}},
    ],
  ),
  _StockPreset(
    id: 'preset.noir_silhouette',
    name: 'Noir Silhouette',
    category: 'Cinematic',
    description: 'High-contrast monochrome film grade with gritty 35mm grain and heavy dramatic vignette.',
    summary: ['Hue & saturation', 'Exposure & contrast', 'Film grain', 'Vignette'],
    effects: [
      {'type': 'color.adjust', 'mix': 1.0, 'params': {'hue': 0.0, 'saturation': 0.0, 'brightness': -0.05}},
      {'type': 'color.grade', 'mix': 1.0, 'params': {'exposure': -0.15, 'contrast': 1.65, 'gamma': 0.9}},
      {'type': 'noise.grain', 'mix': 0.9, 'params': {'amount': 0.35, 'size': 2.0, 'seed': 3.0}},
      {'type': 'vignette', 'mix': 1.0, 'params': {'amount': 0.78, 'softness': 0.45, 'color': 0x000000FF}},
    ],
  ),
];

const _stockPlugins = <_StockPlugin>[
  // ---- Scripts ----
  _StockPlugin(
    id: 'barkfart.spring_bounce',
    fileName: 'spring_bounce.luau',
    name: 'Spring Bounce',
    kind: 'script',
    description: 'Adds snappy spring-mass overshoot and squashy settle whenever a figure moves.',
    source: r'''--!strict
-- Spring Bounce: juicy damped spring follow-through on figure movement.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.spring_bounce",
	name = "Spring Bounce",
	version = "1.0.0",
	author = "Barkfart",
	description = "Adds snappy spring-mass overshoot and squashy settle whenever a figure moves.",
	targets = {
		figure = bf.figure { label = "Figure" },
	},
	props = {
		stiffness = bf.number { default = 0.32, min = 0.05, max = 1.0, step = 0.05, label = "Stiffness" },
		damping = bf.number { default = 0.80, min = 0.4, max = 0.96, step = 0.02, label = "Damping" },
		bounce = bf.number { default = 1.0, min = 0.0, max = 3.0, step = 0.1, label = "Bounce" },
		kick = bf.number { default = 14.0, min = 0.0, max = 80.0, step = 2.0, label = "Start hop" },
	},

	start = function(s, p)
		s.ox, s.oy = 0, 0
		s.vx, s.vy = 0, p.kick * 0.45
		s.rot, s.vrot = 0, 0
	end,

	step = function(s, p, t, scene)
		local fx = -scene.figure.velocity.x * t.dt * 0.35 * p.bounce
		local fy = -scene.figure.velocity.y * t.dt * 0.35 * p.bounce
		s.vx = (s.vx + (fx - s.ox) * p.stiffness) * p.damping
		s.vy = (s.vy + (fy - s.oy) * p.stiffness) * p.damping
		s.ox += s.vx
		s.oy += s.vy
		local tilt = -s.ox * 0.25
		s.vrot = (s.vrot + (tilt - s.rot) * p.stiffness) * p.damping
		s.rot += s.vrot
		return {
			figure = {
				move = { x = s.ox, y = s.oy },
				rotate = { root = s.rot },
			},
		}
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.camera_shake',
    fileName: 'camera_shake.luau',
    name: 'Camera Shake',
    kind: 'script',
    description: 'Impact shake with smooth decay. Point it at a figure, or at Camera X and Camera Y values.',
    source: r'''--!strict
-- Camera Shake: impact trauma shake with smooth decay for camera X/Y or any figure.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.camera_shake",
	name = "Camera Shake",
	version = "1.0.0",
	author = "Barkfart",
	description = "Impact shake with smooth decay. Point it at a figure, or at Camera X and Camera Y values.",
	targets = {
		figure = bf.figure { label = "Figure (optional)" },
		x = bf.value { label = "Camera X (optional)" },
		y = bf.value { label = "Camera Y (optional)" },
	},
	props = {
		amount = bf.number { default = 18.0, min = 0.0, max = 120.0, step = 1.0, label = "Strength" },
		frequency = bf.number { default = 18.0, min = 1.0, max = 60.0, step = 1.0, label = "Speed" },
		decay = bf.number { default = 1.4, min = 0.0, max = 10.0, step = 0.1, label = "Decay" },
		roll = bf.number { default = 2.0, min = 0.0, max = 20.0, step = 0.5, label = "Tilt" },
	},

	start = function(s, _p, t)
		s.p1 = t.random() * math.pi * 2
		s.p2 = t.random() * math.pi * 2
		s.p3 = t.random() * math.pi * 2
	end,

	step = function(s, p, t)
		local env = math.exp(-t.elapsed * p.decay)
		local w = t.elapsed * p.frequency
		local jx = (math.sin(w * 1.13 + s.p1) * 0.6 + t.random(-1, 1) * 0.4) * p.amount * env
		local jy = (math.cos(w * 1.47 + s.p2) * 0.6 + t.random(-1, 1) * 0.4) * p.amount * env
		local jr = math.sin(w * 0.91 + s.p3) * p.roll * env
		return {
			figure = { move = { x = jx, y = jy }, rotate = { root = jr } },
			x = jx,
			y = jy,
		}
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.orbit_bob',
    fileName: 'orbit_bob.luau',
    name: 'Orbit & Bob',
    kind: 'script',
    description: 'Smooth floating hover or elliptical orbit with gentle banking tilt.',
    source: r'''--!strict
-- Orbit & Bob: smooth hovering bob or circular/elliptical orbit motion.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.orbit_bob",
	name = "Orbit & Bob",
	version = "1.0.0",
	author = "Barkfart",
	description = "Smooth floating hover or elliptical orbit with gentle banking tilt.",
	targets = {
		figure = bf.figure { label = "Figure" },
	},
	props = {
		radius_x = bf.number { default = 32.0, min = 0.0, max = 300.0, step = 2.0, label = "Width" },
		radius_y = bf.number { default = 16.0, min = 0.0, max = 300.0, step = 2.0, label = "Height" },
		speed = bf.number { default = 0.75, min = -5.0, max = 5.0, step = 0.05, label = "Turns/sec" },
		tilt = bf.number { default = 8.0, min = -45.0, max = 45.0, step = 1.0, label = "Bank tilt" },
	},

	step = function(_s, p, t)
		local a = t.elapsed * p.speed * math.pi * 2
		local x = math.cos(a) * p.radius_x
		local y = math.sin(a) * p.radius_y
		local bank = math.cos(a) * p.tilt
		return {
			figure = {
				move = { x = x, y = y },
				rotate = { root = bank },
			},
		}
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.blink_flicker',
    fileName: 'blink_flicker.luau',
    name: 'Blink & Flicker',
    kind: 'script',
    description: 'Drives opacity, glow strength, or tint amount for damage blinks and neon flicker.',
    source: r'''--!strict
-- Blink & Flicker: crisp on/off blink or noisy neon flicker for any value.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.blink_flicker",
	name = "Blink & Flicker",
	version = "1.0.0",
	author = "Barkfart",
	description = "Drives opacity, glow strength, or tint amount for damage blinks and neon flicker.",
	targets = {
		value = bf.value { label = "Value" },
	},
	props = {
		rate = bf.number { default = 6.0, min = 0.5, max = 30.0, step = 0.5, label = "Blinks/sec" },
		duty = bf.number { default = 0.5, min = 0.05, max = 0.95, step = 0.05, label = "On share" },
		jitter = bf.number { default = 0.3, min = 0.0, max = 1.0, step = 0.05, label = "Randomness" },
		amount = bf.number { default = 1.0, min = 0.0, max = 4.0, step = 0.1, label = "Strength" },
	},

	step = function(_s, p, t)
		local phase = (t.elapsed * p.rate) % 1.0
		local on = if phase < p.duty then 1.0 else 0.0
		if t.random() < p.jitter * 0.45 then
			on = 1.0 - on
		end
		return { value = on * p.amount }
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.particle_burst',
    fileName: 'particle_burst.luau',
    name: 'Particle Burst',
    kind: 'script',
    description: 'Launches a figure or prop along a ballistic arc with seeded spread, gravity, and spin.',
    source: r'''--!strict
-- Particle Burst: ballistic launch with spread, gravity, drag, and spin.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.particle_burst",
	name = "Particle Burst",
	version = "1.0.0",
	author = "Barkfart",
	description = "Launches a figure or prop along a ballistic arc with seeded spread, gravity, and spin.",
	targets = {
		figure = bf.figure { label = "Particle" },
	},
	props = {
		speed = bf.number { default = 260.0, min = 0.0, max = 1500.0, step = 10.0, label = "Launch speed" },
		angle = bf.angle { default = 65.0, label = "Angle" },
		spread = bf.number { default = 35.0, min = 0.0, max = 180.0, step = 5.0, label = "Spread" },
		gravity = bf.number { default = 520.0, min = -1000.0, max = 2000.0, step = 20.0, label = "Gravity" },
		spin = bf.number { default = 180.0, min = -1440.0, max = 1440.0, step = 15.0, label = "Spin deg/s" },
		drag = bf.number { default = 0.04, min = 0.0, max = 0.5, step = 0.01, label = "Air drag" },
	},

	start = function(s, p, t)
		local a = math.rad(p.angle + t.random(-p.spread, p.spread))
		local spd = p.speed * t.random(0.75, 1.25)
		s.x, s.y = 0, 0
		s.vx = math.cos(a) * spd
		s.vy = math.sin(a) * spd
		s.rot = 0
		s.vrot = p.spin * t.random(0.6, 1.4)
	end,

	step = function(s, p, t)
		local keep = math.max(0.0, 1.0 - p.drag)
		s.vy -= p.gravity * t.dt
		s.vx *= keep
		s.vy *= keep
		s.x += s.vx * t.dt
		s.y += s.vy * t.dt
		s.rot += s.vrot * t.dt
		return {
			figure = {
				move = { x = s.x, y = s.y },
				rotate = { root = s.rot },
			},
		}
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.breathe',
    fileName: 'breathe.luau',
    name: 'Breathe',
    kind: 'script',
    description: 'A gentle rise and fall, so a figure standing still looks alive.',
    source: r'''--!strict
-- Breathe: a standing figure rises and falls a little, as if breathing.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.breathe", name = "Breathe", version = "1.0.0", author = "Barkfart",
	description = "A gentle rise and fall, so a figure standing still looks alive.",
	targets = { figure = bf.figure { label = "Figure" } },
	props = {
		depth = bf.number { default = 2, min = 0, max = 10, label = "Depth" },
		rate = bf.number { default = 0.3, min = 0.05, max = 2, label = "Breaths a second" },
	},

	step = function(s, p, t)
		-- each run starts at its own point of the breath
		s.phase = s.phase or t.random() * math.pi * 2
		local y = math.sin(s.phase + t.elapsed * p.rate * math.pi * 2) * p.depth
		return { figure = { move = { y = y } } }
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.sway',
    fileName: 'sway.luau',
    name: 'Sway',
    kind: 'script',
    description: 'The figure leans into its movement and settles when it stops.',
    source: r'''--!strict
-- Sway: a figure leans into its movement and settles when it stops, like a
-- body with a little weight. A Barkfart script (SCRIPTING.md "Script layers").
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.sway", name = "Sway", version = "1.0.0", author = "Barkfart",
	description = "The figure leans into its movement and settles when it stops.",
	targets = { figure = bf.figure { label = "Figure" } },
	props = {
		lean = bf.number { default = 0.6, min = 0, max = 2, label = "Lean" },
		stiffness = bf.number { default = 0.35, min = 0.05, max = 1, label = "Stiffness" },
	},

	start = function(s)
		s.angle, s.speed = 0, 0
	end,

	step = function(s, p, t, scene)
		-- moving right leans it back to the left, and the other way round
		local pull = -scene.figure.velocity.x * 0.02 * p.lean
		s.speed = (s.speed + (pull - s.angle) * p.stiffness) * 0.85
		s.angle += s.speed
		return { figure = { rotate = { root = s.angle } } }
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.pulse',
    fileName: 'pulse.luau',
    name: 'Pulse',
    kind: 'script',
    description: 'Makes a value rise and fall: point it at a glow’s strength or a segment’s length.',
    source: r'''--!strict
-- Pulse: any value that can be keyed (an effect's strength, a segment's
-- length, a plugin effect's setting) rises and falls on top of its keys.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.pulse", name = "Pulse", version = "1.0.0", author = "Barkfart",
	description = "Makes a value rise and fall: point it at a glow's strength or a segment's length.",
	targets = { value = bf.value { label = "Value" } },
	props = {
		amount = bf.number { default = 5, min = 0, max = 100, label = "Amount" },
		rate = bf.number { default = 1, min = 0.05, max = 10, label = "Pulses a second" },
	},

	step = function(s, p, t)
		return { value = math.abs(math.sin(t.elapsed * p.rate * math.pi)) * p.amount }
	end,
}
''',
  ),
  // ---- Shader Effects ----
  _StockPlugin(
    id: 'barkfart.trail_echo',
    fileName: 'trail_echo.luau',
    name: 'Trail & Ghost Echo',
    kind: 'effect',
    description: 'Draws tinted speed-afterimage echoes trailing behind a figure or dash.',
    source: r'''--!strict
-- Trail & Ghost Echo: directional fading afterimage copies behind a figure.
local bf = require("@barkfart")

return bf.effect {
	id = "barkfart.trail_echo",
	name = "Trail & Ghost Echo",
	version = "1.0.0",
	author = "Barkfart",
	description = "Draws tinted speed-afterimage echoes trailing behind a figure or dash.",
	props = {
		copies = bf.number { default = 5, min = 1, max = 10, step = 1, label = "Echoes" },
		spacing = bf.number { default = 0.018, min = 0.002, max = 0.1, step = 0.004, label = "Spacing" },
		angle = bf.angle { default = 180.0, label = "Trail angle" },
		fade = bf.number { default = 0.65, min = 0.0, max = 1.0, step = 0.05, label = "Opacity" },
		tint = bf.colour { default = bf.rgb(0.22, 0.76, 1.0), label = "Echo tint" },
	},
	shader = bf.wgsl([[
fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> {
    let rad = radians(u.angle);
    let step_uv = vec2<f32>(cos(rad), -sin(rad)) * u.spacing;
    var acc = vec4<f32>(0.0);
    let n = clamp(u.copies, 1.0, 10.0);
    for (var i = 10; i >= 1; i--) {
        let fi = f32(i);
        if (fi > n) { continue; }
        let s = layer(uv + step_uv * fi);
        let w = (1.0 - (fi - 1.0) / (n + 0.5)) * u.fade * 0.55;
        let tinted = vec4<f32>(mix(s.rgb, u.tint.rgb * s.a, 0.7) * w, s.a * w);
        acc = vec4<f32>(tinted.rgb + acc.rgb * (1.0 - tinted.a), tinted.a + acc.a * (1.0 - tinted.a));
    }
    let src = layer(uv);
    return vec4<f32>(src.rgb + acc.rgb * (1.0 - src.a), src.a + acc.a * (1.0 - src.a));
}
]]),
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.speed_lines',
    fileName: 'speed_lines.luau',
    name: 'Speed Lines',
    kind: 'effect',
    description: 'Radial anime action lines converging toward the center. Best on an Effect layer!',
    source: r'''--!strict
-- Anime Speed Lines: radial manga/anime action lines animated over time.
local bf = require("@barkfart")

return bf.effect {
	id = "barkfart.speed_lines",
	name = "Speed Lines",
	version = "1.0.0",
	author = "Barkfart",
	description = "Radial anime action lines converging toward the center. Best on an Effect layer!",
	props = {
		density = bf.number { default = 52.0, min = 12.0, max = 160.0, step = 4.0, label = "Density" },
		intensity = bf.number { default = 0.78, min = 0.0, max = 1.0, step = 0.05, label = "Intensity" },
		inner = bf.number { default = 0.30, min = 0.05, max = 0.85, step = 0.05, label = "Center clear" },
		speed = bf.number { default = 14.0, min = 0.0, max = 60.0, step = 2.0, label = "Flicker speed" },
		color = bf.colour { default = bf.rgb(1.0, 1.0, 1.0), label = "Line color" },
	},
	shader = bf.wgsl([[
fn sl_hash(n: u32) -> f32 {
    var x = (n ^ 61u) ^ (n >> 16u);
    x = x + (x << 3u);
    x = x ^ (x >> 4u);
    x = x * 0x27d4eb2du;
    x = x ^ (x >> 15u);
    return f32(x & 0xffffu) / 65535.0;
}

fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> {
    let src = layer(uv);
    let p = uv * 2.0 - vec2<f32>(1.0);
    let r = length(p);
    let ang = atan2(p.y, p.x);
    let norm_a = (ang + 3.14159265) / 6.2831853;
    let sector = floor(norm_a * u.density);
    let local_a = fract(norm_a * u.density);
    let tick = u32(floor(u.time * max(u.speed, 0.0)));
    let h = sl_hash(u32(sector) ^ (tick * 0x9e3779b9u + 13u));
    let h2 = sl_hash(u32(sector) * 3u + 7u ^ (tick * 0x85ebca6bu));
    let reach = u.inner + h * 0.48;
    let radial = smoothstep(reach, reach + 0.32, r);
    let spike = smoothstep(0.32, 0.04, abs(local_a - 0.5)) * step(0.38, h2);
    let la = clamp(radial * spike * u.intensity * u.color.a, 0.0, 1.0);
    let k = 1.0 - la;
    return vec4<f32>(u.color.rgb * la + src.rgb * k, la + src.a * k);
}
]]),
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.halftone_comic',
    fileName: 'halftone_comic.luau',
    name: 'Comic Halftone',
    kind: 'effect',
    description: 'Turns shading into crisp manga/comic halftone dots.',
    source: r'''--!strict
-- Comic Halftone: crisp manga/comic screentone dot shade preserving alpha.
local bf = require("@barkfart")

return bf.effect {
	id = "barkfart.halftone_comic",
	name = "Comic Halftone",
	version = "1.0.0",
	author = "Barkfart",
	description = "Turns shading into crisp manga/comic halftone dots.",
	props = {
		scale = bf.number { default = 6.0, min = 2.0, max = 24.0, step = 1.0, label = "Dot size" },
		contrast = bf.number { default = 1.25, min = 0.2, max = 3.0, step = 0.05, label = "Contrast" },
		amount = bf.number { default = 0.8, min = 0.0, max = 1.0, step = 0.05, label = "Amount" },
	},
	shader = bf.wgsl([[
fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> {
    let src = layer(uv);
    if (src.a <= 0.0) { return src; }
    let rgb = clamp(src.rgb / src.a, vec3<f32>(0.0), vec3<f32>(1.0));
    let luma = clamp((dot(rgb, vec3<f32>(0.299, 0.587, 0.114)) - 0.5) * u.contrast + 0.5, 0.0, 1.0);
    let cell = max(u.scale, 2.0);
    let px = uv * u.size;
    let rot = vec2<f32>(px.x * 0.866 - px.y * 0.5, px.x * 0.5 + px.y * 0.866);
    let g = fract(rot / cell) - vec2<f32>(0.5);
    let dist = length(g);
    let rad = sqrt(max(1.0 - luma, 0.0)) * 0.62;
    let dot_mask = smoothstep(rad + 0.06, rad - 0.06, dist);
    let toned = mix(rgb, rgb * 0.18, dot_mask);
    let out_rgb = mix(rgb, toned, clamp(u.amount, 0.0, 1.0));
    return vec4<f32>(out_rgb * src.a, src.a);
}
]]),
}
''',
  ),
  // ---- Modifiers ----
  _StockPlugin(
    id: 'barkfart.spring_overshoot',
    fileName: 'spring_overshoot.luau',
    name: 'Spring Overshoot',
    kind: 'modifier',
    description: 'Adds bouncy elastic follow-through after fast keyframe changes in the graph.',
    source: r'''--!strict
-- Spring Overshoot: curve modifier that adds elastic follow-through to any keyed value.
local bf = require("@barkfart")

return bf.modifier {
	id = "barkfart.spring_overshoot",
	name = "Spring Overshoot",
	version = "1.0.0",
	author = "Barkfart",
	description = "Adds bouncy elastic follow-through after fast keyframe changes in the graph.",
	props = {
		amount = bf.number { default = 0.45, min = 0.0, max = 2.0, step = 0.05, label = "Overshoot" },
		frequency = bf.number { default = 3.5, min = 0.5, max = 15.0, step = 0.25, label = "Frequency" },
		damping = bf.number { default = 3.0, min = 0.5, max = 12.0, step = 0.25, label = "Damping" },
	},

	value = function(v, p, t)
		local dt = 1.0 / math.max(t.fps, 1.0)
		local acc = 0.0
		local w = p.frequency * math.pi * 2
		for k = 1, 10 do
			local f1 = t.frame - k
			local f0 = f1 - 1
			local dv = v:at(f1) - v:at(f0)
			local age = k * dt
			acc += dv * math.exp(-age * p.damping) * math.sin(age * w)
		end
		return v.value + acc * p.amount
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.stutter_step',
    fileName: 'stutter_step.luau',
    name: 'Anime Stepped',
    kind: 'modifier',
    description: 'Holds a curve on every 2nd or 3rd frame for snappy hand-drawn anime timing.',
    source: r'''--!strict
-- Anime Stepped: holds curves on 2s, 3s, or Nth frames for hand-drawn timing.
local bf = require("@barkfart")

return bf.modifier {
	id = "barkfart.stutter_step",
	name = "Anime Stepped",
	version = "1.0.0",
	author = "Barkfart",
	description = "Holds a curve on every 2nd or 3rd frame for snappy hand-drawn anime timing.",
	props = {
		every = bf.number { default = 2, min = 1, max = 12, step = 1, label = "Hold frames" },
	},

	value = function(v, p, t)
		local step = math.max(1, math.floor(p.every + 0.5))
		return v:at(math.floor(t.frame / step) * step)
	end,
}
''',
  ),
  _StockPlugin(
    id: 'barkfart.flicker_noise',
    fileName: 'flicker_noise.luau',
    name: 'Organic Flicker',
    kind: 'modifier',
    description: 'Adds repeatable jitter to any curve: great for fire glow, neon signs, or handheld shake.',
    source: r'''--!strict
-- Organic Flicker: deterministic random jitter for handheld energy or flame intensity.
local bf = require("@barkfart")

return bf.modifier {
	id = "barkfart.flicker_noise",
	name = "Organic Flicker",
	version = "1.0.0",
	author = "Barkfart",
	description = "Adds repeatable jitter to any curve: great for fire glow, neon signs, or handheld shake.",
	props = {
		amount = bf.number { default = 0.3, min = 0.0, max = 100.0, step = 0.05, label = "Amount" },
		bias = bf.number { default = 0.0, min = -1.0, max = 1.0, step = 0.1, label = "Bias" },
	},

	value = function(v, p, t)
		local n = (t.random(-1, 1) + t.random(-1, 1)) * 0.5 + p.bias
		return v.value + n * p.amount
	end,
}
''',
  ),
];
