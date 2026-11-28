// The editor: sidebar left, frame strip on top, stage in the middle
// (docs/barkfart/UI-SPEC.md). All editing happens in the Rust session; this
// file draws its state and turns touches into commands.
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:file_picker/file_picker.dart';
import 'package:share_plus/share_plus.dart';

import 'engine.dart';
import 'files.dart';
import 'workshop.dart';
import 'ui/blend_picker.dart';
import 'ui/figure_studio.dart';
import 'ui/kit.dart' as kit;
import 'ui/pictograms.dart';
import 'ui/rows.dart' as rw;
import 'ui/target_picker.dart';
import 'ui/tokens.dart';

export 'ui/tokens.dart';

/// Wraps the whole app (dialogs included) for scripted screenshots.
final GlobalKey appShot = GlobalKey();

class EditorPage extends StatefulWidget {
  const EditorPage({super.key});

  @override
  State<EditorPage> createState() => _EditorPageState();
}

/// The stage view: centre (cx, cy) in stage units, s points per unit, y up.
class StageView {
  double cx = 0, cy = 0, s = 0.4;
  Size size = Size.zero;

  double get x0 => cx - size.width / 2 / s;
  double get x1 => cx + size.width / 2 / s;
  double get y0 => cy - size.height / 2 / s;
  double get y1 => cy + size.height / 2 / s;

  Offset toScreen(double x, double y) => Offset((x - x0) * s, (y1 - y) * s);
  Offset toStage(Offset p) => Offset(x0 + p.dx / s, y1 - p.dy / s);
}

class _EditorPageState extends State<EditorPage> with SingleTickerProviderStateMixin, WidgetsBindingObserver {
  Engine? _engine;
  String _fatal = '';
  Map<String, dynamic> _st = const {};
  final StageView _view = StageView();
  bool _fitted = false;
  /// The view was zoomed or panned by hand since it was last fitted: a
  /// rotation then keeps it as it is instead of fitting the stage again.
  bool _viewMoved = false;
  double _screenW = 0;

  ui.Image? _image;
  /// The stage area the picture was drawn for (stage units; in the camera's
  /// own units while playing): it is drawn there, so a resize or a pan shows
  /// it in place, never stretched, until the next picture.
  Rect _imageRect = Rect.zero;
  bool _imageCamera = false;
  bool _imageOwned = true;
  /// The editor state the shown picture was drawn from: handles and the
  /// camera frame use it, so they move together with the figure.
  Map<String, dynamic>? _pictureSt;
  /// Playback quality (fraction of full resolution) and the frames drawn
  /// while playing, reused on the next loop (cleared when anything changes).
  double _quality = 0.5;
  final Map<int, ui.Image> _frameCache = {};
  String _cacheKey = '';
  int _cacheBytes = 0;
  static const _cacheBudget = 160 * 1024 * 1024;

  void _setImage(ui.Image img, {required bool owned}) {
    if (_imageOwned && _image != null && !identical(_image, img)) _image!.dispose();
    _image = img;
    _imageOwned = owned;
  }

  void _clearCache() {
    for (final img in _frameCache.values) {
      if (identical(img, _image)) {
        _imageOwned = true;
      } else {
        img.dispose();
      }
    }
    _frameCache.clear();
    _cacheBytes = 0;
  }
  // what the current image shows; a new one is drawn when any of these change
  String _drawn = '';
  bool _busy = false;
  late final Ticker _ticker;
  double _dpr = 1;
  double _engineMs = 0;
  /// Isolates stage texture + handle overlay repaints from the rest of the editor
  /// widget tree so live joint drags, pan/zoom, and frame renders never rebuild
  /// the timeline, rails, or Properties card.
  final ValueNotifier<int> _stageTick = ValueNotifier<int>(0);
  final ValueNotifier<int> _zoomTick = ValueNotifier<int>(0);
  /// Latest stage coordinate received from high-frequency (120Hz/240Hz) touch
  /// moves; coalesced to at most one FFI `move` + render per vsync tick.
  Offset? _pendingDragMove;

  // playback
  bool _playing = false;
  final Stopwatch _clock = Stopwatch();
  double _playT = 0;

  // files
  ProjectFile? _file;
  String _status = '';

  final GlobalKey _shot = GlobalKey();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    try {
      _engine = Engine.open();
      if (_engine == null) _fatal = 'This device has no graphics support the engine can use.';
    } catch (e) {
      _fatal = 'The engine failed to load: $e';
    }
    _ticker = createTicker((_) {
      _tick();
      _thumbTick();
      _rowPicTick();
      _magTick();
      _timelineEdgeScrollTick();
    })..start();
    _start();
  }

  Future<void> _start() async {
    // the default stickfigure, for the figure editor's "Stock size" reference
    try {
      final bytes = (await rootBundle.load('assets/stock/figures/Stickfigure.nodes')).buffer.asUint8List();
      _engine?.setStock('Stickfigure.nodes', bytes);
    } catch (_) {}
    final e = _engine;
    if (e == null) return;
    var firstRun = false;
    final open = Platform.environment['BARKFART_OPEN'];
    if (open != null && open.isNotEmpty) {
      final err = e.load(File(open).readAsBytesSync());
      if (err != null) _say('Could not open $open: $err');
    } else {
      final rec = await Recovery.read();
      final last = await ProjectFile.mostRecent();
      if (rec != null && e.load(rec.$1) == null) {
        // unsaved work from last time comes back, still marked unsaved
        _file = rec.$2 == null ? null : ProjectFile(rec.$2!);
        e.command({'op': 'mark_unsaved'});
        _say('Restored unsaved changes', light: true);
      } else if (last != null && e.load(await last.read()) == null) {
        _file = last;
      } else {
        await _freshProject();
        firstRun = true;
      }
    }
    // K2: load saved plugins AFTER session exists so every plugin/modifier registers cleanly
    try {
      final dir = await pluginsDir();
      for (final f in dir.listSync().whereType<File>().where((f) => f.path.endsWith('.luau') || f.path.endsWith('.lua'))) {
        final st = e.command({'op': 'plugin_install', 'name': f.uri.pathSegments.last, 'source': await f.readAsString()});
        if (st?['installed'] case final String id) _pluginFiles[id] = f.path;
      }
    } catch (_) {}
    await AppSettings.load();
    final savedTh = AppSettings.values['timelineHeight'];
    if (savedTh is num && savedTh > 0) _timelineHeight = savedTh.toDouble();
    e.command({'op': 'set', 'k': 'axis', 'v': AppSettings.flag('axisLock')});
    e.command({'op': 'set', 'k': 'path', 'v': AppSettings.values['motionPath'] != false});
    var st0 = e.command({'op': 'state'});
    final script = Platform.environment['BARKFART_SCRIPT'];
    if (script == null && st0 != null && st0['selected'] == null) {
      final firstFig = ((st0['rows'] as List?) ?? const []).cast<Map>().where((r) => r['kind'] == 'figure').firstOrNull;
      if (firstFig != null) st0 = e.command({'op': 'select', 'layer': firstFig['id']}) ?? st0;
    }
    _refresh(st0);
    // draw every frame picture for the project just opened
    _thumbsChanged('open');
    _reminder = Timer.periodic(const Duration(seconds: 30), (_) => _remindToSave());
    if (script != null) Timer(const Duration(milliseconds: 1500), () => _runScript(script));
    // nothing saved yet: say hello first (REDESIGN §8, E1)
    if (firstRun && script == null && mounted) unawaited(_firstRun());
  }

  /// The first run (REDESIGN §8): no animations yet.
  /// The figure a new animation starts with, drawn by the engine (the real
  /// figure, not a likeness), w x h with transparency around it.
  Future<ui.Image?> _starterFigureImage(int w, int h) async {
    final e = _engine;
    if (e == null) return null;
    try {
      final bytes = (await rootBundle.load('assets/stock/figures/Stickfigure.nodes')).buffer.asUint8List();
      final px = e.renderFile('Stickfigure.nodes', bytes, w, h);
      if (px == null) return null;
      final c = Completer<ui.Image>();
      ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, c.complete);
      return await c.future;
    } catch (_) {
      return null;
    }
  }

  Future<void> _firstRun() async {
    final figure = await _starterFigureImage(280, 400);
    if (!mounted) return;
    await Navigator.of(context).push(PageRouteBuilder<void>(
        transitionDuration: const Duration(milliseconds: 120),
        pageBuilder: (c, a1, a2) => Scaffold(
          backgroundColor: cPanel,
          body: SafeArea(
            child: Column(children: [
              Container(
                height: 44,
                width: double.infinity,
                decoration: const BoxDecoration(color: cCard, border: Border(bottom: BorderSide(color: cLine))),
                child: Stack(alignment: Alignment.center, children: [
                  const Text('Your animations', style: TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w700)),
                  Positioned(
                    right: 8,
                    child: Row(children: [
                      kit.WordButton('Open a file…', onTap: () {
                        Navigator.pop(c);
                        _importFromFiles();
                      }),
                      kit.WordButton('Settings', onTap: _settingsDialog),
                    ]),
                  ),
                ]),
              ),
              Expanded(
                child: Center(
                  child: SingleChildScrollView(
                    child: Column(mainAxisSize: MainAxisSize.min, children: [
                      SizedBox(width: 140, height: 200, child: figure == null ? null : RawImage(image: figure, fit: BoxFit.contain, filterQuality: FilterQuality.medium)),
                      const SizedBox(height: 10),
                      const Text('No animations yet', style: TextStyle(color: cText, fontSize: 20, fontWeight: FontWeight.w700)),
                      const SizedBox(height: 4),
                      const Text('Start one, then pose your first stick figure.', style: TextStyle(color: cSecondary, fontSize: 14)),
                      const SizedBox(height: 14),
                      kit.WordButton('Make an animation', primary: true, height: 44, size: 16, onTap: () => Navigator.pop(c)),
                      const SizedBox(height: 18),
                      const kit.HintBubble('Stick Nodes projects, figures and movieclips (.stknds, .nodes, .nodemc) open here too: use "Open a file…".', width: 420),
                    ]),
                  ),
                ),
              ),
            ]),
          ),
        ),
      ));
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState s) {
    // keep work safe when the app goes to the background
    if (s == AppLifecycleState.paused || s == AppLifecycleState.inactive) {
      if (_st['dirty'] == true) _writeRecovery();
    }
  }

  /// Unsaved work goes to a hidden recovery copy, never to a new animation.
  Future<void> _writeRecovery() async {
    final e = _engine;
    if (e == null) return;
    final bytes = e.saveCopy();
    if (bytes != null) await Recovery.write(bytes, _file?.path);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _ticker.dispose();
    _reminder?.cancel();
    _clearPan?.cancel();
    _fitAnim?.cancel();
    _stripScroll.dispose();
    _layerVScroll.dispose();
    _panelScroll.dispose();
    _stageTick.dispose();
    _zoomTick.dispose();
    _clearCache();
    for (final img in _thumbs.values) {
      img.dispose();
    }
    _thumbs.clear();
    for (final img in _rowPics.values) {
      img.dispose();
    }
    _rowPics.clear();
    for (final m in _groupThumbs.values) {
      for (final img in m.values) {
        img.dispose();
      }
    }
    _groupThumbs.clear();
    if (_imageOwned) _image?.dispose();
    _engine?.dispose();
    super.dispose();
  }

  // ---------------------------------------------------------------- state

  void _refresh(Map<String, dynamic>? st) {
    if (st == null) return;
    final selChanged = st['selected']?['layer'] != _st['selected']?['layer'] ||
        st['script']?['id'] != _st['script']?['id'] ||
        st['group']?['id'] != _st['group']?['id'] ||
        st['textfield']?['layer'] != _st['textfield']?['layer'] ||
        st['picture']?['layer'] != _st['picture']?['layer'] ||
        st['movieclip']?['layer'] != _st['movieclip']?['layer'] ||
        st['adjustment']?['layer'] != _st['adjustment']?['layer'] ||
        st['mode'] != _st['mode'];
    if (_soundPage && (selChanged || st['frame'] != _st['frame'] || st['frameSound'] == null)) _soundPage = false;
    final hasAnySelection = st['mode'] == 'camera' ||
        st['selected'] != null ||
        st['script'] != null ||
        st['group'] != null ||
        st['textfield'] != null ||
        st['picture'] != null ||
        st['movieclip'] != null ||
        st['adjustment'] != null ||
        st['multi'] != null;
    if (_frameSelected && selChanged && hasAnySelection) _frameSelected = false;
    final buildingChanged = (st['building'] != null) != (_st['building'] != null);
    final enteringBuilder = st['building'] != null && _st['building'] == null;
    // another figure selected: once the finger is up, keep it clear of the card
    if (st['selected']?['layer'] != null && st['selected']?['layer'] != _st['selected']?['layer']) _wantClear = true;
    setState(() {
      if (buildingChanged) {
        _fitted = false;
        _viewMoved = false;
      }
      if (enteringBuilder && mounted) {
        final sz = MediaQuery.maybeSizeOf(context);
        if (sz != null && (sz.width < 900 || sz.height < 500)) _partsOpen = false;
      }
      _st = st;
    });
    if (_wantClear) WidgetsBinding.instance.addPostFrameCallback((_) => _keepSelectionClear());
    final m = st['message'] as String? ?? '';
    if (m.isNotEmpty) _say(m, light: m.startsWith('Imported ') || m.startsWith('Saved '));
  }

  Map<String, dynamic>? _cmd(Map<String, Object?> c) {
    final e = _engine;
    if (e == null) return null;
    final st = e.command(c);
    _refresh(st);
    _thumbsChanged(c['op'] as String? ?? '');
    return st;
  }

  /// A toast (REDESIGN D19): light for confirmations, dark for problems.
  void _say(String s, {bool light = false}) {
    if (!mounted || s.isEmpty) return;
    kit.showToast(context, s, light: light);
  }

  int get _frame => (_st['frame'] as num?)?.toInt() ?? 0;
  int get _frames => (_st['frames'] as num?)?.toInt() ?? 1;
  double get _fps => (_st['fps'] as num?)?.toDouble() ?? 30;
  Map<String, dynamic>? get _sel => _st['selected'] as Map<String, dynamic>?;
  Map<String, dynamic> get _opts => (_st['opts'] as Map<String, dynamic>?) ?? const {};

  // ---------------------------------------------------------------- drawing

  /// A selected figure the floating Properties card would hide (an iPad held
  /// upright, a narrow window): the view eases sideways so the figure sits in
  /// the open part of the stage. Never while a finger is down (it would pull
  /// the figure from under it) or while playing.
  bool _wantClear = false;
  Timer? _clearPan;

  void _keepSelectionClear() {
    if (!mounted || _pointers.isNotEmpty) return;
    _wantClear = false;
    final screen = MediaQuery.sizeOf(context);
    if (!_panelOpen || _hidePanels || _playing || _fsNow || _view.size.isEmpty) return;
    final cardW = screen.shortestSide > 600 ? 360.0 : 320.0;
    final open = _view.size.width - 8 - cardW; // the stage left of the card
    if (open < 120) return;
    double? x0, x1;
    for (final h in ((_st['handles'] as List?) ?? const []).cast<List>()) {
      if (h.length < 3 || ((h[2] as num).toInt() & 4) == 0) continue;
      final x = _view.toScreen((h[0] as num).toDouble(), (h[1] as num).toDouble()).dx;
      x0 = x0 == null ? x : math.min(x0, x);
      x1 = x1 == null ? x : math.max(x1, x);
    }
    if (x0 == null || x1 == null || x1 <= open - 16) return;
    // its middle to the middle of the open part (its left edge stays on screen)
    var by = open / 2 - (x0 + x1) / 2;
    if (x0 + by < 16) by = 16 - x0;
    if (by.abs() < 4) return;
    final from = _view.cx, to = _view.cx - by / _view.s;
    _clearPan?.cancel();
    final ms = kit.motion(context, 240).inMilliseconds;
    if (ms == 0) {
      setState(() => _view.cx = to);
      return;
    }
    final sw = Stopwatch()..start();
    _clearPan = Timer.periodic(const Duration(milliseconds: 16), (tm) {
      final u = (sw.elapsedMilliseconds / ms).clamp(0.0, 1.0);
      if (!mounted || _pointers.isNotEmpty) {
        tm.cancel();
        return;
      }
      setState(() => _view.cx = from + (to - from) * kit.kEase.transform(u));
      if (u >= 1) tm.cancel();
    });
  }

  void _fit() {
    final cam = (_st['camera'] as List?)?.map((v) => (v as num).toDouble()).toList();
    final size = _view.size;
    if (cam == null || cam.length < 8 || size.isEmpty) return;
    var minX = cam[0], maxX = cam[0], minY = cam[1], maxY = cam[1];
    for (var i = 0; i < 8; i += 2) {
      minX = math.min(minX, cam[i]);
      maxX = math.max(maxX, cam[i]);
      minY = math.min(minY, cam[i + 1]);
      maxY = math.max(maxY, cam[i + 1]);
    }
    _view.s = math.min(size.width / ((maxX - minX) * 1.08), size.height / ((maxY - minY) * 1.08));
    _view.cx = (minX + maxX) / 2;
    _view.cy = (minY + maxY) / 2;
    _fitted = true;
    _viewMoved = false;
  }

  Timer? _fitAnim;

  void _animateFit() {
    final cam = (_st['camera'] as List?)?.map((v) => (v as num).toDouble()).toList();
    final size = _view.size;
    if (cam == null || cam.length < 8 || size.isEmpty) return;
    var minX = cam[0], maxX = cam[0], minY = cam[1], maxY = cam[1];
    for (var i = 0; i < 8; i += 2) {
      minX = math.min(minX, cam[i]);
      maxX = math.max(maxX, cam[i]);
      minY = math.min(minY, cam[i + 1]);
      maxY = math.max(maxY, cam[i + 1]);
    }
    final toS = math.min(size.width / ((maxX - minX) * 1.08), size.height / ((maxY - minY) * 1.08));
    final toCx = (minX + maxX) / 2, toCy = (minY + maxY) / 2;
    final fromS = _view.s, fromCx = _view.cx, fromCy = _view.cy;
    _fitAnim?.cancel();
    _clearPan?.cancel();
    final ms = kit.motion(context, 180).inMilliseconds;
    if (ms <= 0) {
      setState(_fit);
      return;
    }
    final sw = Stopwatch()..start();
    _fitAnim = Timer.periodic(const Duration(milliseconds: 16), (tm) {
      if (!mounted || _pointers.isNotEmpty) {
        tm.cancel();
        return;
      }
      final u = (sw.elapsedMilliseconds / ms).clamp(0.0, 1.0);
      final e = kit.kEase.transform(u);
      setState(() {
        _view.s = fromS + (toS - fromS) * e;
        _view.cx = fromCx + (toCx - fromCx) * e;
        _view.cy = fromCy + (toCy - fromCy) * e;
        if (u >= 1) {
          _fitted = true;
          _viewMoved = false;
        }
      });
      if (u >= 1) tm.cancel();
    });
  }

  /// Merges a lightweight `{"op":"move","fast":true}` response into `_st`
  /// without rebuilding the surrounding editor widget tree. Allocates a shallow
  /// copy of `_st` so `_pictureSt` stays synchronized with the rendered texture
  /// until the new frame finishes decoding.
  void _applyDragPatch(Map<String, dynamic>? patch) {
    if (patch == null) return;
    final next = Map<String, dynamic>.of(_st);
    if (patch.containsKey('rev')) next['rev'] = patch['rev'];
    if (patch.containsKey('handles')) next['handles'] = patch['handles'];
    if (patch.containsKey('drawLine')) next['drawLine'] = patch['drawLine'];
    if (patch.containsKey('camera')) next['camera'] = patch['camera'];
    if (patch.containsKey('motionPath')) next['motionPath'] = patch['motionPath'];
    if (patch['textfieldPatch'] case final Map tp when next['textfield'] is Map) {
      next['textfield'] = {...(next['textfield'] as Map<String, dynamic>), ...tp.cast<String, dynamic>()};
    }
    if (patch['picturePatch'] case final Map pp when next['picture'] is Map) {
      next['picture'] = {...(next['picture'] as Map<String, dynamic>), ...pp.cast<String, dynamic>()};
    }
    if (patch['selectedPatch'] case final Map sp when next['selected'] is Map) {
      next['selected'] = {...(next['selected'] as Map<String, dynamic>), ...sp.cast<String, dynamic>()};
    }
    _st = next;
    final m = patch['message'] as String? ?? '';
    if (m.isNotEmpty) _say(m);
  }

  /// Flushes at most one coalesced touch-move coordinate per vsync right before
  /// the stage renders that frame.
  void _flushDragMove() {
    final p = _pendingDragMove;
    final e = _engine;
    if (p == null || e == null || _oneFinger != 1 || _busy) return;
    _pendingDragMove = null;
    final patch = e.command({'op': 'move', 'x': p.dx, 'y': p.dy, 'fast': true});
    _applyDragPatch(patch);
  }

  void _tick() {
    final e = _engine;
    if (e == null || _busy || _view.size.isEmpty || _st.isEmpty) return;
    _flushDragMove();
    if (!_fitted) _fit();
    double t;
    if (_playing) {
      // wall time: frame delays and repeats stretch it (the engine's schedule)
      final dur = e.wallTotal;
      // a steady clock started with the sound: the sound player only knows its
      // position per audio buffer (tens of ms, and ahead of what is heard), which
      // made tweened playback hold and jump
      var secs = _clockOffset + _clock.elapsedMicroseconds / 1e6;
      // past the loop end, round again from the loop start ("Loop to")
      final (ls, le) = (_loopStartWall, _loopEndWall > 0 ? _loopEndWall : dur);
      // the toolbar's Loop off: play once and stop on the last frame
      if (secs >= le && le > ls && AppSettings.values['loopPlay'] == false) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (!_playing) return;
          _togglePlay();
          _cmd({'op': 'frame', 'i': _frames - 1});
        });
        return;
      }
      if (secs >= le && le > ls) secs = ls + (secs - le) % (le - ls);
      // one picture per frame, or more with tweening on (Animation Tools)
      final rate = (_st['rate'] as num?)?.toDouble() ?? _fps;
      t = e.wallTime(dur > 0 ? secs % dur : 0, rate);
      if (t != _playT) {
        final prevFrame = (_playT * _fps).floor();
        final nextFrame = (t * _fps).floor();
        if (nextFrame != prevFrame) {
          setState(() => _playT = t);
        } else {
          _playT = t;
        }
      }
    } else {
      t = _frame / _fps;
    }
    final scale = math.min(_dpr, 1600 / _view.size.longestSide) * (_playing ? _quality : 1.0);
    final w = (_view.size.width * scale).round(), h = (_view.size.height * scale).round();
    if (w <= 0 || h <= 0) return;
    final camera = _playing;
    final onion = !_playing && _opts['onion'] == true && _frames > 1;
    // View options: outline the selected figure (editing only)
    final outline = !_playing && AppSettings.flag('outlineSelection') && _sel != null;
    // View options "Show filters" (stage), or the playback switch
    final filters = _playing ? (AppSettings.flag('playFilters') ? 2 : 0) : AppSettings.number('showFilters');
    double x0, y0, x1, y1;
    if (camera) {
      // the shot fitted into the stage area
      final sw = (_st['w'] as num).toDouble(), sh = (_st['h'] as num).toDouble();
      final k = math.min(_view.size.width / sw, _view.size.height / sh);
      x0 = -_view.size.width / 2 / k;
      x1 = -x0;
      y0 = -_view.size.height / 2 / k;
      y1 = -y0;
    } else {
      x0 = _view.x0;
      y0 = _view.y0;
      x1 = _view.x1;
      y1 = _view.y1;
    }
    // a repeat's in-between blending into another frame is its own picture
    final redirect = camera ? e.wallRedirect : -1.0;
    final key = '${_st['rev']} $t $redirect $w $h $x0 $y0 $x1 $y1 $camera $onion $filters ${outline ? _sel!['layer'] : ''}';
    if (key == _drawn) return;
    final fi = (t * ((_st['rate'] as num?)?.toDouble() ?? _fps)).round() + (redirect >= 0 ? 1000000 * (1 + (redirect * _fps).round()) : 0);
    if (camera) {
      final ck = '${_st['rev']} $w $h $filters';
      if (ck != _cacheKey) {
        _clearCache();
        _cacheKey = ck;
      }
      final cached = _frameCache[fi];
      if (cached != null) {
        _drawn = key;
        if (!identical(cached, _image)) {
          _setImage(cached, owned: false);
          _imageRect = Rect.fromLTRB(x0, y0, x1, y1);
          _imageCamera = true;
          _stageTick.value++;
        }
        return;
      }
    }
    _drawn = key;
    _busy = true;
    final sw = Stopwatch()..start();
    final px = e.renderView(t, w, h, x0, y0, x1, y1, camera: camera, onion: onion, outline: outline, filters: filters);
    if (px == null) {
      _busy = false;
      return;
    }
    final ms = e.lastMicros / 1000.0;
    final drawnFrom = _st;
    ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, (img) {
      if (!mounted) {
        img.dispose();
        return;
      }
      _pictureSt = drawnFrom;
      final keep = camera && _cacheBytes + w * h * 4 <= _cacheBudget && '${_st['rev']} $w $h' == _cacheKey;
      if (keep) {
        _frameCache[fi] = img;
        _cacheBytes += w * h * 4;
      }
      _setImage(img, owned: !keep);
      _imageRect = Rect.fromLTRB(x0, y0, x1, y1);
      _imageCamera = camera;
      _engineMs = ms;
      _stageTick.value++;
      if (Platform.environment['BARKFART_TRACE'] != null) debugPrint('picture ${w}x$h engine ${ms.toStringAsFixed(1)} ms, total ${sw.elapsedMicroseconds / 1000} ms${camera ? ' (cached ${_frameCache.length})' : ''}');
      _busy = false;
    });
  }

  // ---------------------------------------------------------------- touches

  final Map<int, Offset> _pointers = {};
  // what the first finger is doing: 0 none, 1 editing (engine), 2 panning the view
  int _oneFinger = 0;
  Offset _panFrom = Offset.zero;
  // pinch
  double _pinchDist = 0;
  Offset _pinchMid = Offset.zero;
  double _pinchStartDist = 0;
  Offset _pinchStartMid = Offset.zero;
  bool _pinchEngaged = false;
  // multi-finger tap recognition: 2-finger tap = Undo, 3-finger tap = Redo
  int _tapMaxFingers = 0;
  DateTime _tapGestureStart = DateTime(0);
  final Map<int, Offset> _tapDownPos = {};
  bool _tapMoved = false;
  DateTime _lastTapUp = DateTime(0);
  Offset _downAt = Offset.zero;
  bool _moved = false;

  // a stylus can aim at a joint; a finger needs a bigger target
  double get _hitRadius => (AppSettings.flag('stylus') ? 14 : 32) / _view.s;

  void _down(PointerDownEvent ev) {
    if (_playing) {
      _togglePlay();
      return;
    }
    _pointers[ev.pointer] = ev.localPosition;
    _tapDownPos[ev.pointer] = ev.localPosition;
    if (_pointers.length > _tapMaxFingers) _tapMaxFingers = _pointers.length;
    if (_frameSelected) setState(() => _frameSelected = false);
    if (_pointers.length == 1) {
      _tapMaxFingers = 1;
      _tapGestureStart = DateTime.now();
      _tapDownPos
        ..clear()
        ..[ev.pointer] = ev.localPosition;
      _tapMoved = false;
      _pinchEngaged = false;
      _downAt = ev.localPosition;
      _moved = false;
      final p = _view.toStage(ev.localPosition);
      final st = _cmd({'op': 'down', 'x': p.dx, 'y': p.dy, 'r': _hitRadius});
      if (st?['hit'] == true) {
        _oneFinger = 1;
        // Settings "Haptic feedback": a tick when a joint is grabbed
        if (AppSettings.flag('haptics')) unawaited(HapticFeedback.selectionClick());
      } else {
        _oneFinger = 2;
        _panFrom = ev.localPosition;
      }
    } else if (_pointers.length == 2) {
      // a second finger: cancel any nascent 1-finger joint drag immediately so
      // 2-finger tap Undo / pinch never nudges a joint
      _pendingDragMove = null;
      if (_oneFinger == 1) _cmd({'op': 'cancel'});
      _oneFinger = 0;
      final ps = _pointers.values.toList();
      _pinchDist = (ps[0] - ps[1]).distance;
      _pinchMid = (ps[0] + ps[1]) / 2;
      _pinchStartDist = _pinchDist;
      _pinchStartMid = _pinchMid;
      _pinchEngaged = false;
    } else if (_pointers.length == 3) {
      _pendingDragMove = null;
      if (_oneFinger == 1) _cmd({'op': 'cancel'});
      _oneFinger = 0;
      _pinchEngaged = false;
    } else {
      _pendingDragMove = null;
      if (_oneFinger == 1) _cmd({'op': 'cancel'});
      _oneFinger = 0;
      _tapMoved = true;
    }
  }

  void _move(PointerMoveEvent ev) {
    if (!_pointers.containsKey(ev.pointer)) return;
    _pointers[ev.pointer] = ev.localPosition;
    final startPos = _tapDownPos[ev.pointer];
    if (startPos != null && (ev.localPosition - startPos).distance > 12) _tapMoved = true;
    if ((ev.localPosition - _downAt).distance > 6) _moved = true;
    if (_pointers.length == 1) {
      if (_oneFinger == 1) {
        // Coalesce 120Hz/240Hz touch moves to at most one FFI `move` per vsync tick.
        _pendingDragMove = _view.toStage(ev.localPosition);
      } else if (_oneFinger == 2) {
        final d = ev.localPosition - _panFrom;
        _panFrom = ev.localPosition;
        _view.cx -= d.dx / _view.s;
        _view.cy += d.dy / _view.s;
        _viewMoved = true;
        _stageTick.value++;
        _zoomTick.value++;
      } else if (_tapMaxFingers >= 2 && _tapMoved) {
        // After a real pinch/pan lifts to 1 finger, resume smooth 1-finger pan
        _oneFinger = 2;
        _panFrom = ev.localPosition;
      }
    } else if (_pointers.length == 2) {
      final ps = _pointers.values.toList();
      final dist = (ps[0] - ps[1]).distance;
      final mid = (ps[0] + ps[1]) / 2;
      // Guard against 2-finger tap jitter before engaging pinch/pan
      if (!_pinchEngaged) {
        if ((dist - _pinchStartDist).abs() <= 12 && (mid - _pinchStartMid).distance <= 12) return;
        _pinchEngaged = true;
        _tapMoved = true;
        _pinchDist = dist;
        _pinchMid = mid;
        return;
      }
      // keep the stage point under the old midpoint under the new one
      final anchor = _view.toStage(_pinchMid);
      if (_pinchDist > 1 && dist > 1) _view.s = (_view.s * dist / _pinchDist).clamp(0.02, 40.0);
      final now = _view.toStage(mid);
      _view.cx += anchor.dx - now.dx;
      _view.cy += anchor.dy - now.dy;
      _viewMoved = true;
      _stageTick.value++;
      _zoomTick.value++;
      _pinchDist = dist;
      _pinchMid = mid;
    }
  }

  Object? _lastFigTap;
  DateTime _lastFigTapAt = DateTime(0);

  void _up(PointerEvent ev) {
    if (!_pointers.containsKey(ev.pointer)) return;
    _pointers.remove(ev.pointer);
    if (ev is PointerCancelEvent) _tapMoved = true;
    if (_pointers.isEmpty && _wantClear) WidgetsBinding.instance.addPostFrameCallback((_) => _keepSelectionClear());
    if (_pointers.isEmpty) {
      final elapsedMs = DateTime.now().difference(_tapGestureStart).inMilliseconds;
      final maxFingers = _tapMaxFingers;
      final wasMoved = _tapMoved;
      _tapMaxFingers = 0;
      _tapDownPos.clear();
      _pinchEngaged = false;

      // 2-finger tap = Undo, 3-finger tap = Redo
      if (!wasMoved && maxFingers == 2 && elapsedMs <= 300) {
        _oneFinger = 0;
        if (_st['canUndo'] == true) {
          final label = _st['undo'] as String? ?? '';
          _cmd({'op': 'undo'});
          if (AppSettings.flag('haptics')) unawaited(HapticFeedback.lightImpact());
          if (mounted) kit.showToast(context, label.isNotEmpty ? 'Undid $label' : 'Undid', light: true, duration: const Duration(milliseconds: 1600));
        } else if (mounted) {
          kit.showToast(context, 'Nothing to undo', light: true, duration: const Duration(milliseconds: 1200));
        }
        return;
      }
      if (!wasMoved && maxFingers == 3 && elapsedMs <= 350) {
        _oneFinger = 0;
        if (_st['canRedo'] == true) {
          final label = _st['redo'] as String? ?? '';
          _cmd({'op': 'redo'});
          if (AppSettings.flag('haptics')) unawaited(HapticFeedback.lightImpact());
          if (mounted) kit.showToast(context, label.isNotEmpty ? 'Redid $label' : 'Redid', light: true, duration: const Duration(milliseconds: 1600));
        } else if (mounted) {
          kit.showToast(context, 'Nothing to redo', light: true, duration: const Duration(milliseconds: 1200));
        }
        return;
      }

      if (_oneFinger == 1) {
        final pending = _pendingDragMove;
        _pendingDragMove = null;
        if (ev is! PointerCancelEvent && pending != null) {
          _engine?.command({'op': 'move', 'x': pending.dx, 'y': pending.dy, 'fast': true});
        }
        _cmd({'op': ev is PointerCancelEvent ? 'cancel' : 'up'});
        // Settings: double-tapping a figure offers to make it a movieclip
        final now = DateTime.now(), layer = _sel?['layer'];
        if (!_moved && layer != null && _st['building'] == null && AppSettings.flag('doubleTapMovieclip')) {
          if (layer == _lastFigTap && now.difference(_lastFigTapAt).inMilliseconds < 350) {
            _lastFigTap = null;
            unawaited(_ask('Convert to movieclip?', 'This figure and the figures joined to it become a movieclip that plays their animation in a loop.', ['Cancel', 'Convert']).then((a) {
              if (a == 'Convert') _cmd({'op': 'to_movieclip'});
            }));
          } else {
            _lastFigTap = layer;
            _lastFigTapAt = now;
          }
        }
      }
      if (_oneFinger == 2 && !_moved && maxFingers == 1) {
        // Click/tap away on empty stage background dismisses floating overlays cleanly
        if (_panelOpen || _viewOptions || _fxBrowser || _modBrowser) {
          setState(() {
            _panelOpen = false;
            _viewOptions = false;
            _fxBrowser = false;
            _modBrowser = false;
          });
        }
        final now = DateTime.now();
        if (now.difference(_lastTapUp).inMilliseconds < 300) {
          // Settings: in the figure editor a stage double-tap picks the None tool
          if (_st['building'] != null && AppSettings.flag('doubleTapNone')) {
            _cmd({'op': 'draw_tool', 'shape': 'none'});
          } else {
            setState(_fit);
          }
          _lastTapUp = DateTime(0);
        } else {
          _lastTapUp = now;
        }
      }
      _oneFinger = 0;
    } else if (_pointers.length == 1 && _pinchEngaged) {
      // back to one finger after a real pinch: it pans
      _oneFinger = 2;
      _panFrom = _pointers.values.first;
    }
  }

  void _scroll(PointerSignalEvent ev) {
    if (ev is PointerScrollEvent) {
      final anchor = _view.toStage(ev.localPosition);
      _view.s = (_view.s * math.pow(0.998, ev.scrollDelta.dy)).clamp(0.02, 40.0);
      final now = _view.toStage(ev.localPosition);
      _view.cx += anchor.dx - now.dx;
      _view.cy += anchor.dy - now.dy;
      _viewMoved = true;
      _stageTick.value++;
      _zoomTick.value++;
    }
  }

  // ---------------------------------------------------------------- actions

  /// Animation Tools "Fullscreen": the next playback shows the stage only
  /// (a tap stops it); one-shot, like the original.
  bool _fullscreen = false, _fsNow = false;

  void _togglePlay() {
    final e = _engine;
    if (e == null) return;
    setState(() {
      if (_playing) {
        _playing = false;
        if (_fsNow) {
          _fsNow = false;
          _fullscreen = false;
        }
        e.stop();
        _clock.stop();
      } else {
        _playing = true;
        _fsNow = _fullscreen;
        final start = e.frameWall(_frame);
        // "Loop to": back to frame 1 or to the start frame; stop after the end frame
        _loopStartWall = _loopToStart ? start : 0;
        _loopEndWall = (_loopEnd > 0 && _loopEnd < _frames) ? e.frameWall(_loopEnd) : 0;
        if (_loopEndWall <= _loopStartWall) _loopEndWall = 0;
        _clock
          ..reset()
          ..start();
        // the clock starts at the current frame
        _clockOffset = start;
        e.play(start, loopStart: _loopStartWall, loopEnd: _loopEndWall > 0 ? _loopEndWall : e.wallTotal);
      }
    });
  }

  double _clockOffset = 0;

  // "Loop to" (long-press Play): where playback goes round
  bool _loopToStart = false;
  int _loopEnd = 0;
  double _loopStartWall = 0, _loopEndWall = 0;

  /// Paste Stickfigure Properties: what to paste, every part ticked at first.
  Future<void> _pastePropsDialog() async {
    const parts = [
      ('position', 'Position', 'Puts this figure where the copied one stood.'),
      ('scale', 'Stickfigure scale', ''),
      ('color', 'Stickfigure colour', ''),
      ('rotation', 'Stickfigure rotation', ''),
      ('filters', 'Stickfigure filters', 'Opacity and every filter.'),
      ('layering', 'Layering', 'Tries to put it at the same place in front / behind.'),
      ('nodes', 'Node properties', 'Every joint\'s angle, length, colour and thickness; only for the same kind of figure.'),
    ];
    final on = {for (final p in parts) p.$1: true};
    final go = await showDialog<bool>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) => AlertDialog(
          backgroundColor: cPanel,
          title: const Text('Paste properties', style: TextStyle(color: cText, fontSize: 16)),
          content: SizedBox(
            width: 340,
            child: ListView(shrinkWrap: true, children: [
              Text('Copied from: ${_st['propsFrom'] ?? '?'}', style: const TextStyle(color: cSecondary, fontSize: 12)),
              const SizedBox(height: 8),
              for (final p in parts) ...[
                _Toggle(p.$2, on[p.$1]!, (v) => set(() => on[p.$1] = v)),
                if (p.$3.isNotEmpty) _hint(p.$3),
                const SizedBox(height: 4),
              ],
            ]),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(c, false), child: const Text('Cancel')),
            TextButton(onPressed: () => Navigator.pop(c, true), child: const Text('Paste')),
          ],
        ),
      ),
    );
    if (go == true) _cmd({'op': 'paste_props', ...on});
  }

  Future<void> _loopToPopup() async {
    final play = await kit.showSheet<bool>(
      context,
      title: 'Loop range',
      width: 360,
      children: (c) => [
        StatefulBuilder(
          builder: (c, set) => Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            const kit.Hint('Choose where playback loops back when it reaches the end frame.'),
            kit.Segmented(const ['From frame 1', 'From playhead'], _loopToStart ? 1 : 0, onSelect: (i) => set(() => _loopToStart = i == 1)),
            kit.StepRow('End frame', _loopEnd == 0 ? 'Last ($_frames)' : '$_loopEnd', onMinus: _loopEnd > 0 ? () => set(() => _loopEnd = math.max(0, _loopEnd - 1)) : null, onPlus: (_loopEnd == 0 ? _frames : _loopEnd) < _frames ? () => set(() => _loopEnd = math.min(_frames, _loopEnd + 1)) : null),
            if (_loopToStart || _loopEnd > 0)
              Padding(
                padding: const EdgeInsets.only(top: 4, bottom: 4),
                child: kit.WordButton('Reset loop range', filled: true, expand: true, height: 34, size: 13, onTap: () {
                  set(() {
                    _loopToStart = false;
                    _loopEnd = 0;
                  });
                  setState(() {});
                }),
              ),
            const SizedBox(height: 8),
            kit.ButtonPair('Close', () {
              setState(() {});
              Navigator.pop(c, false);
            }, 'Play', () {
              setState(() {});
              Navigator.pop(c, true);
            }, true),
          ]),
        ),
      ],
    );
    if (play == true && !_playing) _togglePlay();
  }

  /// Writes the project to f and forgets the recovery copy.
  Future<bool> _saveTo(ProjectFile f, {bool quiet = false}) async {
    final e = _engine;
    if (e == null) return false;
    final bytes = e.save();
    if (bytes == null) {
      _say('Saving failed: ${e.lastError}');
      return false;
    }
    await f.write(bytes);
    await _writeThumb(f);
    _savedAt = DateTime.now();
    _file = f;
    await Recovery.clear();
    _refresh(e.command({'op': 'state'}));
    if (!quiet) _say('Saved ${f.name}', light: true);
    return true;
  }

  static File _thumbFile(ProjectFile f) {
    final sep = Platform.pathSeparator;
    return File('${File(f.path).parent.path}$sep.thumbs$sep${f.name}.png');
  }

  /// A card's line on the Projects screen: "24 frames, 2 seconds. Edited today".
  String _cardMeta(ProjectFile f) {
    final when = File(f.path).lastModifiedSync();
    final now = DateTime.now();
    final days = DateTime(now.year, now.month, now.day).difference(DateTime(when.year, when.month, when.day)).inDays;
    final edited = switch (days) {
      <= 0 => 'today',
      1 => 'yesterday',
      < 7 => '$days days ago',
      < 14 => 'last week',
      < 31 => '${days ~/ 7} weeks ago',
      < 62 => 'last month',
      _ => _friendlyTime(when).split(',').first,
    };
    try {
      final note = File('${_thumbFile(f).path.substring(0, _thumbFile(f).path.length - 4)}.json');
      if (note.existsSync()) {
        final m = jsonDecode(note.readAsStringSync()) as Map;
        final n = (m['frames'] as num).toInt(), fps = (m['fps'] as num).toDouble();
        final secs = n / (fps > 0 ? fps : 30);
        final st = secs == secs.roundToDouble() ? '${secs.round()}' : secs.toStringAsFixed(1);
        return '$n frame${n == 1 ? '' : 's'}${n > 1 ? ', $st second${st == '1' ? '' : 's'}' : ''}. Edited $edited';
      }
    } catch (_) {}
    return 'Edited $edited';
  }

  /// A picture of the first frame beside the save, for the Projects screen.
  Future<void> _writeThumb(ProjectFile f) async {
    final img = _thumbs[0];
    if (img == null) return;
    try {
      final data = await img.toByteData(format: ui.ImageByteFormat.png);
      if (data == null) return;
      final out = _thumbFile(f);
      await out.parent.create(recursive: true);
      await out.writeAsBytes(data.buffer.asUint8List());
      // the card's "24 frames, 2 seconds" (Projects board)
      await File('${out.path.substring(0, out.path.length - 4)}.json').writeAsString(jsonEncode({'frames': _frames, 'fps': _fps}));
    } catch (_) {
      // a missing picture only means a plain card
    }
  }

  /// The Projects screen (REDESIGN §8): every saved animation as a card,
  /// "New animation" first.
  Future<void> _projectsScreen() async {
    final files = await ProjectFile.all();
    if (!mounted) return;
    await Navigator.of(context).push(PageRouteBuilder<void>(
      transitionDuration: const Duration(milliseconds: 120),
      pageBuilder: (c, a1, a2) {
        Future<void> open(ProjectFile? f) async {
          if (!await _leaveUnsaved()) return;
          if (c.mounted) Navigator.pop(c);
          if (f == null) {
            await _newProject();
          } else {
            await _openFile(f);
          }
        }

        Widget card(String name, String meta, Widget picture, VoidCallback onTap, {bool isNew = false, bool current = false, VoidCallback? onLongPress}) => GestureDetector(
              onTap: onTap,
              onLongPress: onLongPress,
              onSecondaryTap: onLongPress, // right-click on Linux
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                AspectRatio(
                  aspectRatio: 16 / 9,
                  child: Container(
                    clipBehavior: Clip.antiAlias,
                    decoration: BoxDecoration(
                      color: isNew ? const Color(0xFFFAFBFC) : cCard,
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(color: current ? cAccent : isNew ? const Color(0xFFC9CED5) : cLine, width: current ? 2 : 1.5),
                    ),
                    child: picture,
                  ),
                ),
                const SizedBox(height: 6),
                Row(children: [
                  Flexible(child: Text(name, maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: isNew ? cAccent : cText, fontSize: 15, fontWeight: FontWeight.w700))),
                  if (current && _st['dirty'] == true) Container(width: 7, height: 7, margin: const EdgeInsets.only(left: 6), decoration: const BoxDecoration(color: Color(0xFFF2A93B), shape: BoxShape.circle)),
                ]),
                if (meta.isNotEmpty) Text(meta, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cSecondary, fontSize: 12)),
              ]),
            );
        final cols = MediaQuery.sizeOf(c).width > 1000 ? 4 : 3;
        // a search and an order only once the list is long enough to need them
        final many = files.length > 6;
        var filter = '';
        var order = 0; // Newest, A–Z, Oldest
        return Scaffold(
          backgroundColor: cPanel,
          body: SafeArea(
            child: Column(children: [
              Container(
                height: 44,
                width: double.infinity,
                decoration: const BoxDecoration(color: cCard, border: Border(bottom: BorderSide(color: cLine))),
                child: Stack(alignment: Alignment.center, children: [
                  const Text('Your animations', style: TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w700)),
                  Positioned(left: 4, child: kit.WordButton('‹ Back', onTap: () => Navigator.pop(c))),
                  Positioned(
                    right: 8,
                    child: Row(children: [
                      kit.WordButton('Open a file…', onTap: () {
                        Navigator.pop(c);
                        _importFromFiles();
                      }),
                      kit.WordButton('Backups', onTap: () async {
                        final f = await _backupsSheet();
                        if (f != null && c.mounted) await open(f);
                      }),
                      kit.WordButton('Settings', onTap: _settingsDialog),
                    ]),
                  ),
                ]),
              ),
              Expanded(
                child: StatefulBuilder(builder: (c2, setList) {
                  final shown = files.where((f) => f.name.toLowerCase().contains(filter.toLowerCase())).toList();
                  if (order == 1) shown.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
                  if (order == 2) shown.setAll(0, shown.reversed.toList());
                  return Column(children: [
                    if (many)
                      Padding(
                        padding: const EdgeInsets.fromLTRB(24, 16, 24, 0),
                        child: Row(children: [
                          SizedBox(
                            width: 260,
                            child: TextField(
                              style: const TextStyle(color: cText, fontSize: 14),
                              decoration: InputDecoration(
                                isDense: true,
                                filled: true,
                                fillColor: cControl,
                                hintText: 'Search',
                                prefixIcon: const Icon(Icons.search, size: 18, color: Color(0xFF8A9099)),
                                contentPadding: const EdgeInsets.symmetric(vertical: 10),
                                border: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: BorderSide.none),
                              ),
                              onChanged: (v) => setList(() => filter = v),
                            ),
                          ),
                          const Spacer(),
                          SizedBox(width: 240, child: kit.Segmented(const ['Newest', 'A–Z', 'Oldest'], order, onSelect: (i) => setList(() => order = i))),
                        ]),
                      ),
                    Expanded(
                      child: GridView.count(
                  crossAxisCount: cols,
                  padding: const EdgeInsets.all(24),
                  mainAxisSpacing: 18,
                  crossAxisSpacing: 20,
                  childAspectRatio: 1.35,
                  children: [
                    if (filter.isEmpty) card('New animation', '', const Center(child: Pictogram('plus', size: 30, color: cAccent, stroke: 2.2)), () => open(null), isNew: true),
                    for (final f in shown)
                      card(
                        f.name,
                        _cardMeta(f),
                        _thumbFile(f).existsSync() ? Image.file(_thumbFile(f), fit: BoxFit.cover) : const Center(child: Pictogram('figure', size: 34, color: cTag)),
                        () => f.path == _file?.path ? Navigator.pop(c) : open(f),
                        current: f.path == _file?.path,
                        // hold a card to delete that animation (its backups stay)
                        onLongPress: f.path == _file?.path
                            ? null
                            : () async {
                                final ok = await _ask('Delete ${f.name}?', 'The animation is removed from this device. Its backups stay under Backups.', ['Cancel', 'Delete']);
                                if (ok != 'Delete') return;
                                try {
                                  File(f.path).deleteSync();
                                  if (_thumbFile(f).existsSync()) _thumbFile(f).deleteSync();
                                } catch (_) {}
                                if (c.mounted) Navigator.pop(c);
                                await _projectsScreen();
                              },
                      ),
                  ],
                      ),
                    ),
                  ]);
                }),
              ),
            ]),
          ),
        );
      },
    ));
  }

  /// Backups (REDESIGN §8): the earlier saves kept beside each animation,
  /// newest first; Restore opens one as an unsaved copy.
  Future<ProjectFile?> _backupsSheet() async {
    // a backup is named "<animation> (YYYY-MM-DD HH.MM.SS).bark": the time
    // that version was saved, which is what the row shows
    final re = RegExp(r'^(.*) \((\d{4})-(\d\d)-(\d\d) (\d\d)\.(\d\d)\.(\d\d)\)$');
    final groups = <String, List<(DateTime, ProjectFile)>>{};
    for (final f in await ProjectFile.pastSaves()) {
      final m = re.firstMatch(f.name);
      final t = m == null ? File(f.path).lastModifiedSync() : DateTime(int.parse(m[2]!), int.parse(m[3]!), int.parse(m[4]!), int.parse(m[5]!), int.parse(m[6]!), int.parse(m[7]!));
      (groups[m?[1] ?? f.name] ??= []).add((t, f));
    }
    for (final g in groups.values) {
      g.sort((a, b) => b.$1.compareTo(a.$1));
    }
    final names = groups.keys.toList()..sort((a, b) => groups[b]!.first.$1.compareTo(groups[a]!.first.$1));
    // the open animation first
    final cur = _file?.name;
    if (cur != null && names.remove(cur)) names.insert(0, cur);
    String label(DateTime t) => _friendlyTime(t, seconds: true);

    if (!mounted) return null;
    return kit.showSheet<ProjectFile>(context, title: 'Backups', width: 460, children: (c) => [
          const kit.Hint('Saving over an animation keeps the version it replaces (the last 10 of each). Restore opens one as an unsaved copy.'),
          if (names.isEmpty) const Padding(padding: EdgeInsets.symmetric(vertical: 12), child: Text('No backups yet. They appear after you save an animation a second time.', textAlign: TextAlign.center, style: TextStyle(color: cSecondary))),
          for (final n in names) ...[
            Padding(padding: const EdgeInsets.only(top: 8, bottom: 2), child: Text(n, style: kit.ink(14, FontWeight.w700))),
            for (final (t, f) in groups[n]!) kit.PropRow(label(t), height: 38, trailing: kit.WordButton('Restore', size: 13, onTap: () => Navigator.pop(c, f))),
          ],
          const SizedBox(height: 6),
          kit.WordButton('Close', filled: true, expand: true, height: 38, onTap: () => Navigator.pop(c)),
        ]);
  }

  /// The Save button: asks for a name the first time, confirms after that.
  Future<bool> _save() async {
    final f = _file;
    if (f == null) return _saveAs();
    final choice = await _ask('Save changes to "${f.name}"?', 'This replaces the saved version.', ['Cancel', 'Save as copy…', 'Save']);
    if (choice == 'Save') return _saveTo(f);
    if (choice == 'Save as copy…') return _saveAs();
    return false;
  }

  Future<bool> _saveAs() async {
    final suggestion = _file == null ? (await ProjectFile.fresh()).name : '${_file!.name} copy';
    final name = await _askName('Save animation', suggestion);
    if (name == null) return false;
    final f = await named(name);
    if (File(f.path).existsSync() && f.path != _file?.path) {
      final ok = await _ask('"${f.name}" already exists.', 'Replace it with this animation?', ['Cancel', 'Replace']);
      if (ok != 'Replace') return false;
    }
    return _saveTo(f);
  }

  /// Before leaving unsaved work: true to go on.
  Future<bool> _leaveUnsaved() async {
    if (_st['dirty'] != true) return true;
    final choice = await _ask('Save changes${_file == null ? '' : ' to "${_file!.name}"'}?', 'Your changes are lost if you don\'t save them.', ['Cancel', 'Don\'t save', 'Save']);
    if (choice == 'Save') return _file == null ? _saveAs() : _saveTo(_file!);
    if (choice == 'Don\'t save') {
      await Recovery.clear();
      return true;
    }
    return false;
  }

  /// A question as the kit's sheet (REDESIGN B1): the last button is the
  /// main one (blue); ones that throw work away read red.
  Future<String?> _ask(String title, String body, List<String> buttons) {
    bool lose(String b) => b == 'Don\'t save' || b == 'Discard' || b.startsWith('Delete') || b.startsWith('Clear') || b.startsWith('Remove');
    final losing = {for (final b in buttons) if (lose(b)) b};
    return kit.showSheet<String>(context, title: title, width: 420, children: (c) => [
          kit.Hint(body),
          Row(children: [
            for (var i = 0; i < buttons.length; i++) ...[
              if (i > 0) const SizedBox(width: 8),
              Expanded(
                child: kit.WordButton(buttons[i],
                    onTap: () => Navigator.pop(c, buttons[i]),
                    primary: i == buttons.length - 1 && !losing.contains(buttons[i]),
                    danger: losing.contains(buttons[i]),
                    filled: i < buttons.length - 1 || losing.contains(buttons[i]),
                    expand: true,
                    height: 38),
              ),
            ],
          ]),
        ]);
  }

  Future<String?> _askName(String title, String initial) {
    final ctl = TextEditingController(text: initial)..selection = TextSelection(baseOffset: 0, extentOffset: initial.length);
    return kit.showSheet<String>(
      context,
      title: title,
      width: 380,
      children: (c) => [
        TextField(
          controller: ctl,
          autofocus: true,
          style: const TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w600),
          decoration: InputDecoration(
            isDense: true,
            filled: true,
            fillColor: cControl,
            hintText: 'Name',
            contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            enabledBorder: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: const BorderSide(color: cLine)),
            focusedBorder: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: const BorderSide(color: cAccent, width: 1.5)),
          ),
          onSubmitted: (v) => Navigator.pop(c, v.trim().isEmpty ? null : v.trim()),
        ),
        const SizedBox(height: 10),
        kit.ButtonPair('Cancel', () => Navigator.pop(c), 'Save', () => Navigator.pop(c, ctl.text.trim().isEmpty ? null : ctl.text.trim()), true),
      ],
    );
  }

  Future<void> _newProject() async {
    if (!await _leaveUnsaved()) return;
    await _freshProject();
    _file = null;
    _fitted = false;
    _frameSelected = false;
    var st0 = _engine?.command({'op': 'state'});
    if (Platform.environment['BARKFART_SCRIPT'] == null && st0 != null && st0['selected'] == null) {
      final firstFig = ((st0['rows'] as List?) ?? const []).cast<Map>().where((r) => r['kind'] == 'figure').firstOrNull;
      if (firstFig != null) st0 = _engine?.command({'op': 'select', 'layer': firstFig['id']}) ?? st0;
    }
    _refresh(st0);
    _thumbsChanged('new');
    _say('New animation', light: true);
  }

  /// A new project starting with the original's default stickfigure.
  Future<void> _freshProject() async {
    final e = _engine;
    if (e == null) return;
    try {
      final bytes = (await rootBundle.load('assets/stock/figures/Stickfigure.nodes')).buffer.asUint8List();
      e.newProjectWith('Stickfigure.nodes', bytes);
    } catch (_) {
      e.newProject();
    }
  }

  Future<void> _openFile(ProjectFile chosen) async {
    final err = _engine?.load(await chosen.read());
    if (err != null) {
      _say('Could not open ${chosen.name}: $err');
      return;
    }
    // a past save opens as an unsaved copy (Save asks for a name)
    _file = chosen.isPastSave ? null : chosen;
    if (chosen.isPastSave) _engine?.command({'op': 'mark_unsaved'});
    _fitted = false;
    _frameSelected = false;
    _refresh(_engine?.command({'op': 'state'}));
    _thumbsChanged('open');
  }

  ExportJob? _export;

  /// After an export: the file's name, and Share (the system share sheet:
  /// save the video to Photos, send it, open it in another app).
  Future<void> _exportDone(String path, {bool scripted = false}) async {
    if (!mounted || (Platform.environment['BARKFART_SCRIPT'] != null && !scripted)) return;
    final name = path.split(Platform.pathSeparator).last;
    final share = await _ask('Export finished', '$name is in the Barkfart Exports folder (Files app on iPhone and iPad).', ['Done', 'Share…']);
    if (share != 'Share…' || !mounted) return;
    final box = context.findRenderObject() as RenderBox?;
    final at = box == null ? null : box.localToGlobal(Offset.zero) & box.size;
    try {
      await SharePlus.instance.share(ShareParams(files: [XFile(path)], sharePositionOrigin: at));
    } catch (err) {
      _say('Could not share: $err');
    }
  }

  Future<void> _runExport(int format, int w, int h, int start, int count, bool transparent, String title) async {
    final e = _engine;
    if (e == null || _export != null) return;
    final dir = await exportsDir();
    final name = title.replaceAll(RegExp(r'[^A-Za-z0-9 _-]'), '').trim();
    final stamp = DateTime.now().toIso8601String().replaceAll(':', '-').split('.').first;
    final base = '${dir.path}/${name.isEmpty ? 'Animation' : name} $stamp';
    final out = format == 0 ? '$base.gif' : format == 2 ? '$base.mov' : '$base.zip';
    final job = e.export(format, out, w, h, start: start, count: count, transparent: transparent);
    if (job == null) {
      _say('Export failed: ${e.lastError}');
      return;
    }
    // a video carries its sound; GIF and PNG frames get it as a WAV beside them
    if (format != 2) e.exportSoundtrack('$base.wav', start: start, count: count);
    _export = job;
    _say('Exporting…');
    Timer.periodic(const Duration(milliseconds: 16), (tm) {
      final j = _export;
      if (j == null) {
        tm.cancel();
        return;
      }
      final sw = Stopwatch()..start();
      try {
        while (sw.elapsedMilliseconds < 20) {
          final n = j.step();
          if (n == null) {
            _export = null;
            tm.cancel();
            _say('Exported to Barkfart Exports (Files app)', light: true);
            unawaited(_exportDone(out));
            return;
          }
          if (mounted) setState(() => _status = 'Exporting frame $n of ${j.count}');
        }
      } catch (err) {
        _export = null;
        tm.cancel();
        _say('Export failed: $err');
      }
    });
  }

  // ---------------------------------------------------------------- script (laptop checks)

  /// BARKFART_SCRIPT="drag 6 40 -30; shot /tmp/a.png; undo; shot /tmp/b.png; quit"
  /// Touches go through the same code as real fingers.
  Future<void> _runScript(String script) async {
    for (final raw in script.split(';')) {
      final w = raw.trim().split(RegExp(r'\s+'));
      if (w.isEmpty || w[0].isEmpty) continue;
      debugPrint('script: ${w.join(' ')}');
      switch (w[0]) {
        case 'wait':
          await Future<void>.delayed(Duration(milliseconds: (double.parse(w[1]) * 1000).round()));
        case 'drag':
          // drag handle i by (dx, dy) screen points
          final hs = _st['handles'] as List;
          final h = hs[int.parse(w[1])] as List;
          final from = _view.toScreen((h[0] as num).toDouble(), (h[1] as num).toDouble());
          final to = from + Offset(double.parse(w[2]), double.parse(w[3]));
          await _fakeDrag(from, to);
        case 'hold':
          // scripted: press handle i and move by (dx, dy) screen points, still held
          final h = (_st['handles'] as List)[int.parse(w[1])] as List;
          final from = _view.toScreen((h[0] as num).toDouble(), (h[1] as num).toDouble());
          _down(PointerDownEvent(pointer: 78, position: from));
          for (var i = 1; i <= 8; i++) {
            await Future<void>.delayed(const Duration(milliseconds: 16));
            _move(PointerMoveEvent(pointer: 78, position: from + Offset(double.parse(w[2]), double.parse(w[3])) * (i / 8)));
          }
        case 'plugin':
          // scripted: install a plugin file (path)
          final path = w.sublist(1).join(' ');
          final st = _cmd({'op': 'plugin_install', 'name': path.split('/').last, 'source': await File(path).readAsString()});
          if (st?['installed'] case final String id) _pluginFiles[id] = path;
        case 'modadd':
          // scripted: a modifier on the selected value (kind [plugin id])
          _cmd({'op': 'modifier_add', 'property': _curveSel, 'kind': w[1], if (w.length > 2) 'id': w[2]});
          setState(() => _modOpen = (((_curveValue?['mods'] as List?) ?? const []).length) - 1);
        case 'modbrowser':
          setState(() => _modBrowser = true);
        case 'iconshot':
          // scripted: the app icon (iconshot <png> <size>): the engine's own
          // render of the stock figure on a tile
          final size = int.parse(w[2]);
          final fig = await _starterFigureImage((size * 0.56).round(), (size * 0.8).round());
          final rec = ui.PictureRecorder();
          final cv = Canvas(rec);
          final tile = RRect.fromRectAndRadius(Rect.fromLTWH(size * 0.04, size * 0.04, size * 0.92, size * 0.92), Radius.circular(size * 0.2));
          cv.drawRRect(tile.shift(Offset(0, size * 0.012)), Paint()..color = const Color(0x33000000)..maskFilter = MaskFilter.blur(BlurStyle.normal, size * 0.012));
          cv.drawRRect(tile, Paint()..shader = ui.Gradient.linear(Offset(0, size * 0.04), Offset(0, size * 0.96), const [Color(0xFFF7F9FC), Color(0xFFDCE6F2)]));
          if (fig != null) {
            final dst = Rect.fromCenter(center: Offset(size / 2, size * 0.5), width: size * 0.56, height: size * 0.8);
            cv.drawImageRect(fig, Rect.fromLTWH(0, 0, fig.width.toDouble(), fig.height.toDouble()), dst, Paint()..filterQuality = FilterQuality.high);
          }
          final img = await rec.endRecording().toImage(size, size);
          final png = await img.toByteData(format: ui.ImageByteFormat.png);
          if (png != null) await File(w[1]).writeAsBytes(png.buffer.asUint8List());
        case 'workshop':
          // scripted: the Workshop (optionally a plugin picked)
          unawaited(_openWorkshop(picked: w.length > 1 ? w[1] : null));
        case 'swipe' || 'mswipe':
          // real touches through Flutter's gestures (window points):
          // swipe x1 y1 x2 y2 [hold ms before moving]; x1 y1 = x2 y2 is a tap.
          // mswipe: the same with the mouse's left button
          final v = [for (final x in w.sublist(1)) double.parse(x)];
          await _realSwipe(Offset(v[0], v[1]), Offset(v[2], v[3]), holdMs: v.length > 4 ? v[4].round() : 0, mouse: w[0] == 'mswipe');
        case 'pinch':
          // real two-finger touches: pinch x y fromGap toGap (window points)
          final v = [for (final x in w.sublist(1)) double.parse(x)];
          await _realPinch(Offset(v[0], v[1]), v[2], v[3]);
          debugPrint('script: zoom ${_cellW.toStringAsFixed(1)} curveScroll ${_curveScroll.toStringAsFixed(1)} frame $_frame');
        case 'pan':
          final from = Offset(_view.size.width - 30, _view.size.height - 30);
          await _fakeDrag(from, from + Offset(double.parse(w[1]), double.parse(w[2])));
        case 'tap':
          final hs = _st['handles'] as List;
          final h = hs[int.parse(w[1])] as List;
          final p = _view.toScreen((h[0] as num).toDouble(), (h[1] as num).toDouble());
          await _fakeDrag(p, p);
        case 'cmd':
          _cmd({'op': w[1], for (var i = 2; i + 1 < w.length; i += 2) w[i]: num.tryParse(w[i + 1]) ?? (w[i + 1] == 'true' ? true : w[i + 1] == 'false' ? false : w[i + 1])});
        case 'play':
          if (!_playing) _togglePlay();
        case 'stop':
          if (_playing) _togglePlay();
        case 'save':
          // scripted: no dialog
          await _saveTo(await named(w.length > 1 ? w.sublist(1).join(' ') : 'Script'));
        case 'shot':
          await Future<void>.delayed(const Duration(milliseconds: 400));
          await _saveShot(w[1]);
        case 'state':
          debugPrint('state: ${_st['frame']}/${_st['frames']} undo=${_st['undo']} selected=${_sel?['node']} handles=${(_st['handles'] as List).length} ms=${_engineMs.toStringAsFixed(1)}');
          if (w.length > 1) debugPrint('handles: ${_st['handles']}');
        case 'import':
          // scripted: import a file (path) onto this frame
          final f = File(w.sublist(1).join(' '));
          _refresh(_engine?.importFile(f.path.split(Platform.pathSeparator).last, await f.readAsBytes()));
          _thumbsChanged('import');
        case 'export':
          // scripted: GIF (0), PNG zip (1) or video (2) at 320 wide, the whole animation
          await _runExport(int.parse(w[1]), 320, (320 * (_st['h'] as num) / (_st['w'] as num)).round(), 0, 0, false, 'Script');
        case 'exportwindow':
          unawaited(_exportDialog());
        case 'copypopup':
          unawaited(_copyPopup(int.parse(w[1])));
        case 'toast':
          _say(w.sublist(1).join(' '));
        case 'view':
          setState(() => _viewOptions = true);
        case 'projects':
          unawaited(_projectsMenu());
        case 'animation':
          unawaited(_animationPopover());
        case 'add':
          unawaited(_addMenu());
        case 'firstrun':
          unawaited(_firstRun());
        case 'projectsscreen':
          unawaited(_projectsScreen());
        case 'editcopy':
          _cmd({'op': 'edit_copy'});
        case 'editcopysheet':
          unawaited(_editCopySheet('${_sel?['name'] ?? 'The figure'}'));
        case 'layers':
          setState(() => _layersOpen = true);
        case 'effects':
          await _openFxBrowser();
        case 'selectrow':
          // test helper: select the figure on timeline row N
          final rows = ((_st['rows'] as List?) ?? const []).cast<Map>();
          final r = rows[int.parse(w[1])];
          _cmd(r['kind'] == 'group' ? {'op': 'select', 'group': r['id']} : {'op': 'select', 'layer': r['id'], if (r['kind'] == 'picture' || r['kind'] == 'movieclip') 'node': ''});
        case 'joinhere':
          // test helper: the selected figure joins the figure on row N from this frame
          final rows2 = ((_st['rows'] as List?) ?? const []).cast<Map>();
          _cmd({'op': 'join_here', 'layer': rows2[int.parse(w[1])]['id']});
        case 'letgo':
          _cmd({'op': 'let_go_here'});
        case 'jointo':
          // test helper: the selected figure joins the figure on row N (its main joint)
          final rows = ((_st['rows'] as List?) ?? const []).cast<Map>();
          _cmd({'op': 'join_to', 'layer': rows[int.parse(w[1])]['id']});
        case 'openrows':
          setState(() => _openRows.addAll([for (final r in ((_st['rows'] as List?) ?? const []).cast<Map>()) '${r['id']}']));
        case 'fxopen':
          setState(() => _fxOpen = w[1]);
        case 'fx':
          _toggleFx();
        case 'blockdrag':
          // test helper: the selected figure's pose, its grip pulled (or held
          // and slid, with "move") by dx
          final pose = _sel?['pose'] as Map?;
          if (pose != null) {
            final mode = w.length > 2 ? w[2] : 'end';
            final edge = mode == 'move' ? 1 : mode == 'start' ? 2 : 0;
            final kind = mode == 'move' ? 'pose_key' : mode == 'start' ? 'block_start' : 'block_end';
            _startTimelineDrag(_BlockDrag('${_sel!['layer']}', (pose['start'] as num).toInt(), (pose['length'] as num).toInt(), _cellW, edge: edge, kind: kind));
            _updateTimelineDrag(double.parse(w[1]), absolute: true);
          }
        case 'blockdrop':
          _endBlockDrag();
        case 'selectall':
          // test helper: every figure row selected together
          final ids = [for (final r in ((_st['rows'] as List?) ?? const []).cast<Map>()) if (r['kind'] == 'figure') r['id']];
          _cmd({'op': 'select_many', 'layers': ids});
        case 'boxdemo':
          // test helper: a lasso shape drawn over the rows, not let go
          setState(() {
            _lasso = true;
            _boxPath = [const Offset(20, 70), const Offset(160, 62), const Offset(200, 110), const Offset(90, 130), const Offset(30, 105)];
          });
        case 'rowactions':
          // test helper: open the first row's swipe actions
          setState(() => _rowActionsOpen = true);
        case 'zoomtimeline':
          setState(() => _cellW = double.parse(w[1]).clamp(14.0, 112.0));
        case 'timelineheight':
          setState(() => _timelineHeight = w[1] == 'auto' ? null : double.parse(w[1]).clamp(88.0, 560.0));
        case 'backups':
          unawaited(_backupsSheet());
        case 'exportfull':
          _vertical = true;
          _cmd({'op': 'watermark', 'on': true});
          unawaited(_exportDialog());
        case 'makeprojects':
          // test helper: n saved animations with distinct names
          for (var i = 0; i < (w.length > 1 ? int.parse(w[1]) : 8); i++) {
            final d = await projectsDir();
            await File('${d.path}/Take ${String.fromCharCode(65 + i)}.bark').writeAsBytes((await File(_file!.path).readAsBytes()));
          }
        case 'savesheet':
          unawaited(_saveSheet('Rooftop fight hasn\u2019t been saved for 10 minutes. The reminder can be changed or turned off in Settings.'));
        case 'savebutton':
          unawaited(_save());
        case 'picker':
          unawaited(_figurePicker());
        case 'back':
          if (mounted) Navigator.of(context).maybePop();
        case 'sounds':
          unawaited(_importSounds());
        case 'framesound':
          // scripted: this frame plays the first imported sound (and its page)
          final l = (_st['soundList'] as List?) ?? const [];
          if (l.isNotEmpty) _cmd({'op': 'frame_sound', 'asset': (l.first as Map)['id']});
          setState(() => _soundPage = true);
        case 'frametools':
          setState(() => _frameSelected = true);
        case 'tab':
          _jumpTo(w.sublist(1).join(' '));
        case 'magnifier':
          setState(() => _magnifier = true);
        case 'colorpicker':
          // scripted: the picker on the selected segment's colour
          unawaited(_colorPicker(((_sel?['color'] as num?) ?? 0xff).toInt(), (c) => _cmd({'op': 'segment', 'color': c})));
        case 'loopto':
          unawaited(_loopToPopup());
        case 'uiscale':
          // test helper: the UI size
          AppSettings.values['uiScale'] = double.parse(w[1]);
          AppSettings.uiScale.value = double.parse(w[1]);
        case 'settings':
          unawaited(_settingsDialog());
        case 'set':
          // scripted: a setting (bool or number)
          AppSettings.values[w[1]] = num.tryParse(w[2])?.toInt() ?? (w[2] == 'true');
          setState(() {});
        case 'arrows':
          setState(() => _arrows = true);
        case 'pastelayering':
          unawaited(_pasteLayeringPopup());
        case 'library':
          if (w.length > 1) setState(() => _libType = int.parse(w[1]));
          unawaited(_libraryDialog());
        case 'cameralock':
          unawaited(_cameraLockDialog());
        case 'autocamera':
          unawaited(_autoCameraDialog());
        case 'savefigure':
          final bytes = _engine?.figureFile();
          if (bytes != null) await File(w[1]).writeAsBytes(bytes);
        case 'bgimage':
          final ok = _engine?.editorImage(await File(w[1]).readAsBytes()) ?? false;
          setState(() {
            _editorImage = ok;
            _drawn = '';
          });
        case 'fullscreen':
          setState(() => _fullscreen = true);
        case 'exportdone':
          unawaited(_exportDone(w.sublist(1).join(' '), scripted: true));
        case 'importfont':
          // scripted: a font file onto the selected textfield
          final f = File(w.sublist(1).join(' '));
          _refresh(_engine?.importFile(f.path.split(Platform.pathSeparator).last, await f.readAsBytes()));
        case 'modifybranch':
          unawaited(_modifyBranchDialog());
        case 'viewoptions':
          setState(() => _viewOptions = true);
        case 'panel':
          setState(() {
            _hidePanels = false;
            _panelOpen = w.length < 2 || w[1] != 'close';
          });
        case 'multitap':
          final n = w.length > 1 ? int.parse(w[1]) : 2;
          final c = Offset(_view.size.width / 2, _view.size.height / 2);
          for (var i = 0; i < n; i++) {
            _down(PointerDownEvent(pointer: 500 + i, position: c + Offset(i * 30.0, 0), kind: PointerDeviceKind.touch, buttons: kTouchContact));
          }
          await Future<void>.delayed(const Duration(milliseconds: 50));
          for (var i = 0; i < n; i++) {
            _up(PointerUpEvent(pointer: 500 + i, position: c + Offset(i * 30.0, 0), kind: PointerDeviceKind.touch));
          }
        case 'quit':
          exit(0);
      }
      await Future<void>.delayed(const Duration(milliseconds: 120));
    }
  }

  /// Scripted: one finger down at `a`, held `holdMs`, moved to `b` in small
  /// steps and lifted, sent as real touch events (the gesture arena decides).
  Future<void> _realPinch(Offset c, double from, double to) async {
    final g = GestureBinding.instance;
    Offset at(int side, double gap) => c + Offset(side * gap / 2, 0);
    for (final (id, side) in [(921, -1), (922, 1)]) {
      g.handlePointerEvent(PointerDownEvent(pointer: id, position: at(side, from), kind: PointerDeviceKind.touch, buttons: kTouchContact));
    }
    const steps = 14;
    for (var i = 1; i <= steps; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 16));
      final gap = from + (to - from) * i / steps;
      for (final (id, side) in [(921, -1), (922, 1)]) {
        g.handlePointerEvent(PointerMoveEvent(pointer: id, position: at(side, gap), kind: PointerDeviceKind.touch, buttons: kTouchContact));
      }
    }
    await Future<void>.delayed(const Duration(milliseconds: 16));
    for (final (id, side) in [(921, -1), (922, 1)]) {
      g.handlePointerEvent(PointerUpEvent(pointer: id, position: at(side, to), kind: PointerDeviceKind.touch));
    }
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }

  Future<void> _realSwipe(Offset a, Offset b, {int holdMs = 0, bool mouse = false}) async {
    final g = GestureBinding.instance;
    const id = 913;
    final kind = mouse ? PointerDeviceKind.mouse : PointerDeviceKind.touch;
    final buttons = mouse ? kPrimaryButton : kTouchContact;
    g.handlePointerEvent(PointerDownEvent(pointer: id, position: a, kind: kind, buttons: buttons));
    if (holdMs > 0) await Future<void>.delayed(Duration(milliseconds: holdMs));
    const steps = 14;
    var at = a;
    if (b != a) {
      for (var i = 1; i <= steps; i++) {
        await Future<void>.delayed(const Duration(milliseconds: 16));
        final next = Offset.lerp(a, b, i / steps)!;
        g.handlePointerEvent(PointerMoveEvent(pointer: id, position: next, delta: next - at, kind: kind, buttons: buttons));
        at = next;
      }
    }
    await Future<void>.delayed(const Duration(milliseconds: 16));
    g.handlePointerEvent(PointerUpEvent(pointer: id, position: b, kind: kind));
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }

  Future<void> _fakeDrag(Offset from, Offset to) async {
    const id = 77;
    _down(PointerDownEvent(pointer: id, position: from));
    for (var i = 1; i <= 8; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 16));
      _move(PointerMoveEvent(pointer: id, position: Offset.lerp(from, to, i / 8)!));
    }
    _up(PointerUpEvent(pointer: id, position: to));
  }

  Future<void> _saveShot(String path) async {
    final b = (appShot.currentContext ?? _shot.currentContext)?.findRenderObject() as RenderRepaintBoundary?;
    if (b == null) return;
    final img = await b.toImage(pixelRatio: 1);
    final data = await img.toByteData(format: ui.ImageByteFormat.png);
    if (data != null) File(path).writeAsBytesSync(data.buffer.asUint8List());
  }

  // ---------------------------------------------------------------- layout
  //
  // Arrangement after the original (docs/barkfart/UI-SPEC.md): left panel
  // with a fixed header and one scrolling list of sections that follows the
  // selection; section tabs above the stage; filmstrip of frame pictures on
  // top with the frame/play grid at the top right.

  /// A frame thumbnail was tapped: the panel shows Frame Tools.
  bool _frameSelected = false;
  bool _viewOptions = false;

  // Magnifier window: a zoomed view around the selected joint, for picking
  // joints that sit close together
  bool _magnifier = false, _magLocked = false, _magNodes = true;
  Offset _magAt = const Offset(16, 260);
  double _magZoom = 3;
  Offset _magCenter = Offset.zero;
  ui.Image? _magImage;
  String _magKey = '';
  static const _magW = 240.0, _magH = 170.0;

  double get _magK => _view.s * _magZoom;

  void _magTick() {
    final e = _engine;
    if (!_magnifier || _playing || e == null || _st.isEmpty || _view.s <= 0) return;
    if (!_magLocked) {
      for (final h in (_st['handles'] as List?) ?? const []) {
        final l = h as List;
        if ((l[2] as num).toInt() & 2 != 0) _magCenter = Offset((l[0] as num).toDouble(), (l[1] as num).toDouble());
      }
    }
    final k = _magK;
    final key = '${_st['rev']} $_frame $_magCenter $k';
    if (key == _magKey) return;
    _magKey = key;
    final x0 = _magCenter.dx - _magW / 2 / k, x1 = _magCenter.dx + _magW / 2 / k;
    final y0 = _magCenter.dy - _magH / 2 / k, y1 = _magCenter.dy + _magH / 2 / k;
    final w = (_magW * math.min(_dpr, 2.0)).round(), h = (_magH * math.min(_dpr, 2.0)).round();
    final px = e.renderView(_frame / _fps, w, h, x0, y0, x1, y1);
    if (px == null) return;
    ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, (img) {
      if (!mounted) return;
      setState(() {
        _magImage?.dispose();
        _magImage = img;
      });
    });
  }

  Widget _magnifierWindow() {
    final k = _magK;
    final x0 = _magCenter.dx - _magW / 2 / k, y1 = _magCenter.dy + _magH / 2 / k;
    return SizedBox(
      width: _magW + 16,
      child: _floating('Magnifier', () => setState(() => _magnifier = false), (d) => setState(() => _magAt += d), Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        // a tap picks the nearest joint; move it on the main stage
        GestureDetector(
          onTapUp: (d) {
            final x = x0 + d.localPosition.dx / k, y = y1 - d.localPosition.dy / k;
            _cmd({'op': 'down', 'x': x, 'y': y, 'r': 14 / k});
            _cmd({'op': 'up', 'x': x, 'y': y});
          },
          child: Container(
            width: _magW,
            height: _magH,
            color: Colors.white,
            child: CustomPaint(painter: _MagPainter(_magImage, _magNodes ? ((_st['handles'] as List?) ?? const []) : const [], x0, y1, k)),
          ),
        ),
        const SizedBox(height: 6),
        Row(children: [
          IconButton(
            tooltip: _magLocked ? 'Follow the selected joint' : 'Keep this spot',
            icon: Icon(_magLocked ? Icons.lock : Icons.lock_open, size: 18, color: _magLocked ? cAccent : cSecondary),
            onPressed: () => setState(() => _magLocked = !_magLocked),
          ),
          SizedBox(width: 36, child: _Btn('−', onTap: () => setState(() => _magZoom = math.max(1.5, _magZoom - 0.5)))),
          const SizedBox(width: 4),
          SizedBox(width: 36, child: _Btn('+', onTap: () => setState(() => _magZoom = math.min(12, _magZoom + 0.5)))),
          const SizedBox(width: 6),
          Expanded(child: _Btn(_magNodes ? 'Joints shown' : 'Joints hidden', small: true, onTap: () => setState(() => _magNodes = !_magNodes))),
        ]),
      ])),
    );
  }

  // Arrows window: nudges the selection by a set amount
  bool _arrows = false;
  Offset _arrowsAt = const Offset(16, 16);
  double _nudge = 1;

  /// A small floating window; drag it by its title bar.
  Widget _floating(String title, VoidCallback close, void Function(Offset) moved, Widget child) {
    return Material(
      color: cPanel,
      elevation: 10,
      borderRadius: BorderRadius.circular(8),
      child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        GestureDetector(
          onPanUpdate: (d) => moved(d.delta),
          child: Container(
            padding: const EdgeInsets.only(left: 10),
            decoration: const BoxDecoration(color: cControl, borderRadius: BorderRadius.vertical(top: Radius.circular(8))),
            child: Row(children: [
              const Icon(Icons.open_with, size: 14, color: cSecondary),
              const SizedBox(width: 6),
              Expanded(child: Text(title, style: const TextStyle(color: cText, fontSize: 12))),
              IconButton(visualDensity: VisualDensity.compact, icon: const Icon(Icons.close, size: 18, color: cDanger), onPressed: close),
            ]),
          ),
        ),
        Padding(padding: const EdgeInsets.all(8), child: child),
      ]),
    );
  }

  Widget _arrowsWindow() {
    Widget arrow(IconData i, double dx, double dy) => SizedBox(
          width: 44,
          height: 44,
          child: Material(
            color: cAccent,
            borderRadius: BorderRadius.circular(6),
            child: InkWell(onTap: () => _cmd({'op': 'nudge', 'dx': dx * _nudge, 'dy': dy * _nudge}), child: Icon(i, color: cText)),
          ),
        );
    // steps of 1 from 1 up, of 0.1 below
    final step = _nudge >= 1 ? 1.0 : 0.1;
    return SizedBox(
      width: 190,
      child: _floating('Arrows', () => setState(() => _arrows = false), (d) => setState(() => _arrowsAt += d), Column(mainAxisSize: MainAxisSize.min, children: [
        arrow(Icons.keyboard_arrow_up, 0, 1),
        Row(mainAxisAlignment: MainAxisAlignment.center, children: [
          arrow(Icons.keyboard_arrow_left, -1, 0),
          const SizedBox(width: 48),
          arrow(Icons.keyboard_arrow_right, 1, 0),
        ]),
        arrow(Icons.keyboard_arrow_down, 0, -1),
        const SizedBox(height: 8),
        Row(children: [
          Expanded(child: Text('Nudge ${_nudge.toStringAsFixed(1)}', style: const TextStyle(color: cText, fontSize: 12))),
          SizedBox(width: 36, child: _Btn('−', onTap: () => setState(() => _nudge = math.max(0.1, ((_nudge - (_nudge > 1 ? 1 : 0.1)) * 10).round() / 10)))),
          const SizedBox(width: 4),
          SizedBox(width: 36, child: _Btn('+', onTap: () => setState(() => _nudge = ((_nudge + (step)) * 10).round() / 10))),
        ]),
      ])),
    );
  }
  bool _showTabs = true;
  final ScrollController _panelScroll = ScrollController();
  final Map<String, GlobalKey> _sectionKeys = {};
  /// "Add tweened frame" positions: 50%, 33/67%, 10/90%.
  String _tweenAt = '50';
  // which of the two new frames 33/67 and 10/90 add: 0 both, 1 the first, 2 the second
  int _tweenPick = 0;

  GlobalKey _sectionKey(String name) => _sectionKeys.putIfAbsent(name, () => GlobalKey());

  /// The sections the panel shows now (also the tabs above the stage).
  List<String> _sections() {
    // the figure editor (FigureEditor board): Shape, Segment, Figure; the
    // main joint has only the figure
    if (_st['building'] != null) return _sel != null && _sel!['root'] != true ? const ['Shape Tools', 'Segment Tools', 'Figure editor'] : const ['Figure editor'];
    if (_st['mode'] == 'camera') return const ['Camera Tools'];
    if (_soundPage && _st['frameSound'] != null) return const ['Sound Tools'];
    // K2c: a script layer's row or run
    if (_st['script'] != null) return const ['Script'];
    if (_frameSelected) return const ['Frame Tools'];
    if (_st['textfield'] != null) return const ['Textfield Tools'];
    if (_st['movieclip'] != null) return const ['Movieclip Tools'];
    if (_st['picture'] != null) return const ['Picture Tools'];
    if (_st['adjustment'] != null) return const ['Effect Layer'];
    if (_st['group'] != null) return const ['Group Tools'];
    // D36: in Curves, the selected value's page (its modifiers)
    if (_curvesOn && _curveValue != null) return const ['Value'];
    final sel = _sel;
    if (_st['multi'] != null) return const ['Selection'];
    if (sel != null) return const ['Stickfigure Tools', 'Segment Tools', 'Pose'];
    if (_st['editingClip'] != null) return const ['Movieclip editor', 'Textfield Tools'];
    return const ['Frame Tools'];
  }

  List<Widget> _sectionBody(String name) {
    switch (name) {
      case 'Sound Tools':
        return _soundsBlock();
      case 'Picture Tools':
        return _pictureTools(_st['picture'] as Map);
      case 'Effect Layer' when _st['adjustment'] != null:
        return _adjustmentTools((_st['adjustment'] as Map).cast<String, dynamic>());
      case 'Group Tools' when _st['group'] != null:
        return _groupTools(_st['group'] as Map);
      case 'Script' when _st['script'] != null:
        return _scriptTools(_st['script'] as Map);
      case 'Value' when _curveValue != null:
        return _valueTools(_curveValue!);
      case 'Project Tools':
        return _projectTools();
      case 'Library Tools':
        return _libraryTools();
      case 'Movieclip Tools':
        return _movieclipTools(_st['movieclip'] as Map);
      case 'Movieclip editor':
        return [
          Text('Editing ${_st['editingClip']}', style: const TextStyle(color: cText, fontSize: 13)),
          _hint("These are the movieclip's own frames: pose, add and delete frames and figures as usual. Finish goes back to the animation, where the movieclip plays them."),
          _gap,
          _Btn('Finish movieclip', accent: true, onTap: () => _cmd({'op': 'movieclip_done'})),
        ];
      case 'Frame Tools':
        return _frameToolsPanel();
      case 'Stickfigure Tools':
        return _stickfigureTools(_sel!);
      case 'Pose':
        return _poseTools(_sel!);
      case 'Selection':
        return _multiTools(((_st['multi'] as List?) ?? const []).cast<Map>());
      case 'Stickfigure Filters':
        return _stickfigureFilters(_sel!);
      case 'Textfield Tools':
        return _textfieldTools(_st['textfield'] as Map<String, dynamic>?);
      case 'Segment Tools':
        return _segmentTools(_sel!);
      case 'Camera Tools':
        return _cameraTools();
      case 'Figure editor':
        return _builder(_sel);
      case 'Shape Tools':
        return _shapeTools(_sel!);
    }
    return const [];
  }

  String _tab = '';

  void _jumpTo(String name) {
    final changed = _tab != name;
    setState(() => _tab = name);
    // a tab is its own page: it opens at the top
    if (changed && _panelScroll.hasClients) _panelScroll.jumpTo(0);
    final c = _sectionKey(name).currentContext;
    if (c != null && !_showTabs) Scrollable.ensureVisible(c, duration: const Duration(milliseconds: 200), alignment: 0);
  }

  /// Whether the last build was while playing (the timeline's fold easing
  /// is skipped when playback starts or stops).
  bool _sizedPlaying = false;

  @override
  Widget build(BuildContext context) {
    _dpr = MediaQuery.devicePixelRatioOf(context);
    if (_playing != _sizedPlaying) WidgetsBinding.instance.addPostFrameCallback((_) => _sizedPlaying = _playing);
    if (_fatal.isNotEmpty) {
      return Scaffold(backgroundColor: cSurround, body: Center(child: Text(_fatal, style: const TextStyle(color: cText))));
    }
    final big = MediaQuery.sizeOf(context).shortestSide > 600;
    final screen = MediaQuery.sizeOf(context);
    // screen padding from the settings (notches, rounded corners)
    final pad = MediaQuery.paddingOf(context) + EdgeInsets.only(left: AppSettings.number('padLeft').toDouble(), right: AppSettings.number('padRight').toDouble());
    final leftHand = AppSettings.flag('leftHand');
    // Properties is ALWAYS a floating overlay (never docks/squeezes the stage).
    // On compact phones (e.g. 844x390), hoist the overlay into the outer Stack
    // so it has room above the bottom bar instead of being squished.
    final compactPhone = !big && screen.height < 500;
    final showFloatingProps = _panelOpen && !_hidePanels && !_playing;
    return Scaffold(
      backgroundColor: cSurround,
      body: RepaintBoundary(
        key: _shot,
        child: Stack(children: [
        Column(children: [
        if (!_fsNow) (_st['building'] != null ? _studioHeader(pad) : _appBar(pad)),
        Expanded(child: Row(textDirection: leftHand ? TextDirection.rtl : TextDirection.ltr, children: [
          if (!_fsNow) _leftRail(pad, leftHand),
          Expanded(
            child: Column(children: [
              Expanded(
                child: Padding(
                  padding: EdgeInsets.only(right: leftHand ? 0 : pad.right, left: leftHand ? pad.left : 0),
                  child: Stack(textDirection: TextDirection.ltr, children: [
                    Positioned.fill(child: _stage()),
                    if (_arrows && !_playing) Positioned(left: _arrowsAt.dx, top: _arrowsAt.dy, child: _arrowsWindow()),
                    if (_magnifier && !_playing) Positioned(left: _magAt.dx, top: _magAt.dy, child: _magnifierWindow()),
                    // Always floating over the stage; opens explicitly via the Properties rail button
                    if (!_fsNow && !compactPhone)
                      Positioned(
                        top: 8,
                        right: 8,
                        bottom: 8,
                        width: _floatingCardWidth(screen, big),
                        child: _slideIn(showFloatingProps ? KeyedSubtree(key: const ValueKey('props'), child: _propertiesCard()) : null, fromRight: true),
                      ),
                    if (_st['building'] != null && _partsOpen && !_viewOptions && !_hidePanels) Positioned(top: 8, left: 8, bottom: 8, width: 220, child: _partsPanel()),
                    Positioned(top: 8, left: 8, bottom: 8, width: 330, child: _slideIn(_viewOptions ? KeyedSubtree(key: const ValueKey('view'), child: _viewOptionsPanel()) : null, fromRight: false)),
                  ]),
                ),
              ),
            ]),
          ),
          if (!_fsNow) _rightRail(pad, leftHand),
        ])),
        // the timeline spans the window under both rails (round 10 board);
        // folded, it is the toolbar, the ruler and the Preview row only
        if (!_fsNow && !_hidePanels)
          Padding(
            padding: EdgeInsets.only(left: pad.left, right: pad.right, bottom: pad.bottom),
            // the figure editor puts its shape shelf where the timeline is (J2)
            child: _st['building'] != null
                ? _shapeShelf()
                // folding and unfolding ease the stage's edge along (D37)
                : AnimatedSize(
                    // playing starting or stopping, or dragging the resize handle, snaps immediately
                    duration: _playing != _sizedPlaying || _timelineDragging ? Duration.zero : kit.motion(context, 180),
                    curve: kit.kEase,
                    alignment: Alignment.bottomCenter,
                    child: Column(children: [
                      if (_layersOpen && !_playing) _timelineResizeHandle(screen.height),
                      _controlBar(),
                      if (_curvesOn && _layersOpen && !_playing) _curvesView() else _previewRow(),
                      if (_layersOpen) _curvesOn && !_playing ? _curvesBar() : _framesBar(),
                    ]),
                  ),
          ),
        ]),
        if (!_fsNow && compactPhone)
          Positioned(
            top: 44 + pad.top,
            right: leftHand ? null : 64 + pad.right + 6,
            left: leftHand ? 64 + pad.left + 6 : null,
            bottom: (_st['building'] != null ? 72 : 8) + pad.bottom,
            width: _floatingCardWidth(screen, big),
            child: _slideIn(showFloatingProps ? KeyedSubtree(key: const ValueKey('props'), child: _propertiesCard()) : null, fromRight: !leftHand),
          ),
        ]),
      ),
    );
  }

  /// A floating panel coming and going (D37): it slides a little from its
  /// edge and fades; nothing there lets touches through to the stage.
  Widget _slideIn(Widget? child, {required bool fromRight}) {
    return AnimatedSwitcher(
      duration: kit.motion(context, 200),
      reverseDuration: kit.motion(context, 140),
      switchInCurve: kit.kEase,
      switchOutCurve: Curves.easeInCubic,
      layoutBuilder: (current, previous) => Stack(fit: StackFit.expand, children: [...previous, ?current]),
      transitionBuilder: (c, a) => FadeTransition(
        opacity: a,
        child: SlideTransition(position: Tween(begin: Offset(fromRight ? 0.06 : -0.06, 0), end: Offset.zero).animate(a), child: c),
      ),
      child: child ?? const SizedBox.shrink(key: ValueKey('none')),
    );
  }

  /// The top bar (REDESIGN §4): Projects, View and Export on the left, the
  /// animation's name and the unsaved dot in the middle.
  Widget _appBar(EdgeInsets pad) {
    final dirty = _st['dirty'] == true;
    return Container(
      height: 40 + pad.top,
      padding: EdgeInsets.only(top: pad.top, left: pad.left + 8, right: pad.right + 8),
      decoration: const BoxDecoration(color: cCard, border: Border(bottom: BorderSide(color: cLine))),
      child: Stack(alignment: Alignment.center, children: [
        Row(children: [
          kit.WordButton('Projects', key: _projectsKey, onTap: _projectsMenu),
          kit.WordButton('View', filled: _viewOptions, onTap: () => setState(() => _viewOptions = !_viewOptions)),
          kit.WordButton('Export', onTap: _exportDialog),
          const Spacer(),
          if (dirty && !_playing) kit.WordButton('Save', size: 14, strong: true, onTap: _save),
        ]),
        Padding(
          padding: const EdgeInsets.only(left: 214, right: 76),
          child: IgnorePointer(
            child: Row(mainAxisSize: MainAxisSize.min, children: [
              Flexible(
                child: Text(_file?.name ?? 'Untitled', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w700)),
              ),
              if (dirty) Container(width: 7, height: 7, margin: const EdgeInsets.only(left: 7), decoration: const BoxDecoration(color: Color(0xFFF2A93B), shape: BoxShape.circle)),
            ]),
          ),
        ),
      ]),
    );
  }

  final GlobalKey _projectsKey = GlobalKey();

  /// "Projects": the animation's file actions, until the Projects screen (E2).
  Future<void> _projectsMenu() async {
    final anchor = kit.Anchored.rectOf(_projectsKey) ?? const Rect.fromLTWH(8, 4, 80, 36);
    final pick = await showGeneralDialog<int>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Projects',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => Stack(children: [
        kit.Anchored(
          anchor: anchor,
          width: 220,
          below: true,
          card: (w) => kit.PanelCard(
            width: w,
            padding: const EdgeInsets.all(6),
            child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, mainAxisSize: MainAxisSize.min, children: [
              for (final (i, label) in const [(0, 'New animation'), (6, 'Create stick figure'), (1, 'Your animations…'), (2, 'Save'), (3, 'Save as…'), (5, 'Workshop'), (4, 'Settings')])
                kit.Tap(
                  onTap: () => Navigator.pop(c, i),
                  minSize: 0,
                  child: Container(height: 40, alignment: Alignment.centerLeft, padding: const EdgeInsets.symmetric(horizontal: 10), child: Text(label, style: const TextStyle(color: cAccent, fontSize: 15, fontWeight: FontWeight.w700))),
                ),
            ]),
          ),
        ),
      ]),
    );
    switch (pick) {
      case 0:
        await _newProject();
      case 1:
        await _projectsScreen();
      case 2:
        await _save();
      case 3:
        await _saveAs();
      case 4:
        await _settingsDialog();
      case 5:
        await _openWorkshop();
      case 6:
        _createNewFigure();
    }
  }

  Widget _stage() {
    return LayoutBuilder(builder: (context, c) {
      // a rotation (or a window resize) fits the stage again, unless the view
      // was zoomed or panned by hand; the timeline folding or a docked panel
      // opening keeps the zoom (the stage stays put)
      final screen = MediaQuery.sizeOf(context);
      final screenW = screen.width;
      final rotated = _screenW > 0 && (screenW - _screenW).abs() > 1;
      _screenW = screenW;
      if (_view.size != c.biggest || rotated) {
        _view.size = c.biggest;
        if (!_fitted || !_viewMoved) _fit();
      }
      final showZoomBtns = AppSettings.flag('zoomButtons');
      final leftInset = _viewOptions ? 346.0 : (_st['building'] != null && _partsOpen && !_hidePanels ? 236.0 : 10.0);
      final rightInset = !_fsNow && _panelOpen && !_hidePanels ? (_floatingCardWidth(screen, screen.shortestSide > 600) + 16) : 10.0;
      final mode = _st['mode'] as String? ?? 'figure';
      final joining = _st['joining'] == true;
      final showModeBanner = !_playing && _st['building'] == null && (joining || mode == 'camera' || mode == 'all');

      void zoomBy(double k) {
        final center = _view.toStage(_view.size.center(Offset.zero));
        _view.s = (_view.s * k).clamp(0.02, 40.0);
        final center2 = _view.toStage(_view.size.center(Offset.zero));
        _view.cx += center.dx - center2.dx;
        _view.cy += center.dy - center2.dy;
        _viewMoved = true;
        _stageTick.value++;
        _zoomTick.value++;
      }

      return Listener(
        onPointerDown: (ev) {
          if (_viewOptions) setState(() => _viewOptions = false);
          _down(ev);
        },
        onPointerMove: _move,
        onPointerUp: _up,
        onPointerCancel: _up,
        onPointerSignal: _scroll,
        child: ClipRect(
          child: Stack(fit: StackFit.expand, children: [
            RepaintBoundary(
              child: ValueListenableBuilder<int>(
                valueListenable: _stageTick,
                builder: (_, _, _) => Stack(fit: StackFit.expand, children: [
                  if (_image != null) CustomPaint(painter: _StagePicture(_image!, _imageRect, _imageCamera, _view, Size(((_st['w'] as num?) ?? 16).toDouble(), ((_st['h'] as num?) ?? 9).toDouble()))),
                  CustomPaint(painter: _Overlay(_pictureSt ?? _st, _view, _playing, studioGrid: _studioGrid, studioCrosshair: _studioCrosshair)),
                ]),
              ),
            ),
            if (showModeBanner)
              Positioned(
                left: leftInset,
                right: rightInset,
                top: 10,
                child: Align(
                  alignment: Alignment.topCenter,
                  child: Container(
                    padding: const EdgeInsets.fromLTRB(11, 3, 5, 3),
                    decoration: BoxDecoration(
                      color: joining ? cHint : cCard.withValues(alpha: 0.94),
                      borderRadius: BorderRadius.circular(18),
                      border: Border.all(color: joining ? const Color(0xFFE5B84B) : cLine),
                      boxShadow: const [BoxShadow(color: Color(0x1A000000), blurRadius: 8, offset: Offset(0, 2))],
                    ),
                    child: Row(mainAxisSize: MainAxisSize.min, children: [
                      Pictogram(joining ? 'attach' : mode == 'camera' ? 'camera' : 'all', size: 15, color: joining ? cText : cTitle),
                      const SizedBox(width: 6),
                      Flexible(
                        child: Text(
                          joining
                              ? (_st['joiningMode'] == 'absolute' ? 'Snap to joint' : 'Attach')
                              : mode == 'camera'
                                  ? 'Camera'
                                  : 'Move all',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(color: cText, fontSize: 12.5, fontWeight: FontWeight.w600),
                        ),
                      ),
                      if (joining) ...[
                        const SizedBox(width: 6),
                        kit.WordButton(
                          _st['joiningMode'] == 'absolute' ? 'Snap' : 'Offset',
                          size: 12,
                          height: 26,
                          filled: true,
                          onTap: () => _cmd({'op': 'join', 'mode': _st['joiningMode'] == 'absolute' ? 'relative' : 'absolute'}),
                        ),
                      ],
                      const SizedBox(width: 4),
                      kit.WordButton(
                        joining ? 'Cancel' : 'Done',
                        size: 12.5,
                        height: 26,
                        filled: !joining,
                        onTap: () => joining ? _cmd({'op': 'join_cancel'}) : _setMode('figure'),
                      ),
                    ]),
                  ),
                ),
              ),
            if (_playing)
              Positioned(
                right: 12,
                bottom: 10,
                child: IgnorePointer(
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
                    decoration: BoxDecoration(
                      color: const Color(0x9E1E2025),
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Text(
                      '${(_playT * _fps).floor().clamp(0, math.max(0, _frames - 1)) + 1} / $_frames',
                      style: const TextStyle(
                        color: Color(0xF0FFFFFF),
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        fontFeatures: [ui.FontFeature.tabularFigures()],
                      ),
                    ),
                  ),
                ),
              ),
            Positioned(
              left: math.min(leftInset, math.max(8.0, c.biggest.width - 170)),
              bottom: 10,
              child: ValueListenableBuilder<int>(
                valueListenable: _zoomTick,
                builder: (_, _, _) {
                  final fS = _fitScale();
                  final zPct = fS > 0 ? (_view.s / fS * 100).round() : 100;
                  final visible = !_playing && (_viewMoved || (zPct - 100).abs() >= 2 || showZoomBtns);
                  if (!visible) return const SizedBox.shrink();
                  return Container(
                    height: 32,
                    decoration: BoxDecoration(
                      color: cCard.withValues(alpha: 0.94),
                      borderRadius: BorderRadius.circular(16),
                      border: Border.all(color: cLine),
                      boxShadow: const [BoxShadow(color: Color(0x1F000000), blurRadius: 8, offset: Offset(0, 2))],
                    ),
                    child: Row(mainAxisSize: MainAxisSize.min, children: [
                      if (showZoomBtns)
                        kit.Tap(
                          label: 'Zoom out',
                          minSize: 34,
                          radius: 16,
                          onTap: () => zoomBy(1 / 1.2),
                          child: const Padding(padding: EdgeInsets.symmetric(horizontal: 8), child: Pictogram('minus', size: 15, color: cInk2b)),
                        ),
                      kit.Tap(
                        label: 'Fit stage (100%)',
                        minSize: 34,
                        radius: 16,
                        onTap: _animateFit,
                        child: Padding(
                          padding: EdgeInsets.symmetric(horizontal: showZoomBtns ? 6 : 11),
                          child: Row(mainAxisSize: MainAxisSize.min, children: [
                            Text('$zPct%', style: const TextStyle(color: cText, fontSize: 12, fontWeight: FontWeight.w700, fontFeatures: [ui.FontFeature.tabularFigures()])),
                            if (_viewMoved || (zPct - 100).abs() >= 2) ...[
                              const SizedBox(width: 5),
                              const Text('Fit', style: TextStyle(color: cAccent, fontSize: 12, fontWeight: FontWeight.w700)),
                            ],
                          ]),
                        ),
                      ),
                      if (showZoomBtns)
                        kit.Tap(
                          label: 'Zoom in',
                          minSize: 34,
                          radius: 16,
                          onTap: () => zoomBy(1.2),
                          child: const Padding(padding: EdgeInsets.symmetric(horizontal: 8), child: Pictogram('plus', size: 15, color: cInk2b)),
                        ),
                    ]),
                  );
                },
              ),
            ),
          ]),
        ),
      );
    });
  }

  /// The figure editor's Shape page (FigureEditor board): the seven shapes as
  /// tiles in the segment's own colour, the current one ringed, then that
  /// shape's own settings.
  List<Widget> _shapeTools(Map<String, dynamic> sel) {
    final shape = sel['shape'] as String? ?? 'line';
    final c = (sel['color'] as num?)?.toInt() ?? 0xff;
    final colour = Color(((c & 0xff) << 24) | (c >> 8));
    void seg(Map<String, Object?> m) => _cmd({'op': 'segment', ...m});
    const tiles = [('square', 'Segment', 'segment'), ('line', 'Rounded', 'rounded'), ('circle', 'Circle', 'circle'), ('ellipse', 'Ellipse', 'ellipse'), ('triangle', 'Triangle', 'triangle'), ('trapezoid', 'Trapezoid', 'trapezoid'), ('polygon', 'Polygon', 'polygon')];
    final name = tiles.where((t) => t.$1 == shape).firstOrNull?.$2 ?? 'Segment';
    final curve = (sel['curve'] as num?)?.toDouble() ?? 0;
    final sides = (sel['sides'] as num?)?.toInt() ?? 5;
    final trap = ((sel['trapRatio'] as num?) ?? 1).toDouble();
    final half = (sel['halfArc'] as num?)?.toInt() ?? 0;
    final right = (sel['rightTriangle'] as num?)?.toInt() ?? 0;
    final grid = GridView.count(
      crossAxisCount: 3,
      shrinkWrap: true,
      physics: const NeverScrollableScrollPhysics(),
      mainAxisSpacing: 8,
      crossAxisSpacing: 8,
      childAspectRatio: 1.25,
      children: [
        for (final (id, label, glyph) in tiles)
          kit.Tap(
            label: label,
            minSize: 0,
            onTap: () => seg({'shape': id}),
            child: Column(children: [
              Expanded(
                child: Container(
                  decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(10), border: Border.all(color: id == shape ? cAccent : cLine, width: id == shape ? 2 : 1)),
                  child: Center(child: CustomPaint(size: const Size(60, 36), painter: ShapeGlyph(glyph, colour))),
                ),
              ),
              const SizedBox(height: 3),
              Text(label, style: TextStyle(color: id == shape ? cAccent : cText, fontSize: 12, fontWeight: id == shape ? FontWeight.w600 : FontWeight.w400)),
            ]),
          ),
      ],
    );
    final rows = <rw.Desc>[
      rw.DWidget(grid),
      rw.DTitle(name),
      if (shape == 'line' || shape == 'square' || shape == 'trapezoid') rw.DStep('Curve radius', curve.round().toString(), onMinus: () => seg({'curve': curve - 5}), onPlus: () => seg({'curve': curve + 5})),
      if (shape == 'trapezoid') ...[
        rw.DStep('End thickness', '${(trap * 100).round()}%', onMinus: () => seg({'trapRatio': math.max(0, trap - 0.1)}), onPlus: () => seg({'trapRatio': trap + 0.1})),
        rw.DTick('Start rounded', sel['trapRoundStart'] == true, onChanged: (v) => seg({'trapRoundStart': v})),
        rw.DTick('End rounded', sel['trapRoundEnd'] == true, onChanged: (v) => seg({'trapRoundEnd': v})),
      ],
      if (shape == 'circle' || shape == 'ellipse') ...[
        rw.DTick('Half arc', half != 0, onChanged: (v) => seg({'halfArc': v ? 1 : 0})),
        if (half != 0) rw.DTick('Upside down', half < 0, onChanged: (v) => seg({'halfArc': v ? -1 : 1})),
      ],
      if (shape == 'circle') ...[
        rw.DTick('Is hollow', sel['hollow'] == true, onChanged: (v) => seg({'hollow': v})),
        rw.DTick('Circle outline colour', sel['outline'] == true, onChanged: (v) => seg({'outline': v})),
      ],
      if (shape == 'triangle') ...[
        rw.DTick('Right triangle', right != 0, onChanged: (v) => seg({'rightTriangle': v ? 1 : 0})),
        if (right != 0) rw.DTick('Flipped', right < 0, onChanged: (v) => seg({'rightTriangle': v ? -1 : 1})),
        rw.DTick('Upside down', sel['upsideDown'] == true, onChanged: (v) => seg({'upsideDown': v})),
      ],
      if (shape == 'polygon') rw.DStep('Vertices', '$sides', onMinus: () => seg({'sides': math.max(3, sides - 1)}), onPlus: () => seg({'sides': sides + 1})),
      if (shape == 'ellipse') const rw.DHint('The segment\u2019s thickness sets how tall the ellipse is.'),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// The Parts list (J3, FigureEditor board): every part of the figure,
  /// front at the top, flat (no tree). Tap one to select it; hold and drag a
  /// segment to change what draws in front. A quiet node count at the foot.
  Widget _partsPanel() {
    final parts = (_st['parts'] as Map?) ?? const {};
    final list = ((parts['list'] as List?) ?? const []).cast<Map>();
    final selNode = _sel?['node'];
    const words = {'line': 'Rounded', 'square': 'Segment', 'circle': 'Circle', 'ellipse': 'Ellipse', 'triangle': 'Triangle', 'trapezoid': 'Trapezoid', 'polygon': 'Polygon', 'none': 'Joint'};
    const glyphs = {'line': 'rounded', 'square': 'segment', 'circle': 'circle', 'ellipse': 'ellipse', 'triangle': 'triangle', 'trapezoid': 'trapezoid', 'polygon': 'polygon', 'none': 'select'};
    Color colour(Map p) {
      final c = (p['color'] as num?)?.toInt() ?? 0xff;
      return Color(((c & 0xff) << 24) | (c >> 8));
    }

    Widget row(int i, Map p) {
      final kind = p['kind'] as String;
      final on = p['id'] == selNode && kind != 'polyfill';
      final label = p['name'] is String ? '${p['name']}' : kind == 'connector' ? 'Connector ${p['n']}' : kind == 'polyfill' ? 'Polyfill ${p['n']}' : '${words[p['shape']] ?? 'Segment'} ${p['n']}';
      final glyph = kind == 'connector' ? 'connector' : kind == 'polyfill' ? 'polyfill' : glyphs[p['shape']] ?? 'segment';
      final item = Container(
        height: 29,
        margin: const EdgeInsets.symmetric(horizontal: 8),
        padding: const EdgeInsets.symmetric(horizontal: 8),
        decoration: BoxDecoration(color: on ? cSelected : null, borderRadius: BorderRadius.circular(8)),
        child: Row(children: [
          CustomPaint(size: const Size(24, 16), painter: ShapeGlyph(glyph, kind == 'connector' ? const Color(0xFF1E9E8F) : colour(p))),
          const SizedBox(width: 10),
          Expanded(child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: on ? cAccent : cText, fontSize: 14, fontWeight: on ? FontWeight.w600 : FontWeight.w400))),
        ]),
      );
      return GestureDetector(
        key: ValueKey('part-${p['id']}'),
        onTap: kind == 'polyfill' ? null : () => _cmd({'op': 'select', 'layer': (_st['building'] as Map)['layer'], 'node': p['id']}),
        // D35: double-tap to name it (holding drags it to a new place)
        onDoubleTap: kind == 'polyfill' ? null : () => _renamePart('${p['id']}', label, named: p['name'] is String),
        // polyfills follow their joint; segments drag to a new place
        child: kind == 'polyfill' ? item : ReorderableDelayedDragStartListener(index: i, child: item),
      );
    }

    return kit.PanelCard(
      padding: EdgeInsets.zero,
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 8, 6, 4),
          child: Row(children: [
            const Pictogram('layers', size: 16, color: cAccent),
            const SizedBox(width: 6),
            const Expanded(child: Text('Parts', style: TextStyle(color: cTitle, fontSize: 14.5, fontWeight: FontWeight.w700))),
            kit.Tap(
              label: 'Close parts',
              minSize: 30,
              onTap: () => setState(() => _partsOpen = false),
              child: const Pictogram('x', size: 14, color: cSecondary),
            ),
          ]),
        ),
        const Divider(height: 1, color: Color(0xFFE6E8EB)),
        const SizedBox(height: 4),
        Expanded(
          child: ReorderableListView.builder(
            buildDefaultDragHandles: false,
            itemCount: list.length,
            itemBuilder: (c, i) => row(i, list[i]),
            onReorderItem: (from, to) {
              final moved = list[from];
              if (moved['kind'] == 'polyfill' || from == to) return;
              final order = [...list]..removeAt(from);
              order.insert(to, moved);
              final nodes = order.where((p) => p['kind'] != 'polyfill').map((p) => p['id']).toList();
              final front = nodes.indexOf(moved['id']);
              _cmd({'op': 'reorder_at', 'node': moved['id'], 'index': nodes.length - 1 - front});
            },
          ),
        ),
        Container(
          height: 30,
          alignment: Alignment.center,
          decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
          child: Text('${parts['nodes'] ?? 0} nodes', style: const TextStyle(color: cSecondary, fontSize: 12)),
        ),
      ]),
    );
  }

  /// The figure studio's bottom Shape & Node Tool Shelf.
  Widget _shapeShelf() {
    final tool = _st['drawTool'] as String? ?? 'none';
    final mirror = (_st['opts'] as Map?)?['mirror'] == true;
    final showAllNodes = _opts['static'] == true;
    final hasSel = _sel != null;
    final nonRoot = hasSel && _sel!['root'] != true;
    return FigureStudioShelf(
      drawTool: tool,
      polyfillActive: _st['polyfill'] != null,
      mirrorOn: mirror,
      gridOn: _studioGrid,
      showAllNodes: showAllNodes,
      onToggleShowAllNodes: () => _cmd({'op': 'set', 'k': 'static', 'v': !showAllNodes}),
      hasSelectedJoint: hasSel,
      canEditNonRoot: nonRoot,
      shapePainterBuilder: (glyph, color) => ShapeGlyph(glyph, color),
      onPickTool: (id) => _cmd({'op': 'draw_tool', 'shape': id}),
      onPickPolyfill: () => hasSel ? _cmd({'op': 'polyfill_new'}) : _say('Select a joint first.', light: true),
      onToggleMirror: () => _cmd({'op': 'set', 'k': 'mirror', 'v': !mirror}),
      onToggleGrid: () => setState(() => _studioGrid = !_studioGrid),
      onSplit: () => _cmd({'op': 'split', 'n': _splitN + 1}),
      onDuplicateBranch: () => _cmd({'op': 'duplicate_branch'}),
      onFlipBranch: () => _cmd({'op': 'flip_branch', 'h': true}),
      onDeleteJoint: () => _cmd({'op': 'delete_segment'}),
    );
  }

  // ---------------------------------------------------------------- filmstrip and grid

  final Map<int, ui.Image> _thumbs = {};
  final Set<int> _thumbDirty = {};
  int _thumbRev = -1;
  // K: a folded group's row is a preview strip of its members merged, by
  // group and frame; only folded groups are drawn (open, its members' own
  // rows show the same)
  final Map<String, Map<int, ui.Image>> _groupThumbs = {};
  final Set<String> _groupDirty = {};
  // User list #9: off-UI-thread thumbnail queue state
  int _thumbNextTag = 1;
  final Map<int, ({int kind, String key, int rev})> _thumbInFlight = {};
  final Set<String> _thumbPendingKeys = {};
  static const _thumbMaxInFlight = 6;

  Iterable<String> get _foldedGroups => ((_st['rows'] as List?) ?? const []).cast<Map>().where((r) => r['kind'] == 'group' && r['open'] == false).map((r) => '${r['id']}');

  /// After a change: during a drag only the current frame's picture is stale.
  void _thumbsChanged(String op, {bool force = false}) {
    final rev = (_st['rev'] as num?)?.toInt() ?? 0;
    // a project just opened, made or imported into always redraws everything
    final whole = force || op == 'open' || op == 'new' || op == 'import';
    if (rev == _thumbRev && !whole) return;
    _thumbRev = rev;
    if (!force && (op == 'move' || op == 'down' || op == 'timeline_drag_start' || op == 'timeline_drag_move')) return;
    final folded = _foldedGroups.toList();
    if (op == 'up') {
      _thumbDirty.add(_frame);
      for (final g in folded) {
        _groupDirty.add('$g@$_frame');
      }
      return;
    }
    _engine?.clearThumbs();
    _thumbInFlight.clear();
    _thumbPendingKeys.clear();
    for (var i = 0; i < _frames; i++) {
      _thumbDirty.add(i);
      for (final g in folded) {
        _groupDirty.add('$g@$i');
      }
    }
    _thumbs.keys.where((k) => k >= _frames).toList().forEach((k) => _thumbs.remove(k)?.dispose());
  }

  // M4: the pictures inside timeline blocks, per layer and frame ("id@frame"):
  // a tween block shows the pose on every frame, a hold block once. Rendered on
  // the engine's background thread and drained non-blockingly per tick.
  final Map<String, ui.Image> _rowPics = {};
  // M5: waveforms, by asset, clip range and bar count
  final Map<String, List<double>> _peaks = {};
  final Set<String> _rowPicDirty = {};
  int _rowPicRev = -1;

  /// The pictures the open rows show, as "id@frame".
  List<String> _rowPicsWanted() {
    if (!_layersOpen || _playing) return const [];
    final out = <String>[];
    for (final r in ((_st['rows'] as List?) ?? const []).cast<Map>()) {
      if (r['kind'] == 'text' || r['kind'] == 'camera' || r['kind'] == 'sound') continue;
      final on = (r['on'] as List?) ?? const [];
      final blocks = (r['blocks'] as List?) ?? const [];
      for (var b = 0; b < blocks.length; b++) {
        final st = (blocks[b][0] as num).toInt(), len = (blocks[b][1] as num).toInt();
        final tween = blocks[b][2] != true && b + 1 < blocks.length;
        for (var f = st; f < (tween ? st + len : st + 1); f++) {
          if (on.elementAtOrNull(f) != false) out.add('${r['id']}@$f');
        }
      }
    }
    return out;
  }

  /// Enqueues stale per-layer block thumbnails onto the engine's worker thread.
  void _rowPicTick() {
    final e = _engine;
    if (e == null || _playing || _oneFinger == 1 || _blockDrag != null || _st.isEmpty || !_layersOpen) return;
    final rev = (_st['rev'] as num?)?.toInt() ?? 0;
    final wanted = _rowPicsWanted();
    if (rev != _rowPicRev) {
      _rowPicRev = rev;
      _rowPicDirty
        ..clear()
        ..addAll(wanted);
      final keep = wanted.toSet();
      for (final k in _rowPics.keys.where((k) => !keep.contains(k)).toList()) {
        _rowPics.remove(k)?.dispose();
      }
    }
    if (_rowPicDirty.isEmpty || _thumbInFlight.length >= _thumbMaxInFlight) return;
    // nearest the playhead first, bounded by in-flight cap
    final order = _rowPicDirty.toList()..sort((a, b) => (int.parse(a.split('@').last) - _frame).abs().compareTo((int.parse(b.split('@').last) - _frame).abs()));
    final px = (44 * math.min(_dpr, 2.0)).round();
    for (final k in order) {
      if (_thumbInFlight.length >= _thumbMaxInFlight) break;
      _rowPicDirty.remove(k);
      final pkey = '1:$k';
      if (_thumbPendingKeys.contains(pkey)) continue;
      final at = k.lastIndexOf('@');
      final id = k.substring(0, at), f = int.parse(k.substring(at + 1));
      final tag = _thumbNextTag++;
      if (e.requestThumb(1, id, f / _fps, px, px, tag)) {
        _thumbInFlight[tag] = (kind: 1, key: k, rev: rev);
        _thumbPendingKeys.add(pkey);
      }
    }
  }

  /// Drains completed off-UI-thread thumbnails (<= 2 per tick) and enqueues
  /// stale preview & folded-group thumbnails nearest the playhead first.
  /// Paused completely during active drags so thumbnail decodes never trigger
  /// full widget-tree rebuilds while posing.
  void _thumbTick() {
    final e = _engine;
    if (e == null || _st.isEmpty || _oneFinger == 1 || _blockDrag != null) return;
    final curRev = (_st['rev'] as num?)?.toInt() ?? 0;
    // Always drain up to 2 ready thumbnails non-blockingly per frame.
    for (var n = 0; n < 2; n++) {
      final res = e.pollThumb();
      if (res == null) break;
      final meta = _thumbInFlight.remove(res.tag);
      if (meta == null) continue;
      _thumbPendingKeys.remove('${meta.kind}:${meta.key}');
      if (res.w == 0 || res.h == 0) continue; // the engine couldn't draw it
      if (meta.rev != curRev) {
        // drawn for an older edit: ask again (its new request was skipped
        // while this one was on its way)
        switch (meta.kind) {
          case 0:
            if (int.tryParse(meta.key) case final i?) _thumbDirty.add(i);
          case 1:
            _rowPicDirty.add(meta.key);
          case 2:
            _groupDirty.add(meta.key);
        }
        continue;
      }
      ui.decodeImageFromPixels(res.rgba, res.w, res.h, ui.PixelFormat.rgba8888, (img) {
        if (!mounted) {
          img.dispose();
          return;
        }
        if (meta.kind == 0) {
          final i = int.tryParse(meta.key);
          if (i == null || i >= _frames) {
            img.dispose();
            return;
          }
          setState(() {
            _thumbs.remove(i)?.dispose();
            _thumbs[i] = img;
          });
        } else if (meta.kind == 1) {
          setState(() {
            _rowPics.remove(meta.key)?.dispose();
            _rowPics[meta.key] = img;
          });
        } else if (meta.kind == 2) {
          final at = meta.key.lastIndexOf('@');
          final g = meta.key.substring(0, at), i = int.tryParse(meta.key.substring(at + 1));
          final m = _groupThumbs[g];
          if (m == null || i == null || i >= _frames) {
            img.dispose();
            return;
          }
          setState(() {
            m.remove(i)?.dispose();
            m[i] = img;
          });
        } else {
          img.dispose();
        }
      });
    }
    if (_playing || _oneFinger == 1 || _blockDrag != null) return;
    // groups just folded get their strip; groups opened or gone drop theirs
    final folded = _foldedGroups.toSet();
    for (final g in _groupThumbs.keys.where((g) => !folded.contains(g)).toList()) {
      for (final img in _groupThumbs.remove(g)!.values) {
        img.dispose();
      }
    }
    _groupDirty.removeWhere((k) => !folded.contains(k.substring(0, k.lastIndexOf('@'))));
    for (final g in folded) {
      if (!_groupThumbs.containsKey(g)) {
        _groupThumbs[g] = {};
        for (var i = 0; i < _frames; i++) {
          _groupDirty.add('$g@$i');
        }
      }
    }
    final sw = (_st['w'] as num).toDouble(), sh = (_st['h'] as num).toDouble();
    // the engine draws thumbnails up to 2048 a side: a very tall canvas
    // gets a narrower one, same shape
    var w = (120 * math.min(_dpr, 2.0)).round(), h = math.max(1, (w * sh / sw).round());
    if (h > 2048) {
      h = 2048;
      w = math.max(1, (h * sw / sh).round());
    }
    while (_thumbDirty.isNotEmpty && _thumbInFlight.length < _thumbMaxInFlight) {
      final i = _thumbDirty.reduce((a, b) => (a - _frame).abs() <= (b - _frame).abs() ? a : b);
      _thumbDirty.remove(i);
      if (i >= _frames) continue;
      final pkey = '0:$i';
      if (_thumbPendingKeys.contains(pkey)) continue;
      final tag = _thumbNextTag++;
      if (e.requestThumb(0, null, i / _fps, w, h, tag)) {
        _thumbInFlight[tag] = (kind: 0, key: '$i', rev: curRev);
        _thumbPendingKeys.add(pkey);
      }
    }
    while (_groupDirty.isNotEmpty && _thumbInFlight.length < _thumbMaxInFlight) {
      int frameOf(String k) => int.parse(k.substring(k.lastIndexOf('@') + 1));
      final k = _groupDirty.reduce((a, b) => (frameOf(a) - _frame).abs() <= (frameOf(b) - _frame).abs() ? a : b);
      _groupDirty.remove(k);
      final g = k.substring(0, k.lastIndexOf('@')), i = frameOf(k);
      if (i >= _frames) continue;
      final pkey = '2:$k';
      if (_thumbPendingKeys.contains(pkey)) continue;
      final tag = _thumbNextTag++;
      if (e.requestThumb(2, g, i / _fps, w, h, tag)) {
        _thumbInFlight[tag] = (kind: 2, key: k, rev: curRev);
        _thumbPendingKeys.add(pkey);
      }
    }
  }

  int _copyCount = 1;

  /// The copy popup on a frame picture: copy it alone, or with N frames after it.
  Future<void> _copyPopup(int frame) async {
    if (_playing) return;
    setState(() {
      _frameSelected = true;
      _soundPage = false;
    });
    _cmd({'op': 'frame', 'i': frame});
    if (!AppSettings.flag('keepSelection')) {
      if (_st['mode'] == 'camera') _cmd({'op': 'mode', 'v': 'figure'});
      _cmd({'op': 'select'});
    }
    await kit.showSheet<void>(
      context,
      title: 'Frame ${frame + 1}',
      width: 380,
      children: (c) => [
        StatefulBuilder(
          builder: (c, set) {
            final maxN = math.max(0, _frames - 1 - frame);
            final n = math.min(_copyCount, maxN);
            return Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              kit.ButtonPair(
                'Copy this frame',
                () {
                  _cmd({'op': 'copy_frame', 'n': 0});
                  Navigator.pop(c);
                },
                'Paste after it',
                _st['frameCopied'] == true
                    ? () {
                        _cmd({'op': 'paste_frame'});
                        Navigator.pop(c);
                      }
                    : null,
              ),
              if (maxN > 0) ...[
                const SizedBox(height: 6),
                const kit.Hint('Or copy a range starting from this frame:'),
                kit.StepRow(
                  'Frames ${frame + 1}–${frame + 1 + n}',
                  '+$n',
                  onMinus: n > 1 ? () => set(() => _copyCount = math.max(1, n - 1)) : null,
                  onPlus: n < maxN ? () => set(() => _copyCount = math.min(maxN, n + 1)) : null,
                ),
                const SizedBox(height: 4),
                kit.WordButton('Copy ${n + 1} frames', primary: true, expand: true, height: 38, onTap: () {
                  _cmd({'op': 'copy_frame', 'n': n});
                  Navigator.pop(c);
                }),
              ] else
                const kit.Hint('This is the last frame in the animation.'),
              const SizedBox(height: 6),
              kit.WordButton('Close', filled: true, expand: true, height: 36, onTap: () => Navigator.pop(c)),
            ]);
          },
        ),
      ],
    );
  }

  /// Export: GIF or PNG frames, size (keeps the shot's shape), frame range,
  /// transparent background for PNG. Files go to Barkfart Exports.
  Future<void> _exportDialog() async {
    if (_export != null) return;
    final sw = (_st['w'] as num).toDouble(), sh = (_st['h'] as num).toDouble();
    final opts = await showDialog<Map<String, Object>>(
        context: context,
        builder: (c) => _ExportDialog(
              sw,
              sh,
              _frames,
              _file?.name ?? 'Animation',
              watermark: _st['watermark'] as Map?,
              cmd: (m) => _cmd(m),
              swatches: _swatches,
              vertical: _vertical,
              onVertical: (v) => _vertical = v,
              fps: _fps,
            ));
    if (opts == null) return;
    await _runExport(opts['format'] as int, opts['w'] as int, opts['h'] as int, opts['start'] as int, opts['count'] as int, opts['transparent'] as bool || opts['silent'] == true, opts['name'] as String);
  }

  final ScrollController _stripScroll = ScrollController();
  final ScrollController _layerVScroll = ScrollController();
  final GlobalKey _timelineTrackKey = GlobalKey();
  final Map<int, Offset> _tlPinch = {};
  double _tlPinchSpan = 0;
  double _tlPinchCellStart = 46;
  double _tlPinchAnchorFrame = 0;
  Offset? _dragPointerGlobal;
  bool _scrubbingPlayhead = false;
  int _dragRunwayFrames = 0;
  double _lastTrackViewportW = 600;

  /// Smoothly scrolls the timeline horizontally so `frame` is comfortably visible.
  void _ensureFrameVisible(int frame, {bool animate = true}) {
    if (!_stripScroll.hasClients) return;
    final pos = _stripScroll.position;
    if (pos.maxScrollExtent <= 0) return;
    const left = 8.0;
    final x0 = left + frame * _cellW;
    final x1 = x0 + _cellW;
    double? target;
    if (x0 < pos.pixels + 12) {
      target = math.max(0.0, x0 - 24);
    } else if (x1 > pos.pixels + pos.viewportDimension - 12) {
      target = math.min(pos.maxScrollExtent, x1 - pos.viewportDimension + _cellW * 1.6);
    }
    if (target == null || (target - pos.pixels).abs() < 1) return;
    final dur = kit.motion(context, 140);
    if (!animate || dur == Duration.zero) {
      _stripScroll.jumpTo(target);
    } else {
      unawaited(_stripScroll.animateTo(target, duration: dur, curve: kit.kEase));
    }
  }

  void _zoomTimeline(double factor, {double? anchorLocalX}) {
    final oldCell = _cellW;
    final newCell = (oldCell * factor).clamp(14.0, 112.0);
    if ((newCell - oldCell).abs() < 0.05) return;
    final hOff = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
    final pivotX = anchorLocalX ?? (_lastTrackViewportW * 0.45);
    final anchorFrame = (pivotX + hOff - 8.0) / oldCell;
    setState(() => _cellW = newCell);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_stripScroll.hasClients) return;
      final to = (8.0 + anchorFrame * newCell - pivotX).clamp(0.0, _stripScroll.position.maxScrollExtent);
      _stripScroll.jumpTo(to);
    });
  }

  void _fitTimeline() {
    final vw = math.max(160.0, _lastTrackViewportW - 24);
    final count = math.max(4, _frames + 1);
    setState(() {
      _cellW = (vw / count).clamp(14.0, 68.0);
      _dragRunwayFrames = 0;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_stripScroll.hasClients) _stripScroll.jumpTo(0);
    });
  }

  /// Auto-scrolls the timeline smoothly at 60fps whenever a grip, keyframe,
  /// playhead scrub, or double-tap selection box approaches the viewport edge.
  void _timelineEdgeScrollTick() {
    final ptr = _dragPointerGlobal;
    if (ptr == null || (_blockDrag == null && !_scrubbingPlayhead && !_boxActive)) return;
    final box = _timelineTrackKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null || !box.hasSize) return;
    final local = box.globalToLocal(ptr);
    const edge = 44.0;
    var vx = 0.0;
    if (local.dx < edge) {
      vx = -((edge - local.dx) / edge).clamp(0.15, 1.6) * 10.5;
    } else if (local.dx > box.size.width - edge) {
      vx = ((local.dx - (box.size.width - edge)) / edge).clamp(0.15, 1.6) * 10.5;
    }

    if (vx != 0 && _stripScroll.hasClients) {
      final pos = _stripScroll.position;
      // Dynamically grow trailing runway while dragging a block/key past the right edge!
      if (vx > 0 && _blockDrag != null && pos.pixels >= pos.maxScrollExtent - _cellW * 3) {
        final needed = ((pos.pixels + box.size.width) / _cellW).ceil() + 18;
        if (needed > _dragRunwayFrames) {
          setState(() => _dragRunwayFrames = needed);
        }
      }
      final before = pos.pixels;
      final next = (before + vx).clamp(pos.minScrollExtent, pos.maxScrollExtent);
      final dh = next - before;
      if (dh.abs() > 0.05) {
        _stripScroll.jumpTo(next);
        if (_blockDrag != null) {
          _updateTimelineDragFromPointer(ptr);
        } else if (_scrubbingPlayhead) {
          final f = ((local.dx.clamp(0.0, box.size.width) + next - 8.0) / _cellW).floor().clamp(0, _frames - 1);
          _scrubTo(f);
        } else if (_boxActive && _boxPath != null && _boxPath!.isNotEmpty) {
          setState(() {
            if (_lasso) {
              for (var i = 0; i < _boxPath!.length; i++) {
                _boxPath![i] = _boxPath![i].translate(-dh, 0);
              }
              _boxPath!.add(local);
            } else {
              _boxPath = [_boxPath!.first.translate(-dh, 0), local];
            }
          });
        }
      }
    }

    // Vertical auto-edge-scroll during double-tap box / lasso selection
    if (_boxActive && _boxPath != null && _layerVScroll.hasClients) {
      final showPreview = AppSettings.values['previewRow'] != false;
      final headerH = 22.0 + (showPreview ? 32.0 : 0.0);
      var vy = 0.0;
      if (local.dy > headerH && local.dy < headerH + 28) {
        vy = -((headerH + 28 - local.dy) / 28).clamp(0.15, 1.2) * 7.0;
      } else if (local.dy > box.size.height - 28) {
        vy = ((local.dy - (box.size.height - 28)) / 28).clamp(0.15, 1.2) * 7.0;
      }
      if (vy != 0) {
        final vpos = _layerVScroll.position;
        final vBefore = vpos.pixels;
        final vNext = (vBefore + vy).clamp(vpos.minScrollExtent, vpos.maxScrollExtent);
        final dv = vNext - vBefore;
        if (dv.abs() > 0.05) {
          _layerVScroll.jumpTo(vNext);
          setState(() {
            if (_lasso) {
              for (var i = 0; i < _boxPath!.length; i++) {
                _boxPath![i] = _boxPath![i].translate(0, -dv);
              }
              _boxPath!.add(local);
            } else {
              _boxPath = [_boxPath!.first.translate(0, -dv), local];
            }
          });
        }
      }
    }
  }

  /// The layer names column's width: wide on big screens, 180 on a tablet
  /// held upright (the bughunt: "Stickfigure" was cut at 140), 140 on a phone.
  static double _namesW(BuildContext context) {
    final s = MediaQuery.sizeOf(context);
    return s.width >= 900 ? 210 : s.shortestSide > 600 ? 180 : 148;
  }

  String? get _activeLayerId => (_sel?['layer'] ??
      (_st['picture'] as Map?)?['layer'] ??
      (_st['movieclip'] as Map?)?['layer'] ??
      (_st['adjustment'] as Map?)?['layer'] ??
      (_st['textfield'] as Map?)?['layer'] ??
      (_st['textfield'] as Map?)?['id']) as String?;

  /// The timeline (REDESIGN §4, D16, D27): pinned ruler + Preview header at top,
  /// vertically & horizontally scrollable layer tracks below with generous
  /// trailing runway, 2-finger pinch zoom (zero 1-finger scroll interference),
  /// crisp scrollbars, and `⋯` + swipe layer actions.
  Widget _previewRow() {
    const rulerH = 22.0, stripH = 28.0, gap = 4.0, rowH = 32.0, left = 8.0, propH = 24.0;
    final cell = _cellW;
    final fps = _fps.round().clamp(1, 240);
    final cur = _playing ? (_playT * _fps).floor().clamp(0, _frames - 1) : _frame;
    final showLayers = _layersOpen && !_playing;
    final allRows = showLayers ? ((_st['rows'] as List?) ?? const []).cast<Map>() : const <Map>[];
    // K: a folded group hides its members' rows
    final folded = {for (final r in allRows) if (r['kind'] == 'group' && r['open'] == false) r['id']};
    final rows = folded.isEmpty ? allRows : allRows.where((r) => !folded.contains(r['group'])).toList();
    final cameraSelected = _st['mode'] == 'camera';
    final soundSelected = !cameraSelected && _soundPage && _st['frameSound'] != null;
    final selLayer = (cameraSelected || soundSelected) ? null : (_activeLayerId ?? (_st['group'] as Map?)?['id'] ?? (_st['script'] as Map?)?['id']);
    final showPreview = AppSettings.values['previewRow'] != false;
    final headerH = rulerH + (showPreview ? stripH + gap : 0);

    // Layer body Y coordinates (0-based inside the scrollable layer body)
    final ys = <double>[];
    final props = <(Map, Map, double)>[]; // layer, property, y
    var yy = 0.0;
    for (final r in rows) {
      ys.add(yy);
      yy += rowH;
      if (_openRows.contains(r['id'])) {
        for (final pr in ((r['props'] as List?) ?? const []).cast<Map>()) {
          props.add((r, pr, yy));
          yy += propH;
        }
      }
    }
    final empty = showLayers && !rows.any((r) => r['kind'] != 'camera');
    if (empty) yy += 2 * rowH;
    final rawBodyH = yy + (showLayers ? 10.0 : 0.0);

    final aspect = ((_st['w'] as num?) ?? 16) / ((_st['h'] as num?) ?? 9);
    int frameAt(double x) => ((x - left) / cell).floor().clamp(0, _frames - 1);
    void frameTap(int i) {
      if (_boxActive || _boxPath != null || DateTime.now().difference(_boxEndedAt) < const Duration(milliseconds: 80)) return;
      if (_playing) _togglePlay();
      setState(() {
        _frameSelected = true;
        _soundPage = false;
        _selPoseKeys.clear();
        _selPropKeys.clear();
      });
      _cmd({'op': 'frame', 'i': i});
      if (!AppSettings.flag('keepSelection')) {
        if (_st['mode'] == 'camera') _cmd({'op': 'mode', 'v': 'figure'});
        _cmd({'op': 'select'});
      }
    }

    Widget mark(String icon, Color c) => Padding(padding: const EdgeInsets.only(right: 1), child: Icon(_markIcons[icon], size: 10, color: c));
    final markList = (_st['frameMarks'] as List?) ?? const [];
    final noTween = (_st['noTweenFrames'] as List?) ?? const [];
    final propDrag = _blockDrag != null && _blockDrag!.kind == 'prop_key' ? _blockDrag : null;
    final int? loopStartF = (_loopEnd > 0 || _loopToStart) ? (_loopToStart ? _frame : 0) : null;
    final int? loopEndF = (_loopEnd > 0 || _loopToStart) ? (_loopEnd > 0 ? math.min(_loopEnd, _frames) : _frames) : null;

    // Trailing runway calculation: always at least +14 frames beyond _frames,
    // expanding dynamically during rightward grip/key drags!
    int dragEndFrame = _frames;
    if (_blockDrag != null) {
      if (_blockDrag!.kind == 'multi_shift' || _blockDrag!.kind == 'multi_scale') {
        final fs = <int>[for (final (_, f) in _selPoseKeys) f, for (final (_, f) in _selPropKeys) f];
        if (fs.isNotEmpty) dragEndFrame = math.max(dragEndFrame, fs.reduce(math.max) + 14);
      } else {
        final (dSt, dLen) = _blockDrag!.preview(allowStart0: _blockDrag!.kind == 'prop_key' || _blockDrag!.run != null);
        dragEndFrame = math.max(dragEndFrame, dSt + dLen + 10);
      }
    }
    final baseCells = math.max(math.max(_frames + 14, _dragRunwayFrames), dragEndFrame);

    final namesColW = _namesW(context);
    final screenH = MediaQuery.sizeOf(context).height;
    final totalNaturalH = headerH + rawBodyH + (!showLayers && !showPreview ? 4.0 : 0.0);
    final double? fixedH = showLayers && _timelineHeight != null ? _timelineHeight!.clamp(96.0, math.max(132.0, screenH * 0.62)).toDouble() : null;
    final autoCap = screenH < 500 ? math.min(188.0, screenH * 0.42) : math.min(284.0, screenH * 0.38);
    final autoMaxH = showLayers ? math.min(totalNaturalH, math.max(headerH + rowH * 2.2, autoCap)) : totalNaturalH;
    final effectiveTotalH = fixedH ?? autoMaxH;
    _lastTimelineBodyH = effectiveTotalH;

    // Build each layer's name cell with eye, lock, swipe actions AND discoverable `⋯` button
    Widget nameCell(Map r) {
      final on = r['visible'] != false;
      final locked = r['locked'] == true;
      final kind = '${r['kind']}';
      final id = '${r['id']}';
      final selected = (kind == 'camera' && cameraSelected) ||
          (kind == 'sound' && soundSelected) ||
          (!cameraSelected && !soundSelected && (id == selLayer || ((_st['multi'] as List?)?.any((m) => (m as Map)['id'] == id) ?? false)));
      const icons = {'figure': 'figure', 'picture': 'picture', 'text': 'text', 'movieclip': 'clip', 'adjustment': 'fx', 'camera': 'camera', 'sound': 'sound', 'group': 'folder', 'script': 'script'};
      final isGroup = kind == 'group';
      final canHide = kind != 'camera' && kind != 'sound';
      final moreKey = GlobalKey();
      final row = Container(
        height: rowH,
        padding: EdgeInsets.only(left: r['group'] != null ? 20 : 5, right: 2),
        decoration: BoxDecoration(color: selected ? cSelected : null, border: const Border(bottom: BorderSide(color: Color(0xFFEEF0F2)))),
        child: Row(children: [
          if (isGroup)
            kit.Tap(
              label: r['open'] == false ? 'Open ${r['name']}' : 'Fold ${r['name']}',
              minSize: 22,
              onTap: () => _cmd({'op': 'group_set', 'group': id, 'open': r['open'] == false}),
              child: Transform.rotate(angle: r['open'] == false ? -math.pi / 2 : 0, child: const Pictogram('chevron', size: 14, color: cSecondary)),
            )
          else if (((r['props'] as List?) ?? const []).isNotEmpty)
            kit.Tap(
              label: _openRows.contains(id) ? 'Hide ${r['name']}\u2019s animated values' : 'Show ${r['name']}\u2019s animated values',
              minSize: 22,
              onTap: () => setState(() => _openRows.contains(id) ? _openRows.remove(id) : _openRows.add(id)),
              child: Transform.rotate(angle: _openRows.contains(id) ? 0 : -math.pi / 2, child: const Pictogram('chevron', size: 14, color: cSecondary)),
            )
          else
            const SizedBox(width: 14),
          const SizedBox(width: 2),
          Pictogram(icons[kind] ?? 'figure', size: 17, color: selected ? cAccent : (kind == 'adjustment' ? const Color(0xFF7B5CE0) : cInk2b)),
          const SizedBox(width: 5),
          Expanded(
            child: Text.rich(
              TextSpan(children: [
                TextSpan(text: '${r['name']}'),
                if (kind == 'script' && r['sub'] != null) TextSpan(text: '  ${r['sub']}', style: const TextStyle(color: cSecondary, fontSize: 11.5, fontWeight: FontWeight.w400)),
              ]),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: selected ? cAccent : on ? cText : cSecondary, fontSize: 13.5, fontWeight: selected ? FontWeight.w700 : FontWeight.w400),
            ),
          ),
          if (locked)
            kit.Tap(
              label: 'Unlock ${r['name']}',
              minSize: 22,
              onTap: _playing ? null : () => _cmd({'op': 'layer_lock', 'layer': id, 'v': false}),
              child: const Padding(padding: EdgeInsets.only(right: 1), child: Pictogram('lock', size: 13, color: cAccent)),
            ),
          if (canHide)
            kit.Tap(
              label: on ? 'Hide ${r['name']}' : 'Show ${r['name']}',
              minSize: 24,
              onTap: _playing ? null : () => _cmd({'op': 'layer_visible', 'layer': id, 'v': !on}),
              child: Pictogram('eye', size: 16, color: on ? const Color(0xFF9AA0A8) : const Color(0xFFD5D8DD)),
            ),
          if (!_playing)
            kit.Tap(
              key: moreKey,
              label: '${r['name']} options',
              minSize: 24,
              onTap: () => _layerActionsPopover(r, moreKey),
              child: const Pictogram('more', size: 15, color: Color(0xFF8A9099)),
            ),
        ]),
      );
      if (_playing) return row;

      void pick() {
        setState(() {
          _frameSelected = false;
          _selPoseKeys.clear();
          _selPropKeys.clear();
          if (kind != 'sound') _soundPage = false;
        });
        if (isGroup) {
          _cmd({'op': 'select', 'group': id});
        } else if (kind == 'script') {
          _pickScript(id, null);
        } else if (kind == 'camera') {
          _setMode('camera');
        } else if (kind == 'sound') {
          final clips = ((r['clips'] as List?) ?? const []).cast<List>();
          _pickSound(frame: clips.isNotEmpty ? (clips.first[0] as num).floor() : null);
        } else {
          final onList = (r['on'] as List?) ?? const [];
          if (onList.isNotEmpty && onList.elementAtOrNull(_frame) == false) {
            final firstOn = onList.indexWhere((v) => v != false);
            if (firstOn >= 0) {
              _cmd({'op': 'frame', 'i': firstOn});
              WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(firstOn));
            }
          }
          _cmd({'op': 'select', 'layer': id});
        }
      }

      final actions = <rw.SwipeAction>[
        if (kind == 'figure' || kind == 'picture' || kind == 'text' || kind == 'movieclip' || kind == 'adjustment') ...[
          rw.SwipeAction(locked ? 'Unlock' : 'Lock', () => _cmd({'op': 'layer_lock', 'layer': id, 'v': !locked}), icon: locked ? 'unlock' : 'lock'),
          rw.SwipeAction('Copy', () => _cmd({'op': 'duplicate_figure', 'layer': id})),
          rw.SwipeAction('Delete', () {
            _cmd({'op': 'delete_everywhere', 'layer': id});
            kit.showToast(context, '${r['name']} deleted from every frame.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          }, danger: true, icon: 'bin'),
        ] else if (isGroup) ...[
          rw.SwipeAction('Ungroup', () => _cmd({'op': 'ungroup', 'group': id})),
          rw.SwipeAction('Delete', () {
            _cmd({'op': 'delete_group', 'group': id});
            kit.showToast(context, '${r['name']} deleted.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          }, danger: true, icon: 'bin'),
        ] else if (kind == 'script') ...[
          rw.SwipeAction(on ? 'Hide' : 'Show', () => _cmd({'op': 'layer_visible', 'layer': id, 'v': !on}), icon: 'eye'),
          rw.SwipeAction('Delete', () {
            _pickScript(id, null);
            _cmd({'op': 'script_delete'});
            kit.showToast(context, '${r['name']} deleted.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          }, danger: true, icon: 'bin'),
        ] else if (kind == 'sound') ...[
          rw.SwipeAction('Delete', () {
            _cmd({'op': 'delete_everywhere', 'layer': id});
            kit.showToast(context, '${r['name']} deleted.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          }, danger: true, icon: 'bin'),
        ],
      ];

      final tappable = GestureDetector(behavior: HitTestBehavior.opaque, onTap: pick, child: row);
      if (actions.isEmpty) return tappable;
      final aw = math.min(62.0, math.max(40.0, (namesColW - 14) / actions.length));
      return rw.SwipeActions(
        key: ValueKey('row-$id-$_rowActionsOpen'),
        initiallyOpen: _rowActionsOpen && id == selLayer,
        actionWidth: aw,
        actions: actions,
        child: tappable,
      );
    }

    // Left pinned header (Timeline zoom controls + Preview toggle)
    final namesHeader = Container(
      width: namesColW,
      height: headerH,
      decoration: const BoxDecoration(color: cPanel, border: Border(right: BorderSide(color: Color(0xFFE6E8EB)))),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Container(
          height: rulerH,
          padding: const EdgeInsets.symmetric(horizontal: 6),
          decoration: const BoxDecoration(border: Border(bottom: BorderSide(color: Color(0xFFE6E8EB)))),
          child: Row(children: [
            if (showLayers && namesColW >= 170)
              const Expanded(child: Text('Layers', style: TextStyle(color: cSecondary, fontSize: 11, fontWeight: FontWeight.w600)))
            else
              const Spacer(),
            kit.HitPad(
              dy: 6,
              child: Row(mainAxisSize: MainAxisSize.min, children: [
                kit.Tap(
                  label: 'Zoom timeline out',
                  minSize: 22,
                  radius: 5,
                  onTap: () => _zoomTimeline(1 / 1.25),
                  child: const Padding(padding: EdgeInsets.symmetric(horizontal: 4, vertical: 2), child: Pictogram('minus', size: 12, color: cInk2b, stroke: 2.2)),
                ),
                kit.Tap(
                  label: 'Fit all frames in view',
                  minSize: 22,
                  radius: 5,
                  onTap: _fitTimeline,
                  child: const Padding(padding: EdgeInsets.symmetric(horizontal: 5, vertical: 2), child: Text('Fit', style: TextStyle(color: cAccent, fontSize: 10.5, fontWeight: FontWeight.w700))),
                ),
                kit.Tap(
                  label: 'Zoom timeline in',
                  minSize: 22,
                  radius: 5,
                  onTap: () => _zoomTimeline(1.25),
                  child: const Padding(padding: EdgeInsets.symmetric(horizontal: 4, vertical: 2), child: Pictogram('plus', size: 12, color: cInk2b, stroke: 2.2)),
                ),
              ]),
            ),
          ]),
        ),
        if (showPreview)
          Container(
            height: stripH + gap,
            decoration: const BoxDecoration(border: Border(bottom: BorderSide(color: Color(0xFFDADDE2)))),
            child: Padding(
              padding: const EdgeInsets.only(left: 24, right: 4),
              child: Row(children: [
                const Expanded(child: Text('Preview', style: TextStyle(color: cSecondary, fontSize: 12))),
                kit.Tap(
                  label: 'Hide the preview',
                  minSize: 26,
                  onTap: () => setState(() {
                    AppSettings.values['previewRow'] = false;
                    unawaited(AppSettings.save());
                  }),
                  child: const Pictogram('eye', size: 16, color: Color(0xFF9AA0A8)),
                ),
              ]),
            ),
          ),
      ]),
    );

    final lockScroll = _boxPath != null || _blockDrag != null || _scrubbingPlayhead || _tlPinch.length >= 2;

    return Container(
      color: cCard,
      height: effectiveTotalH,
      child: LayoutBuilder(builder: (context, outerBox) {
        final trackVw = math.max(120.0, outerBox.maxWidth - namesColW);
        _lastTrackViewportW = trackVw;
        final minViewportCells = ((trackVw - left * 2) / cell).ceil() + 8;
        final displayCells = math.max(baseCells, minViewportCells);
        final width = left * 2 + displayCells * cell;
        final bodyViewH = math.max(0.0, outerBox.maxHeight - headerH);
        final bodyContentH = math.max(rawBodyH, bodyViewH);

        // Pinned ruler + Preview strip track content
        final headerTrack = SizedBox(
          width: width,
          height: headerH,
          child: Stack(clipBehavior: Clip.none, children: [
            Positioned.fill(
              child: CustomPaint(
                painter: _TimelinePainter(
                  frames: _frames,
                  displayCells: displayCells,
                  cell: cell,
                  left: left,
                  fps: fps,
                  cur: cur,
                  rulerH: rulerH,
                  stripH: showPreview ? stripH : 0,
                  lines: const [],
                  bands: const [],
                  aspect: aspect.toDouble(),
                  thumbs: _thumbs,
                  loopStart: loopStartF,
                  loopEnd: loopEndF,
                  drawHeader: true,
                  drawBody: false,
                ),
              ),
            ),
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTapUp: (d) {
                  if (_boxActive || _boxPath != null) return;
                  frameTap(frameAt(d.localPosition.dx));
                },
                onLongPressStart: _playing
                    ? null
                    : (d) {
                        if (_boxActive || _boxPath != null) return;
                        _copyPopup(frameAt(d.localPosition.dx));
                      },
                onHorizontalDragStart: (d) {
                  if (_boxActive || _boxPath != null || _tlPinch.length >= 2) return;
                  if (_playing) _togglePlay();
                  _scrubbingPlayhead = true;
                  _dragPointerGlobal = d.globalPosition;
                  _scrubTo(frameAt(d.localPosition.dx));
                },
                onHorizontalDragUpdate: (d) {
                  if (_boxActive || _boxPath != null || _tlPinch.length >= 2) return;
                  _dragPointerGlobal = d.globalPosition;
                  _scrubTo(frameAt(d.localPosition.dx));
                },
                onHorizontalDragEnd: (_) {
                  _scrubbingPlayhead = false;
                  _dragPointerGlobal = null;
                },
                onHorizontalDragCancel: () {
                  _scrubbingPlayhead = false;
                  _dragPointerGlobal = null;
                },
              ),
            ),
            if (showPreview)
              for (var i = 0; i < _frames; i++)
                if ((markList.elementAtOrNull(i) as List?)?.any((m) => m == true) == true || (_st['tweening'] == true && noTween.elementAtOrNull(i) == true && i < _frames - 1))
                  Positioned(
                    left: left + i * cell + 3,
                    top: rulerH + stripH - 12,
                    child: IgnorePointer(
                      child: Row(children: [
                        if ((markList[i] as List?)?.first == true) mark('pause', const Color(0xFFD9821E)),
                        if ((markList[i] as List?)?.elementAtOrNull(1) == true) mark('repeat', cConnected),
                        if ((markList[i] as List?)?.elementAtOrNull(2) == true) mark('silence', cDanger),
                        if ((markList[i] as List?)?.elementAtOrNull(3) == true) mark('slow', cConnected),
                        if ((markList[i] as List?)?.elementAtOrNull(4) == true) mark('camera', const Color(0xFFD94F94)),
                        if (_st['tweening'] == true && noTween.elementAtOrNull(i) == true && i < _frames - 1) mark('notween', cDanger),
                      ]),
                    ),
                  ),
            if (_frameSelected && showPreview && !_playing)
              Positioned(
                left: left + cur * cell - 1,
                top: rulerH,
                width: cell + 2,
                height: stripH,
                child: IgnorePointer(child: Container(decoration: BoxDecoration(border: Border.all(color: cAccent, width: 2), borderRadius: BorderRadius.circular(5)))),
              ),
            Positioned(left: left + cur * cell + cell / 2 - 0.75, top: rulerH - 2, width: 1.5, height: headerH - rulerH + 2, child: const IgnorePointer(child: ColoredBox(color: Color(0xFF8DB8E8)))),
            Positioned(
              left: left + cur * cell + cell / 2 - 22,
              top: 0,
              child: Semantics(
                button: true,
                label: 'Frame ${cur + 1} of $_frames',
                child: MouseRegion(
                  cursor: SystemMouseCursors.grab,
                  child: GestureDetector(
                    key: _playheadKey,
                    behavior: HitTestBehavior.opaque,
                    dragStartBehavior: DragStartBehavior.down,
                    onTap: _playing ? null : _frameJumpPopover,
                    onHorizontalDragStart: (d) {
                      if (_boxActive || _boxPath != null || _tlPinch.length >= 2) return;
                      if (_playing) _togglePlay();
                      setState(() => _scrubbingPlayhead = true);
                      _dragPointerGlobal = d.globalPosition;
                      final box = _timelineTrackKey.currentContext?.findRenderObject() as RenderBox?;
                      if (box != null && box.hasSize) {
                        final scrollX = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
                        _scrubTo(frameAt(box.globalToLocal(d.globalPosition).dx + scrollX));
                      }
                    },
                    onHorizontalDragUpdate: (d) {
                      if (_boxActive || _boxPath != null || _tlPinch.length >= 2) return;
                      _dragPointerGlobal = d.globalPosition;
                      final box = _timelineTrackKey.currentContext?.findRenderObject() as RenderBox?;
                      if (box != null && box.hasSize) {
                        final scrollX = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
                        _scrubTo(frameAt(box.globalToLocal(d.globalPosition).dx + scrollX));
                      }
                    },
                    onHorizontalDragEnd: (_) {
                      setState(() => _scrubbingPlayhead = false);
                      _dragPointerGlobal = null;
                    },
                    onHorizontalDragCancel: () {
                      setState(() => _scrubbingPlayhead = false);
                      _dragPointerGlobal = null;
                    },
                    child: Container(
                      width: 44,
                      height: 22,
                      alignment: Alignment.center,
                      child: Container(
                        constraints: const BoxConstraints(minWidth: 36),
                        height: 19,
                        padding: const EdgeInsets.symmetric(horizontal: 6),
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          color: _scrubbingPlayhead ? cAccent : const Color(0xFFDCEAF9),
                          borderRadius: BorderRadius.circular(9),
                        ),
                        child: Text(
                          '${cur + 1}',
                          style: TextStyle(
                            color: _scrubbingPlayhead ? cCard : cAccent,
                            fontSize: 11,
                            fontWeight: FontWeight.w700,
                            fontFeatures: const [ui.FontFeature.tabularFigures()],
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ]),
        );

        // Folded or playing: only the header row shows, directly scrollable horizontally
        if (!showLayers) {
          return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            namesHeader,
            Expanded(
              key: _timelineTrackKey,
              child: _wrapTimelinePinchAndWheel(
                SingleChildScrollView(
                  controller: _stripScroll,
                  scrollDirection: Axis.horizontal,
                  physics: lockScroll ? const NeverScrollableScrollPhysics() : const ClampingScrollPhysics(),
                  child: headerTrack,
                ),
              ),
            ),
          ]);
        }

        // Unfolded: Pinned Header at top + Synchronized Vertical & Horizontal Layer Body below
        final namesBody = Container(
          width: namesColW,
          constraints: BoxConstraints(minHeight: bodyContentH),
          decoration: const BoxDecoration(color: cPanel, border: Border(right: BorderSide(color: Color(0xFFE6E8EB)))),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            for (final r in rows) ...[
              nameCell(r),
              if (_openRows.contains(r['id']))
                for (final pr in ((r['props'] as List?) ?? const []).cast<Map>())
                  Container(
                    height: propH,
                    padding: const EdgeInsets.only(left: 28, right: 2),
                    decoration: const BoxDecoration(color: Color(0xFFFAFBFC), border: Border(bottom: BorderSide(color: Color(0xFFEEF0F2)))),
                    child: Row(children: [
                      Expanded(child: Text('${pr['name']}', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cSecondary, fontSize: 12))),
                      kit.Tap(
                        label: 'Stop animating ${pr['name']}',
                        minSize: 24,
                        onTap: () => _cmd({'op': 'stop_animating', 'property': pr['id']}),
                        child: const Pictogram('x', size: 13, color: Color(0xFF9AA0A8)),
                      ),
                    ]),
                  ),
            ],
          ]),
        );

        final bodyTrack = SizedBox(
          width: width,
          height: bodyContentH,
          child: Stack(clipBehavior: Clip.none, children: [
            Positioned.fill(
              child: CustomPaint(
                painter: _TimelinePainter(
                  frames: _frames,
                  displayCells: displayCells,
                  cell: cell,
                  left: left,
                  fps: fps,
                  cur: cur,
                  rulerH: 0,
                  stripH: 0,
                  lines: [for (final y in ys) y + rowH, for (final (_, _, y) in props) y + propH],
                  bands: [for (final (_, _, y) in props) (y, propH)],
                  aspect: aspect.toDouble(),
                  thumbs: const {},
                  groupStrips: [
                    for (var k = 0; k < rows.length; k++)
                      if (rows[k]['kind'] == 'group' && rows[k]['open'] == false) (ys[k] + 2, rowH - 5, _groupThumbs['${rows[k]['id']}'] ?? const <int, ui.Image>{}),
                  ],
                  drawHeader: false,
                  drawBody: true,
                ),
              ),
            ),
            // Empty timeline / runway cell tap handler (Tap ONLY — zero drag recognizers, so 1-finger horizontal & vertical scroll work everywhere!)
            if (!_playing && rows.isNotEmpty)
              Positioned.fill(
                child: GestureDetector(
                  behavior: HitTestBehavior.translucent,
                  onTapUp: (d) {
                    if (_boxActive || _boxPath != null || DateTime.now().difference(_boxEndedAt) < const Duration(milliseconds: 80)) return;
                    final f = ((d.localPosition.dx - left) / cell).floor();
                    if (f < 0 || f >= displayCells) return;
                    final dy = d.localPosition.dy;
                    for (var k = 0; k < rows.length; k++) {
                      if (dy >= ys[k] && dy < ys[k] + rowH) {
                        _onEmptyTimelineCellTap(rows[k], f, d.globalPosition, menu: false);
                        return;
                      }
                    }
                  },
                  // a hold on an empty cell offers Extend / New pose there
                  onLongPressStart: (d) {
                    if (_boxActive || _boxPath != null) return;
                    final f = ((d.localPosition.dx - left) / cell).floor();
                    if (f < 0 || f >= displayCells) return;
                    final dy = d.localPosition.dy;
                    for (var k = 0; k < rows.length; k++) {
                      if (dy >= ys[k] && dy < ys[k] + rowH) {
                        if (AppSettings.flag('haptics')) unawaited(HapticFeedback.selectionClick());
                        _onEmptyTimelineCellTap(rows[k], f, d.globalPosition, menu: true);
                        return;
                      }
                    }
                  },
                ),
              ),
            // Layer pose blocks
            for (var k = 0; k < rows.length; k++) ..._rowBlocks(rows[k], ys[k], rowH, cell, left, rows[k]['id'] == selLayer),
            // Property keyframe rows & diamonds
            for (final (r, pr, y) in props) ...[
              if (!_playing && _st['propKeyCopied'] == true)
                Positioned(
                  left: left,
                  top: y,
                  width: displayCells * cell,
                  height: propH,
                  child: GestureDetector(
                    behavior: HitTestBehavior.translucent,
                    onTapUp: (d) {
                      if (_boxActive || _boxPath != null) return;
                      final f = (d.localPosition.dx / cell).floor().clamp(0, displayCells - 1);
                      final existing = ((pr['keys'] as List?) ?? const []).cast<num>().map((v) => v.toInt());
                      if (!existing.contains(f)) _propPastePopover(pr, f, d.globalPosition);
                    },
                  ),
                ),
              for (var ki = 0; ki < ((pr['keys'] as List?) ?? const []).length; ki++)
                () {
                  final f = (((pr['keys'] as List)[ki]) as num).toInt();
                  final pid = '${pr['id']}';
                  final eases = (pr['eases'] as List?) ?? const [];
                  final ease = ki < eases.length ? '${eases[ki]}' : 'auto';
                  final isDragged = propDrag != null && propDrag.property == pid && (propDrag.live ? f == (propDrag.lastSent ?? propDrag.start) : propDrag.start == f);
                  final isBoxSel = _selPropKeys.contains((pid, f));
                  final diamondSize = isBoxSel || isDragged ? 10.0 : 8.0;
                  return Positioned(
                    left: left + f * cell + cell / 2 - 12,
                    top: y + propH / 2 - 12,
                    width: 24,
                    height: 24,
                    child: _playing
                        ? Center(child: Transform.rotate(angle: math.pi / 4, child: Container(width: 8, height: 8, decoration: BoxDecoration(color: const Color(0xFF8A9099), borderRadius: BorderRadius.circular(1.5)))))
                        : MouseRegion(
                            cursor: SystemMouseCursors.grab,
                            child: GestureDetector(
                              behavior: HitTestBehavior.opaque,
                              dragStartBehavior: DragStartBehavior.start,
                              onTapUp: (d) {
                                if (_boxActive || _boxPath != null || DateTime.now().difference(_boxEndedAt) < const Duration(milliseconds: 80)) return;
                                setState(() {
                                  _frameSelected = false;
                                  _selPoseKeys.clear();
                                  _selPropKeys
                                    ..clear()
                                    ..add((pid, f));
                                });
                                _cmd({'op': 'frame', 'i': f.clamp(0, _frames - 1)});
                              },
                              onLongPressStart: (d) {
                                if (_boxActive || _boxPath != null) return;
                                if (AppSettings.flag('haptics')) unawaited(HapticFeedback.selectionClick());
                                final rect = kit.Anchored.rectFromGlobal(context, d.globalPosition, size: 20);
                                final totalSel = _selPoseKeys.length + _selPropKeys.length;
                                if (totalSel > 1 && isBoxSel) {
                                  _multiKeyPopover(rect);
                                } else {
                                  _propKeyPopover(r, pr, f, ease, rect);
                                }
                              },
                              onSecondaryTapUp: (d) {
                                final rect = kit.Anchored.rectFromGlobal(context, d.globalPosition, size: 20);
                                final totalSel = _selPoseKeys.length + _selPropKeys.length;
                                if (totalSel > 1 && isBoxSel) {
                                  _multiKeyPopover(rect);
                                } else {
                                  _propKeyPopover(r, pr, f, ease, rect);
                                }
                              },
                              onHorizontalDragStart: (d) {
                                _dragPointerGlobal = d.globalPosition;
                                final totalSel = _selPoseKeys.length + _selPropKeys.length;
                                if (isBoxSel && totalSel > 1) {
                                  _startMultiKeyDrag(kind: 'multi_shift', cell: cell, rawStartX: d.globalPosition.dx);
                                } else {
                                  _startTimelineDrag(
                                    _BlockDrag('${r['id']}', f, 1, cell, edge: 1, kind: 'prop_key', property: pid),
                                    rawStartX: d.globalPosition.dx,
                                  );
                                }
                              },
                              onHorizontalDragUpdate: (d) {
                                _dragPointerGlobal = d.globalPosition;
                                _updateTimelineDragFromPointer(d.globalPosition);
                              },
                              onHorizontalDragEnd: (_) {
                                _dragPointerGlobal = null;
                                _endBlockDrag();
                              },
                              onHorizontalDragCancel: () {
                                _dragPointerGlobal = null;
                                _endBlockDrag(cancel: true);
                              },
                              child: Center(
                                child: Transform.rotate(
                                  angle: math.pi / 4,
                                  child: Container(
                                    width: diamondSize,
                                    height: diamondSize,
                                    decoration: BoxDecoration(
                                      color: isBoxSel || isDragged ? cAccent : const Color(0xFF8A9099),
                                      borderRadius: BorderRadius.circular(1.5),
                                      border: isBoxSel || isDragged ? Border.all(color: cCard, width: 1.2) : null,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ),
                  );
                }(),
              if (propDrag != null && !propDrag.live && propDrag.property == '${pr['id']}')
                () {
                  final (st, _) = propDrag.preview(allowStart0: true);
                  return Positioned(
                    left: left + st * cell + cell / 2 - 6,
                    top: y + propH / 2 - 6,
                    width: 12,
                    height: 12,
                    child: IgnorePointer(
                      child: Center(
                        child: Transform.rotate(
                          angle: math.pi / 4,
                          child: Container(
                            width: 10,
                            height: 10,
                            decoration: BoxDecoration(color: cAccent, borderRadius: BorderRadius.circular(2), border: Border.all(color: cCard, width: 1.2)),
                          ),
                        ),
                      ),
                    ),
                  );
                }(),
            ],
            // Multi-keyframe selection bounding bracket + left/right proportional stretch handles
            if (!_playing && (_selPoseKeys.length + _selPropKeys.length) >= 2)
              ..._multiKeyRangeOverlay(rows, ys, props, cell, left, rowH, propH),
            // Vertical playhead line through all layer rows
            Positioned(left: left + cur * cell + cell / 2 - 0.75, top: 0, width: 1.5, height: bodyContentH, child: const IgnorePointer(child: ColoredBox(color: Color(0xFF8DB8E8)))),
          ]),
        );

        // Synced header strip (follows _stripScroll with zero controller conflict)
        final syncedHeader = ListenableBuilder(
          listenable: _stripScroll,
          builder: (context, child) {
            final off = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
            return ClipRect(
              child: OverflowBox(
                alignment: Alignment.topLeft,
                minWidth: 0,
                maxWidth: double.infinity,
                child: Transform.translate(offset: Offset(-off, 0), child: child),
              ),
            );
          },
          child: headerTrack,
        );

        Widget rightTrackColumn = Column(children: [
          SizedBox(height: headerH, width: double.infinity, child: syncedHeader),
          Expanded(
            child: SingleChildScrollView(
              controller: _layerVScroll,
              physics: lockScroll ? const NeverScrollableScrollPhysics() : const ClampingScrollPhysics(),
              child: SizedBox(
                height: bodyContentH,
                child: SingleChildScrollView(
                  controller: _stripScroll,
                  scrollDirection: Axis.horizontal,
                  physics: lockScroll ? const NeverScrollableScrollPhysics() : const ClampingScrollPhysics(),
                  child: bodyTrack,
                ),
              ),
            ),
          ),
        ]);

        rightTrackColumn = _wrapTimelinePinchAndWheel(rightTrackColumn);

        // Double-tap selection box & triple-tap lasso across Preview strip + Layer rows
        if (rows.isNotEmpty) {
          rightTrackColumn = Listener(
            behavior: HitTestBehavior.translucent,
            onPointerDown: (e) {
              final now = DateTime.now();
              final quick = now.difference(_lastDown) < const Duration(milliseconds: 240) && (e.localPosition - _lastDownAt).distance < 24;
              _taps = quick ? _taps + 1 : 1;
              _lastDown = now;
              _lastDownAt = e.localPosition;
              if (_taps == 1) {
                _boxActive = false;
                _preBoxFrame = _frame;
                _preBoxFrameSel = _frameSelected;
                _preBoxInPreview = e.localPosition.dy >= rulerH && e.localPosition.dy < headerH;
              } else if (_taps >= 2 && !_playing && e.localPosition.dy >= rulerH) {
                _boxActive = true;
                _scrubbingPlayhead = false;
                _dragPointerGlobal = e.position;
                if (_preBoxInPreview && _preBoxFrame != null) {
                  if (_frame != _preBoxFrame) _cmd({'op': 'frame', 'i': _preBoxFrame});
                  _frameSelected = _preBoxFrameSel ?? false;
                }
                setState(() {
                  _boxPath = [e.localPosition];
                  _lasso = _taps >= 3;
                });
              }
            },
            onPointerMove: (e) {
              final path = _boxPath;
              if (path == null || !_boxActive) return;
              _dragPointerGlobal = e.position;
              setState(() {
                if (_lasso) {
                  path.add(e.localPosition);
                } else {
                  _boxPath = [path.first, e.localPosition];
                }
              });
            },
            onPointerUp: (_) {
              if (_boxActive) {
                _dragPointerGlobal = null;
                _finishBox(rows, ys, props, cell, rowH, propH, headerH: headerH);
              }
              _endOrphanedDrag();
            },
            onPointerCancel: (_) {
              if (_boxActive) {
                _dragPointerGlobal = null;
                setState(() {
                  _boxActive = false;
                  _boxPath = null;
                });
              }
              _endOrphanedDrag(cancel: true);
            },
            child: Stack(children: [
              rightTrackColumn,
              if (_boxPath != null) Positioned.fill(child: AbsorbPointer(child: CustomPaint(painter: _BoxPainter(_boxPath!, _lasso)))),
            ]),
          );
        }

        // Sync vertical scroll between left `namesBody` and right `_layerVScroll`!
        final syncedNamesBody = ListenableBuilder(
          listenable: _layerVScroll,
          builder: (context, child) {
            final vOff = _layerVScroll.hasClients ? _layerVScroll.offset : 0.0;
            return ClipRect(
              child: OverflowBox(
                alignment: Alignment.topLeft,
                minHeight: 0,
                maxHeight: double.infinity,
                child: Transform.translate(offset: Offset(0, -vOff), child: child),
              ),
            );
          },
          child: Listener(
            onPointerSignal: (ev) {
              if (ev is PointerScrollEvent && _layerVScroll.hasClients) {
                final pos = _layerVScroll.position;
                _layerVScroll.jumpTo((pos.pixels + ev.scrollDelta.dy).clamp(pos.minScrollExtent, pos.maxScrollExtent));
              }
            },
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onVerticalDragUpdate: (d) {
                if (!_layerVScroll.hasClients) return;
                final pos = _layerVScroll.position;
                _layerVScroll.jumpTo((pos.pixels - d.delta.dy).clamp(pos.minScrollExtent, pos.maxScrollExtent));
              },
              onVerticalDragEnd: (d) {
                if (!_layerVScroll.hasClients) return;
                final v = -(d.primaryVelocity ?? 0);
                if (v.abs() < 60) return;
                final pos = _layerVScroll.position;
                final target = (pos.pixels + v * 0.22).clamp(pos.minScrollExtent, pos.maxScrollExtent);
                unawaited(_layerVScroll.animateTo(target, duration: const Duration(milliseconds: 260), curve: Curves.decelerate));
              },
              child: namesBody,
            ),
          ),
        );

        return Stack(children: [
          Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            SizedBox(
              width: namesColW,
              child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                namesHeader,
                Expanded(child: syncedNamesBody),
              ]),
            ),
            Expanded(
              key: _timelineTrackKey,
              child: RawScrollbar(
                controller: _layerVScroll,
                notificationPredicate: (n) => n.metrics.axis == Axis.vertical,
                thumbVisibility: rawBodyH > bodyViewH + 4,
                thickness: 3.5,
                radius: const Radius.circular(2),
                thumbColor: const Color(0x666B7078),
                padding: EdgeInsets.only(top: headerH + 2, bottom: 6, right: 1),
                child: rightTrackColumn,
              ),
            ),
          ]),
          if (empty)
            Positioned(
              left: namesColW,
              right: 0,
              top: headerH + math.max(0, yy - 2 * rowH),
              height: 2 * rowH,
              child: const IgnorePointer(child: Center(child: Text('Nothing here yet. Tap + to add a stick figure.', style: TextStyle(color: cSecondary, fontSize: 14)))),
            ),
        ]);
      }),
    );
  }

  /// Pure raw-pointer 2-finger pinch-to-zoom + Ctrl/wheel zoom wrapper.
  /// Never enters the gesture arena for 1 finger, so horizontal and vertical
  /// `SingleChildScrollView` scrolling work with native momentum everywhere!
  Widget _wrapTimelinePinchAndWheel(Widget child) {
    return Listener(
      behavior: HitTestBehavior.translucent,
      onPointerSignal: (e) {
        if (e is PointerScrollEvent) {
          if (HardwareKeyboard.instance.isControlPressed) {
            _zoomTimeline(e.scrollDelta.dy > 0 ? 1 / 1.15 : 1.15, anchorLocalX: e.localPosition.dx);
          } else if (e.scrollDelta.dx.abs() > 0 && _stripScroll.hasClients) {
            final pos = _stripScroll.position;
            _stripScroll.jumpTo((pos.pixels + e.scrollDelta.dx).clamp(pos.minScrollExtent, pos.maxScrollExtent));
          }
        }
      },
      onPointerDown: (e) {
        _tlPinch[e.pointer] = e.localPosition;
        if (_tlPinch.length == 2) {
          _scrubbingPlayhead = false;
          final pts = _tlPinch.values.toList();
          _tlPinchSpan = (pts[0] - pts[1]).distance;
          _tlPinchCellStart = _cellW;
          final midX = (pts[0].dx + pts[1].dx) / 2;
          final hOff = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
          _tlPinchAnchorFrame = (midX + hOff - 8.0) / _cellW;
          setState(() {});
        }
      },
      onPointerMove: (e) {
        if (!_tlPinch.containsKey(e.pointer)) return;
        _tlPinch[e.pointer] = e.localPosition;
        if (_tlPinch.length >= 2 && _tlPinchSpan > 12) {
          final pts = _tlPinch.values.toList();
          final span = (pts[0] - pts[1]).distance;
          final midX = (pts[0].dx + pts[1].dx) / 2;
          final nextCell = (_tlPinchCellStart * (span / _tlPinchSpan)).clamp(14.0, 112.0);
          if ((nextCell - _cellW).abs() > 0.05) {
            setState(() => _cellW = nextCell);
            if (_stripScroll.hasClients) {
              final to = (8.0 + _tlPinchAnchorFrame * nextCell - midX).clamp(0.0, _stripScroll.position.maxScrollExtent);
              _stripScroll.jumpTo(to);
            }
          }
        }
      },
      onPointerUp: (e) {
        final wasPinch = _tlPinch.length >= 2;
        _tlPinch.remove(e.pointer);
        if (wasPinch && _tlPinch.length < 2) setState(() {});
      },
      onPointerCancel: (e) {
        final wasPinch = _tlPinch.length >= 2;
        _tlPinch.remove(e.pointer);
        if (wasPinch && _tlPinch.length < 2) setState(() {});
      },
      child: child,
    );
  }

  /// Discoverable `⋯` layer actions popover (complements left-swipe on layer rows).
  Future<void> _layerActionsPopover(Map r, GlobalKey key) async {
    final screen = MediaQuery.sizeOf(context);
    final compact = screen.height < 450;
    final topSafe = 44.0 + MediaQuery.paddingOf(context).top;
    final anchor = kit.Anchored.rectOf(key) ?? Rect.fromLTWH(120, screen.height - 160, 28, 28);
    final kind = '${r['kind']}';
    final id = '${r['id']}';
    final name = '${r['name']}';
    final visible = r['visible'] != false;
    final locked = r['locked'] == true;
    final isGroup = kind == 'group';
    final isItem = kind == 'figure' || kind == 'picture' || kind == 'text' || kind == 'movieclip' || kind == 'adjustment';

    Widget item(String label, String? icon, VoidCallback onTap, {bool danger = false, double iconTurn = 0}) => kit.Tap(
          minSize: 0,
          onTap: onTap,
          child: Container(
            height: compact ? 31 : 38,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            child: Row(children: [
              if (icon != null) ...[
                iconTurn == 0
                    ? Pictogram(icon, size: compact ? 15.5 : 17, color: danger ? cDanger : cAccent)
                    : Transform.rotate(angle: iconTurn, child: Pictogram(icon, size: compact ? 15.5 : 17, color: danger ? cDanger : cAccent)),
                const SizedBox(width: 10),
              ],
              Expanded(child: Text(label, style: TextStyle(color: danger ? cDanger : cText, fontSize: compact ? 13.5 : 14, fontWeight: danger ? FontWeight.w600 : FontWeight.w500))),
            ]),
          ),
        );

    final pick = await showGeneralDialog<String>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Layer options',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 140),
      pageBuilder: (c, a1, a2) => Stack(children: [
        kit.Anchored(
          anchor: anchor,
          width: 224,
          topMin: topSafe,
          card: (w) => kit.PanelCard(
            width: w,
            padding: EdgeInsets.fromLTRB(6, compact ? 6 : 8, 6, compact ? 4 : 6),
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              Padding(
                padding: EdgeInsets.fromLTRB(10, 0, 10, compact ? 3 : 6),
                child: Text(name, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cTitle, fontSize: 13.5, fontWeight: FontWeight.w700)),
              ),
              if (isItem || isGroup) item('Rename…', 'edit', () => Navigator.pop(c, 'rename')),
              if (isItem) item('Duplicate', 'copy', () => Navigator.pop(c, 'dup')),
              if (isItem) item(locked ? 'Unlock layer' : 'Lock layer', locked ? 'unlock' : 'lock', () => Navigator.pop(c, 'lock')),
              if (isItem || kind == 'script' || isGroup) item(visible ? 'Hide layer' : 'Show layer', 'eye', () => Navigator.pop(c, 'vis')),
              if (isItem || isGroup) item('Bring forward', 'chevron', () => Navigator.pop(c, 'fwd'), iconTurn: math.pi),
              if (isItem || isGroup) item('Send back', 'chevron', () => Navigator.pop(c, 'back')),
              if (isGroup) item('Ungroup', 'folder', () => Navigator.pop(c, 'ungroup')),
              if (kind == 'camera') ...[
                item('Center camera', 'camera', () => Navigator.pop(c, 'cam_center')),
                item('Reset camera', 'undo', () => Navigator.pop(c, 'cam_reset')),
              ],
              if (kind != 'camera') ...[
                Container(height: 1, margin: EdgeInsets.symmetric(horizontal: 8, vertical: compact ? 2 : 4), color: const Color(0xFFE6E8EB)),
                item(isGroup ? 'Delete group' : 'Delete', 'bin', () => Navigator.pop(c, 'del'), danger: true),
              ],
            ]),
          ),
        ),
      ]),
    );
    if (pick == null || !mounted) return;
    switch (pick) {
      case 'rename':
        final n = await _askName('Rename', name);
        if (n != null && n.trim().isNotEmpty) {
          if (isGroup) {
            _cmd({'op': 'group_set', 'group': id, 'name': n.trim()});
          } else {
            _cmd({'op': 'rename', 'layer': id, 'name': n.trim()});
          }
        }
      case 'dup':
        _cmd({'op': 'duplicate_figure', 'layer': id});
      case 'lock':
        _cmd({'op': 'layer_lock', 'layer': id, 'v': !locked});
      case 'vis':
        _cmd({'op': 'layer_visible', 'layer': id, 'v': !visible});
      case 'fwd' || 'back':
        if (isGroup) {
          _cmd({'op': 'group_order', 'group': id, 'forward': pick == 'fwd'});
        } else {
          _cmd({'op': 'select', 'layer': id});
          _cmd({'op': 'layer_order', 'forward': pick == 'fwd'});
        }
      case 'ungroup':
        _cmd({'op': 'ungroup', 'group': id});
      case 'cam_center':
        _cmd({'op': 'camera', 'center': true});
      case 'cam_reset':
        _cmd({'op': 'reset_camera'});
      case 'del':
        if (isGroup) {
          _cmd({'op': 'delete_group', 'group': id});
          kit.showToast(context, '$name deleted.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
        } else if (kind == 'script') {
          _pickScript(id, null);
          _cmd({'op': 'script_delete'});
          kit.showToast(context, '$name deleted.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
        } else {
          _cmd({'op': 'delete_everywhere', 'layer': id});
          kit.showToast(context, '$name deleted from every frame.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
        }
    }
  }

  /// An empty timeline cell (or trailing runway cell) on a layer row: a tap
  /// selects the layer and moves the playhead there (within the animation);
  /// a hold (`menu`) also offers "Extend to frame N" or "New pose at frame N".
  Future<void> _onEmptyTimelineCellTap(Map r, int f, Offset globalPos, {required bool menu}) async {
    final kind = '${r['kind']}';
    final id = '${r['id']}';
    final name = '${r['name']}';
    if (kind == 'camera') {
      if (f < _frames) {
        _cmd({'op': 'frame', 'i': f});
        _setMode('camera');
      }
      if (!menu) return;
      final pick = await showGeneralDialog<String>(
        context: context,
        barrierDismissible: true,
        barrierLabel: 'Camera frame action',
        barrierColor: const Color(0x00000000),
        transitionDuration: kit.motion(context, 140),
        pageBuilder: (c, a1, a2) => Stack(children: [
          kit.Anchored(
            anchor: kit.Anchored.rectFromGlobal(context, globalPos),
            width: 248,
            topMin: 44 + MediaQuery.paddingOf(c).top,
            card: (w) => kit.PanelCard(
              width: w,
              padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                Text('Camera · Frame ${f + 1}', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cTitle, fontSize: 13.5, fontWeight: FontWeight.w700)),
                const SizedBox(height: 8),
                kit.WordButton('Add camera key', icon: 'key', primary: true, expand: true, height: 36, size: 13.5, onTap: () => Navigator.pop(c, 'pose')),
              ]),
            ),
          ),
        ]),
      );
      if (pick == 'pose' && mounted) {
        _cmd({'op': 'new_pose', 'layer': id, 'frame': f});
        _cmd({'op': 'frame', 'i': f.clamp(0, _frames - 1)});
        _thumbsChanged('new_pose', force: true);
      }
      return;
    }
    if (kind != 'figure' && kind != 'picture' && kind != 'movieclip' && kind != 'text' && kind != 'adjustment') return;

    final on = (r['on'] as List?) ?? const [];
    final blockList = ((r['blocks'] as List?) ?? const []).cast<List>();
    var hasVisibleSpan = false;
    for (final b in blockList) {
      final st = (b[0] as num).toInt(), len = (b[1] as num).toInt();
      var a = st, z = st + len - 1;
      while (a <= z && on.elementAtOrNull(a) == false) {
        a++;
      }
      while (z >= a && on.elementAtOrNull(z) == false) {
        z--;
      }
      if (a > z) continue;
      hasVisibleSpan = true;
      if (f >= a && f <= z) return; // Already inside a visible block!
    }
    if (!hasVisibleSpan && on.any((v) => v == true)) hasVisibleSpan = true;

    // Select the layer immediately and jump playhead if tapping within existing frames
    setState(() {
      _frameSelected = false;
      _soundPage = false;
      _selPoseKeys.clear();
      _selPropKeys.clear();
    });
    if (f < _frames && f != _frame) _cmd({'op': 'frame', 'i': f});
    _cmd({'op': 'select', 'layer': id});

    if (!menu) return;
    final canExtend = hasVisibleSpan;
    final isFigure = kind == 'figure';
    if (!canExtend && !isFigure) return;

    final pick = await showGeneralDialog<String>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Frame action',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 140),
      pageBuilder: (c, a1, a2) => Stack(children: [
        kit.Anchored(
          anchor: kit.Anchored.rectFromGlobal(context, globalPos),
          width: 264,
          topMin: 44 + MediaQuery.paddingOf(c).top,
          card: (w) => kit.PanelCard(
            width: w,
            padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              Text('$name · Frame ${f + 1}', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cTitle, fontSize: 13.5, fontWeight: FontWeight.w700)),
              const SizedBox(height: 8),
              if (canExtend) ...[
                kit.WordButton('Extend to frame ${f + 1}', icon: 'extend', primary: true, expand: true, height: 36, size: 13.5, onTap: () => Navigator.pop(c, 'extend')),
                const SizedBox(height: 6),
              ],
              if (isFigure) ...[
                kit.WordButton('New pose at frame ${f + 1}', icon: 'key', primary: !canExtend, filled: canExtend, expand: true, height: 36, size: 13.5, onTap: () => Navigator.pop(c, 'pose')),
                if (_st['pose'] == true) ...[
                  const SizedBox(height: 6),
                  kit.WordButton('Paste pose', icon: 'paste', filled: true, expand: true, height: 34, size: 13, onTap: () => Navigator.pop(c, 'paste')),
                ],
              ],
            ]),
          ),
        ),
      ]),
    );
    if (pick == null || !mounted) return;
    setState(() {
      _frameSelected = false;
      _selPoseKeys.clear();
      _selPropKeys.clear();
    });
    if (pick == 'extend') {
      _cmd({'op': 'extend_to', 'layer': id, 'frame': f});
      _cmd({'op': 'frame', 'i': f.clamp(0, _frames - 1)});
      _cmd({'op': 'select', 'layer': id});
      _thumbsChanged('extend_to', force: true);
      WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(_frame));
    } else if (pick == 'pose') {
      _cmd({'op': 'new_pose', 'layer': id, 'frame': f});
      _cmd({'op': 'frame', 'i': f.clamp(0, _frames - 1)});
      _cmd({'op': 'select', 'layer': id});
      _jumpTo('Pose');
      _thumbsChanged('new_pose', force: true);
      WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(_frame));
    } else if (pick == 'paste') {
      _cmd({'op': 'new_pose', 'layer': id, 'frame': f});
      _cmd({'op': 'paste_pose', 'layer': id, 'frame': f, 'withTransform': true});
      _cmd({'op': 'frame', 'i': f.clamp(0, _frames - 1)});
      _cmd({'op': 'select', 'layer': id});
      _thumbsChanged('paste_pose', force: true);
      WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(_frame));
    }
  }

  /// A layer row's poses as clear, tactile blocks: active pose highlighted in
  /// blue with generous capsule grips at its ends, subtle tween vs hold cue,
  /// and long-press-drag to slide poses freely without blocking 1-finger scroll!
  List<Widget> _rowBlocks(Map r, double y, double rowH, double cell, double left, bool selectedRow) {
    final on = (r['on'] as List?) ?? const [];
    final out = <Widget>[];
    if (r['kind'] == 'group' && r['open'] == false) {
      out.add(Positioned(
        left: left,
        top: y + 2,
        width: _frames * cell,
        height: rowH - 5,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: () {
            setState(() {
              _frameSelected = false;
              _soundPage = false;
              _selPoseKeys.clear();
              _selPropKeys.clear();
            });
            _cmd({'op': 'select', 'group': r['id']});
          },
          child: selectedRow ? Container(decoration: BoxDecoration(borderRadius: BorderRadius.circular(5), border: Border.all(color: cAccent, width: 2))) : const SizedBox.expand(),
        ),
      ));
      return out;
    }
    if (r['kind'] == 'group') {
      final ticks = ((r['ticks'] as List?) ?? const []).cast<num>();
      final w = _frames * cell;
      out.add(Positioned(
        left: left,
        top: y + rowH / 2 - 6,
        width: w,
        height: 12,
        child: GestureDetector(
          onTap: () {
            setState(() {
              _frameSelected = false;
              _soundPage = false;
              _selPoseKeys.clear();
              _selPropKeys.clear();
            });
            _cmd({'op': 'select', 'group': r['id']});
          },
          child: Container(
            decoration: BoxDecoration(
              color: selectedRow ? const Color(0xFFE8F0FA) : const Color(0xFFEEF0F3),
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: selectedRow ? cAccent : const Color(0xFFDCDFE4), width: selectedRow ? 2 : 1),
            ),
            child: Stack(children: [
              for (final f in ticks)
                if (f < _frames) Positioned(left: f * cell + cell / 2 - 0.75, top: 2, bottom: 2, width: 1.5, child: ColoredBox(color: selectedRow ? cAccent : const Color(0xFF9AA0A8))),
            ]),
          ),
        ),
      ));
      return out;
    }
    if (r['kind'] == 'script') {
      final picked = (_st['script'] as Map?)?['id'] == r['id'] ? (_st['script'] as Map)['run'] : null;
      final sdrag = _blockDrag != null && _blockDrag!.layer == r['id'] ? _blockDrag : null;
      for (final run in ((r['runs'] as List?) ?? const []).cast<List>()) {
        final a = (run[0] as num).toInt(), len = (run[1] as num).toInt(), rid = '${run[2]}';
        final trace = ((run[3] as List?) ?? const []).map((v) => (v as num?)?.toDouble()).toList();
        final sel = rid == picked;
        final dragged = sdrag != null && sdrag.run == rid;
        final x = left + a * cell + 1, w = math.max(10.0, len * cell - 2);
        Widget grip(bool atStart) => Positioned(
              left: atStart ? -4 : null,
              right: atStart ? null : -6,
              top: -3,
              bottom: -3,
              width: 20,
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onHorizontalDragStart: (d) {
                  _dragPointerGlobal = d.globalPosition;
                  _startTimelineDrag(_BlockDrag('${r['id']}', a, len, cell, edge: atStart ? 2 : 0, run: rid), rawStartX: d.globalPosition.dx);
                },
                onHorizontalDragUpdate: (d) {
                  _dragPointerGlobal = d.globalPosition;
                  _updateTimelineDragFromPointer(d.globalPosition);
                },
                onHorizontalDragEnd: (_) {
                  _dragPointerGlobal = null;
                  _endBlockDrag();
                },
                onHorizontalDragCancel: () {
                  _dragPointerGlobal = null;
                  _endBlockDrag(cancel: true);
                },
                child: Center(child: _gripPill()),
              ),
            );
        final block = AnimatedContainer(
          duration: kit.motion(context, 120),
          width: w,
          height: 26,
          decoration: BoxDecoration(
            color: sel ? const Color(0xFFE4EEFA) : const Color(0xFFEEF0F3),
            borderRadius: BorderRadius.circular(5),
            border: Border.all(color: sel ? cAccent : const Color(0xFFDCDFE4), width: sel ? 2 : 1),
          ),
          child: CustomPaint(painter: _TracePainter(trace, sel ? cAccent : const Color(0xFF8A9099), missing: r['missing'] == true || r['error'] != null)),
        );
        out.add(Positioned(
          left: x,
          top: y + (rowH - 26) / 2,
          child: _playing
              ? block
              : Opacity(
                  opacity: dragged ? 0 : 1,
                  child: GestureDetector(
                    onTapUp: (_) => _pickScript('${r['id']}', rid),
                    onLongPressStart: (d) {
                      _pickScript('${r['id']}', rid);
                      _dragPointerGlobal = d.globalPosition;
                      _startTimelineDrag(_BlockDrag('${r['id']}', a, len, cell, edge: 1, run: rid), rawStartX: d.globalPosition.dx);
                    },
                    onLongPressMoveUpdate: (d) {
                      _dragPointerGlobal = d.globalPosition;
                      _updateTimelineDragFromPointer(d.globalPosition);
                    },
                    onLongPressEnd: (_) {
                      _dragPointerGlobal = null;
                      _endBlockDrag();
                    },
                    child: SizedBox(width: w, height: 26, child: Stack(clipBehavior: Clip.none, children: [block, if (sel) grip(true), if (sel) grip(false)])),
                  ),
                ),
        ));
      }
      if (sdrag != null) {
        final (st, len) = sdrag.preview(allowStart0: true);
        out.add(Positioned(
          left: left + st * cell + 1,
          top: y + (rowH - 26) / 2,
          child: IgnorePointer(child: Container(width: math.max(10.0, len * cell - 2), height: 26, decoration: BoxDecoration(color: const Color(0x4DE4EEFA), borderRadius: BorderRadius.circular(5), border: Border.all(color: cAccent, width: 2)))),
        ));
      }
      return out;
    }
    if (r['kind'] == 'sound') {
      for (final c in ((r['clips'] as List?) ?? const []).cast<List>()) {
        final x = left + (c[0] as num).toDouble() * cell + 1;
        final w = math.max(8.0, math.min((c[1] as num).toDouble() * cell, (_frames - (c[0] as num).toDouble()) * cell) - 2);
        final n = math.max(1, ((w - 12) / 4).floor());
        final key = '${c[2]}@${c[3]}@${c[4]}@$n';
        final peaks = _peaks[key] ??= _engine?.soundPeaks('${c[2]}', (c[3] as num).toDouble(), (c[4] as num).toDouble(), n) ?? const <double>[];
        out.add(Positioned(
          left: x,
          top: y + (rowH - 26) / 2,
          child: GestureDetector(
            onTap: _playing ? null : () => _pickSound(frame: (c[0] as num).floor()),
            child: Container(
              width: w,
              height: 26,
              decoration: BoxDecoration(color: const Color(0xFFE9F2F7), borderRadius: BorderRadius.circular(5), border: Border.all(color: const Color(0xFFC9DDE8))),
              child: CustomPaint(painter: _WavePeaks(peaks)),
            ),
          ),
        ));
      }
      return out;
    }

    final drag = _blockDrag != null && _blockDrag!.layer == r['id'] && _blockDrag!.kind != 'prop_key' && _blockDrag!.kind != 'multi_shift' && _blockDrag!.kind != 'multi_scale' ? _blockDrag : null;
    final layerId = '${r['id']}';
    final isAdj = r['kind'] == 'adjustment';
    Widget bar(double w, {bool strong = false, bool boxSel = false, bool ghost = false, bool tween = false, int cells = 1}) => AnimatedContainer(
          duration: _blockDrag != null ? Duration.zero : kit.motion(context, 120),
          width: w,
          height: 26,
          decoration: BoxDecoration(
            color: boxSel
                ? const Color(0xFFD6E8FC)
                : strong
                    ? const Color(0xFFE4EEFA)
                    : isAdj
                        ? const Color(0xFFEEEAFC)
                        : (tween ? const Color(0xFFEDF2F8) : const Color(0xFFF1F3F5)),
            borderRadius: BorderRadius.circular(6),
            border: Border.all(
              color: strong || boxSel
                  ? cAccent
                  : isAdj
                      ? const Color(0xFFB3A2E8)
                      : (tween ? const Color(0xFFC7D7EC) : const Color(0xFFD8DCE2)),
              width: strong || boxSel ? 2 : 1,
            ),
          ),
          foregroundDecoration: ghost ? BoxDecoration(color: cCard.withValues(alpha: 0.3), borderRadius: BorderRadius.circular(6)) : null,
          child: cells > 1
              ? Align(
                  alignment: Alignment.bottomCenter,
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(8, 0, 8, 2.5),
                    child: CustomPaint(size: Size(math.max(0, w - 16), 3), painter: _BlockSpanCuePainter(tween: tween, active: strong || boxSel)),
                  ),
                )
              : null,
        );

    final blockList = ((r['blocks'] as List?) ?? const []).cast<List>();
    for (var bi = 0; bi < blockList.length; bi++) {
      final b = blockList[bi];
      final st = (b[0] as num).toInt(), len = (b[1] as num).toInt();
      final prevStart = bi > 0 ? (blockList[bi - 1][0] as num).toInt() : 0;
      var a = st, z = st + len - 1;
      while (a <= z && on.elementAtOrNull(a) == false) {
        a++;
      }
      while (z >= a && on.elementAtOrNull(z) == false) {
        z--;
      }
      if (a > z) continue;
      final visLen = z - a + 1;
      final dragged = drag != null && !drag.live && drag.start == a;
      final strong = selectedRow && _frame >= a && _frame <= z;
      final boxSel = _selPoseKeys.contains((layerId, a)) || _selPoseKeys.contains((layerId, st));
      final x = left + a * cell + 1, w = math.max(10.0, visLen * cell - 2);

      void select(double dx) {
        setState(() {
          _frameSelected = false;
          _soundPage = false;
          _selPoseKeys.clear();
          _selPropKeys.clear();
        });
        _cmd({'op': 'frame', 'i': (a + dx ~/ cell).clamp(0, _frames - 1)});
        if (r['kind'] == 'camera') {
          _setMode('camera');
          return;
        }
        if (r['kind'] == 'picture' || r['kind'] == 'movieclip' || r['kind'] == 'text' || r['kind'] == 'adjustment') {
          _cmd({'op': 'select', 'layer': r['id']});
          return;
        }
        if (r['kind'] == 'figure') {
          _cmd({'op': 'select', 'layer': r['id']});
          _jumpTo('Pose');
        }
      }

      final figure = r['kind'] == 'figure';
      final isCam = r['kind'] == 'camera';
      final extensible = figure || isCam || r['kind'] == 'picture' || r['kind'] == 'movieclip' || r['kind'] == 'text' || r['kind'] == 'adjustment';
      final multiSelectedHere = boxSel && (_selPoseKeys.length + _selPropKeys.length) >= 2;
      // a grip being dragged stays put even when the playhead leaves its pose
      final gripDrag = drag != null && drag.block == bi && (drag.kind == 'block_start' || drag.kind == 'block_end');
      final hasLeftGrip = !_playing && extensible && ((strong && a > 0) || (gripDrag && drag.kind == 'block_start'));
      final hasRightGrip = !_playing && extensible && (strong || (gripDrag && drag.kind == 'block_end'));
      // grips are big touch targets: 16 px past the block's end, up to 16 px
      // into it, and 9 px above and below the bar
      const gripOut = 16.0, gripPadY = 9.0;
      final gripIn = math.min(16.0, w * (hasLeftGrip && hasRightGrip ? 0.45 : 0.6));

      void tapBlock(Offset globalPos, double localDx) {
        if (_boxActive || _boxPath != null || DateTime.now().difference(_boxEndedAt) < const Duration(milliseconds: 80)) return;
        select(localDx);
        if (figure || isCam) {
          setState(() {
            _selPoseKeys
              ..clear()
              ..add((layerId, st));
            _selPropKeys.clear();
          });
        }
      }

      void openBlockMenu(Offset globalPos) {
        if (_boxActive || _boxPath != null) return;
        final rect = kit.Anchored.rectFromGlobal(context, globalPos);
        final totalSel = _selPoseKeys.length + _selPropKeys.length;
        if (totalSel > 1 && boxSel) {
          _multiKeyPopover(rect);
        } else if (figure) {
          _poseKeyPopover(r, b, bi, rect);
        } else if (r['kind'] == 'camera') {
          _cameraKeyPopover(r, b, bi, rect);
        }
      }

      // in track coordinates, not inside the bar: a touch outside a widget's
      // own box never reaches it, so a grip inside the bar lost its outer half
      Widget grip(bool atStart) => Positioned(
            key: ValueKey('grip:$layerId:$bi:${atStart ? 'start' : 'end'}'),
            left: atStart ? x - gripOut : x + w - gripIn,
            top: y + (rowH - 26) / 2 - gripPadY,
            width: gripOut + gripIn,
            height: 26 + gripPadY * 2,
            child: Listener(
              onPointerDown: (e) => _gripDownX = e.position.dx,
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                dragStartBehavior: DragStartBehavior.down,
                onTapUp: (d) => tapBlock(d.globalPosition, atStart ? 0 : math.max(0, w - 1)),
                onHorizontalDragStart: (d) {
                  final startX = _gripDownX ?? d.globalPosition.dx;
                  _dragPointerGlobal = d.globalPosition;
                  _startTimelineDrag(
                    _BlockDrag(
                      layerId,
                      a,
                      visLen,
                      cell,
                      edge: atStart ? 2 : 0,
                      kind: atStart ? 'block_start' : 'block_end',
                      block: bi,
                      minStart: atStart ? math.max(1, prevStart + 1) : null,
                      maxStart: atStart ? a + visLen - 1 : null,
                    ),
                    rawStartX: startX,
                  );
                  _updateTimelineDragFromPointer(d.globalPosition);
                },
                onHorizontalDragUpdate: (d) {
                  _dragPointerGlobal = d.globalPosition;
                  _updateTimelineDragFromPointer(d.globalPosition);
                },
                onHorizontalDragEnd: (_) {
                  _dragPointerGlobal = null;
                  _endBlockDrag();
                },
                onHorizontalDragCancel: () {
                  _dragPointerGlobal = null;
                  _endBlockDrag(cancel: true);
                },
                child: MouseRegion(
                  cursor: SystemMouseCursors.resizeLeftRight,
                  child: Align(
                    alignment: atStart ? Alignment.centerLeft : Alignment.centerRight,
                    child: Padding(
                      padding: EdgeInsets.only(left: atStart ? gripOut - 3 : 0, right: atStart ? 0 : gripOut - 3),
                      child: _gripPill(),
                    ),
                  ),
                ),
              ),
            ),
          );

      final tween = !(b[2] == true) && bi + 1 < blockList.length;
      final chain = r['joinedTo'] != null && a == on.indexWhere((v) => v != false);
      final nudge = chain ? 9.0 : 0.0;
      final pics = <Widget>[
        for (var f = a; f <= (tween ? z : a); f++)
          if (r['kind'] == 'camera')
            Positioned(left: (f - a) * cell + (cell - 16) / 2 - 1, top: 5, child: const IgnorePointer(child: Pictogram('camera', size: 16, color: Color(0xFF8A9099))))
          else if (_rowPics['${r['id']}@$f'] case final img?)
            Positioned(left: (f - a) * cell + (cell - 22) / 2 - 1 + (f == a ? nudge : 0), top: 2, width: 22, height: 22, child: IgnorePointer(child: RawImage(image: img, fit: BoxFit.contain, filterQuality: FilterQuality.medium))),
        if (chain)
          Positioned(
            left: 3,
            top: 6,
            child: kit.Tap(
              label: 'Joined to ${r['joinedTo']}',
              minSize: 14,
              onTap: () {
                _cmd({'op': 'select', 'layer': r['id']});
                setState(() => _panelOpen = true);
                _jumpTo('Stickfigure Tools');
                _say('${r['name']} is attached to ${r['joinedTo']}.', light: true);
              },
              child: const Pictogram('chain', size: 13, color: Color(0xFF5D636B), stroke: 2.2),
            ),
          ),
      ];
      out.add(Positioned(
        key: ValueKey('pose:$layerId:$bi'),
        left: x,
        top: y + (rowH - 26) / 2,
        child: _playing
            ? Stack(children: [bar(w, tween: tween, cells: visLen), ...pics])
            : Opacity(
                opacity: dragged ? 0 : 1,
                child: GestureDetector(
                  onTapUp: (d) => tapBlock(d.globalPosition, d.localPosition.dx),
                  onLongPressStart: figure || isCam || multiSelectedHere
                      ? (d) {
                          if (_boxActive || _boxPath != null) return;
                          if (AppSettings.flag('haptics')) unawaited(HapticFeedback.selectionClick());
                          openBlockMenu(d.globalPosition);
                        }
                      : null,
                  onSecondaryTapUp: figure || isCam || multiSelectedHere ? (d) => openBlockMenu(d.globalPosition) : null,
                  onHorizontalDragStart: multiSelectedHere
                      ? (d) {
                          _dragPointerGlobal = d.globalPosition;
                          _startMultiKeyDrag(kind: 'multi_shift', cell: cell, rawStartX: d.globalPosition.dx);
                        }
                      : (isCam && st > 0)
                          ? (d) {
                              _dragPointerGlobal = d.globalPosition;
                              _startTimelineDrag(_BlockDrag(layerId, a, visLen, cell, edge: 1, kind: 'block_start', block: bi, minStart: math.max(1, prevStart + 1)), rawStartX: d.globalPosition.dx);
                            }
                          : null,
                  onHorizontalDragUpdate: (multiSelectedHere || (isCam && st > 0))
                      ? (d) {
                          _dragPointerGlobal = d.globalPosition;
                          _updateTimelineDragFromPointer(d.globalPosition);
                        }
                      : null,
                  onHorizontalDragEnd: (multiSelectedHere || (isCam && st > 0))
                      ? (_) {
                          _dragPointerGlobal = null;
                          _endBlockDrag();
                        }
                      : null,
                  onHorizontalDragCancel: (multiSelectedHere || (isCam && st > 0))
                      ? () {
                          _dragPointerGlobal = null;
                          _endBlockDrag(cancel: true);
                        }
                      : null,
                  child: SizedBox(
                    width: w,
                    height: 26,
                    child: Stack(clipBehavior: Clip.none, children: [
                      bar(w, strong: strong, boxSel: boxSel, tween: tween, cells: visLen),
                      ...pics,
                    ]),
                  ),
                ),
              ),
      ));
      if (hasLeftGrip) out.add(grip(true));
      if (hasRightGrip) out.add(grip(false));
    }
    // I2: joins over time: a grey chain where it joins, a red broken chain
    // where it lets go (round 10 board); tap one for what it is
    for (final j in ((r['joins'] as List?) ?? const []).cast<List>()) {
      final f = (j[0] as num).toInt();
      final to = j[1] as String?, from = j[2] as String?;
      final key = GlobalKey();
      out.add(Positioned(
        key: key,
        left: f == 0 ? left + 2 : left + f * cell - 9,
        top: y + 7,
        child: kit.Tap(
          label: to != null ? 'Joins $to at frame ${f + 1}' : 'Lets go of ${from ?? 'its figure'} at frame ${f + 1}',
          minSize: 18,
          onTap: _playing ? null : () => _joinPopover(r, f, to, from, key),
          child: Container(
            padding: const EdgeInsets.all(2),
            decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(4)),
            child: Pictogram(to != null ? 'chain' : 'broken', size: 13, color: to != null ? const Color(0xFF5D636B) : cDanger, stroke: 2.2),
          ),
        ),
      ));
    }
    if (drag != null && !drag.live) {
      final (st, len) = drag.preview();
      out.add(Positioned(left: left + st * cell + 1, top: y + (rowH - 26) / 2, child: IgnorePointer(child: bar(len * cell - 2, strong: true, ghost: true, cells: len))));
    }
    return out;
  }

  Widget _gripPill() => Container(
        width: 8,
        height: 20,
        decoration: BoxDecoration(
          color: cAccent,
          borderRadius: BorderRadius.circular(3.5),
          boxShadow: const [BoxShadow(color: Color(0x26000000), blurRadius: 2, offset: Offset(0, 1))],
        ),
        alignment: Alignment.center,
        child: Container(width: 1.5, height: 8, decoration: BoxDecoration(color: cCard.withValues(alpha: 0.9), borderRadius: BorderRadius.circular(1))),
      );

  /// User list #4 & #5: a selection box or lasso let go genuinely selects the
  /// pose keys, property keys, and layers it covers (accounting for both
  /// horizontal `_stripScroll` and vertical `_layerVScroll`!).
  void _finishBox(List<Map> rows, List<double> ys, List<(Map, Map, double)> props, double cell, double rowH, double propH, {double headerH = 0}) {
    final path = _boxPath;
    _boxActive = false;
    if (path == null) return;
    _boxEndedAt = DateTime.now();
    setState(() => _boxPath = null);
    final off = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
    final vOff = _layerVScroll.hasClients ? _layerVScroll.offset : 0.0;
    final hitLayers = <String>[];
    final hitFigures = <String>[];
    final hitPoses = <(String, int)>{};
    final hitProps = <(String, int)>{};
    final rect = Rect.fromPoints(path.first, path.last);
    final point = !_lasso && rect.width < 6 && rect.height < 6;
    bool inLasso(Offset p) {
      var inside = false;
      for (var a = 0, b = path.length - 1; a < path.length; b = a++) {
        final (pa, pb) = (path[a], path[b]);
        if ((pa.dy > p.dy) != (pb.dy > p.dy) && p.dx < (pb.dx - pa.dx) * (p.dy - pa.dy) / (pb.dy - pa.dy) + pa.dx) inside = !inside;
      }
      return inside;
    }

    for (var k = 0; k < rows.length; k++) {
      final r = rows[k];
      final kind = '${r['kind']}';
      if (kind != 'figure' && kind != 'picture' && kind != 'text' && kind != 'movieclip' && kind != 'adjustment') continue;
      final id = '${r['id']}';
      final y0 = headerH + ys[k] - vOff, cy = y0 + rowH / 2;
      final on = (r['on'] as List?) ?? const [];
      final blocks = ((r['blocks'] as List?) ?? const []).cast<List>();
      if (point) {
        if (path.first.dy >= y0 && path.first.dy < y0 + rowH) {
          final f = ((path.first.dx + off - 8) / cell).floor();
          for (final b in blocks) {
            final st = (b[0] as num).toInt(), len = (b[1] as num).toInt();
            var a = st, z = st + len - 1;
            while (a <= z && on.elementAtOrNull(a) == false) {
              a++;
            }
            while (z >= a && on.elementAtOrNull(z) == false) {
              z--;
            }
            if (a <= z && f >= a && f <= z) hitPoses.add((id, a));
          }
          hitLayers.add(id);
          if (kind == 'figure') hitFigures.add(id);
        }
        continue;
      }
      var rowHit = false;
      for (final b in blocks) {
        final st = (b[0] as num).toInt(), len = (b[1] as num).toInt();
        var a = st, z = st + len - 1;
        while (a <= z && on.elementAtOrNull(a) == false) {
          a++;
        }
        while (z >= a && on.elementAtOrNull(z) == false) {
          z--;
        }
        if (a > z) continue;
        final bx0 = 8 + a * cell - off, bx1 = 8 + (z + 1) * cell - off;
        final blockRect = Rect.fromLTRB(bx0, y0 + 3, bx1, y0 + rowH - 3);
        final hit = _lasso
            ? inLasso(Offset(bx0 + cell / 2, cy)) || inLasso(blockRect.center)
            : rect.overlaps(blockRect);
        if (hit) {
          rowHit = true;
          hitPoses.add((id, a));
        }
      }
      if (rowHit) {
        hitLayers.add(id);
        if (kind == 'figure') hitFigures.add(id);
      }
    }

    for (final (_, pr, y) in props) {
      final pid = '${pr['id']}';
      final cy = headerH + y - vOff + propH / 2;
      for (final fn in ((pr['keys'] as List?) ?? const []).cast<num>()) {
        final f = fn.toInt();
        final c = Offset(8 + f * cell + cell / 2 - off, cy);
        final hit = point
            ? (c - path.first).distance < 16
            : (_lasso ? inLasso(c) : rect.inflate(8).contains(c));
        if (hit) hitProps.add((pid, f));
      }
    }

    setState(() {
      _selPoseKeys
        ..clear()
        ..addAll(hitPoses);
      _selPropKeys
        ..clear()
        ..addAll(hitProps);
      if (hitPoses.isNotEmpty || hitProps.isNotEmpty || hitLayers.isNotEmpty) _frameSelected = false;
    });

    if (hitFigures.length > 1) {
      _cmd({'op': 'select_many', 'layers': hitFigures});
    } else if (hitLayers.length == 1) {
      _cmd({'op': 'select', 'layer': hitLayers.first});
    }
  }

  /// A chain marker tapped (I2): what it is, and Let go / Join elsewhere (a
  /// join) or Join again (a let-go). Each change is one step with an Undo toast.
  Future<void> _joinPopover(Map r, int f, String? to, String? from, GlobalKey key) async {
    final anchor = kit.Anchored.rectOf(key) ?? const Rect.fromLTWH(200, 300, 18, 18);
    final name = '${r['name']}';
    final pick = await showGeneralDialog<int>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Join',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => Stack(children: [
        kit.Anchored(
          anchor: anchor,
          width: 280,
          topMin: 44 + MediaQuery.paddingOf(c).top,
          card: (w) => kit.PanelCard(
            width: w,
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              Text(to != null ? '$name attached to $to at frame ${f + 1}' : '$name detached from ${from ?? 'figure'} at frame ${f + 1}', style: const TextStyle(color: cText, fontSize: 13.5, fontWeight: FontWeight.w500)),
              const SizedBox(height: 8),
              if (to != null)
                kit.ButtonPair('Detach', () => Navigator.pop(c, 1), 'Reattach', () => Navigator.pop(c, 2), false, 'broken', 'attach')
              else
                kit.ButtonPair('Reattach', from == null ? null : () => Navigator.pop(c, 3), 'Close', () => Navigator.pop(c), false, 'attach'),
            ]),
          ),
        ),
      ]),
    );
    if (pick == null || !mounted) return;
    _cmd({'op': 'select', 'layer': r['id']});
    if (pick == 1) {
      // from the playhead on (not before the join itself)
      final at = math.max(_frame, f);
      _cmd({'op': 'frame', 'i': at});
      _cmd({'op': 'select', 'layer': r['id']});
      _cmd({'op': 'unjoin'});
      if (!mounted) return;
      kit.showToast(context, 'Detached $name from $to at frame ${at + 1}', light: true, icon: 'broken', iconColor: cDanger, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
    } else if (pick == 2) {
      _cmd({'op': 'frame', 'i': f});
      _cmd({'op': 'select', 'layer': r['id']});
      _cmd({'op': 'join'});
    } else if (pick == 3) {
      final target = ((_st['rows'] as List?) ?? const []).cast<Map>().where((x) => x['name'] == from).firstOrNull;
      if (target == null) return;
      _cmd({'op': 'frame', 'i': f});
      _cmd({'op': 'select', 'layer': r['id']});
      _cmd({'op': 'join_here', 'layer': target['id']});
      if (!mounted) return;
      kit.showToast(context, 'Reattached $name to $from at frame ${f + 1}', light: true, icon: 'chain', action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
    }
  }

  /// Scrubbing: the playhead follows the finger along the ruler (a command
  /// only when the frame changes).
  void _scrubTo(int i) {
    if (i != _frame) _cmd({'op': 'frame', 'i': i});
  }

  /// Renders the multi-keyframe selection bounding frame + left/right
  /// proportional stretch handles (`multi_scale`) over `bodyTrack`.
  List<Widget> _multiKeyRangeOverlay(List<Map> rows, List<double> ys, List<(Map, Map, double)> props, double cell, double left, double rowH, double propH) {
    final frames = <int>[for (final (_, f) in _selPoseKeys) f, for (final (_, f) in _selPropKeys) f];
    if (frames.length < 2) return const [];
    final minF = frames.reduce(math.min);
    final maxF = frames.reduce(math.max);
    double? topY, botY;
    for (var k = 0; k < rows.length; k++) {
      final id = '${rows[k]['id']}';
      if (_selPoseKeys.any((e) => e.$1 == id)) {
        topY = topY == null ? ys[k] : math.min(topY, ys[k]);
        botY = botY == null ? ys[k] + rowH : math.max(botY, ys[k] + rowH);
      }
    }
    for (final (_, pr, y) in props) {
      final pid = '${pr['id']}';
      if (_selPropKeys.any((e) => e.$1 == pid)) {
        topY = topY == null ? y : math.min(topY, y);
        botY = botY == null ? y + propH : math.max(botY, y + propH);
      }
    }
    if (topY == null || botY == null) return const [];
    final x0 = left + minF * cell;
    final x1 = left + (maxF + 1) * cell;
    final h = math.max(24.0, botY - topY);
    final span = maxF - minF;

    Widget stretchHandle({required bool fromLeft}) => Positioned(
          left: fromLeft ? x0 - 11 : x1 - 11,
          top: topY!,
          width: 22,
          height: h,
          child: MouseRegion(
            cursor: SystemMouseCursors.resizeLeftRight,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              dragStartBehavior: DragStartBehavior.down,
              onHorizontalDragStart: (d) {
                _dragPointerGlobal = d.globalPosition;
                _startMultiKeyDrag(
                  kind: 'multi_scale',
                  cell: cell,
                  rawStartX: d.globalPosition.dx,
                  anchor: fromLeft ? maxF : minF,
                  baseSpan: span,
                  scaleFromLeft: fromLeft,
                );
              },
              onHorizontalDragUpdate: (d) {
                _dragPointerGlobal = d.globalPosition;
                _updateTimelineDragFromPointer(d.globalPosition);
              },
              onHorizontalDragEnd: (_) {
                _dragPointerGlobal = null;
                _endBlockDrag();
              },
              onHorizontalDragCancel: () {
                _dragPointerGlobal = null;
                _endBlockDrag(cancel: true);
              },
              child: Center(
                child: Container(
                  width: 8,
                  height: math.min(28.0, math.max(16.0, h - 8)),
                  decoration: BoxDecoration(
                    color: cAccent,
                    borderRadius: BorderRadius.circular(4),
                    border: Border.all(color: cCard, width: 1.2),
                    boxShadow: const [BoxShadow(color: Color(0x28000000), blurRadius: 3, offset: Offset(0, 1))],
                  ),
                ),
              ),
            ),
          ),
        );

    return [
      Positioned(
        left: x0,
        top: topY,
        width: math.max(cell, x1 - x0),
        height: h,
        child: IgnorePointer(
          child: Container(
            decoration: BoxDecoration(
              color: cAccent.withValues(alpha: 0.06),
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: cAccent.withValues(alpha: 0.65), width: 1.5),
            ),
          ),
        ),
      ),
      if (span > 0) ...[
        stretchHandle(fromLeft: true),
        stretchHandle(fromLeft: false),
      ],
    ];
  }

  /// Syncs highlighted multi-key sets strictly from engine-returned `movedPoses`/`movedProps`.
  void _applyMovedKeySelection(Map<String, dynamic>? st) {
    if (st == null) return;
    final mp = st['movedPoses'] as List?;
    final mpr = st['movedProps'] as List?;
    if (mp == null && mpr == null) return;
    setState(() {
      if (mp != null) {
        _selPoseKeys
          ..clear()
          ..addAll([for (final e in mp.cast<List>()) ('${e[0]}', (e[1] as num).toInt())]);
      }
      if (mpr != null) {
        _selPropKeys
          ..clear()
          ..addAll([for (final e in mpr.cast<List>()) ('${e[0]}', (e[1] as num).toInt())]);
      }
    });
  }

  /// Starts a live multi-keyframe unison shift (`multi_shift`) or proportional stretch (`multi_scale`).
  void _startMultiKeyDrag({
    required String kind,
    required double cell,
    required double rawStartX,
    int? anchor,
    int baseSpan = 1,
    bool scaleFromLeft = false,
  }) {
    final poses = [for (final (l, f) in _selPoseKeys) [l, f]];
    final props = [for (final (p, f) in _selPropKeys) [p, f]];
    final allF = <int>[for (final (_, f) in _selPoseKeys) f, for (final (_, f) in _selPropKeys) f];
    final minF = allF.isEmpty ? 0 : allF.reduce(math.min);
    final maxF = allF.isEmpty ? 0 : allF.reduce(math.max);
    final d = _BlockDrag(
      '',
      minF,
      math.max(1, maxF - minF + 1),
      cell,
      edge: 1,
      kind: kind,
      multiPoses: Set<(String, int)>.of(_selPoseKeys),
      multiProps: Set<(String, int)>.of(_selPropKeys),
      scaleAnchor: anchor,
      scaleBaseSpan: math.max(1, baseSpan),
      scaleFromLeft: scaleFromLeft,
    );
    d.rawStartX = rawStartX;
    d.startScrollX = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
    d.live = true;
    d.lastSent = 0;
    d.lastFactor = 1.0;
    _cmd({
      'op': 'timeline_drag_start',
      'kind': kind,
      'poses': poses,
      'props': props,
      'anchor': ?anchor,
    });
    setState(() => _blockDrag = d);
  }

  /// Starts a live timeline drag (grips, pose key, or property key).
  void _startTimelineDrag(_BlockDrag d, {double? rawStartX}) {
    d.rawStartX = rawStartX;
    d.startScrollX = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
    if (d.run == null && d.kind != 'block_move') {
      d.live = true;
      d.lastSent = d.kind == 'block_end' ? d.length : d.start;
      _cmd({
        'op': 'timeline_drag_start',
        'kind': d.kind,
        'layer': d.layer,
        'frame': d.start,
        if (d.property != null) 'property': d.property,
      });
    }
    setState(() => _blockDrag = d);
  }

  /// Updates an active timeline drag from a global pointer position, including
  /// any horizontal auto-edge-scroll distance accumulated since drag start!
  void _updateTimelineDragFromPointer(Offset globalPos) {
    final d = _blockDrag;
    if (d == null || d.rawStartX == null) return;
    final curScrollX = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
    final totalDx = (globalPos.dx - d.rawStartX!) + (curScrollX - d.startScrollX);
    _updateTimelineDrag(totalDx, absolute: true);
  }

  /// Updates an active timeline drag; sends a live preview op only when the
  /// target integer frame, length, or scale factor actually changes (zero undo spam).
  void _updateTimelineDrag(double delta, {bool absolute = false}) {
    final d = _blockDrag;
    if (d == null) return;
    d.dx = absolute ? delta : d.dx + delta;
    if (d.live) {
      if (d.kind == 'multi_shift') {
        final by = (d.dx / d.cell).round();
        if (by != d.lastSent) {
          d.lastSent = by;
          final st = _cmd({'op': 'timeline_drag_move', 'by': by});
          _applyMovedKeySelection(st);
        }
      } else if (d.kind == 'multi_scale') {
        final deltaFrames = d.dx / d.cell;
        final rawSpan = d.scaleFromLeft ? (d.scaleBaseSpan - deltaFrames) : (d.scaleBaseSpan + deltaFrames);
        final factor = ((rawSpan / d.scaleBaseSpan).clamp(0.15, 8.0) * 20).round() / 20.0;
        if (d.lastFactor == null || (factor - d.lastFactor!).abs() >= 0.025) {
          d.lastFactor = factor;
          final st = _cmd({'op': 'timeline_drag_move', 'factor': factor});
          _applyMovedKeySelection(st);
        }
      } else {
        final (st, len) = d.preview(allowStart0: d.kind == 'prop_key');
        final target = d.kind == 'block_end' ? len : st;
        if (target != d.lastSent) {
          d.lastSent = target;
          _cmd(d.kind == 'block_end' ? {'op': 'timeline_drag_move', 'frames': len} : {'op': 'timeline_drag_move', 'to': st});
        }
      }
    }
    setState(() {});
  }

  /// Every finger is off the timeline but a drag is still on: the widget that
  /// started it was rebuilt mid-drag, so its end never came, and the timeline
  /// stayed locked (no scrolling). Finish it once this pointer event is done:
  /// a drag that ended normally has cleared `_blockDrag` by then.
  void _endOrphanedDrag({bool cancel = false}) {
    if (_blockDrag == null) return;
    scheduleMicrotask(() {
      if (!mounted || _blockDrag == null || _tlPinch.isNotEmpty) return;
      _endBlockDrag(cancel: cancel);
    });
  }

  /// A grip or held block/key let go: commits the live drag (1 undo step) or cancels it.
  void _endBlockDrag({bool cancel = false}) {
    final d = _blockDrag;
    if (d == null) return;
    _dragPointerGlobal = null;
    setState(() {
      _blockDrag = null;
      _dragRunwayFrames = 0;
    });
    // K2c: a script's run (frames from, to; a move keeps its length)
    if (d.run != null) {
      if (cancel) return;
      final (st, len) = d.preview(allowStart0: true);
      if (d.edge == 1) {
        if (st != d.start) _cmd({'op': 'script_run', 'layer': d.layer, 'run': d.run, 'from': st, 'move': true});
      } else if (st != d.start || len != d.length) {
        _cmd({'op': 'script_run', 'layer': d.layer, 'run': d.run, 'from': st, 'to': st + len});
      }
      return;
    }
    if (d.live) {
      if (cancel) {
        _cmd({'op': 'cancel'});
        if (d.multiPoses != null && d.multiProps != null) {
          setState(() {
            _selPoseKeys
              ..clear()
              ..addAll(d.multiPoses!);
            _selPropKeys
              ..clear()
              ..addAll(d.multiProps!);
          });
        }
      } else {
        _cmd({'op': 'up'});
        _thumbsChanged('timeline_commit', force: true);
      }
      return;
    }
    if (cancel) return;
    final (st, len) = d.preview();
    _cmd({'op': 'frame', 'i': d.start});
    if (d.edge == 0) {
      if (len != d.length) _cmd({'op': 'block_length', 'layer': d.layer, 'frames': len});
    } else if (st != d.start) {
      _cmd({'op': 'move_block', 'layer': d.layer, 'to': st});
    }
  }

  /// Anchored popover for a Camera keyframe (Copy, Paste, Duplicate, Ease, Shift, Delete).
  Future<void> _cameraKeyPopover(Map r, List b, int blockIndex, Rect anchor) async {
    final layerId = '${r['id']}';
    final st = (b[0] as num).toInt();
    final isHold = b[2] == true;
    final blockEases = (r['blockEases'] as List?) ?? const [];
    final rawEase = blockIndex < blockEases.length ? blockEases[blockIndex] : (b.length > 3 ? b[3] : null);
    var ease = rawEase is String ? rawEase : (isHold ? 'hold' : 'auto');
    const eases = ['auto', 'linear', 'out', 'smooth', 'bounce'];
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Camera keyframe',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        final hold = ease == 'hold';
        void setEase(String v) {
          _cmd({'op': 'easing', 'layer': layerId, 'frame': st, 'v': v});
          set(() => ease = v);
        }
        return Stack(children: [
          kit.Anchored(
            anchor: anchor,
            width: 300,
            topMin: 44 + MediaQuery.paddingOf(c).top,
            card: (w) => kit.PanelCard(
              width: w,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                rw.renderDesc(rw.DTitle('Camera · Frame ${st + 1}')),
                rw.renderDesc(rw.DSeg(const ['Hold', 'Tween'], hold ? 0 : 1, onSelect: (i) => setEase(i == 0 ? 'hold' : 'auto'))),
                rw.renderDesc(rw.DEase(math.max(0, eases.indexOf(ease == 'in' ? 'linear' : ease)), enabled: !hold, onSelect: (i) => setEase(eases[i]))),
                rw.renderDesc(rw.DButtons(
                  'Copy',
                  () {
                    _cmd({'op': 'copy_pose', 'layer': layerId, 'frame': st, 'withTransform': true});
                    Navigator.pop(c);
                  },
                  iconA: 'copy',
                  b: 'Paste',
                  onB: _st['cameraPose'] == true
                      ? () {
                          _cmd({'op': 'paste_pose', 'layer': layerId, 'frame': st, 'withTransform': true});
                          Navigator.pop(c);
                        }
                      : null,
                  iconB: 'paste',
                )),
                if (st > 0)
                  rw.renderDesc(rw.DButtons(
                    'Earlier',
                    () {
                      _cmd({'op': 'shift_keys', 'poses': [[layerId, st]], 'props': const [], 'by': -1, 'overwrite': true});
                      Navigator.pop(c);
                    },
                    iconA: 'nudgeLeft',
                    b: 'Later',
                    onB: () {
                      _cmd({'op': 'shift_keys', 'poses': [[layerId, st]], 'props': const [], 'by': 1, 'overwrite': true});
                      Navigator.pop(c);
                    },
                    iconB: 'nudgeRight',
                  )),
                kit.ButtonPair(
                  'Duplicate',
                  () {
                    _cmd({'op': 'duplicate_pose_key', 'layer': layerId, 'from': st});
                    Navigator.pop(c);
                  },
                  null,
                  null,
                  false,
                  'duplicate',
                ),
                if (st > 0)
                  rw.renderDesc(rw.DDanger('Delete keyframe', () {
                    _cmd({'op': 'delete_pose', 'layer': layerId, 'frame': st});
                    Navigator.pop(c);
                  })),
              ]),
            ),
          ),
        ]);
      }),
    );
  }

  /// User list #3: anchored popover for a pose keyframe (copy, paste, duplicate, easing, delete).
  Future<void> _poseKeyPopover(Map r, List b, int blockIndex, Rect anchor) async {
    final layerId = '${r['id']}';
    final name = '${r['name']}';
    final st = (b[0] as num).toInt();
    final isHold = b[2] == true;
    final blockEases = (r['blockEases'] as List?) ?? const [];
    final rawEase = blockIndex < blockEases.length ? blockEases[blockIndex] : (b.length > 3 ? b[3] : null);
    var ease = rawEase is String ? rawEase : (isHold ? 'hold' : 'auto');
    const eases = ['auto', 'linear', 'out', 'smooth', 'bounce'];
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Pose keyframe',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        final hold = ease == 'hold';
        void setEase(String v) {
          _cmd({'op': 'easing', 'layer': layerId, 'frame': st, 'v': v});
          set(() => ease = v);
        }
        return Stack(children: [
          kit.Anchored(
            anchor: anchor,
            width: 300,
            topMin: 44 + MediaQuery.paddingOf(c).top,
            card: (w) => kit.PanelCard(
              width: w,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                rw.renderDesc(rw.DTitle('$name (frame ${st + 1})')),
                rw.renderDesc(rw.DSeg(const ['Hold', 'Tween'], hold ? 0 : 1, onSelect: (i) => setEase(i == 0 ? 'hold' : 'auto'))),
                rw.renderDesc(rw.DEase(math.max(0, eases.indexOf(ease == 'in' ? 'linear' : ease)), enabled: !hold, onSelect: (i) => setEase(eases[i]))),
                rw.renderDesc(rw.DButtons(
                  'Copy pose',
                  () {
                    _cmd({'op': 'copy_pose', 'layer': layerId, 'frame': st, 'withTransform': true});
                    Navigator.pop(c);
                  },
                  b: 'Paste pose',
                  onB: _st['pose'] == true
                      ? () {
                          _cmd({'op': 'paste_pose', 'layer': layerId, 'frame': st, 'withTransform': true});
                          Navigator.pop(c);
                        }
                      : null,
                )),
                if (st > 0)
                  rw.renderDesc(rw.DButtons(
                    'Earlier',
                    () {
                      _cmd({'op': 'shift_keys', 'poses': [[layerId, st]], 'props': const [], 'by': -1, 'overwrite': true});
                      Navigator.pop(c);
                    },
                    iconA: 'nudgeLeft',
                    b: 'Later',
                    onB: () {
                      _cmd({'op': 'shift_keys', 'poses': [[layerId, st]], 'props': const [], 'by': 1, 'overwrite': true});
                      Navigator.pop(c);
                    },
                    iconB: 'nudgeRight',
                  )),
                kit.ButtonPair(
                  'Duplicate',
                  () {
                    _cmd({'op': 'duplicate_pose_key', 'layer': layerId, 'from': st});
                    Navigator.pop(c);
                  },
                  null,
                  null,
                  false,
                  'duplicate',
                ),
                if (st > 0)
                  rw.renderDesc(rw.DDanger('Delete pose', () {
                    _cmd({'op': 'delete_pose', 'layer': layerId, 'frame': st});
                    Navigator.pop(c);
                  })),
              ]),
            ),
          ),
        ]);
      }),
    );
  }

  /// User list #3: anchored popover for an animated property keyframe.
  Future<void> _propKeyPopover(Map r, Map pr, int f, String initialEase, Rect anchor) async {
    final pid = '${pr['id']}';
    final pname = '${pr['name']}';
    var ease = initialEase;
    const eases = ['auto', 'linear', 'out', 'smooth', 'bounce'];
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Property keyframe',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        final hold = ease == 'hold';
        void setEase(String v) {
          _cmd({'op': 'easing', 'property': pid, 'frame': f, 'v': v});
          set(() => ease = v);
        }
        return Stack(children: [
          kit.Anchored(
            anchor: anchor,
            width: 300,
            topMin: 44 + MediaQuery.paddingOf(c).top,
            card: (w) => kit.PanelCard(
              width: w,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                rw.renderDesc(rw.DTitle('$pname (frame ${f + 1})')),
                rw.renderDesc(rw.DSeg(const ['Hold', 'Tween'], hold ? 0 : 1, onSelect: (i) => setEase(i == 0 ? 'hold' : 'auto'))),
                rw.renderDesc(rw.DEase(math.max(0, eases.indexOf(ease == 'in' ? 'linear' : ease)), enabled: !hold, onSelect: (i) => setEase(eases[i]))),
                rw.renderDesc(rw.DButtons(
                  'Copy key',
                  () {
                    _cmd({'op': 'copy_prop_key', 'property': pid, 'frame': f});
                    Navigator.pop(c);
                  },
                  b: 'Paste key',
                  onB: _st['propKeyCopied'] == true
                      ? () {
                          _cmd({'op': 'paste_prop_key', 'property': pid, 'frame': f});
                          Navigator.pop(c);
                        }
                      : null,
                )),
                kit.ButtonPair(
                  'Duplicate',
                  () {
                    _cmd({'op': 'duplicate_prop_key', 'property': pid, 'from': f});
                    Navigator.pop(c);
                  },
                ),
                rw.renderDesc(rw.DDanger('Delete key', () {
                  _cmd({'op': 'delete_prop_key', 'property': pid, 'frame': f});
                  Navigator.pop(c);
                })),
              ]),
            ),
          ),
        ]);
      }),
    );
  }

  /// Quick paste popover when tapping an empty cell on a property row with a copied key.
  Future<void> _propPastePopover(Map pr, int f, Offset at) async {
    final pid = '${pr['id']}';
    final pname = '${pr['name']}';
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Paste key',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => Stack(children: [
        kit.Anchored(
          anchor: kit.Anchored.rectFromGlobal(context, at),
          width: 240,
          topMin: 44 + MediaQuery.paddingOf(c).top,
          card: (w) => kit.PanelCard(
            width: w,
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              rw.renderDesc(rw.DTitle('$pname · Frame ${f + 1}')),
              kit.WordButton('Paste key', icon: 'paste', primary: true, expand: true, height: 36, onTap: () {
                _cmd({'op': 'paste_prop_key', 'property': pid, 'frame': f});
                Navigator.pop(c);
              }),
            ]),
          ),
        ),
      ]),
    );
  }

  /// Sorted unique keyframe numbers on the selected layer (or across all layers).
  List<int> _keyFramesForJump() {
    final rows = ((_st['rows'] as List?) ?? const []).cast<Map>();
    final selId = _sel?['layer'];
    final target = selId != null ? rows.where((r) => r['id'] == selId) : rows;
    final set = <int>{};
    for (final r in target) {
      for (final b in ((r['blocks'] as List?) ?? const []).cast<List>()) {
        set.add((b[0] as num).toInt());
      }
      for (final pr in ((r['props'] as List?) ?? const []).cast<Map>()) {
        for (final f in ((pr['keys'] as List?) ?? const []).cast<num>()) {
          set.add(f.toInt());
        }
      }
    }
    return set.toList()..sort();
  }

  /// Personal addition: quick Jump to Frame popover anchored to the playhead pill.
  Future<void> _frameJumpPopover() async {
    final screen = MediaQuery.sizeOf(context);
    final anchor = kit.Anchored.rectOf(_playheadKey) ?? Rect.fromLTWH(screen.width / 2 - 20, screen.height - 160, 40, 22);
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Jump to frame',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        final keys = _keyFramesForJump();
        final prevKey = keys.where((k) => k < _frame).lastOrNull;
        final nextKey = keys.where((k) => k > _frame).firstOrNull;
        void jump(int i) {
          _goFrame(i);
          set(() {});
        }
        return Stack(children: [
          kit.Anchored(
            anchor: anchor,
            width: 280,
            topMin: 44 + MediaQuery.paddingOf(c).top,
            card: (w) => kit.PanelCard(
              width: w,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                rw.renderDesc(const rw.DTitle('Jump to frame')),
                rw.renderDesc(rw.DStep('Frame', '${_frame + 1} / $_frames', onMinus: _frame > 0 ? () => jump(_frame - 1) : null, onPlus: _frame < _frames - 1 ? () => jump(_frame + 1) : null)),
                rw.renderDesc(rw.DButtons(
                  'Prev key',
                  prevKey != null ? () => jump(prevKey) : null,
                  b: 'Next key',
                  onB: nextKey != null ? () => jump(nextKey) : null,
                )),
                rw.renderDesc(rw.DButtons(
                  'First frame',
                  _frame > 0 ? () => jump(0) : null,
                  b: 'Last frame',
                  onB: _frame < _frames - 1 ? () => jump(_frames - 1) : null,
                )),
              ]),
            ),
          ),
        ]);
      }),
    );
  }

  /// Unified hold-to-open popover for multi-selected pose & property keyframes.
  Future<void> _multiKeyPopover(Rect anchor) async {
    final total = _selPoseKeys.length + _selPropKeys.length;
    if (total == 0) return;
    const eases = [('hold', 'Hold'), ('auto', 'Auto'), ('linear', 'Even'), ('out', 'Slow end'), ('smooth', 'Slow both'), ('bounce', 'Bounce')];
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Selected keyframes',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        final curTotal = _selPoseKeys.length + _selPropKeys.length;
        final allFrames = <int>[for (final (_, f) in _selPoseKeys) f, for (final (_, f) in _selPropKeys) f];
        final minF = allFrames.isEmpty ? 0 : allFrames.reduce(math.min);
        final maxF = allFrames.isEmpty ? 0 : allFrames.reduce(math.max);
        final canRange = maxF > minF;
        return Stack(children: [
          kit.Anchored(
            anchor: anchor,
            width: 300,
            topMin: 44 + MediaQuery.paddingOf(c).top,
            card: (w) => kit.PanelCard(
              width: w,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                rw.renderDesc(rw.DTitle('$curTotal key${curTotal == 1 ? '' : 's'} selected')),
                rw.renderDesc(rw.DButtons(
                  'Earlier',
                  () {
                    _shiftSelectedKeys(-1);
                    set(() {});
                  },
                  iconA: 'nudgeLeft',
                  b: 'Later',
                  onB: () {
                    _shiftSelectedKeys(1);
                    set(() {});
                  },
                  iconB: 'nudgeRight',
                )),
                if (canRange) ...[
                  rw.renderDesc(rw.DButtons(
                    '0.75×',
                    () {
                      _scaleSelectedKeys(minF, 0.75);
                      set(() {});
                    },
                    iconA: 'compress',
                    b: '1.5×',
                    onB: () {
                      _scaleSelectedKeys(minF, 1.5);
                      set(() {});
                    },
                    iconB: 'stretch',
                  )),
                  rw.renderDesc(rw.DButtons(
                    'Reverse',
                    () {
                      _reverseSelectedRange(pingPong: false);
                      Navigator.pop(c);
                    },
                    iconA: 'reverse',
                    b: 'Ping-pong',
                    onB: () {
                      _reverseSelectedRange(pingPong: true);
                      Navigator.pop(c);
                    },
                    iconB: 'pingpong',
                  )),
                ],
                const Padding(padding: EdgeInsets.only(top: 6, bottom: 4), child: Text('Ease all', style: TextStyle(color: cSecondary, fontSize: 12.5, fontWeight: FontWeight.w600))),
                Wrap(spacing: 6, runSpacing: 6, children: [
                  for (final (id, label) in eases)
                    kit.WordButton(label, filled: true, height: 30, size: 12.5, onTap: () {
                      _cmd({
                        'op': 'ease_keys',
                        'poses': [for (final (l, f) in _selPoseKeys) [l, f]],
                        'props': [for (final (p, f) in _selPropKeys) [p, f]],
                        'v': id,
                      });
                      Navigator.pop(c);
                    }),
                ]),
                const SizedBox(height: 8),
                kit.ButtonPair(
                  'Clear selection',
                  () {
                    setState(() {
                      _selPoseKeys.clear();
                      _selPropKeys.clear();
                    });
                    Navigator.pop(c);
                  },
                ),
                rw.renderDesc(rw.DDanger('Delete $curTotal key${curTotal == 1 ? '' : 's'}', () {
                  _deleteSelectedKeys();
                  Navigator.pop(c);
                })),
              ]),
            ),
          ),
        ]);
      }),
    );
  }

  void _shiftSelectedKeys(int by) {
    final poses = [for (final (l, f) in _selPoseKeys) [l, f]];
    final props = [for (final (p, f) in _selPropKeys) [p, f]];
    final st = _cmd({'op': 'shift_keys', 'poses': poses, 'props': props, 'by': by, 'overwrite': true});
    _applyMovedKeySelection(st);
  }

  void _scaleSelectedKeys(int anchor, double factor) {
    final poses = [for (final (l, f) in _selPoseKeys) [l, f]];
    final props = [for (final (p, f) in _selPropKeys) [p, f]];
    final st = _cmd({'op': 'scale_keys', 'poses': poses, 'props': props, 'anchor': anchor, 'factor': factor});
    _applyMovedKeySelection(st);
  }

  void _deleteSelectedKeys() {
    _cmd({
      'op': 'delete_keys',
      'poses': [for (final (l, f) in _selPoseKeys) [l, f]],
      'props': [for (final (p, f) in _selPropKeys) [p, f]],
    });
    setState(() {
      _selPoseKeys.clear();
      _selPropKeys.clear();
    });
    kit.showToast(context, 'Deleted selected keys.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
  }

  void _reverseSelectedRange({required bool pingPong}) {
    final allFrames = <int>[for (final (_, f) in _selPoseKeys) f, for (final (_, f) in _selPropKeys) f];
    if (allFrames.length < 2) return;
    final from = allFrames.reduce(math.min), to = allFrames.reduce(math.max);
    if (to <= from) return;
    final layers = {for (final (l, _) in _selPoseKeys) l};
    final layer = layers.length == 1 && _selPropKeys.isEmpty ? layers.first : null;
    _cmd({
      'op': pingPong ? 'ping_pong_range' : 'reverse_range',
      'layer': ?layer,
      'from': from,
      'to': to,
    });
    setState(() {
      _selPoseKeys.clear();
      _selPropKeys.clear();
    });
  }


  static const _markIcons = {
    'pause': Icons.timer_outlined,
    'repeat': Icons.repeat,
    'silence': Icons.volume_off,
    'slow': Icons.slow_motion_video,
    'camera': Icons.videocam,
    'notween': Icons.block,
  };

  void _goFrame(int i) {
    if (_playing) _togglePlay();
    final clamped = i.clamp(0, _frames - 1);
    _cmd({'op': 'frame', 'i': clamped});
    if (!AppSettings.flag('keepSelection')) _cmd({'op': 'select'});
    WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(clamped));
  }

  /// Drag handle along the top of the timeline to resize its height (User list #8).
  Widget _timelineResizeHandle(double screenH) {
    final maxH = math.max(132.0, screenH * 0.62);
    return Semantics(
      label: 'Resize timeline',
      hint: 'Drag up or down to resize the timeline; double-tap to reset.',
      child: kit.HitPad(
        dy: 10,
        child: MouseRegion(
          cursor: SystemMouseCursors.resizeUpDown,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onVerticalDragStart: (_) => setState(() {
              _timelineDragging = true;
              _timelineHeight ??= _lastTimelineBodyH.clamp(96.0, maxH);
            }),
            onVerticalDragUpdate: (d) => setState(() {
              final base = _timelineHeight ?? _lastTimelineBodyH;
              _timelineHeight = (base - d.delta.dy).clamp(96.0, maxH);
            }),
            onVerticalDragEnd: (_) {
              setState(() => _timelineDragging = false);
              if (_timelineHeight != null) {
                AppSettings.values['timelineHeight'] = _timelineHeight!.round();
                unawaited(AppSettings.save());
              }
            },
            onVerticalDragCancel: () => setState(() => _timelineDragging = false),
            onDoubleTap: () {
              setState(() {
                _timelineHeight = null;
                _timelineDragging = false;
              });
              AppSettings.values.remove('timelineHeight');
              unawaited(AppSettings.save());
            },
            child: Container(
              height: 14,
              width: double.infinity,
              decoration: const BoxDecoration(color: cCard, border: Border(top: BorderSide(color: cLine))),
              alignment: Alignment.center,
              child: AnimatedContainer(
                duration: kit.motion(context, 100),
                width: _timelineDragging ? 56 : 40,
                height: 4.5,
                decoration: BoxDecoration(
                  color: _timelineDragging ? cAccent : const Color(0xFFC2C8D0),
                  borderRadius: BorderRadius.circular(2.5),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// The timeline's control toolbar (REDESIGN §4): frame steps on the left,
  /// onion skin, transport in the middle, the frame rate on the right.
  Widget _controlBar() {
    return Container(
      height: 40,
      decoration: BoxDecoration(color: cCard, border: Border(top: _layersOpen && !_playing ? BorderSide.none : const BorderSide(color: cLine), bottom: const BorderSide(color: Color(0xFFE6E8EB)))),
      padding: const EdgeInsets.symmetric(horizontal: 6),
      child: Stack(alignment: Alignment.center, children: [
        Row(children: [
          kit.IconTool('stepBack', label: 'Back one frame', onTap: _frame > 0 ? () => _goFrame(_frame - 1) : null),
          kit.IconTool('stepFwd', label: 'Forward one frame', onTap: _frame < _frames - 1 ? () => _goFrame(_frame + 1) : null),
          const SizedBox(width: 20),
          kit.IconTool('onion', label: 'Onion skin', on: _opts['onion'] == true, onTap: _playing ? null : () => _cmd({'op': 'set', 'k': 'onion', 'v': _opts['onion'] != true})),
          // the board's Loop: playback goes round (on) or stops at the end
          kit.IconTool('loop', label: AppSettings.values['loopPlay'] == false ? 'Play once' : 'Play in a loop', on: AppSettings.values['loopPlay'] != false, onTap: () => setState(() {
                AppSettings.values['loopPlay'] = AppSettings.values['loopPlay'] == false;
                unawaited(AppSettings.save());
              })),
          // H1 (D6): effects while editing, only when something has them
          if (_st['hasEffects'] == true)
            kit.Tap(
              label: AppSettings.number('showFilters') > 0 ? 'Effects shown while editing' : 'Effects hidden while editing',
              onTap: _playing ? null : _toggleFx,
              radius: 8,
              child: Container(
                width: 36,
                height: 32,
                alignment: Alignment.center,
                decoration: BoxDecoration(color: AppSettings.number('showFilters') > 0 ? cSelected : null, borderRadius: BorderRadius.circular(8)),
                child: Text('FX', style: TextStyle(color: AppSettings.number('showFilters') > 0 ? cAccent : cInk2b, fontSize: 12, fontWeight: FontWeight.w800, letterSpacing: 0.3)),
              ),
            ),
          // D34: the timeline as curves of the selected figure's values
          kit.IconTool('curves', label: _curvesOn ? 'Back to the layers' : 'Curves', on: _curvesOn, onTap: _playing ? null : () => _cmd({'op': 'set', 'k': 'curves', 'v': !_curvesOn})),
          const Spacer(),
          kit.Tap(
            key: _fpsKey,
            label: 'Animation: $_fpsLabel frames per second',
            onTap: _playing ? null : _animationPopover,
            radius: 8,
            child: Padding(padding: const EdgeInsets.symmetric(horizontal: 8), child: Text('$_fpsLabel fps', style: const TextStyle(color: cSecondary, fontSize: 13))),
          ),
          // G1: unfold the layer rows (folded: the ruler and Preview row only)
          AnimatedRotation(
            turns: _layersOpen ? 0 : 0.5,
            duration: kit.motion(context, 180),
            curve: kit.kEase,
            child: kit.IconTool('chevron', label: _layersOpen ? 'Fold the timeline' : 'Show the layers', onTap: () => setState(() => _layersOpen = !_layersOpen)),
          ),
        ]),
        Row(mainAxisSize: MainAxisSize.min, children: [
          kit.IconTool('toStart', label: 'Go to the first frame', onTap: () => _goFrame(0)),
          GestureDetector(
            onLongPress: _playing ? null : _loopToPopup,
            child: kit.IconTool(_playing ? 'pause' : 'play', label: _playing ? 'Stop' : 'Play (hold: loop to…)', size: 22, onTap: _togglePlay),
          ),
          kit.IconTool('toEnd', label: 'Go to the last frame', onTap: () => _goFrame(_frames - 1)),
        ]),
      ]),
    );
  }

  /// The motion path on the stage (View › Motion path; the Segment page's
  /// "Show motion path"): one setting, kept between runs.
  void _setMotionPath(bool v) {
    AppSettings.values['motionPath'] = v;
    unawaited(AppSettings.save());
    _cmd({'op': 'set', 'k': 'path', 'v': v});
  }

  /// The FX toggle: effects while editing off, or back to how they were
  /// (Settings › While editing keeps the choice of All or No blur).
  void _toggleFx() {
    final now = AppSettings.number('showFilters');
    if (now > 0) _fxLast = now;
    setState(() => AppSettings.values['showFilters'] = now > 0 ? 0 : _fxLast);
    unawaited(AppSettings.save());
  }

  int _fxLast = 2;

  String get _fpsLabel {
    final f = _fps;
    return f == f.roundToDouble() ? f.round().toString() : f.toStringAsFixed(1);
  }

  final GlobalKey _addKey = GlobalKey();

  /// The "+" menu (REDESIGN §4): everything that can be added, in one place.
  /// It replaces Library Tools.
  Future<void> _addMenu() async {
    final screen = MediaQuery.sizeOf(context);
    final anchor = kit.Anchored.rectOf(_addKey) ?? Rect.fromLTWH(8, screen.height - 48, 36, 36);
    final items = <(String, String, VoidCallback?)>[
      ('figure', 'Stick figure', () {
        setState(() => _libType = 0);
        _figurePicker();
      }),
      ('createFigure', 'Create stick figure', _createNewFigure),
      ('picture', 'Picture', _importImages),
      ('text', 'Text', () => _cmd({'op': 'add_textfield'})),
      ('sound', 'Sound', _importSounds),
      ('clip', 'Movieclip', () {
        setState(() => _libType = 1);
        _figurePicker(movieclip: true);
      }),
      ('fx', 'Effect layer', () => _cmd({'op': 'add_adjustment'})),
      // K2c: a script that moves figures or values (sway, breathing, physics)
      ('script', 'Script', _scriptPicker),
      ('folder', 'Group', () {
        if (_st['multi'] != null || _sel != null || _st['picture'] != null || _st['movieclip'] != null || _st['textfield'] != null || _st['adjustment'] != null) {
          _cmd({'op': 'group'});
        } else {
          _say('Select layers first to group.', light: true);
        }
      }),
    ];
    final pick = await showGeneralDialog<int>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Add',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => Stack(children: [
        kit.Anchored(
          anchor: anchor,
          width: 250,
          topMin: 44 + MediaQuery.paddingOf(c).top,
          card: (w) => kit.PanelCard(
            width: w,
            padding: const EdgeInsets.all(6),
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              for (var i = 0; i < items.length; i++) ...[
                if (i == 7) Container(height: 1, margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 4), color: const Color(0xFFE6E8EB)),
                kit.Tap(
                  onTap: items[i].$3 == null ? null : () => Navigator.pop(c, i),
                  minSize: 0,
                  child: Opacity(
                    opacity: items[i].$3 == null ? 0.4 : 1,
                    child: Container(
                      height: 40,
                      padding: const EdgeInsets.symmetric(horizontal: 10),
                      child: Row(children: [
                        Pictogram(items[i].$1, size: 22, color: cAccent),
                        const SizedBox(width: 12),
                        Text(items[i].$2, style: const TextStyle(color: cText, fontSize: 15)),
                      ]),
                    ),
                  ),
                ),
              ],
            ]),
          ),
        ),
      ]),
    );
    if (pick != null) items[pick].$3?.call();
  }

  // ---------------------------------------------------------------- curves

  // D34 (the graph board): the selected figure's values as curves. The engine
  // sends each stretch's exact shape; this draws it and turns drags into
  // curve_down / curve_move / up (one undo step each).
  bool get _curvesOn => _opts['curves'] == true;
  String? _curveSel; // the selected value (its property id)
  double? _curveKey; // the selected key's frame
  final Map<String, bool> _curveVis = {};
  Map<String, (double, double)>? _curveRanges; // kept still after a drag, until Fit
  double _curveScroll = 0;
  String? _curveDrag; // key, left, right or scroll
  Offset _curveFrom = Offset.zero;
  double _curveScrollFrom = 0;
  Offset? _curveDownAt; // where the finger went down (a drag starts there, not past the slop)
  int _curvePointers = 0;
  double? _curvePinchCell; // the zoom when a pinch began
  double _curvePinchAnchor = 0; // the frame under the pinch
  _CurveGeom? _curveGeom;

  static const _curveColours = [Color(0xFFD9482B), Color(0xFF2E8B57), Color(0xFFC28A00), Color(0xFF2D63E0), Color(0xFF9A6BD0), Color(0xFF0E9AA7), Color(0xFFB3446C)];

  List<Map> get _curveChannels => (((_st['curves'] as Map?)?['channels'] as List?) ?? const []).cast<Map>();
  bool _curveShown(Map c) => _curveVis['${c['id']}'] ?? (c['focus'] == true);

  Widget _curvesView() {
    final cv = _st['curves'] as Map?;
    final screenH = MediaQuery.sizeOf(context).height;
    final double h = _timelineHeight != null ? _timelineHeight!.clamp(100.0, math.max(140.0, screenH * 0.62)).toDouble() : math.min(300.0, screenH * 0.42);
    _lastTimelineBodyH = h;
    final nameW = _namesW(context);
    if (cv == null) {
      return Container(
        height: 72,
        color: cCard,
        alignment: Alignment.center,
        child: const Text('Select a figure to see its values as curves.', style: TextStyle(color: cSecondary, fontSize: 14)),
      );
    }
    final chans = _curveChannels;
    // nothing picked yet (or it's gone): the selected joint's first value
    if (!chans.any((c) => c['id'] == _curveSel)) {
      _curveSel = '${(chans.where((c) => c['focus'] == true && c['unit'] != 'colour').firstOrNull ?? chans.firstOrNull)?['id'] ?? ''}';
    }
    // a key gone (undo, a deleted pose) is no longer selected
    final sel = chans.where((c) => c['id'] == _curveSel).firstOrNull;
    if (sel == null || !((sel['keys'] as List).any((k) => ((k as List)[0] as num).toDouble() == _curveKey))) _curveKey = null;
    final addable = ((cv['addable'] as List?) ?? const []).cast<Map>();
    Widget nameRow(int i, Map c) {
      final on = _curveShown(c), picked = c['id'] == _curveSel;
      final colour = _curveColours[i % _curveColours.length];
      return GestureDetector(
        onTap: () => setState(() {
          _curveSel = '${c['id']}';
          _curveVis['${c['id']}'] = true;
        }),
        child: Container(
          height: 30,
          padding: const EdgeInsets.only(left: 14, right: 4),
          decoration: BoxDecoration(color: picked ? cSelected : null, border: const Border(bottom: BorderSide(color: Color(0xFFE6E8EB)))),
          child: Row(children: [
            Container(
              width: 10,
              height: 10,
              decoration: c['unit'] == 'colour'
                  ? BoxDecoration(borderRadius: BorderRadius.circular(3), color: _rgba((((c['keys'] as List).firstOrNull as List?)?[1] as num?)?.toInt() ?? 0x808080ff))
                  : BoxDecoration(shape: BoxShape.circle, color: on ? colour : cCard, border: Border.all(color: colour, width: 2)),
            ),
            const SizedBox(width: 8),
            Expanded(child: Text('${c['name']}', maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(fontSize: 13, color: picked ? cAccent : on ? cText : cSecondary, fontWeight: picked ? FontWeight.w600 : FontWeight.w400))),
            // D36: a value with modifiers
            if (((c['mods'] as List?) ?? const []).isNotEmpty) const Padding(padding: EdgeInsets.only(left: 2), child: Pictogram('modifier', size: 14, color: cAccent)),
            kit.Tap(
              label: on ? 'Hide ${c['name']}' : 'Show ${c['name']}',
              minSize: 26,
              onTap: () => setState(() => _curveVis['${c['id']}'] = !on),
              child: Pictogram('eye', size: 16, color: on ? const Color(0xFF9AA0A8) : const Color(0xFFD5D8DD)),
            ),
          ]),
        ),
      );
    }

    final names = Container(
      width: nameW,
      decoration: const BoxDecoration(color: cPanel, border: Border(right: BorderSide(color: Color(0xFFE6E8EB)))),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Container(height: 22, decoration: const BoxDecoration(border: Border(bottom: BorderSide(color: Color(0xFFE6E8EB))))),
        Container(
          height: 32,
          padding: const EdgeInsets.only(left: 10),
          decoration: const BoxDecoration(border: Border(bottom: BorderSide(color: Color(0xFFE6E8EB)))),
          child: Row(children: [
            const Pictogram('figure', size: 18, color: cInk2b),
            const SizedBox(width: 6),
            Flexible(child: Text('${cv['name']}', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 14))),
            const SizedBox(width: 6),
            Text('${chans.length} animated', style: const TextStyle(color: cSecondary, fontSize: 12)),
          ]),
        ),
        Expanded(
          child: ListView(padding: EdgeInsets.zero, children: [
            for (var i = 0; i < chans.length; i++) nameRow(i, chans[i]),
            kit.Tap(
              label: 'Add property',
              minSize: 0,
              onTap: addable.isEmpty ? null : () => _addCurveValue(addable),
              child: Container(
                height: 30,
                padding: const EdgeInsets.only(left: 12),
                alignment: Alignment.centerLeft,
                child: Row(children: [
                  Pictogram('plus', size: 15, color: addable.isEmpty ? cSecondary : cAccent),
                  const SizedBox(width: 6),
                  Text('Add property…', style: TextStyle(color: addable.isEmpty ? cSecondary : cAccent, fontSize: 13, fontWeight: FontWeight.w600)),
                ]),
              ),
            ),
          ]),
        ),
      ]),
    );
    final graph = LayoutBuilder(builder: (context, box) {
      final colours = {for (var i = 0; i < chans.length; i++) '${chans[i]['id']}': _curveColours[i % _curveColours.length]};
      final geom = _CurveGeom(
        size: Size(box.maxWidth, h),
        frames: _frames,
        cell: _cellW,
        scroll: _curveScroll,
        channels: [for (final c in chans) if (_curveShown(c)) c],
        selected: _curveSel,
        frozen: _curveRanges,
      );
      _curveGeom = geom;
      return Stack(children: [
        // the same hands as the timeline: drag the ruler to scrub, pinch to
        // zoom, wheel to scroll (Ctrl + wheel zooms); one finger elsewhere
        // moves a key or a handle, or scrolls
        Listener(
          onPointerDown: (e) {
            _curvePointers++;
            if (_curvePointers == 1) _curveDownAt = e.localPosition;
          },
          onPointerUp: (_) => _curvePointers = math.max(0, _curvePointers - 1),
          onPointerCancel: (_) => _curvePointers = math.max(0, _curvePointers - 1),
          onPointerSignal: (e) {
            if (e is! PointerScrollEvent) return;
            if (HardwareKeyboard.instance.isControlPressed) {
              _curveZoom(_cellW * (e.scrollDelta.dy > 0 ? 1 / 1.15 : 1.15), e.localPosition.dx);
            } else {
              _setCurveScroll(_curveScroll + (e.scrollDelta.dx != 0 ? e.scrollDelta.dx : e.scrollDelta.dy));
            }
          },
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapUp: (d) => _curveTap(geom, d.localPosition),
            onScaleStart: (d) {
              if (d.pointerCount >= 2) {
                _curvePinchStart(d.localFocalPoint);
              } else {
                _curvePanStart(geom, _curveDownAt ?? d.localFocalPoint);
              }
            },
            onScaleUpdate: (d) {
              if (d.pointerCount >= 2) {
                // a second finger mid-drag: the drag ends, the pinch begins
                if (_curvePinchCell == null) {
                  _curvePanEnd();
                  _curvePinchStart(d.localFocalPoint);
                }
                _curveZoom(_curvePinchCell! * d.horizontalScale, d.localFocalPoint.dx, anchor: _curvePinchAnchor);
                return;
              }
              if (_curvePinchCell != null) return; // a finger left over from a pinch
              _curvePanUpdate(d.localFocalPoint);
            },
            onScaleEnd: (_) {
              if (_curvePinchCell != null) {
                _curvePinchCell = null;
                _curveDrag = null;
                return;
              }
              _curvePanEnd();
            },
            child: CustomPaint(size: Size(box.maxWidth, h), painter: _CurvesPainter(geom, colours, _frame, _curveKey, fps: _fps.round().clamp(1, 240))),
          ),
        ),
        if (chans.isEmpty)
          Positioned.fill(
            top: _CurveGeom.rulerH,
            child: Center(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Column(mainAxisSize: MainAxisSize.min, children: [
                  const Text(
                    'Move a figure on two frames to edit its motion curve.',
                    textAlign: TextAlign.center,
                    style: TextStyle(color: cSecondary, fontSize: 13, fontWeight: FontWeight.w500),
                  ),
                  if (addable.isNotEmpty) ...[
                    const SizedBox(height: 6),
                    kit.WordButton('Add property…', icon: 'plus', filled: true, height: 32, size: 13, onTap: () => _addCurveValue(addable)),
                  ],
                ]),
              ),
            ),
          ),
      ]);
    });
    return Container(
      height: h,
      color: cCard,
      child: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [names, Expanded(child: ClipRect(child: graph))]),
    );
  }

  Future<void> _addCurveValue(List<Map> addable) async {
    final options = [for (final a in addable) TargetOption.fromMap(a)];
    final picked = await showTargetPickerSheet(
      context,
      title: 'Add property',
      options: options,
      emptyText: 'No properties available.',
      searchHint: 'Search properties…',
    );
    if (picked == null || picked.isEmpty) return;
    _cmd({'op': 'curve_add', 'property': picked});
    setState(() {
      _curveVis[picked] = true;
      _curveSel = picked;
    });
  }

  /// The key under a point: (value id, frame), within a finger's reach.
  (String, double)? _curveKeyAt(_CurveGeom g, Offset p) {
    (String, double)? best;
    var bestD = 18.0;
    for (final c in g.channels) {
      if (c['unit'] == 'colour') continue;
      for (final k in (c['keys'] as List).cast<List>()) {
        final f = (k[0] as num).toDouble();
        final q = Offset(g.xOf(f), g.yOf(c, (k[1] as num).toDouble()));
        final d = (q - p).distance;
        if (d < bestD) {
          bestD = d;
          best = ('${c['id']}', f);
        }
      }
    }
    return best;
  }

  /// The selected key's handle under a point: "left" or "right".
  String? _curveHandleAt(_CurveGeom g, Offset p) {
    final hs = g.handles(_curveSel, _curveKey);
    if (hs == null) return null;
    if ((hs.$1 - p).distance < 18) return 'left';
    if ((hs.$2 - p).distance < 18) return 'right';
    return null;
  }

  void _curveTap(_CurveGeom g, Offset p) {
    if (p.dy < _CurveGeom.rulerH) {
      _cmd({'op': 'frame', 'i': g.frameAt(p.dx)});
      return;
    }
    final hit = _curveKeyAt(g, p);
    setState(() {
      if (hit != null) {
        _curveSel = hit.$1;
        _curveKey = hit.$2;
      } else {
        _curveKey = null;
      }
    });
  }

  void _curvePanStart(_CurveGeom g, Offset p) {
    _curveFrom = p;
    if (p.dy < _CurveGeom.rulerH) {
      // the ruler scrubs, as on the timeline
      if (_playing) _togglePlay();
      _curveDrag = 'scrub';
      _scrubTo(g.frameAt(p.dx));
      return;
    }
    final part = p.dy >= _CurveGeom.rulerH ? _curveHandleAt(g, p) : null;
    final hit = part == null && p.dy >= _CurveGeom.rulerH ? _curveKeyAt(g, p) : null;
    if (part == null && hit == null) {
      _curveDrag = 'scroll';
      _curveScrollFrom = _curveScroll;
      return;
    }
    if (hit != null) {
      _curveSel = hit.$1;
      _curveKey = hit.$2;
    }
    // the graph keeps its scale under the finger (Fit lets go of it)
    _curveRanges = g.ranges();
    _curveDrag = part ?? 'key';
    _cmd({'op': 'curve_down', 'property': _curveSel, 'frame': _curveKey, 'part': _curveDrag});
  }

  void _curvePanUpdate(Offset p) {
    final g = _curveGeom;
    final d = p - _curveFrom;
    if (_curveDrag == 'scrub') {
      if (g != null) _scrubTo(g.frameAt(p.dx));
      return;
    }
    if (_curveDrag == 'scroll' || g == null) {
      _setCurveScroll(_curveScrollFrom - d.dx);
      return;
    }
    if (_curveDrag == null) return;
    final c = g.channels.where((x) => x['id'] == _curveSel).firstOrNull;
    if (c == null) return;
    _cmd({'op': 'curve_move', 'dv': -d.dy * g.valuePerPx(c), 'dt': d.dx / _cellW});
  }

  void _curvePanEnd() {
    if (_curveDrag != null && _curveDrag != 'scroll' && _curveDrag != 'scrub') _cmd({'op': 'up'});
    _curveDrag = null;
    _curveDownAt = null;
  }

  void _setCurveScroll(double v) {
    final w = _curveGeom?.size.width ?? 0;
    final most = math.max(0.0, 16 + _frames * _cellW - w);
    setState(() => _curveScroll = v.clamp(0.0, most));
  }

  void _curvePinchStart(Offset focal) {
    _curvePinchCell = _cellW;
    _curvePinchAnchor = (focal.dx + _curveScroll - _CurveGeom.left - _cellW / 2) / _cellW;
    _curveDrag = 'pinch';
  }

  /// Zooms the graph (the same zoom as the timeline's) keeping the frame
  /// `anchor` (by default the one under x) under x.
  void _curveZoom(double cell, double x, {double? anchor}) {
    final at = anchor ?? (x + _curveScroll - _CurveGeom.left - _cellW / 2) / _cellW;
    final next = cell.clamp(14.0, 112.0).toDouble();
    setState(() => _cellW = next);
    _setCurveScroll(_CurveGeom.left + at * next + next / 2 - x);
  }

  /// The bottom bar while Curves is open: the selected key's handles, Fit,
  /// New pose.
  Widget _curvesBar() {
    final c = _curveChannels.where((x) => x['id'] == _curveSel).firstOrNull;
    final key = c == null || _curveKey == null ? null : (c['keys'] as List).cast<List>().where((k) => (k[0] as num).toDouble() == _curveKey).firstOrNull;
    const kinds = [('auto_clamped', 'Settle'), ('auto', 'Swing'), ('vector', 'Sharp'), ('free', 'Free'), ('aligned', 'Aligned')];
    final now = key == null ? null : '${key[3]}';
    return Container(
      height: 40,
      color: cCard,
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Row(children: [
        Text('Handles', style: TextStyle(color: key == null ? const Color(0xFFB8BDC4) : cSecondary, fontSize: 13)),
        const SizedBox(width: 8),
        Flexible(
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Container(
              padding: const EdgeInsets.all(2),
              decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(9)),
              child: Row(mainAxisSize: MainAxisSize.min, children: [
                for (final (k, label) in kinds)
                  kit.Tap(
                    label: label,
                    minSize: 0,
                    onTap: key == null || c?['unit'] == 'colour' ? null : () => _cmd({'op': 'handles', 'v': k, 'frame': _curveKey!.round(), 'property': _curveSel}),
                    child: Container(
                      height: 30,
                      padding: const EdgeInsets.symmetric(horizontal: 11),
                      alignment: Alignment.center,
                      decoration: k == now ? BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(7), boxShadow: const [BoxShadow(color: Color(0x14000000), blurRadius: 2, offset: Offset(0, 1))]) : null,
                      child: Text(label, style: TextStyle(fontSize: 13, fontWeight: k == now ? FontWeight.w700 : FontWeight.w500, color: key == null ? const Color(0xFFB8BDC4) : k == now ? cAccent : cText)),
                    ),
                  ),
              ]),
            ),
          ),
        ),
        const Spacer(),
        kit.WordButton('Fit', size: 14, onTap: _curveRanges == null ? null : () => setState(() => _curveRanges = null)),
        kit.WordButton('New pose', size: 14, onTap: _sel == null || (_sel!['pose'] as Map?)?['here'] == true ? null : () => _cmd({'op': 'new_pose'})),
      ]),
    );
  }

  /// The timeline's bottom bar: "+" alone on the left, frame words on the right.
  Widget _framesBar() {
    final hasSel = _sel != null;
    final activeLayer = _activeLayerId;
    final pose = _sel?['pose'] as Map?;
    final poseHere = pose?['here'] == true;
    return Container(
      height: 40,
      decoration: const BoxDecoration(color: cCard, border: Border(top: BorderSide(color: Color(0xFFEEF0F2)))),
      padding: const EdgeInsets.symmetric(horizontal: 6),
      child: Row(children: [
        kit.IconTool('plus', key: _addKey, label: 'Add', color: cAccent, onTap: _playing ? null : _addMenu),
        if (!_playing && !((_st['rows'] as List?) ?? const []).any((r) => (r as Map)['kind'] != 'camera'))
          const Padding(padding: EdgeInsets.only(left: 4, right: 4), child: _StartHere()),
        // H3 (D5): keying. Off, a change to an effect or the opacity is the
        // same on every frame; on, it is a key on this frame
        kit.IconTool('key', label: _opts['keying'] == true ? 'Keying: changes make keys' : 'Keying off: changes apply to every frame', on: _opts['keying'] == true, onTap: _playing ? null : () => _cmd({'op': 'set', 'k': 'keying', 'v': _opts['keying'] != true})),
        const SizedBox(width: 8),
        Expanded(
          child: Align(
            alignment: Alignment.centerRight,
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(mainAxisSize: MainAxisSize.min, children: [
                kit.WordButton('Add frame', size: 14, onTap: _playing ? null : () {
                  _cmd({'op': 'insert_frame'});
                  WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(_frame));
                }),
                kit.WordButton('New pose', size: 14, onTap: _playing || !hasSel || poseHere ? null : () {
                  _cmd({'op': 'new_pose'});
                  WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(_frame));
                }),
                kit.WordButton('+1 frame', size: 14, onTap: _playing || activeLayer == null ? null : () {
                  _cmd({'op': 'hold_longer', if (!hasSel) 'layer': activeLayer});
                  final nextP = _sel?['pose'] as Map?;
                  final endF = nextP != null ? ((nextP['start'] as num).toInt() + (nextP['length'] as num).toInt() - 1) : _frame;
                  WidgetsBinding.instance.addPostFrameCallback((_) => _ensureFrameVisible(endF));
                }),
              ]),
            ),
          ),
        ),
      ]),
    );
  }


  // ---------------------------------------------------------------- left panel

  /// Switch between posing figures, moving everything and the camera.
  void _setMode(String m) {
    setState(() {
      _frameSelected = false;
      _soundPage = false;
      if (m != 'figure') {
        _selPoseKeys.clear();
        _selPropKeys.clear();
      }
    });
    if (m == 'camera' && _st['hasCamera'] != true) {
      _cmd({'op': 'add_camera'});
    } else {
      _cmd({'op': 'mode', 'v': m});
    }
    if (m != 'figure') _cmd({'op': 'select'});
  }

  /// The left rail (REDESIGN §4): the modes as stick-figure pictograms; the
  /// current one is red.
  Widget _leftRail(EdgeInsets pad, bool leftHand) {
    final mode = _st['mode'] as String? ?? 'figure';
    return Container(
      width: 64 + (leftHand ? pad.right : pad.left),
      padding: EdgeInsets.only(left: leftHand ? 0 : pad.left, right: leftHand ? pad.right : 0, top: 6, bottom: _fsNow || _hidePanels ? pad.bottom : 0),
      decoration: BoxDecoration(color: cPanel, border: Border(right: leftHand ? BorderSide.none : const BorderSide(color: cLine), left: leftHand ? const BorderSide(color: cLine) : BorderSide.none)),
      child: Column(children: [
        // the figure editor's rail (FigureEditor board): Parts and which handles show
        if (_st['building'] != null) ...[
          Padding(padding: const EdgeInsets.only(bottom: 4), child: kit.RailButton('parts', label: 'Parts', pressed: _partsOpen, onTap: () => setState(() => _partsOpen = !_partsOpen))),
          kit.RailButton('handles', label: 'Which handles show', pressed: _viewOptions, onTap: () => setState(() => _viewOptions = !_viewOptions)),
        ] else
          for (final (m, icon, label) in const [('figure', 'pose', 'Pose one figure'), ('all', 'all', 'Move everything'), ('camera', 'camera', 'Camera')])
            Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: kit.RailButton(icon, label: label, active: mode == m, onTap: _playing ? null : () => _setMode(m)),
            ),
      ]),
    );
  }

  /// What the rail's Delete removes: the selected layer everywhere (1-step
  /// undoable), selected text, or selected frame.
  VoidCallback? get _deleteSelection {
    if (_playing) return null;
    if (_st['textfield'] != null) return () => _cmd({'op': 'delete_textfield'});
    if (_sel != null && _st['building'] == null) {
      final name = _sel!['name'] ?? 'Figure';
      final id = _sel!['layer'];
      return () {
        _cmd({'op': 'delete_everywhere', 'layer': id});
        if (mounted) kit.showToast(context, 'Deleted $name.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
      };
    }
    if (_st['picture'] case final Map pic) {
      return () {
        _cmd({'op': 'delete_everywhere', 'layer': pic['layer']});
        if (mounted) kit.showToast(context, 'Deleted ${pic['name'] ?? 'picture'}.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
      };
    }
    if (_st['movieclip'] case final Map mc) {
      return () {
        _cmd({'op': 'delete_everywhere', 'layer': mc['layer']});
        if (mounted) kit.showToast(context, 'Deleted ${mc['name'] ?? 'movieclip'}.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
      };
    }
    if (_st['adjustment'] case final Map adj) {
      return () {
        _cmd({'op': 'delete_everywhere', 'layer': adj['layer']});
        if (mounted) kit.showToast(context, 'Deleted ${adj['name'] ?? 'Effect layer'}.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
      };
    }
    if (_frameSelected && _frames > 1) return () => _cmd({'op': 'delete_frame'});
    return null;
  }

  bool _panelOpen = false, _hidePanels = false;
  bool _partsOpen = true;
  bool _layersOpen = true, _rowActionsOpen = false;
  // User list #8: user-resized timeline body height (null = auto-fit up to cap)
  double? _timelineHeight;
  bool _timelineDragging = false;
  double _lastTimelineBodyH = 120;
  // the Sound page (a sound block tapped); any other pick leaves it
  bool _soundPage = false;
  final Set<String> _openRows = {};
  // G3: the timeline's zoom (a frame's width) and active block drag
  double _cellW = 46;
  _BlockDrag? _blockDrag;
  // G5: quick taps counted for the box (2) and lasso (3); the shape being drawn
  int _taps = 0;
  DateTime _lastDown = DateTime(0);
  Offset _lastDownAt = Offset.zero;
  List<Offset>? _boxPath;
  bool _lasso = false;
  bool _boxActive = false;
  DateTime _boxEndedAt = DateTime(0);
  double? _gripDownX;
  int? _preBoxFrame;
  bool? _preBoxFrameSel;
  bool _preBoxInPreview = false;
  // User list #4 & #5: box-selected pose keys (layerId, startFrame) and property keys (propId, frame)
  final Set<(String, int)> _selPoseKeys = {};
  final Set<(String, int)> _selPropKeys = {};
  final GlobalKey _playheadKey = GlobalKey();

  /// The right rail (REDESIGN §4): undo, redo, Properties, delete; hide the
  /// panels at the bottom.
  Widget _rightRail(EdgeInsets pad, bool leftHand) {
    return Container(
      width: 64 + (leftHand ? pad.left : pad.right),
      padding: EdgeInsets.only(right: leftHand ? 0 : pad.right, left: leftHand ? pad.left : 0, top: 6, bottom: 6 + (_fsNow || _hidePanels ? pad.bottom : 0)),
      decoration: BoxDecoration(color: cPanel, border: Border(left: leftHand ? BorderSide.none : const BorderSide(color: cLine), right: leftHand ? const BorderSide(color: cLine) : BorderSide.none)),
      // too short a window (a phone in landscape, the timeline open): the
      // rail scrolls rather than cutting off Delete and Hide the panels
      child: LayoutBuilder(builder: (context, box) => SingleChildScrollView(child: ConstrainedBox(constraints: BoxConstraints(minHeight: box.maxHeight), child: IntrinsicHeight(child: Column(children: [
        kit.RailButton('undo', label: 'Undo', onTap: !_playing && _st['canUndo'] == true ? () => _cmd({'op': 'undo'}) : null),
        const SizedBox(height: 4),
        kit.RailButton('redo', label: 'Redo', onTap: !_playing && _st['canRedo'] == true ? () => _cmd({'op': 'redo'}) : null),
        const SizedBox(height: 10),
        kit.RailButton('props', label: 'Properties', pressed: _panelOpen && !_hidePanels, onTap: () {
          setState(() {
            _hidePanels = false;
            _panelOpen = !_panelOpen;
          });
          // opened over the selected figure: ease it clear of the card
          if (_panelOpen) WidgetsBinding.instance.addPostFrameCallback((_) => _keepSelectionClear());
        }),
        const SizedBox(height: 4),
        kit.RailButton('bin', label: 'Delete', onTap: _deleteSelection),
        const Spacer(),
        kit.RailButton('full', label: _hidePanels ? 'Show the panels' : 'Hide the panels', pressed: _hidePanels, onTap: () => setState(() => _hidePanels = !_hidePanels)),
      ]))))),
    );
  }

  static String _tabLabel(String n) => n == 'Stickfigure Filters' ? 'Filters' : n == 'Figure editor' ? 'Figure' : n.replaceAll(' Tools', '');

  bool get _hasFxTarget =>
      _sel != null ||
      _st['multi'] != null ||
      _st['group'] != null ||
      _st['adjustment'] != null ||
      _st['picture'] != null ||
      _st['movieclip'] != null ||
      _st['textfield'] != null;

  /// Width of the floating Properties / Effects card.
  double _floatingCardWidth(Size screen, bool big) {
    if (_fxBrowser && _hasFxTarget && !_playing && screen.height < 500) {
      return math.min(440.0, math.max(300.0, screen.width - 144));
    }
    return big ? 360 : 320;
  }

  /// The Properties card (REDESIGN §4): always a clean floating overlay over
  /// the right side of the stage, with a 1-tap `×` close button.
  Widget _propertiesCard() {
    if (_fxBrowser && _hasFxTarget && !_playing) {
      return kit.PanelCard(padding: EdgeInsets.zero, child: _effectsBrowser());
    }
    if (_modBrowser && _curvesOn && _curveValue != null && !_playing) return kit.PanelCard(padding: EdgeInsets.zero, child: _modifierBrowser(_curveValue!));
    final names = _sections();
    final current = names.contains(_tab) ? names.indexOf(_tab) : 0;
    final foot = _pageFoot(_playing ? '' : names[current]);
    return kit.PanelCard(
      padding: EdgeInsets.zero,
      child: LayoutBuilder(builder: (context, box) {
        final compact = box.maxHeight < 260;
        final hasTabsBar = !_playing && _showTabs && names.length > 1;
        final singlePageTitle = !hasTabsBar && !_playing ? _pageTitle(names[current]) : '';
        final showHeaderRow = !hasTabsBar;
        final closeBtn = kit.Tap(
          label: 'Close properties',
          minSize: 32,
          onTap: () => setState(() => _panelOpen = false),
          child: Container(
            width: 28,
            height: 28,
            decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(8)),
            alignment: Alignment.center,
            child: const Pictogram('x', size: 13, color: cSecondary, stroke: 2.1),
          ),
        );
        final footBtn = foot == null
            ? null
            : GestureDetector(onLongPress: foot.$3, child: kit.WordButton(foot.$1, onTap: foot.$2, filled: true, danger: true, expand: true, height: 36));
        return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          if (hasTabsBar)
            Padding(
              padding: EdgeInsets.fromLTRB(compact ? 8 : 10, compact ? 6 : 10, compact ? 8 : 10, 0),
              child: Row(children: [
                Expanded(
                  child: names.length <= 3
                      ? kit.Segmented([for (final n in names) _tabLabel(n)], current, onSelect: (i) => _jumpTo(names[i]))
                      : SingleChildScrollView(
                          scrollDirection: Axis.horizontal,
                          child: Row(children: [
                            for (var i = 0; i < names.length; i++)
                              Container(
                                margin: const EdgeInsets.only(right: 2),
                                decoration: i == current ? BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(9)) : null,
                                child: kit.WordButton(_tabLabel(names[i]), size: 13, height: 32, onTap: () => _jumpTo(names[i])),
                              ),
                          ]),
                        ),
                ),
                const SizedBox(width: 6),
                closeBtn,
              ]),
            )
          else if (showHeaderRow)
            Padding(
              padding: const EdgeInsets.fromLTRB(14, 8, 10, 0),
              child: Row(children: [
                if (singlePageTitle.isNotEmpty)
                  Expanded(child: Text(singlePageTitle, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w700)))
                else
                  const Spacer(),
                closeBtn,
              ]),
            ),
          Expanded(
            child: _playing
                ? ListView(padding: const EdgeInsets.fromLTRB(14, 10, 14, 10), children: _playbackPanel())
                : RawScrollbar(
                    controller: _panelScroll,
                    thumbVisibility: true,
                    thickness: 3.5,
                    radius: const Radius.circular(2),
                    thumbColor: const Color(0x556B7078),
                    padding: const EdgeInsets.only(top: 4, bottom: 4, right: 2),
                    child: SingleChildScrollView(
                      controller: _panelScroll,
                      padding: EdgeInsets.fromLTRB(14, compact ? 2 : 4, 14, compact ? 12 : 20),
                      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                        // one page per tab (round 10 boards); without tabs, the pages stack
                        for (final n in (_showTabs && names.length > 1 ? [names[current]] : names)) ...[
                          if (_pageTitle(n).isNotEmpty && !(showHeaderRow && singlePageTitle.isNotEmpty))
                            Container(key: _sectionKey(n), width: double.infinity, padding: EdgeInsets.only(top: compact ? 4 : 10, bottom: 4), child: Text(_pageTitle(n), textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w600)))
                          else
                            SizedBox(key: _sectionKey(n), height: compact ? 2 : 6),
                          ..._sectionBody(n),
                          const SizedBox(height: 10),
                        ],
                        if (compact && footBtn != null) ...[
                          const Divider(height: 14, color: Color(0xFFE6E8EB)),
                          footBtn,
                        ],
                      ]),
                    ),
                  ),
          ),
          // the page's pinned button (round 10 boards): Delete stickfigure,
          // Reset this segment, Delete this pose, Clear frame, ...
          if (!compact && footBtn != null)
            Container(
              decoration: const BoxDecoration(
                color: cCard,
                border: Border(top: BorderSide(color: Color(0xFFE0E3E8))),
                boxShadow: [BoxShadow(color: Color(0x0A000000), blurRadius: 6, offset: Offset(0, -2))],
              ),
              padding: const EdgeInsets.fromLTRB(14, 8, 14, 8),
              child: footBtn,
            ),
          // export progress (until the Export sheet shows it, B5)
          if (_status.isNotEmpty)
            Container(
              color: cControl,
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
              child: Text(_status, maxLines: 2, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cSecondary, fontSize: 12)),
            ),
        ]);
      }),
    );
  }

  /// A page's red title in the Properties card (round 10 boards). The
  /// Stickfigure page opens with the figure's name instead.
  String _pageTitle(String section) => switch (section) {
        'Stickfigure Tools' => '',
        'Pose' => 'Pose Tools',
        'Frame Tools' => 'Frame ${_frame + 1}',
        'Textfield Tools' => 'Text Tools',
        'Figure editor' => 'Figure Tools',
        'Selection' => '${((_st['multi'] as List?) ?? const []).length} figures',
        'Group Tools' => '',
        'Script' => '',
        'Value' => '',
        _ => section,
      };

  /// A page's pinned button: (label, tap, hold).
  (String, VoidCallback, VoidCallback?)? _pageFoot(String section) {
    final sel = _sel;
    switch (section) {
      case 'Stickfigure Tools' when sel != null:
        return (
          'Delete stickfigure',
          () {
            final name = sel['name'] ?? 'Figure';
            _cmd({'op': 'delete_everywhere', 'layer': sel['layer']});
            if (mounted) kit.showToast(context, 'Deleted $name.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          },
          () => _cmd({'op': 'delete_here', 'joined': true}),
        );
      case 'Segment Tools' when sel != null && _segAnyChanged(sel):
        return (
          sel['defaults'] is Map ? 'Reset this segment' : 'Reset thickness and length',
          () {
            if (sel['defaults'] is Map) {
              _cmd({'op': 'reset_segment'});
            } else {
              final d = _segDefaults(sel);
              _cmd({'op': 'segment', 'thickness': d['thickness']});
              if (d['length'] != null) _cmd({'op': 'node', 'length': d['length']});
            }
          },
          null
        );
      case 'Pose' when sel != null && ((sel['pose'] as Map?)?['start'] as num? ?? 0) > 0:
        return ('Delete this pose', () => _cmd({'op': 'delete_pose'}), null);
      case 'Frame Tools':
        return (
          'Clear frame',
          () async {
            final ok = await _ask('Clear this frame?', 'Every figure and textfield on this frame is taken off it. The frame itself stays. Undo brings them back.', ['Cancel', 'Clear frame']);
            if (ok == 'Clear frame') _cmd({'op': 'clear_frame'});
          },
          null
        );
      case 'Picture Tools' when _st['picture'] != null:
        return (
          'Delete picture',
          () {
            final pic = _st['picture'] as Map;
            _cmd({'op': 'delete_everywhere', 'layer': pic['layer']});
            if (mounted) kit.showToast(context, 'Deleted ${pic['name'] ?? 'picture'}.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          },
          null,
        );
      case 'Effect Layer' when _st['adjustment'] != null:
        return (
          'Delete effect layer',
          () {
            final adj = _st['adjustment'] as Map;
            _cmd({'op': 'delete_everywhere', 'layer': adj['layer']});
            if (mounted) kit.showToast(context, 'Deleted ${adj['name'] ?? 'Effect layer'}.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          },
          null,
        );
      case 'Script' when _st['script'] != null:
        return ('Delete script', () => _cmd({'op': 'script_delete'}), null);
      case 'Group Tools' when _st['group'] != null:
        final g = _st['group'] as Map;
        return (
          'Delete group',
          () async {
            final n = ((g['members'] as List?) ?? const []).length;
            final ok = await _ask('Delete ${g['name']}?', 'The group and the ${n == 1 ? 'layer' : '$n layers'} in it are deleted from every frame. Undo brings them back.', ['Cancel', 'Delete group']);
            if (ok == 'Delete group') _cmd({'op': 'delete_group', 'group': g['id']});
          },
          null
        );
      case 'Sound Tools' when _st['frameSound'] != null:
        return ('Delete sound', () => _cmd({'op': 'frame_sound', 'asset': null}), null);
      case 'Textfield Tools' when _st['textfield'] != null:
        return ('Delete text', () => _cmd({'op': 'delete_textfield'}), null);
      case 'Movieclip Tools' when _st['movieclip'] != null:
        return (
          'Delete movieclip',
          () {
            final mc = _st['movieclip'] as Map;
            _cmd({'op': 'delete_everywhere', 'layer': mc['layer']});
            if (mounted) kit.showToast(context, 'Deleted ${mc['name'] ?? 'movieclip'}.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          },
          null,
        );
    }
    return null;
  }

  double _fitScale() {
    final size = _view.size;
    if (size.isEmpty || _st.isEmpty) return 0;
    final sw = (_st['w'] as num).toDouble(), sh = (_st['h'] as num).toDouble();
    return math.min(size.width / (sw * 1.08), size.height / (sh * 1.08));
  }

  List<Widget> _playbackPanel() {
    return [
      _title('Playing'),
      _hint('The animation settings are under "fps" in the timeline, once playback stops. Playback in the editor can stutter on big animations; exports never do.'),
    ];
  }

  Timer? _reminder;
  DateTime _savedAt = DateTime.now();
  bool _reminding = false;

  /// "Remember to save": every few minutes (Settings) while there is unsaved work.
  /// "Save your work?" (REDESIGN §8): Later, Save as…, Save. Returns 0, 2 or 1.
  Future<int?> _saveSheet(String text) => kit.showSheet<int>(context, title: 'Save your work?', width: 420, children: (c) => [
        kit.Hint(text),
        Row(children: [
          Expanded(child: kit.WordButton('Later', onTap: () => Navigator.pop(c, 0), filled: true, expand: true, height: 38)),
          const SizedBox(width: 8),
          Expanded(child: kit.WordButton('Save as…', onTap: () => Navigator.pop(c, 2), filled: true, expand: true, height: 38)),
          const SizedBox(width: 8),
          Expanded(child: kit.WordButton('Save', onTap: () => Navigator.pop(c, 1), primary: true, expand: true, height: 38)),
        ]),
      ]);

  Future<void> _remindToSave() async {
    final mins = AppSettings.number('saveReminder');
    if (mins <= 0 || _reminding || _playing || _st['dirty'] != true || !mounted) return;
    final since = DateTime.now().difference(_savedAt).inMinutes;
    if (since < mins) return;
    // only over the editor itself, never on top of another window
    if (ModalRoute.of(context)?.isCurrent != true) return;
    _reminding = true;
    final pick = await _saveSheet('${_file?.name ?? 'This animation'} hasn\u2019t been saved for $since minutes. The reminder can be changed or turned off in Settings.');
    _reminding = false;
    // asked again after another interval either way
    _savedAt = DateTime.now();
    if (pick == 1) await _save();
    if (pick == 2) await _saveAs();
  }

  /// The Settings screen (REDESIGN §8): two cards of rows. Changes apply at
  /// once and are kept for next time.
  Future<void> _settingsDialog() async {
    final e = _engine;
    await Navigator.of(context).push(PageRouteBuilder<void>(
      transitionDuration: const Duration(milliseconds: 120),
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        void put(String k, Object v) {
          set(() => AppSettings.values[k] = v);
          setState(() {});
          unawaited(AppSettings.save());
          if (k == 'axisLock') _cmd({'op': 'set', 'k': 'axis', 'v': v});
        }

        final nerd = AppSettings.flag('nerd');
        List<rw.Desc> toggle(String k, String label, String help) => [
              rw.DTick(label, AppSettings.flag(k), onChanged: (v) => put(k, v)),
              if (!nerd) rw.DHint(help),
            ];
        rw.Desc number(String k, String label, int min, int max, int step) => rw.DStep(label, '${AppSettings.number(k)}',
            onMinus: AppSettings.number(k) > min ? () => put(k, math.max(min, AppSettings.number(k) - step)) : null,
            onPlus: AppSettings.number(k) < max ? () => put(k, math.min(max, AppSettings.number(k) + step)) : null);
        Widget card(List<rw.Desc> rows) => Container(
              decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(16), boxShadow: const [BoxShadow(color: Color(0x0F000000), spreadRadius: 1), BoxShadow(color: Color(0x1410141A), blurRadius: 8, offset: Offset(0, 2))]),
              padding: const EdgeInsets.fromLTRB(18, 4, 18, 12),
              child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [for (final d in rows) rw.renderDesc(d)]),
            );
        final left = card([
          const rw.DTitle('Screen'),
          // the Settings board's UI size: 75% to 150%
          rw.DSlider('UI size', '${(AppSettings.uiScale.value * 100).round()}%', (AppSettings.uiScale.value - 0.75) / 0.75, onChanged: (x) {
            final k = ((0.75 + x * 0.75) * 20).round() / 20;
            AppSettings.values['uiScale'] = k;
            AppSettings.uiScale.value = k;
            unawaited(AppSettings.save());
            set(() {});
          }),
          number('padLeft', 'Padding on the left', 0, 120, 4),
          number('padRight', 'Padding on the right', 0, 120, 4),
          if (!nerd) const rw.DHint('Padding keeps controls clear of notches and rounded corners.'),
          ...toggle('leftHand', 'Left-hand mode', 'The tools move to the other side of the screen.'),
          const rw.DTitle('Joints'),
          rw.DSeg(const ['Legacy', 'Modern', 'Solid'], AppSettings.number('nodeStyle').clamp(0, 2), label: 'Joint style', onSelect: (i) => put('nodeStyle', i)),
          number('lineWidth', 'Line width', 1, 5, 1),
          const rw.DTitle('Saving'),
          number('saveReminder', 'Save reminder, minutes', 0, 120, 1),
          if (!nerd) const rw.DHint('A reminder after this many minutes of unsaved work; 0 turns it off.'),
        ]);
        final middle = card([
          const rw.DTitle('Editing'),
          ...toggle('axisLock', 'Axis locking', 'Drags of a figure or the camera keep to a straight line when they nearly follow one.'),
          ...toggle('keepSelection', 'Keep the selection when changing frames', 'Picking another frame leaves the selected figure selected.'),
          ...toggle('stylus', 'Precise touches (stylus)', 'Joints have to be touched more exactly: for a stylus rather than a finger.'),
          ...toggle('haptics', 'Haptic feedback', 'A light tick when you grab a joint, on devices that have it.'),
          ...toggle('doubleTapMovieclip', 'Double-tap a figure to make a movieclip', 'Two quick taps on a joint ask whether to convert the figure.'),
          ...toggle('doubleTapNone', 'Double-tap to select', 'In the figure editor, a double tap on the stage selects instead of fitting the view.'),
          ...toggle('nerd', 'Fewer explanations', 'Hides most help text once you know your way around.'),
        ]);
        final right = card([
          const rw.DTitle('This device'),
          rw.DInfo('Graphics', e?.adapter ?? '?'),
          rw.DInfo('Sound', e?.audioDevice ?? '?'),
          const rw.DTitle('About'),
          const rw.DInfo('Version', '0.1.0'),
          const rw.DHint('Barkfart makes no network requests: no ads, no tracking, nothing sent anywhere.'),
          rw.DLink('Licences', '', onTap: () => showLicensePage(context: c, applicationName: 'Barkfart')),
          rw.DLink('Backups', '', onTap: () async {
            final f = await _backupsSheet();
            if (f != null && mounted) await _openFile(f);
          }),
        ]);
        final wide = MediaQuery.sizeOf(c).width > 900;
        return Scaffold(
          backgroundColor: cPanel,
          body: SafeArea(
            child: Column(children: [
              Container(
                height: 44,
                width: double.infinity,
                decoration: const BoxDecoration(color: cCard, border: Border(bottom: BorderSide(color: cLine))),
                child: Stack(alignment: Alignment.center, children: [
                  const Text('Settings', style: TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w700)),
                  Positioned(left: 4, child: kit.WordButton('‹ Back', onTap: () => Navigator.pop(c))),
                ]),
              ),
              Expanded(
                child: ListView(padding: const EdgeInsets.all(20), children: [
                  if (wide)
                    Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                      Expanded(child: left),
                      const SizedBox(width: 16),
                      Expanded(child: middle),
                      const SizedBox(width: 16),
                      Expanded(child: right),
                    ])
                  else ...[
                    Row(crossAxisAlignment: CrossAxisAlignment.start, children: [Expanded(child: left), const SizedBox(width: 16), Expanded(child: right)]),
                    const SizedBox(height: 16),
                    middle,
                  ],
                ]),
              ),
            ]),
          ),
        );
      }),
    ));
  }

  List<Widget> _projectTools() => [
        Text(_file?.name ?? '(Not saved yet)', style: const TextStyle(color: cSecondary, fontSize: 12)),
        _gap,
        _pair(_Btn('Save', onTap: _save), _Btn('Save as…', onTap: _saveAs)),
        _gap,
        _Btn('Export…', onTap: _exportDialog),
      ];

  bool _vertical = false;

  final GlobalKey _fpsKey = GlobalKey();

  /// The Animation popover (REDESIGN §8), from the toolbar's "fps": every
  /// setting of the old Animation Tools, as rows.
  Future<void> _animationPopover() async {
    final screen = MediaQuery.sizeOf(context);
    final anchor = kit.Anchored.rectOf(_fpsKey) ?? Rect.fromLTWH(screen.width - 80, screen.height - 90, 70, 32);
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Animation',
      barrierColor: const Color(0x00000000),
      transitionDuration: kit.motion(context, 150),
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        void cmd(Map<String, Object?> m) {
          _cmd(m);
          set(() {});
        }

        final tween = _st['tweening'] == true;
        final n = (_st['tweenedFrames'] as num?)?.toInt() ?? 5;
        final rate = (_st['rate'] as num?)?.toDouble() ?? _fps;
        // On short landscape screens (e.g. 844x390 phone), lay out in two columns
        // so all rows stay visible without clipping or covering the top bar.
        final wideCompact = screen.height < 500 && screen.width >= 600;
        final leftRows = <rw.Desc>[
          rw.DStep(wideCompact ? 'Frame rate' : 'Frames per second', _fpsLabel, onMinus: _fps > 1 ? () => cmd({'op': 'fps', 'v': _fps - 1}) : null, onPlus: () => cmd({'op': 'fps', 'v': _fps + 1})),
          rw.DTick('Tweening', tween, onChanged: (v) => cmd({'op': 'animation', 'tweening': v})),
          rw.DStep('Extra smoothness', '$n', onMinus: tween && n > 0 ? () => cmd({'op': 'animation', 'tweened': n - 1}) : null, onPlus: tween ? () => cmd({'op': 'animation', 'tweened': n + 1}) : null),
          rw.DTick('Loop back to the start', _st['loopTween'] == true, onChanged: tween ? (v) => cmd({'op': 'animation', 'loop': v}) : null),
        ];
        final rightRows = <rw.Desc>[
          rw.DTick('Effects while playing', AppSettings.flag('playFilters'), onChanged: (v) {
            AppSettings.values['playFilters'] = v;
            unawaited(AppSettings.save());
            set(() {});
            setState(() {});
          }),
          rw.DTick('Fullscreen when playing', _fullscreen, onChanged: (v) {
            set(() {});
            setState(() => _fullscreen = v);
          }),
          rw.DHint(tween
              ? 'In-between pictures added between frames when playing and exporting (${rate.round()} a second). Your poses stay the same.'
              : 'Each frame shows for 1/${_fps.round()} s. Tweening on adds in-between pictures; your poses stay the same.'),
        ];
        final topMin = MediaQuery.paddingOf(c).top + 44;
        return Stack(children: [
          kit.Anchored(
            anchor: anchor,
            width: wideCompact ? math.min(560.0, screen.width - 24) : 320,
            alignRight: true,
            topMin: anchor.top > topMin + 110 ? topMin : 0,
            card: (w) => kit.PanelCard(
              width: w,
              padding: wideCompact ? const EdgeInsets.fromLTRB(12, 6, 12, 8) : const EdgeInsets.fromLTRB(14, 10, 14, 10),
              child: wideCompact
                  ? Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                      Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                        Expanded(flex: 11, child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [for (final d in leftRows) rw.renderDesc(d)])),
                        const SizedBox(width: 14),
                        Expanded(flex: 10, child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [for (final d in rightRows) rw.renderDesc(d)])),
                      ]),
                    ])
                  : Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                      rw.renderDesc(const rw.DTitle('Animation')),
                      for (final d in [...leftRows, ...rightRows]) rw.renderDesc(d),
                    ]),
            ),
          ),
        ]);
      }),
    );
  }

  /// Library Tools type: 0 stickfigures, 1 movieclips, 2 sprites (the
  /// reference's [<] / [>] switcher).
  int _libType = 0;
  static const _libTypes = ['Stickfigure', 'Movieclip', 'Sprite'];
  static const _libKinds = ['figure', 'movieclip', 'sprite'];

  List<Widget> _libraryTools() {
    final type = _libTypes[_libType];
    return [
      Row(children: [
        SizedBox(width: 40, child: _Btn('<', onTap: () => setState(() => _libType = (_libType + 2) % 3))),
        Expanded(child: Center(child: Text('${type}s', style: const TextStyle(color: cText, fontSize: 13)))),
        SizedBox(width: 40, child: _Btn('>', onTap: () => setState(() => _libType = (_libType + 1) % 3))),
      ]),
      _gap,
      if (_libType == 0) GestureDetector(onLongPress: _pasteLayeringPopup, child: _Btn('Paste stickfigure', enabled: _st['figureCopied'] == true, onTap: _pasteFigure)),
      if (_libType == 0) _gap,
      Row(children: [
        Expanded(
          child: _libType == 2
              ? _Btn('Create sprite…', accent: true, onTap: _importImages)
              : _Btn(_libType == 0 ? 'Add stickfigure…' : 'Add movieclip…', accent: true, onTap: _figurePicker),
        ),
        const SizedBox(width: 8),
        SizedBox(width: 44, child: _Btn('', icon: Icons.edit, onTap: _libraryDialog)),
      ]),
      _hint(_libType == 2 ? 'Put pictures in the Barkfart Images folder; each becomes a sprite on this frame.' : 'The pencil lists the ${type.toLowerCase()}s in this project: rename them, or delete one from every frame.'),
      _gap,
      _Btn('Import from Files…', onTap: _importFromFiles),
      _hint('Stickfigures, movieclips and Stick Nodes projects (.nodes, .nodemc, .stknds), sounds and pictures from anywhere: iCloud, Downloads, other apps.'),
      if (_libType == 1 && _st['editingClip'] == null) ...[
        _gap,
        _Btn('Create new movieclip ›', onTap: () => _cmd({'op': 'new_movieclip'})),
      ],
    ];
  }

  /// Fonts that ship with Barkfart (the engine's BUNDLED list, same names).
  static const _bundledFonts = ['Bangers', 'Permanent Marker', 'Press Start 2P', 'Lobster'];

  /// Textfield Tools: the font (default, bundled or imported into the
  /// project) and Import font... (.ttf / .otf from Files).
  List<Widget> _fontChoice(Map<String, dynamic> tf) {
    final cur = tf['font'] as String?;
    final project = ((_st['projectFonts'] as List?) ?? const []).cast<Map>();
    return [
      Wrap(spacing: 4, runSpacing: 4, children: [
        _Chip('Default', cur == null, () => _cmd({'op': 'textfield', 'font': null})),
        for (final f in _bundledFonts) _Chip(f, cur == f, () => _cmd({'op': 'textfield', 'font': f})),
        for (final f in project) _Chip('${f['name']}', cur == f['key'], () => _cmd({'op': 'textfield', 'font': f['key']})),
      ]),
      const SizedBox(height: 6),
      _Btn('Import font…', small: true, onTap: _importFont),
      _hint('A .ttf or .otf font from Files; it is kept in this project.'),
    ];
  }

  Future<void> _importFont() async {
    final e = _engine;
    if (e == null) return;
    List<PlatformFile> picked;
    try {
      picked = await FilePicker.pickFiles(dialogTitle: 'Import a font', type: FileType.custom, allowedExtensions: const ['ttf', 'otf']);
    } catch (err) {
      _say('Could not open the file picker: $err');
      return;
    }
    for (final f in picked.take(1)) {
      final st = e.importFile(f.name, await f.readAsBytes());
      if (st == null) {
        _say('${f.name}: ${e.lastError}');
      } else {
        _refresh(st);
      }
    }
  }

  /// The system file picker (on iOS the Files browser): every file it gives
  /// goes through the same import as files from Barkfart's folders.
  Future<void> _importFromFiles() async {
    final e = _engine;
    if (e == null) return;
    List<PlatformFile> picked;
    try {
      picked = await FilePicker.pickFiles(dialogTitle: 'Import into Barkfart');
    } catch (err) {
      _say('Could not open the file picker: $err');
      return;
    }
    var done = 0;
    for (final f in picked) {
      final st = e.importFile(f.name, await f.readAsBytes());
      if (st == null) {
        _say('${f.name}: ${e.lastError}');
      } else {
        _refresh(st);
        done++;
      }
    }
    if (done > 0) {
      _thumbsChanged('import');
      _say(done == 1 ? 'Imported ${picked.first.name}' : 'Imported $done files', light: true);
    }
  }

  Future<void> _importImages() async {
    final e = _engine;
    if (e == null) return;
    final files = await imageFiles();
    if (!mounted) return;
    final chosen = <String>{};
    final ok = await showDialog<bool>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) => AlertDialog(
          backgroundColor: cPanel,
          title: const Text('Create sprites', style: TextStyle(color: cText, fontSize: 16)),
          content: SizedBox(
            width: 380,
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              const Text('Put pictures (png, jpg, gif, webp) in the Barkfart Images folder, then pick them here. Each becomes a sprite on this frame.', style: TextStyle(color: cSecondary, fontSize: 12)),
              const SizedBox(height: 8),
              if (files.isEmpty) const Padding(padding: EdgeInsets.all(12), child: Text('No pictures in the folder yet.', style: TextStyle(color: cText))),
              Flexible(
                child: ListView(shrinkWrap: true, children: [
                  for (final f in files)
                    CheckboxListTile(
                      dense: true,
                      value: chosen.contains(f.path),
                      onChanged: (v) => set(() => v == true ? chosen.add(f.path) : chosen.remove(f.path)),
                      title: Text(f.path.split(Platform.pathSeparator).last, style: const TextStyle(color: cText, fontSize: 13)),
                    ),
                ]),
              ),
            ]),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(c, false), child: const Text('Cancel')),
            TextButton(onPressed: chosen.isEmpty ? null : () => Navigator.pop(c, true), child: Text('Create (${chosen.length})')),
          ],
        ),
      ),
    );
    if (ok != true) return;
    for (final path in chosen) {
      _refresh(e.importFile(path.split(Platform.pathSeparator).last, await File(path).readAsBytes()));
    }
    _thumbsChanged('import');
  }

  /// The pencil: this project's figures / movieclips / sprites, with the
  /// frames each is on; rename, or delete from every frame.
  Future<void> _libraryDialog() => showDialog<void>(
        context: context,
        builder: (c) => StatefulBuilder(builder: (c, set) {
          final kind = _libKinds[_libType];
          final items = ((_st['library'] as List?) ?? const []).cast<Map>().where((m) => m['kind'] == kind).toList();
          return AlertDialog(
            backgroundColor: cPanel,
            title: Text('${_libTypes[_libType]}s in this project', style: const TextStyle(color: cText, fontSize: 16)),
            content: SizedBox(
              width: 420,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                const Text('Rename one, or delete it from every frame of the project. Deleting unused ones keeps the project small; undo brings one back.', style: TextStyle(color: cSecondary, fontSize: 12)),
                const SizedBox(height: 8),
                if (items.isEmpty) Padding(padding: const EdgeInsets.all(12), child: Text('No ${_libTypes[_libType].toLowerCase()}s yet.', style: const TextStyle(color: cText))),
                Flexible(
                  child: ListView(shrinkWrap: true, children: [
                    for (final m in items)
                      Row(children: [
                        Expanded(
                          child: Text('${m['name']}  ·  ${m['frames']} frame${m['frames'] == 1 ? '' : 's'}${kind == 'figure' ? '  ·  ${m['nodes']} nodes' : ''}',
                              style: const TextStyle(color: cText, fontSize: 13), overflow: TextOverflow.ellipsis),
                        ),
                        if (kind == 'movieclip')
                          IconButton(
                            tooltip: 'Break apart into figures (posed as on this frame)',
                            icon: const Icon(Icons.call_split, size: 18, color: cText),
                            onPressed: () {
                              _cmd({'op': 'break_apart', 'layer': m['layer']});
                              set(() {});
                            },
                          ),
                        IconButton(
                          tooltip: 'Rename',
                          icon: const Icon(Icons.edit, size: 18, color: cText),
                          onPressed: () async {
                            final n = await _askName('Rename', '${m['name']}');
                            if (n == null || n.trim().isEmpty) return;
                            _cmd({'op': 'rename', 'layer': m['layer'], 'name': n.trim()});
                            set(() {});
                          },
                        ),
                        IconButton(
                          tooltip: 'Delete from the project',
                          icon: const Icon(Icons.close, size: 18, color: cDanger),
                          onPressed: () async {
                            final used = (m['frames'] as num) > 0, joined = (m['joined'] as num) > 0;
                            final ok = await _ask(
                                'Are you sure?',
                                'Name: ${m['name']}\n'
                                    '${kind == 'figure' ? 'Nodes: ${m['nodes']}\n' : ''}'
                                    '${used ? 'On ${m['frames']} frame${m['frames'] == 1 ? '' : 's'}: it disappears from all of them.' : 'Not on any frame: safe to delete.'}'
                                    '${joined ? '\n${m['joined']} figure(s) joined to it stay where they are, unjoined.' : ''}',
                                ['Cancel', 'Delete']);
                            if (ok != 'Delete') return;
                            _cmd({'op': 'library_delete', 'layer': m['layer']});
                            set(() {});
                          },
                        ),
                      ]),
                  ]),
                ),
              ]),
            ),
            actions: [TextButton(onPressed: () => Navigator.pop(c), child: const Text('Okay'))],
          );
        }),
      );

  List<Widget> _frameToolsPanel() {
    final last = _frame >= _frames - 1;
    final both = {'50': [0.5], '33/67': [1 / 3, 2 / 3], '10/90': [0.1, 0.9]}[_tweenAt]!;
    final at = both.length == 2 && _tweenPick > 0 ? [both[_tweenPick - 1]] : both;
    final label = at.map((f) => '${(f * 100).round()}%').join(', ');
    final bg = (_st['frameBg'] as Map<String, dynamic>?) ?? const <String, dynamic>{};
    final bgColor = (bg['color'] as num?)?.toInt() ?? 0xffffffff;
    final bgTop = (bg['gradient'] as num?)?.toInt() ?? 0x808080ff;
    final bgGradient = bg['useGradient'] == true;
    final delay = (_st['frameDelay'] as num?)?.toDouble() ?? 0;
    final slow = (_st['frameSlow'] as num?)?.toInt() ?? 0;
    final rep = (_st['frameRepeat'] as Map<String, dynamic>?) ?? const <String, dynamic>{};
    final back = (rep['back'] as num?)?.toInt() ?? 0;
    final times = math.max(1, (rep['times'] as num?)?.toInt() ?? 1);
    VoidCallback? when(bool ok, VoidCallback f) => ok ? f : null;
    final tweening = _st['tweening'] == true;
    final rows = <rw.Desc>[
      rw.DHint('Changes here apply to every layer at this moment. (Frame ${_frame + 1} of $_frames, ${_st['nodeCount'] ?? 0} nodes.)'),
      if (_st['figureCopied'] == true || _st['textfieldCopied'] == true)
        rw.DButtons(
          _st['figureCopied'] == true ? 'Paste figure' : 'Paste text',
          _st['figureCopied'] == true ? _pasteFigure : () => _cmd({'op': 'paste_textfield'}),
          b: _st['figureCopied'] == true && _st['textfieldCopied'] == true ? 'Paste text' : null,
          onB: () => _cmd({'op': 'paste_textfield'}),
        ),
      rw.DButtons('Copy frame', () => _cmd({'op': 'copy_frame'}), b: 'Paste frame', onB: when(_st['frameCopied'] == true, () => _cmd({'op': 'paste_frame'}))),
      rw.DButtons('Add time here', () => _cmd({'op': 'add_time'}), b: 'Remove this frame', onB: when(_frames > 1, () => _cmd({'op': 'delete_frame'}))),
      rw.DSeg(const ['Halfway', 'A third in', 'A tenth in'], ['50', '33/67', '10/90'].indexOf(_tweenAt), label: 'Add an in-between', onSelect: (i) => setState(() {
            final v = ['50', '33/67', '10/90'][i];
            // tapping the chosen pair again: both, then the first, then the second
            _tweenPick = v == _tweenAt && v != '50' ? (_tweenPick + 1) % 3 : 0;
            _tweenAt = v;
          })),
      rw.DWidget(GestureDetector(
        onLongPress: last ? null : _tweenCountPopup,
        child: rw.renderDesc(rw.DButtons(at.length == 1 && _tweenAt == '50' ? 'Add an in-between frame' : 'Add in-between frames ($label)', when(!last, () => _cmd({'op': 'tween_add', 'at': at})))),
      )),
      rw.DWidget(Align(alignment: Alignment.centerRight, child: kit.WordButton('How in-betweens work', size: 12, onTap: _tweenHelp))),
      rw.DHint(last
          ? 'Tweened frames go between two frames.'
          : '${_tweenAt != '50' ? 'Tap ${_tweenAt.replaceAll('/', ' / ')} again to add only one of the two frames. ' : ''}Hold the button to add several evenly spaced frames.'),
      const rw.DTitle('Background'),
      rw.DWidget(kit.PropRow('Colour', trailing: _swatches(bgColor, (c) => _cmd({'op': 'frame_bg', 'color': c})))),
      rw.DTick('Use gradient', bgGradient, onChanged: (v) => _cmd({'op': 'frame_bg', 'useGradient': v})),
      if (bgGradient) ...[
        rw.DWidget(kit.PropRow('Top colour', trailing: _swatches(bgTop, (c) => _cmd({'op': 'frame_bg', 'gradient': c})))),
        rw.DTick('Expand gradient', (_st['frameBg'] as Map?)?['expand'] == true, onChanged: (v) => _cmd({'op': 'frame_bg', 'expand': v})),
        const rw.DHint('Stretches the gradient taller: most of the stage is the gradient colour.'),
      ],
      const rw.DTitle('Timing'),
      rw.DTick('Do not tween this frame', _st['frameNoTween'] == true, onChanged: (v) => _cmd({'op': 'no_tween', 'on': v})),
      // slow motion: only while the project tweens
      rw.DTick('Slow motion', slow > 0, onChanged: tweening ? (v) => _cmd({'op': 'frame_slow', 'n': v ? 10 : 0}) : null),
      if (slow > 0) ...[
        rw.DStep('In-between pictures', '$slow', onMinus: when(tweening, () => _cmd({'op': 'frame_slow', 'n': math.max(1, slow - 1)})), onPlus: when(tweening, () => _cmd({'op': 'frame_slow', 'n': slow + 1}))),
        const rw.DHint('More in-between pictures for this one frame, so it plays slower.'),
      ],
      rw.DTick('Pause here', delay > 0, onChanged: (v) => _cmd({'op': 'frame_delay', 'seconds': v ? 0.5 : 0})),
      if (delay > 0) rw.DStep('Seconds', delay.toStringAsFixed(2), onMinus: () => _cmd({'op': 'frame_delay', 'seconds': math.max(0.01, delay - 0.05)}), onPlus: () => _cmd({'op': 'frame_delay', 'seconds': delay + 0.05})),
      if (_frame > 0) rw.DTick('Repeat the frames before this', back > 0, onChanged: (v) => _cmd({'op': 'frame_repeat', 'back': v ? 1 : 0, 'times': 1})),
      if (back > 0) ...[
        const rw.DHint('After this frame, playback jumps back and plays the series again: a short loop without a movieclip.'),
        rw.DStep('Back', '$back', onMinus: () => _cmd({'op': 'frame_repeat', 'back': math.max(1, back - 1), 'times': times}), onPlus: () => _cmd({'op': 'frame_repeat', 'back': math.min(_frame, back + 1), 'times': times})),
        rw.DStep('Times', '$times', onMinus: () => _cmd({'op': 'frame_repeat', 'back': back, 'times': math.max(1, times - 1)}), onPlus: () => _cmd({'op': 'frame_repeat', 'back': back, 'times': times + 1})),
        rw.DTick('Use tweening during repeat', (_st['frameRepeat'] as Map?)?['tween'] == true, onChanged: (v) => _cmd({'op': 'frame_repeat', 'back': back, 'times': times, 'tween': v})),
        const rw.DHint('On: this frame tweens into the frame it jumps back to. Off: it holds until the jump.'),
      ],
      rw.DTick('Silence all sounds', _st['frameSilence'] == true, onChanged: (v) => _cmd({'op': 'frame_silence', 'on': v})),
      const rw.DHint('Sounds still playing stop when this frame starts.'),
      if (_frames > 1) ...[
        const rw.DTitle('Range'),
        rw.DButtons(
          'Reverse all',
          () => _cmd({'op': 'reverse_range', 'from': 0, 'to': _frames - 1}),
          b: 'Ping-pong',
          onB: () => _cmd({'op': 'ping_pong_range', 'from': 0, 'to': _frames - 1}),
        ),
        rw.DHint('Reverse flips frames 1 to $_frames in place. Ping-pong appends the animation playing back to the start.'),
      ],
    ];
    // a frame's sound has its own page (the Sound board): tap its block
    final snd = _st['frameSound'] as Map?;
    return [
      for (final d in rows) rw.renderDesc(d),
      rw.renderDesc(const rw.DTitle('Sound')),
      rw.renderDesc(snd != null
          ? rw.DLink('This frame plays', '${snd['name']}', onTap: () => _pickSound())
          : rw.DButtons('Add a sound here…', _importSounds)),
    ];
  }


  /// Frame Tools > Sounds: the sound this frame starts, and its settings.
  /// The sound this frame plays, as a page of rows (D15; REDESIGN §8 Sound page).
  List<Widget> _soundsBlock() {
    final list = ((_st['soundList'] as List?) ?? const []).cast<Map<String, dynamic>>();
    final snd = _st['frameSound'] as Map<String, dynamic>?;
    if (snd == null) return const [];
    double v(String k, double d) => (snd[k] as num?)?.toDouble() ?? d;
    void set(Map<String, Object?> m) => _cmd({'op': 'frame_sound', ...m});
    final vol = v('volume', 1), pan = v('pan', 0), pitch = v('pitch', 1), fin = v('fadeIn', 0), fout = v('fadeOut', 0);
    // the Sound board, top to bottom
    final rows = <rw.Desc>[
      rw.DLink('Sound', snd['name'] as String, onTap: () => _pickFrameSound(list, snd['asset'] as String?)),
      rw.DSlider('Volume', '${(vol * 100).round()}%', vol, onChanged: (x) => set({'volume': (x * 100).round() / 100})),
      rw.DSlider('Pan', pan.abs() < 0.05 ? 'Centre' : pan < 0 ? 'Left ${(-pan * 100).round()}' : 'Right ${(pan * 100).round()}', (pan + 1) / 2, onChanged: (x) => set({'pan': ((x * 2 - 1) * 20).round() / 20})),
      rw.DStep('Pitch', pitch.toStringAsFixed(2), onMinus: pitch > 0.25 ? () => set({'pitch': math.max(0.25, pitch - 0.05)}) : null, onPlus: pitch < 4 ? () => set({'pitch': math.min(4, pitch + 0.05)}) : null),
      rw.DStep('Fade in', '${fin.toStringAsFixed(1)} s', onMinus: fin > 0 ? () => set({'fadeIn': math.max(0, fin - 0.1)}) : null, onPlus: () => set({'fadeIn': fin + 0.1})),
      rw.DStep('Fade out', '${fout.toStringAsFixed(1)} s', onMinus: fout > 0 ? () => set({'fadeOut': math.max(0, fout - 0.1)}) : null, onPlus: () => set({'fadeOut': fout + 0.1})),
      rw.DTick('Loop', snd['loop'] == true, onChanged: (x) => set({'loop': x})),
      rw.DButtons('Copy sound', () => _cmd({'op': 'copy_sound'}), b: 'Paste sound', onB: _st['soundCopied'] == true ? () => _cmd({'op': 'paste_sound'}) : null),
      rw.DButtons('Import a sound…', _importSounds),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  Future<void> _pickFrameSound(List<Map<String, dynamic>> list, String? current) async {
    final picked = await showDialog<String>(
      context: context,
      builder: (c) => SimpleDialog(
        backgroundColor: cPanel,
        title: const Text('Sound this frame plays', style: TextStyle(color: cText, fontSize: 15)),
        children: [
          SimpleDialogOption(onPressed: () => Navigator.pop(c, ''), child: const Text('(none)', style: TextStyle(color: cSecondary))),
          for (final s in list)
            SimpleDialogOption(
              onPressed: () => Navigator.pop(c, s['id'] as String),
              child: Text(s['name'] as String, style: TextStyle(color: s['id'] == current ? cAccent : cText)),
            ),
        ],
      ),
    );
    if (picked == null) return;
    _cmd({'op': 'frame_sound', 'asset': picked.isEmpty ? null : picked});
  }

  /// Import a sound: the sound files in the Barkfart Sounds folder, several
  /// at once.
  Future<void> _importSounds() async {
    final e = _engine;
    if (e == null) return;
    final files = await soundFiles();
    if (!mounted) return;
    final chosen = <String>{};
    final ok = await showDialog<bool>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) => AlertDialog(
          backgroundColor: cPanel,
          title: const Text('Import sounds', style: TextStyle(color: cText, fontSize: 16)),
          content: SizedBox(
            width: 380,
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              const Text('Put sound files (mp3, m4a, wav, ogg, flac) in the Barkfart Sounds folder, then pick them here. The project keeps its own copy.',
                  style: TextStyle(color: cSecondary, fontSize: 12)),
              const SizedBox(height: 8),
              if (files.isEmpty) const Padding(padding: EdgeInsets.all(12), child: Text('No sound files in the folder yet.', style: TextStyle(color: cText))),
              Flexible(
                child: ListView(shrinkWrap: true, children: [
                  for (final f in files)
                    CheckboxListTile(
                      dense: true,
                      value: chosen.contains(f.path),
                      onChanged: (v) => set(() => v == true ? chosen.add(f.path) : chosen.remove(f.path)),
                      title: Text(f.path.split(Platform.pathSeparator).last, style: const TextStyle(color: cText, fontSize: 13)),
                    ),
                ]),
              ),
            ]),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(c, false), child: const Text('Cancel')),
            TextButton(onPressed: chosen.isEmpty ? null : () => Navigator.pop(c, true), child: Text('Import (${chosen.length})')),
          ],
        ),
      ),
    );
    if (ok != true) return;
    for (final path in chosen) {
      _refresh(e.importFile(path.split(Platform.pathSeparator).last, await File(path).readAsBytes()));
    }
    // a sound added from + plays from this frame: the newest one, and its page
    final list = ((_st['soundList'] as List?) ?? const []).cast<Map>();
    if (chosen.isNotEmpty && list.isNotEmpty && _st['frameSound'] == null) {
      _cmd({'op': 'frame_sound', 'asset': list.last['id']});
      _pickSound();
      setState(() => _panelOpen = true);
    }
  }

  int _tweenCount = 3;

  void _pasteFigure() => _cmd({'op': 'paste_figure', 'layering': AppSettings.flag('pasteLayering')});

  /// Long-press on Paste stickfigure: the "Paste with layering" switch.
  Future<void> _pasteLayeringPopup() => showDialog<void>(
        context: context,
        builder: (c) => StatefulBuilder(
          builder: (c, set) => AlertDialog(
            backgroundColor: cPanel,
            content: SizedBox(
              width: 340,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                const Text('The app will try to paste the copied figure at the same layering it had on the frame it was copied from.', style: TextStyle(color: cText, fontSize: 13)),
                const SizedBox(height: 12),
                _Toggle('Paste with layering', AppSettings.flag('pasteLayering'), (v) {
                  set(() => AppSettings.values['pasteLayering'] = v);
                  unawaited(AppSettings.save());
                }),
              ]),
            ),
            actions: [TextButton(onPressed: () => Navigator.pop(c), child: const Text('Done'))],
          ),
        ),
      );

  Future<void> _tweenHelp() => _ask(
        'Adding a tweened frame',
        'A tweened frame is a new frame between this one and the next, where every figure stands part of the way from its pose here to its pose on the next frame: halfway (50), a third and two thirds (33/67), or close to either end (10/90). Because it needs a next frame, it cannot be added after the last one. Hold the button to add several evenly spaced frames at once.',
        ['Okay'],
      );

  Future<void> _tweenCountPopup() async {
    final n = await showDialog<int>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) => AlertDialog(
          backgroundColor: cPanel,
          content: Column(mainAxisSize: MainAxisSize.min, children: [
            const Text('Add evenly spaced tweened frames between this frame and the next.', style: TextStyle(color: cSecondary, fontSize: 13)),
            const SizedBox(height: 12),
            Row(mainAxisAlignment: MainAxisAlignment.center, children: [
              SizedBox(width: 44, child: _Btn('−', onTap: () => set(() => _tweenCount = math.max(1, _tweenCount - 1)))),
              const SizedBox(width: 8),
              SizedBox(width: 140, child: _Btn('$_tweenCount frames', accent: true, onTap: () => Navigator.pop(c, _tweenCount))),
              const SizedBox(width: 8),
              SizedBox(width: 44, child: _Btn('+', onTap: () => set(() => _tweenCount = math.min(60, _tweenCount + 1)))),
            ]),
          ]),
        ),
      ),
    );
    if (n != null) _cmd({'op': 'tween_add', 'n': n});
  }

  /// The multi-select page (REDESIGN D18, G5): what applies to every selected
  /// figure, each change one step. A value that differs between them shows a
  /// dash (tick boxes), no choice (segments) or "Mixed" (colour).
  List<Widget> _multiTools(List<Map> multi) {
    void each(String label, Map<String, Object?> cmd) => _cmd({'op': 'each', 'label': label, 'cmd': cmd});
    bool? same(String k) {
      final v = multi.map((m) => m[k] == true).toSet();
      return v.length == 1 ? v.first : null;
    }

    const words = ['', 'one', 'both', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten'];
    final n = multi.length;
    final colors = multi.map((m) => m['color']).toSet();
    final color = (colors.length == 1 ? colors.first as num? : null)?.toInt();
    final hold = same('hold');
    // "Hero has Glow. Head and Rival have none."
    String fxLine() {
      String names(List<Map> ms) => ms.length == 1 ? '${ms.first['name']}' : '${ms.take(ms.length - 1).map((m) => m['name']).join(', ')} and ${ms.last['name']}';
      const labels = {'blur': 'Blur', 'motion': 'Motion blur', 'glow': 'Glow', 'adjust': 'Colour', 'tint': 'Tint', 'invert': 'Invert', 'pixelate': 'Pixelate', 'outline': 'Outline', 'shadow': 'Shadow'};
      final have = multi.where((m) => ((m['effects'] as List?) ?? const []).isNotEmpty).toList();
      final none = multi.where((m) => ((m['effects'] as List?) ?? const []).isEmpty).toList();
      if (have.isEmpty) return 'No effects yet.';
      final parts = [
        for (final m in have) '${m['name']} has ${((m['effects'] as List).map((e) => labels[e] ?? e)).join(', ')}.',
        if (none.isNotEmpty) '${names(none)} ${none.length == 1 ? 'has' : 'have'} none.',
      ];
      return parts.join(' ');
    }

    final opacity = ((multi.first['opacity'] as num?) ?? 1).toDouble();
    final rows = <rw.Desc>[
      // the multi-select board, top to bottom
      rw.DHint('Changes here apply to ${n < words.length ? (n == 2 ? 'both' : 'all ${words[n]}') : 'all $n'}.'),
      rw.DWidget(kit.PropRow('Colour', secondary: color == null ? 'Mixed' : null, trailing: _swatches(color ?? 0x808080ff, (c) => each('Colour figures', {'op': 'figure', 'color': c})))),
      rw.DTitle('Effects', onAdd: _openFxBrowser, addLabel: 'Add an effect to ${n == 2 ? 'both' : 'all ${n < words.length ? words[n] : n}'}'),
      rw.DHint(fxLine()),
      const rw.DTitle('Blending'),
      rw.DSlider('Opacity', '${(opacity * 100).round()}%', opacity, onChanged: (x) => each('Opacity', {'op': 'filter', 'opacity': (x * 100).round() / 100})),
      rw.DTick('Locked', same('locked'), onChanged: (v) => each(v ? 'Lock figures' : 'Unlock figures', {'op': 'figure', 'locked': v})),
      // more than the board: their poses and the usual figure actions
      const rw.DTitle('Poses'),
      rw.DSeg(const ['Hold until the next pose', 'Tween into it'], hold == null ? -1 : (hold ? 0 : 1), onSelect: (i) => each(i == 0 ? 'Hold' : 'Tween', {'op': 'easing', 'v': i == 0 ? 'hold' : 'linear', 'block': true})),
      rw.DButtons('New pose', () => each('New pose', {'op': 'new_pose'}), b: '+1 frame', onB: () => each('+1 frame', {'op': 'hold_longer'})),
      rw.DButtons('Flip sideways', () => each('Flip figures', {'op': 'flip_figure', 'h': true}), iconA: 'flipH', b: 'Flip over', onB: () => each('Flip figures', {'op': 'flip_figure', 'h': false}), iconB: 'flipV'),
      rw.DTick('Visible in onion skin', same('onion'), onChanged: (v) => each('Onion skin', {'op': 'figure', 'onion': v})),
      rw.DButtons('Group', () => _cmd({'op': 'group'})),
      rw.DButtons('Copy poses', () => _cmd({'op': 'copy_poses'}), b: 'Paste poses', onB: ((_st['posesCopied'] as num?) ?? 0) > 0 ? () => _cmd({'op': 'paste_poses'}) : null),
      rw.DButtons('Make movieclip…', () async {
        final ok = await _ask('Make a movieclip?', 'These ${n < words.length ? words[n] : n} figures and the figures joined to them become one movieclip that plays their animation in a loop, in their place. It can be broken apart again.', ['Cancel', 'Make movieclip']);
        if (ok == 'Make movieclip') _cmd({'op': 'to_movieclip'});
      }),
      rw.DButtons('Duplicate', () => each('Duplicate figures', {'op': 'duplicate_figure'}), b: 'Pick one', onB: () => _cmd({'op': 'select', 'layer': multi.first['id']})),
      rw.DDanger('Hide here', () => each('Hide figures', {'op': 'delete_here'})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// The Group page (round 10 Group board): its name with Rename, what a
  /// group does, its opacity; moving it whole, Ungroup and Make movieclip.
  /// "Delete group" is pinned. (Group effects and "Draw order: free" come
  /// later, D33.)
  List<Widget> _groupTools(Map g) {
    final gid = g['id'];
    final members = ((g['members'] as List?) ?? const []).cast<Map>();
    final opacity = ((g['opacity'] as num?) ?? 1).toDouble();
    final rows = <rw.Desc>[
      rw.DName('${g['name']}', link: 'Rename', onLink: () async {
        final n = await _askName('Rename group', '${g['name']}');
        if (n != null && n.trim().isNotEmpty) _cmd({'op': 'group_set', 'group': gid, 'name': n.trim()});
      }),
      ..._effectRows({'filters': g['filters'], 'effectStack': g['effectStack'], 'layer': gid}, group: true, showBlending: false),
      const rw.DTitle('Blending'),
      rw.DSlider('Opacity', '${(opacity * 100).round()}%', opacity, onChanged: (x) => _cmd({'op': 'group_set', 'group': gid, 'opacity': (x * 100).round() / 100})),
      rw.DWidget(BlendModeRow(current: '${g['blend'] ?? 'NORMAL'}', onChanged: (v) => _cmd({'op': 'group_set', 'group': gid, 'blend': v}))),
      const rw.DTitle('Group'),
      rw.DButtons('Send back', () => _cmd({'op': 'group_order', 'group': gid, 'forward': false}), b: 'Bring forward', onB: () => _cmd({'op': 'group_order', 'group': gid, 'forward': true})),
      rw.DButtons('Ungroup', () => _cmd({'op': 'ungroup', 'group': gid}), b: 'Make movieclip…', onB: members.isEmpty
          ? null
          : () async {
              final ok = await _ask('Make a movieclip?', 'Everything in ${g['name']} becomes one movieclip that plays its animation in a loop, in its place. It can be broken apart again.', ['Cancel', 'Make movieclip']);
              if (ok == 'Make movieclip') _cmd({'op': 'to_movieclip', 'group': gid});
            }),
      rw.DHint('In it: ${members.map((m) => m['name']).join(', ')}.'),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  // ---------------------------------------------------------------- script page

  /// Select the Sound page cleanly, clearing any Camera / layer / script / group selection.
  void _pickSound({int? frame}) {
    if (_playing) return;
    if (_st['mode'] == 'camera') _cmd({'op': 'mode', 'v': 'figure'});
    _cmd({'op': 'select'});
    if (frame != null) _cmd({'op': 'frame', 'i': frame});
    setState(() {
      _frameSelected = false;
      _selPoseKeys.clear();
      _selPropKeys.clear();
      _soundPage = true;
    });
  }

  /// K2c: a script layer's row or one of its runs: the Script page.
  void _pickScript(String layer, String? run) {
    if (_playing) return;
    _cmd({'op': 'script_pick', 'layer': layer, 'run': ?run});
    setState(() {
      _frameSelected = false;
      _soundPage = false;
    });
  }

  /// A plugin's settings as rows (modifiers on a value, scripts): numbers with
  /// both limits are sliders, others steppers; toggles tick; choices cycle.
  List<rw.Desc> _pluginSettingRows(List settings, void Function(Map<String, Object?> props) set) {
    return [
      for (final p in settings.cast<Map>())
        switch (p['kind']) {
          'number' || 'angle' => () {
              final x = (p['extra'] as Map?) ?? const {};
              final lo = (x['min'] as num?)?.toDouble(), hi = (x['max'] as num?)?.toDouble();
              final v = ((p['value'] as num?) ?? 0).toDouble();
              final step = ((x['step'] as num?) ?? (p['kind'] == 'angle' ? 5 : (lo != null && hi != null ? (hi - lo) / 20 : 1))).toDouble();
              final text = step >= 1 && v == v.roundToDouble() ? '${v.round()}' : v.toStringAsFixed(2);
              if (lo != null && hi != null && hi > lo) {
                return rw.DSlider('${p['label']}', text, ((v - lo) / (hi - lo)).clamp(0.0, 1.0), onChanged: (f) {
                  var y = lo + f * (hi - lo);
                  if (x['step'] != null) y = (y / step).round() * step;
                  set({p['name'] as String: double.parse(y.toStringAsFixed(4))});
                });
              }
              double clampTo(double y) => y.clamp(lo ?? -1e9, hi ?? 1e9);
              return rw.DStep('${p['label']}', text, onMinus: () => set({p['name'] as String: clampTo(v - step)}), onPlus: () => set({p['name'] as String: clampTo(v + step)}));
            }(),
          'toggle' => rw.DTick('${p['label']}', p['value'] == true, onChanged: (v) => set({p['name'] as String: v})),
          'choice' => () {
              final opts = (((p['extra'] as Map?)?['options'] as List?) ?? const []).cast<String>();
              final now = opts.indexOf('${p['value']}');
              return rw.DLink('${p['label']}', '${p['value']}', onTap: opts.isEmpty ? null : () => set({p['name'] as String: opts[(now + 1) % opts.length]}));
            }(),
          _ => rw.DInfo('${p['label']}', '${p['value']}'),
        },
    ];
  }

  /// The Script page (Script board): what it moves, its settings, the picked
  /// run (how it mixes in, Bake, a new seed). "Delete script" is pinned.
  List<Widget> _scriptTools(Map sc) {
    final lid = sc['id'];
    final runs = ((sc['runs'] as List?) ?? const []).cast<Map>();
    final run = runs.where((r) => r['id'] == sc['run']).firstOrNull;
    void setRun(Map<String, Object?> m) => _cmd({'op': 'script_run', 'layer': lid, 'run': run?['id'], ...m});
    final mix = ((run?['mix'] as num?) ?? 1).toDouble();
    final rows = <rw.Desc>[
      rw.DName('${sc['name']}', link: sc['author'] == null ? null : 'by ${sc['author']}'),
      if (sc['missing'] == true)
        rw.DHint('Missing plugin: ${sc['plugin']} ${sc['version'] ?? ''}. Import it to run this script; its settings are kept.'.replaceAll(' .', '.'))
      else if (sc['error'] != null)
        rw.DHint('${sc['error']} Import the plugin again to switch it back on.')
      else if (sc['description'] != null && AppSettings.flag('nerd') != true)
        rw.DHint('${sc['description']}'),
      const rw.DTitle('Moves'),
      for (final t in ((sc['targets'] as List?) ?? const []).cast<Map>())
        rw.DLink('${t['label']}', '${t['toName'] ?? 'Pick…'}', onTap: () => _pickTarget(sc, t)),
      if (((sc['settings'] as List?) ?? const []).isNotEmpty) ...[
        const rw.DTitle('Settings'),
        ..._pluginSettingRows(sc['settings'] as List, (props) => _cmd({'op': 'script_set', 'layer': lid, 'props': props})),
      ],
      if (run != null) ...[
        const rw.DTitle('This run'),
        rw.DHint(run['frames'] == 1
            ? 'Frame ${(run['from'] as num) + 1}, with its own seed. Drag the block\u2019s ends to change it.'
            : 'Frames ${(run['from'] as num) + 1} to ${(run['from'] as num) + (run['frames'] as num)}, with its own seed. Drag the block\u2019s ends to change them.'),
        rw.DSlider('Mix', '${(mix * 100).round()}%', mix, onChanged: (x) => setRun({'mix': (x * 100).round() / 100})),
        rw.DTick('Replaces the keys', run['replace'] == true, onChanged: (v) => setRun({'replace': v})),
        rw.DTick('Keeps the last frame', run['keep'] == true, onChanged: (v) => setRun({'keep': v})),
        rw.DButtons('Bake to keys', sc['missing'] == true || sc['error'] != null
            ? null
            : () {
                _cmd({'op': 'script_bake', 'layer': lid, 'run': run['id']});
                kit.showToast(context, 'The run is keys now.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
              }, b: 'New seed', onB: () => setRun({'newSeed': true})),
      ],
      rw.DButtons('New run here', () => _cmd({'op': 'script_run_add', 'layer': lid, 'frame': _frame}),
          b: 'Delete this run', onB: run == null || runs.length < 2 ? null : () => _cmd({'op': 'script_run_remove', 'layer': lid, 'run': run['id']})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// What a script target moves: a figure, or a value (grouped by layer) with
  /// instant search and category filtering.
  Future<void> _pickTarget(Map sc, Map t) async {
    final figure = t['kind'] == 'figure';
    final list = ((figure ? sc['figures'] : sc['values']) as List? ?? const []).cast<Map>();
    final options = [for (final v in list) TargetOption.fromMap(v, isFigure: figure)];
    final pick = await showTargetPickerSheet(
      context,
      title: figure ? 'Target figure' : 'Target property',
      options: options,
      currentId: t['to'] as String?,
      allowClear: t['to'] != null,
      emptyText: figure ? 'No stick figures in this animation.' : 'No target properties available.',
      searchHint: figure ? 'Search figures…' : 'Search layers or properties…',
    );
    if (pick == null) return;
    _cmd({'op': 'script_set', 'layer': sc['id'], 'target': {'name': t['name'], 'to': pick.isEmpty ? null : pick}});
  }

  /// "+ › Script": the installed scripts, and Import… (K2c).
  Future<void> _scriptPicker() async {
    final scripts = ((_st['plugins'] as List?) ?? const []).cast<Map>().where((p) => p['kind'] == 'script').toList();
    final pick = await kit.showSheet<String>(context, title: 'Add a script', width: 400, children: (c) => [
          kit.Hint(scripts.isEmpty ? 'No scripts yet. A script is a plugin (a .luau file) that moves figures or values: sway, breathing, simple physics.' : 'It runs from this frame to the end; drag its block to change that. It moves the selected figure.'),
          for (final p in scripts)
            kit.Tap(
              minSize: 0,
              onTap: p['off'] != null ? null : () => Navigator.pop(c, '${p['id']}'),
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 8),
                child: Row(children: [
                  Pictogram('script', size: 20, color: p['off'] != null ? cTag : cAccent),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                      Text('${p['name']}', style: kit.ink(15, FontWeight.w700, p['off'] != null ? cSecondary : cText)),
                      Text(p['off'] != null ? 'Switched off: ${p['off']}' : [if (p['author'] != null) 'by ${p['author']}', p['version']].join(' · '), maxLines: 2, overflow: TextOverflow.ellipsis, style: kit.ink(12, FontWeight.w400, cSecondary)),
                    ]),
                  ),
                ]),
              ),
            ),
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Row(children: [
              Expanded(child: kit.WordButton('Import a script…', onTap: () => Navigator.pop(c, ''), filled: true, expand: true, height: 38)),
              const SizedBox(width: 8),
              Expanded(child: kit.WordButton('Workshop…', onTap: () => Navigator.pop(c, '\u0000workshop'), filled: true, expand: true, height: 38)),
            ]),
          ),
        ]);
    if (pick == null || !mounted) return;
    if (pick == '\u0000workshop') {
      await _openWorkshop();
      return;
    }
    if (pick.isEmpty) {
      await _importPlugin();
      return;
    }
    _cmd({'op': 'script_add', 'id': pick});
    setState(() {
      _frameSelected = false;
      _soundPage = false;
      _panelOpen = true;
      _hidePanels = false;
      _layersOpen = true;
    });
  }

  // ---------------------------------------------------------------- value page

  // D36 (the graph-mods boards): the selected value's modifiers, applied top
  // to bottom after its keys; "+" opens a browser like the effects one.
  bool _modBrowser = false;
  int? _modOpen; // the modifier whose settings show
  String _modSearch = '';
  Map? get _curveValue => _curvesOn ? _curveChannels.where((c) => c['id'] == _curveSel).firstOrNull : null;

  List<Widget> _valueTools(Map c) {
    final id = c['id'];
    final mods = ((c['mods'] as List?) ?? const []).cast<Map>();
    void set(int i, Map<String, Object?> m) => _cmd({'op': 'modifier_set', 'property': id, 'index': i, ...m});
    List<rw.Desc> settings(Map m) {
      final i = (m['index'] as num).toInt();
      num n(String k, num d) => (m[k] as num?) ?? d;
      switch (m['kind']) {
        case 'wiggle' || 'swing':
          return [
            rw.DStep(m['kind'] == 'swing' ? 'Swing' : 'Amount', n('amount', 0).toStringAsFixed(1), onMinus: () => set(i, {'amount': math.max(0, n('amount', 0) - 1)}), onPlus: () => set(i, {'amount': n('amount', 0) + 1})),
            rw.DStep('Speed', n('speed', 1).toStringAsFixed(2), onMinus: () => set(i, {'speed': math.max(0.05, n('speed', 1) - 0.25)}), onPlus: () => set(i, {'speed': n('speed', 1) + 0.25})),
          ];
        case 'loop':
          return [
            rw.DStep('From frame', '${n('start', 0).round() + 1}', onMinus: () => set(i, {'start': math.max(0, n('start', 0) - 1)}), onPlus: () => set(i, {'start': n('start', 0) + 1})),
            rw.DStep('To frame', '${n('end', 1).round() + 1}', onMinus: () => set(i, {'end': math.max(n('start', 0) + 1, n('end', 1) - 1)}), onPlus: () => set(i, {'end': n('end', 1) + 1})),
            rw.DTick('Back and forth', m['pingPong'] == true, onChanged: (v) => set(i, {'pingPong': v})),
          ];
        case 'plugin':
          return [
            ..._pluginSettingRows((m['settings'] as List?) ?? const [], (props) => set(i, {'props': props})),
            if (m['off'] != null) rw.DHint('${m['off']} Import the plugin again to switch it back on.'),
          ];
        default:
          return const [];
      }
    }

    final keys = ((c['keys'] as List?) ?? const []).length;
    final rows = <rw.Desc>[
      rw.DInfo('${c['name']}', keys == 1 ? '1 key' : '$keys keys'),
      rw.DTitle('Modifiers', onAdd: () => setState(() => _modBrowser = true), addLabel: 'Add modifier'),
      if (mods.isEmpty) const rw.DHint('No modifiers.'),
      for (final m in mods) ...[
        rw.DFx(
          m['missing'] == true ? 'Missing: ${m['name']} ${m['version'] ?? ''}'.trim() : '${m['name']}',
          note: m['author'] != null ? 'by ${m['author']}' : null,
          missing: m['missing'] == true || m['off'] != null,
          shown: m['enabled'] != false,
          open: _modOpen == m['index'],
          onTap: () => setState(() => _modOpen = _modOpen == m['index'] ? null : (m['index'] as num).toInt()),
          onEye: () => set((m['index'] as num).toInt(), {'enabled': m['enabled'] == false}),
          onRemove: () => _cmd({'op': 'modifier_remove', 'property': id, 'index': m['index']}),
        ),
        if (_modOpen == m['index'] && m['missing'] != true) ...settings(m),
      ],
      if (mods.any((m) => m['missing'] == true)) const rw.DHint('Missing plugin — import it again to restore.'),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// The Modifiers browser (graph-mods-add board): Yours (plugins), Built
  /// in, Import….
  Widget _modifierBrowser(Map c) {
    final q = _modSearch.toLowerCase();
    bool match(String s) => q.isEmpty || s.toLowerCase().contains(q);
    void pick(Map<String, Object?> cmd) {
      _cmd({'op': 'modifier_add', 'property': c['id'], ...cmd});
      setState(() {
        _modBrowser = false;
        _modSearch = '';
        _modOpen = ((((_curveValue?['mods'] as List?) ?? const []).length) - 1);
      });
    }

    Widget tile(String label, Widget glyph, VoidCallback? onTap, {String? sub}) => Opacity(
          opacity: onTap == null ? 0.4 : 1,
          child: kit.Tap(
            label: label,
            minSize: 0,
            onTap: onTap,
            child: Column(mainAxisSize: MainAxisSize.min, children: [
              Container(height: 58, alignment: Alignment.center, decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(8), border: Border.all(color: cLine)), child: glyph),
              const SizedBox(height: 3),
              Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 12)),
              if (sub != null) Text(sub, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cSecondary, fontSize: 11)),
            ]),
          ),
        );
    Widget grid(List<Widget> tiles) => GridView.count(crossAxisCount: 3, shrinkWrap: true, physics: const NeverScrollableScrollPhysics(), mainAxisSpacing: 10, crossAxisSpacing: 8, childAspectRatio: 1.05, children: tiles);
    Widget title(String t) => Padding(padding: const EdgeInsets.only(top: 12, bottom: 6), child: Text(t, textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w700)));
    final plugins = ((_st['plugins'] as List?) ?? const []).cast<Map>().where((p) => p['kind'] == 'modifier').toList();
    final yours = [
      for (final p in plugins)
        if (match('${p['name']}'))
          tile('${p['name']}', const Pictogram('modifier', size: 26, color: cAccent), p['off'] != null ? null : () => pick({'kind': 'plugin', 'id': p['id']}), sub: p['author'] != null ? 'by ${p['author']}' : null),
    ];
    final built = [
      for (final (k, l, icon) in _fxMovement)
        if (match(l)) tile(l, Pictogram(icon, size: 28, color: cAccent, stroke: 1.8), k == 'wiggle' || k == 'swing' || k == 'loop' ? () => pick({'kind': k}) : null),
    ];
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(6, 8, 10, 0),
        child: Row(children: [
          Flexible(child: kit.WordButton('‹ ${c['name']}', onTap: () => setState(() => _modBrowser = false))),
          const Spacer(),
          kit.WordButton('Import…', onTap: _importPlugin),
        ]),
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(14, 4, 14, 0),
        child: Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(10)),
          child: Row(children: [
            const Pictogram('search', size: 16, color: cSecondary),
            const SizedBox(width: 8),
            Expanded(
              child: TextField(
                style: const TextStyle(color: cText, fontSize: 13.5),
                decoration: const InputDecoration(isDense: true, border: InputBorder.none, hintText: 'Search modifiers…', hintStyle: TextStyle(color: cSecondary, fontSize: 13)),
                onChanged: (v) => setState(() => _modSearch = v),
              ),
            ),
          ]),
        ),
      ),
      Expanded(
        child: ListView(padding: const EdgeInsets.fromLTRB(14, 0, 14, 16), children: [
          if (yours.isNotEmpty) ...[title('Plugins'), grid(yours)],
          if (built.isNotEmpty) ...[title('Built-in'), grid(built)],
        ]),
      ),
    ]);
  }

  /// K2: a plugin file in: checked and installed, then kept in Barkfart
  /// Plugins so it is there next time.
  /// Plugin id → its file in Barkfart Plugins (the Workshop shares it, and
  /// switches a plugin back on from it).
  final Map<String, String> _pluginFiles = {};

  /// Import a plugin file: installed and kept in Barkfart Plugins. Returns
  /// its id (null: nothing imported).
  Future<String?> _importPlugin() async {
    List<PlatformFile> picked;
    try {
      picked = await FilePicker.pickFiles(dialogTitle: 'Import a plugin', type: FileType.custom, allowedExtensions: const ['luau', 'lua']);
    } catch (err) {
      _say('Could not open the file picker: $err');
      return null;
    }
    for (final f in picked.take(1)) {
      final source = utf8.decode(await f.readAsBytes(), allowMalformed: true);
      final st = _cmd({'op': 'plugin_install', 'name': f.name, 'source': source});
      if (st?['installed'] case final String id) {
        try {
          final path = '${(await pluginsDir()).path}/${f.name}';
          await File(path).writeAsString(source);
          _pluginFiles[id] = path;
        } catch (_) {}
        // a new effect's tile gets its picture
        if (_fxBrowser) await _openFxBrowser();
        return id;
      }
    }
    return null;
  }

  // ---------------------------------------------------------------- the workshop

  /// K3: the Workshop, a page for plugins. Add and Show close it and act here.
  Future<void> _openWorkshop({String? picked}) async {
    final a = await Navigator.of(context).push<WorkshopAction>(PageRouteBuilder<WorkshopAction>(
      transitionDuration: kit.motion(context, 180),
      reverseTransitionDuration: kit.motion(context, 140),
      transitionsBuilder: (c, a, _, child) {
        final e = CurvedAnimation(parent: a, curve: kit.kEase, reverseCurve: Curves.easeInCubic);
        return FadeTransition(opacity: e, child: SlideTransition(position: Tween(begin: const Offset(0, 0.03), end: Offset.zero).animate(e), child: child));
      },
      pageBuilder: (c, a1, a2) => WorkshopPage(
        cmd: _cmd,
        import: _importPlugin,
        reinstall: _reinstallPlugin,
        remove: _removePlugin,
        share: _sharePlugin,
        files: _pluginFiles,
        figureSelected: _sel != null && _st['building'] == null,
        picked: picked,
      ),
    ));
    if (a == null || !mounted) return;
    switch (a) {
      case WorkshopAdd(:final id, :final kind):
        if (kind == 'script') {
          _cmd({'op': 'script_add', 'id': id});
          setState(() {
            _frameSelected = false;
            _soundPage = false;
            _panelOpen = true;
            _hidePanels = false;
            _layersOpen = true;
          });
        } else if (kind == 'effect') {
          _cmd({'op': 'plugin_effect', 'id': id});
          setState(() {
            _panelOpen = true;
            _hidePanels = false;
          });
          _jumpTo('Stickfigure Tools');
        } else {
          // Modifier plugin: pick target property via searchable Target Picker and attach directly
          _cmd({'op': 'set', 'k': 'curves', 'v': true});
          final targetOptions = _allModifierTargetOptions();
          final pickedProp = targetOptions.isNotEmpty
              ? await showTargetPickerSheet(
                  context,
                  title: 'Apply modifier to…',
                  options: targetOptions,
                  currentId: _curveSel,
                  searchHint: 'Search layers or properties…',
                )
              : _curveSel;
          if (!mounted) return;
          if (pickedProp != null && pickedProp.isNotEmpty) {
            _cmd({'op': 'modifier_add', 'property': pickedProp, 'kind': 'plugin', 'id': id});
            setState(() {
              _layersOpen = true;
              _curveSel = pickedProp;
              _curveVis[pickedProp] = true;
              _panelOpen = true;
              _hidePanels = false;
              _modBrowser = false;
              _modOpen = ((((_curveValue?['mods'] as List?) ?? const []).length) - 1);
            });
          } else {
            setState(() {
              _layersOpen = true;
              _panelOpen = true;
              _hidePanels = false;
            });
          }
        }
      case WorkshopShow(:final use):
        if (use['kind'] == 'script') {
          setState(() => _layersOpen = true);
          _pickScript('${use['layer']}', use['run'] as String?);
        } else if (use['group'] != null) {
          _cmd({'op': 'select', 'group': use['group']});
          setState(() => _panelOpen = true);
        } else {
          _cmd({'op': 'select', 'layer': use['layer']});
          setState(() {
            _panelOpen = true;
            _hidePanels = false;
          });
          if (use['kind'] == 'modifier') {
            _cmd({'op': 'set', 'k': 'curves', 'v': true});
            setState(() {
              _layersOpen = true;
              _curveSel = use['property'] as String?;
            });
          } else {
            _jumpTo('Stickfigure Tools');
          }
        }
    }
  }

  /// A switched-off plugin installed again from its file.
  Future<void> _reinstallPlugin(String id) async {
    final path = _pluginFiles[id];
    if (path == null) return;
    try {
      final f = File(path);
      _cmd({'op': 'plugin_install', 'name': f.uri.pathSegments.last, 'source': await f.readAsString()});
    } catch (err) {
      _say('Could not read the plugin\u2019s file: $err');
    }
  }

  /// "Remove from Barkfart": uninstalled and its file deleted. Animations
  /// that use it keep its settings and show it missing.
  Future<void> _removePlugin(String id) async {
    final name = (((_st['plugins'] as List?) ?? const []).cast<Map>().where((p) => p['id'] == id).firstOrNull?['name']) ?? id;
    final ok = await _ask('Remove $name?', 'Animations that use it keep its settings and show it as missing until it\u2019s imported again.', ['Cancel', 'Remove']);
    if (ok != 'Remove') return;
    _cmd({'op': 'plugin_uninstall', 'id': id});
    final path = _pluginFiles.remove(id);
    if (path != null) {
      try {
        await File(path).delete();
      } catch (_) {}
    }
  }

  /// "Share the file…": the system share sheet with the plugin's .luau.
  Future<void> _sharePlugin(String id, Rect from) async {
    final path = _pluginFiles[id];
    if (path == null) return;
    try {
      await SharePlus.instance.share(ShareParams(files: [XFile(path)], sharePositionOrigin: from.isEmpty ? null : from));
    } catch (err) {
      _say('Could not share: $err');
    }
  }

  /// D35: name a part (empty: back to its shape and place).
  Future<void> _renamePart(String node, String now, {bool named = true}) async {
    final n = await _askName('Name this part', named ? now : '');
    if (n == null) return;
    _cmd({'op': 'rename_part', 'node': node, 'name': n.trim()});
  }

  /// Collects all animatable target properties from engine state (`availableTargets`,
  /// `curves['channels']`, and `curves['addable']`) for the searchable Target Picker.
  List<TargetOption> _allModifierTargetOptions() {
    final seen = <String>{};
    final out = <TargetOption>[];
    void addAll(Object? raw, {bool animated = false}) {
      if (raw is! List) return;
      for (final item in raw) {
        if (item is! Map) continue;
        final opt = TargetOption.fromMap(item);
        if (opt.id.isNotEmpty && seen.add(opt.id)) {
          out.add(animated && !opt.animated
              ? TargetOption(
                  id: opt.id,
                  label: opt.label,
                  group: opt.group,
                  layerKind: opt.layerKind,
                  category: opt.category,
                  valueText: opt.valueText,
                  animated: true,
                  modifierCount: opt.modifierCount,
                )
              : opt);
        }
      }
    }

    addAll(_st['modifierTargets']);
    addAll(_st['availableTargets']);
    final cv = _st['curves'];
    if (cv is Map) {
      final layerName = '${cv['name'] ?? ''}';
      final rawChans = cv['channels'];
      if (rawChans is List) {
        for (final ch in rawChans) {
          if (ch is! Map) continue;
          final enriched = {...ch, if (ch['group'] == null && ch['layer'] == null) 'group': layerName, 'animated': true};
          final opt = TargetOption.fromMap(enriched);
          if (opt.id.isNotEmpty && seen.add(opt.id)) out.add(opt);
        }
      }
      addAll(cv['addable']);
    }
    return out;
  }

  /// The Picture page (round 10 Picture board).
  List<Widget> _pictureTools(Map pic) {
    void set(Map<String, Object?> m) => _cmd({'op': 'picture', ...m});
    final scale = ((pic['scale'] as num?) ?? 1).toDouble(), opacity = ((pic['opacity'] as num?) ?? 1).toDouble();
    final rows = <rw.Desc>[
      rw.DInfo('Picture', '${pic['file'] ?? pic['name']}'),
      rw.DStep('Scale', scale.toStringAsFixed(2), onMinus: scale > 0.05 ? () => set({'scale': math.max(0.05, scale - 0.05)}) : null, onPlus: () => set({'scale': scale + 0.05})),
      rw.DButtons('Flip sideways', () => set({'flipH': true}), iconA: 'flipH', b: 'Flip over', onB: () => set({'flipV': true}), iconB: 'flipV'),
      ..._effectRows(pic.cast<String, dynamic>(), showBlending: false),
      const rw.DTitle('Blending'),
      rw.DSlider('Opacity', '${(opacity * 100).round()}%', opacity, onChanged: (x) => set({'opacity': (x * 100).round() / 100})),
      rw.DWidget(BlendModeRow(current: '${pic['blend'] ?? 'NORMAL'}', onChanged: (v) => _cmd({'op': 'blend', 'v': v}))),
      rw.DTick('Locked', pic['locked'] == true, onChanged: (v) => set({'locked': v})),
      rw.DTick('Locked to camera', pic['cameraLocked'] == true, onChanged: (v) => _cmd({'op': 'camera_lock', 'layer': pic['layer'], 'on': v})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// The Pose page (REDESIGN §8, F4): the selected figure's pose under the
  /// playhead, a block lasting until its next pose. Hold or tween into the
  /// next, the ease, its length, a new pose here, copy and paste.
  List<Widget> _poseTools(Map<String, dynamic> sel) {
    final pose = sel['pose'] as Map?;
    if (pose == null) return [rw.renderDesc(const rw.DHint('This figure has no pose yet.'))];
    final start = (pose['start'] as num).toInt();
    final len = (pose['length'] as num).toInt();
    final last = pose['last'] == true;
    final ease = pose['ease'] as String? ?? 'linear';
    final hold = ease == 'hold';
    void easing(String v) => _cmd({'op': 'easing', 'v': v, 'frame': start});
    // D34: Auto (the curve through the poses) first, then the board's four:
    // even, slow end, slow both ends, bounce ("in": slow start, from older
    // files, shows as even)
    const eases = ['auto', 'linear', 'out', 'smooth', 'bounce'];
    // the handles: how the curve meets this pose (Free and Aligned need a
    // graph to drag them in, so they don't show here)
    const handles = ['auto_clamped', 'auto', 'vector'];
    final handle = handles.indexOf('${pose['handles'] ?? 'auto_clamped'}');
    final rows = <rw.Desc>[
      rw.DInfo(sel['name'] as String? ?? 'Figure', len == 1 ? 'frame ${start + 1}' : last ? 'frame ${start + 1} to the end' : 'frames ${start + 1} to ${start + len}'),
      rw.DSeg(const ['Hold until the next pose', 'Tween into it'], hold ? 0 : 1, onSelect: (i) => easing(i == 0 ? 'hold' : 'auto')),
      rw.DEase(math.max(0, eases.indexOf(ease == 'in' ? 'linear' : ease)), enabled: !hold, onSelect: (i) => easing(eases[i])),
      if (ease == 'auto') ...[
        rw.DSeg(const ['Settle', 'Swing', 'Sharp'], handle, label: 'Through this pose', onSelect: (i) => _cmd({'op': 'handles', 'v': handles[i], 'frame': start})),
        if (handle < 0) const rw.DHint('This pose has handles set by hand.'),
      ],
      rw.DStep('Length in frames', '$len',
          onMinus: len > 1 ? () => _cmd({'op': 'block_length', 'frames': len - 1}) : null,
          onPlus: () => _cmd({'op': 'block_length', 'frames': len + 1})),
      rw.DButtons('Copy pose', () => _cmd({'op': 'copy_pose'}), b: 'Paste pose', onB: _st['pose'] == true ? () => _cmd({'op': 'paste_pose'}) : null),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }



  // ---------------------------------------------------------------- effects

  /// Built-in effect catalog fallback matching `barkfart_core::fx::catalog()`
  /// (15 effects across 5 categories; live catalog from `_st['effectCatalog']` takes precedence).
  static const List<Map<String, Object?>> _fallbackEffectCatalog = [
    {'type': 'blur.gaussian', 'label': 'Gaussian blur', 'category': 'Blur', 'preview': {'radius': 24.0}},
    {'type': 'blur.motion', 'label': 'Motion blur', 'category': 'Blur', 'preview': {'distance': 120.0, 'angle': 0.0}},
    {'type': 'glow', 'label': 'Glow', 'category': 'Light & Glow', 'preview': {'color': 0xFFC83DFF, 'radius': 20.0, 'strength': 2.2}},
    {'type': 'shadow.drop', 'label': 'Drop shadow', 'category': 'Light & Glow', 'preview': {'color': 0x000000AA, 'dx': 24.0, 'dy': -24.0, 'radius': 10.0}},
    {'type': 'vignette', 'label': 'Vignette', 'category': 'Light & Glow', 'preview': {'amount': 0.85, 'softness': 0.55}},
    {'type': 'color.adjust', 'label': 'Hue & saturation', 'category': 'Color', 'preview': {'hue': 120.0, 'saturation': 1.4}},
    {'type': 'color.grade', 'label': 'Exposure & contrast', 'category': 'Color', 'preview': {'exposure': 0.5, 'contrast': 1.6}},
    {'type': 'tint', 'label': 'Tint', 'category': 'Color', 'preview': {'color': 0xC0392BFF, 'amount': 0.65}},
    {'type': 'invert', 'label': 'Invert', 'category': 'Color', 'preview': <String, Object?>{}},
    {'type': 'chromatic.aberration', 'label': 'Chromatic aberration', 'category': 'Distort', 'preview': {'amount': 24.0, 'angle': 0.0}},
    {'type': 'distort.wave', 'label': 'Wave distort', 'category': 'Distort', 'preview': {'amplitude': 28.0, 'frequency': 4.0, 'phase': 0.0}},
    {'type': 'outline', 'label': 'Outline', 'category': 'Stylize', 'preview': {'color': 0x222428FF, 'width': 16.0}},
    {'type': 'pixelate', 'label': 'Pixelate', 'category': 'Stylize', 'preview': {'size': 26.0}},
    {'type': 'noise.grain', 'label': 'Film grain', 'category': 'Stylize', 'preview': {'amount': 0.5, 'size': 3.0}},
    {'type': 'stylize.scanlines', 'label': 'Scanlines & glitch', 'category': 'Stylize', 'preview': {'amount': 0.65, 'lines': 60.0, 'offset': 14.0}},
  ];

  static const _fxCategories = ['All', 'Blur', 'Light & Glow', 'Color', 'Distort', 'Stylize', 'Motion', 'Plugins'];
  static const _fxMovement = [('wiggle', 'Wiggle', 'wiggle'), ('swing', 'Swing', 'swing')];

  bool _fxBrowser = false;
  List<Map> get _effectPlugins => ((_st['plugins'] as List?) ?? const []).cast<Map>().where((p) => p['kind'] == 'effect').toList();
  String? _fxOpen;
  String _fxSearch = '';
  String _fxCat = 'All';
  final Map<String, ui.Image> _fxPreviews = {};

  List<Map<String, Object?>> get _effectCatalog {
    final raw = (_st['effectCatalog'] as List?)?.cast<Map>();
    if (raw != null && raw.isNotEmpty) {
      return [for (final m in raw) m.map((k, v) => MapEntry('$k', v))];
    }
    return _fallbackEffectCatalog;
  }

  /// Adjustment ("Effect") Layer inspector: name, stacked keyable effects,
  /// blending, lock & draw order.
  List<Widget> _adjustmentTools(Map<String, dynamic> adj) {
    final id = '${adj['layer']}';
    final name = '${adj['name'] ?? 'Effect layer'}';
    final opacity = ((adj['opacity'] as num?) ?? 1).toDouble();
    final rows = <rw.Desc>[
      rw.DName(name, link: 'Rename', onLink: () async {
        final n = await _askName('Rename effect layer', name);
        if (n != null && n.trim().isNotEmpty) _cmd({'op': 'rename', 'layer': id, 'name': n.trim()});
      }),
      ..._effectRows(adj, adjustment: true, showBlending: false),
      const rw.DTitle('Blending'),
      rw.DSlider('Opacity', '${(opacity * 100).round()}%', opacity, onChanged: (x) => _cmd({'op': 'filter', 'opacity': (x * 100).round() / 100})),
      rw.DWidget(BlendModeRow(current: '${adj['blend'] ?? 'NORMAL'}', onChanged: (v) => _cmd({'op': 'blend', 'v': v}))),
      const rw.DTitle('Layer'),
      rw.DButtons('Send back', () => _cmd({'op': 'layer_order', 'forward': false}), b: 'Bring forward', onB: () => _cmd({'op': 'layer_order', 'forward': true})),
      rw.DTick('Locked', adj['locked'] == true, onChanged: (v) => _cmd({'op': 'layer_lock', 'layer': id, 'v': v})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// Opens the categorized Add Effect browser and renders live engine previews.
  Future<void> _openFxBrowser() async {
    final e = _engine;
    if (e == null) return;
    for (final img in _fxPreviews.values) {
      img.dispose();
    }
    _fxPreviews.clear();
    final w = (106 * math.min(_dpr, 2.0)).round(), h = (58 * math.min(_dpr, 2.0)).round();
    final previewMap = {for (final f in _fallbackEffectCatalog) '${f['type']}': (f['preview'] as Map?) ?? const <String, Object?>{}};
    for (final item in _effectCatalog) {
      final ty = '${item['type']}';
      final px = e.renderEffectPreview(jsonEncode({'type': ty, 'params': previewMap[ty] ?? const <String, Object?>{}}), w, h);
      if (px == null) continue;
      final c = Completer<ui.Image>();
      ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, c.complete);
      _fxPreviews[ty] = await c.future;
    }
    for (final p in _effectPlugins) {
      final px = e.renderEffectPreview(jsonEncode({'plugin': p['id']}), w, h);
      if (px == null) continue;
      final c = Completer<ui.Image>();
      ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, c.complete);
      _fxPreviews['plugin:${p['id']}'] = await c.future;
    }
    if (mounted) {
      setState(() {
        _fxBrowser = true;
        _fxCat = 'All';
      });
    }
  }

  /// Compact keyframe diamond toggle button for every keyable effect parameter:
  /// filled blue = keyed at playhead, amber outline = animated on other frames,
  /// muted outline = static.
  Widget _fxKeyDiamond({required bool keyed, required bool animated, required String label, required VoidCallback onTap}) {
    final color = keyed ? cAccent : (animated ? const Color(0xFFD9821E) : const Color(0xFF9AA0A8));
    return kit.Tap(
      label: keyed ? 'Remove keyframe on $label' : 'Add keyframe on $label',
      minSize: 30,
      radius: 8,
      onTap: onTap,
      child: Center(
        child: Transform.rotate(
          angle: math.pi / 4,
          child: Container(
            width: 10,
            height: 10,
            decoration: BoxDecoration(
              color: keyed ? cAccent : Colors.transparent,
              borderRadius: BorderRadius.circular(1.8),
              border: Border.all(color: color, width: keyed ? 1.5 : 1.7),
            ),
          ),
        ),
      ),
    );
  }

  /// One keyable parameter control inside an expanded Effect Stack card.
  Widget _fxParamControl(Map p, String fxId, Map<String, Object?> target, {bool isMix = false}) {
    final pName = isMix ? 'mix' : '${p['name']}';
    final label = isMix ? 'Mix' : '${p['label'] ?? pName}';
    final kind = isMix ? 'float' : '${p['kind'] ?? 'float'}';
    final keyed = p['keyed'] == true;
    final animated = p['animated'] == true;

    void setVal(Object val) {
      if (isMix) {
        _cmd({'op': 'fx_set', 'id': fxId, 'mix': val, ...target});
      } else {
        _cmd({'op': 'fx_set', 'id': fxId, 'params': {pName: val}, ...target});
      }
    }

    // a group's effects aren't keyed (set_fx_prop sets them outright): no diamond
    final diamond = target.containsKey('group')
        ? const SizedBox.shrink()
        : _fxKeyDiamond(
            keyed: keyed,
            animated: animated,
            label: label,
            onTap: () => _cmd({'op': 'fx_key', 'id': fxId, 'param': pName, ...target}),
          );

    if (kind == 'color' || kind == 'colour') {
      final col = ((p['value'] as num?) ?? 0xffffffff).toInt();
      return Padding(
        padding: const EdgeInsets.only(top: 4, bottom: 2),
        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Row(children: [
            Expanded(child: Text(label, style: const TextStyle(color: cText, fontSize: 13, fontWeight: FontWeight.w500))),
            diamond,
          ]),
          const SizedBox(height: 2),
          _swatches(col, (c) => setVal(c)),
        ]),
      );
    }

    if (kind == 'bool' || kind == 'toggle') {
      final on = p['value'] == true;
      return Row(children: [
        Expanded(child: kit.TickRow(label, on, onChanged: (v) => setVal(v))),
        diamond,
      ]);
    }

    if (kind == 'choice') {
      final opts = ((p['options'] as List?) ?? const []).map((e) => '$e').toList();
      final cur = '${p['value'] ?? ''}';
      final idx = opts.indexOf(cur);
      return Row(children: [
        Expanded(
          child: kit.LinkRow(label, cur, onTap: opts.isEmpty ? null : () => setVal(opts[(idx + 1) % opts.length])),
        ),
        diamond,
      ]);
    }

    // Numeric (`float`, `number`, `angle`)
    final v = ((p['value'] as num?) ?? (isMix ? 1.0 : 0.0)).toDouble();
    final lo = (p['min'] as num?)?.toDouble() ?? (isMix ? 0.0 : null);
    final hi = (p['max'] as num?)?.toDouble() ?? (isMix ? 1.0 : null);
    final step = ((p['step'] as num?) ?? (isMix ? 0.05 : (kind == 'angle' ? 5.0 : 1.0))).toDouble();
    final isAngle = kind == 'angle';
    final valueText = isMix
        ? '${(v * 100).round()}%'
        : isAngle
            ? '${v.round()}°'
            : (step >= 1 && (v - v.roundToDouble()).abs() < 1e-3 ? '${v.round()}' : v.toStringAsFixed(step < 0.1 ? 2 : 1));
    double clampV(double x) {
      var y = x;
      if (lo != null) y = math.max(lo, y);
      if (hi != null) y = math.min(hi, y);
      return double.parse(y.toStringAsFixed(4));
    }

    final useSlider = lo != null && hi != null && hi > lo && (hi - lo) <= 800;
    return Padding(
      padding: const EdgeInsets.only(top: 2, bottom: 2),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Row(children: [
          Expanded(child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 13, fontWeight: FontWeight.w500))),
          kit.Tap(
            label: 'Decrease $label',
            minSize: 26,
            radius: 6,
            onTap: lo != null && v <= lo + 1e-6 ? null : () => setVal(clampV(v - step)),
            child: const Pictogram('minus', size: 12, color: cSecondary),
          ),
          ConstrainedBox(
            constraints: const BoxConstraints(minWidth: 42),
            child: Text(valueText, textAlign: TextAlign.center, style: const TextStyle(color: cText, fontSize: 12.5, fontWeight: FontWeight.w700, fontFeatures: [ui.FontFeature.tabularFigures()])),
          ),
          kit.Tap(
            label: 'Increase $label',
            minSize: 26,
            radius: 6,
            onTap: hi != null && v >= hi - 1e-6 ? null : () => setVal(clampV(v + step)),
            child: const Pictogram('plus', size: 12, color: cSecondary),
          ),
          const SizedBox(width: 2),
          diamond,
        ]),
        if (useSlider)
          kit.SliderBar(
            ((v - lo) / (hi - lo)).clamp(0.0, 1.0),
            onChanged: (t) {
              var raw = lo + t * (hi - lo);
              if (step > 0) raw = (raw / step).round() * step;
              setVal(clampV(raw));
            },
          ),
      ]),
    );
  }

  /// Collapsible Pro Effect Stack card (Toonsquid + Procreate aesthetic):
  /// enable/disable eye, animated indicator, reorder up/down, duplicate, delete,
  /// and keyable parameter rows.
  Widget _effectStackCard(Map fx, int index, int count, Map<String, Object?> target) {
    final fxId = '${fx['id']}';
    final label = '${fx['label'] ?? fx['type']}';
    final enabled = fx['enabled'] != false;
    final mix = (fx['mix'] as Map?) ?? const {'value': 1.0, 'keyed': false, 'animated': false};
    final params = ((fx['params'] as List?) ?? const []).cast<Map>();
    final anyKeyed = mix['keyed'] == true || params.any((p) => p['keyed'] == true);
    final anyAnimated = anyKeyed || mix['animated'] == true || params.any((p) => p['animated'] == true);
    final isOpen = _fxOpen == fxId;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: rw.SwipeActions(
        actionWidth: 74,
        actions: [
          rw.SwipeAction('Copy', () => _cmd({'op': 'fx_duplicate', 'id': fxId, ...target}), icon: 'copy'),
          rw.SwipeAction('Remove', () => _cmd({'op': 'fx_remove', 'id': fxId, ...target}), danger: true, icon: 'bin'),
        ],
        child: Container(
          decoration: BoxDecoration(
            color: isOpen ? cSelected : cControl,
            borderRadius: BorderRadius.circular(10),
            border: isOpen ? Border.all(color: cAccent.withValues(alpha: 0.35), width: 1) : null,
          ),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            SizedBox(
              height: 38,
              child: Row(children: [
                Expanded(
                  child: kit.Tap(
                    label: isOpen ? 'Collapse $label' : 'Expand $label',
                    minSize: 0,
                    onTap: () => setState(() => _fxOpen = isOpen ? null : fxId),
                    child: Padding(
                      padding: const EdgeInsets.only(left: 10, right: 4),
                      child: Row(children: [
                        Transform.rotate(
                          angle: isOpen ? 0 : -math.pi / 2,
                          child: const Pictogram('chevron', size: 13, color: cSecondary),
                        ),
                        const SizedBox(width: 6),
                        Flexible(
                          child: Text(
                            label,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: kit.ink(13.5, FontWeight.w600, enabled ? cText : cSecondary),
                          ),
                        ),
                        if (anyAnimated) ...[
                          const SizedBox(width: 6),
                          Transform.rotate(
                            angle: math.pi / 4,
                            child: Container(
                              width: 7,
                              height: 7,
                              decoration: BoxDecoration(
                                color: anyKeyed ? cAccent : Colors.transparent,
                                border: Border.all(color: anyKeyed ? cAccent : const Color(0xFFD9821E), width: 1.5),
                                borderRadius: BorderRadius.circular(1.2),
                              ),
                            ),
                          ),
                        ],
                      ]),
                    ),
                  ),
                ),
                kit.Tap(
                  label: enabled ? 'Disable $label' : 'Enable $label',
                  minSize: 34,
                  onTap: () => _cmd({'op': 'fx_set', 'id': fxId, 'enabled': !enabled, ...target}),
                  child: Pictogram('eye', size: 16, color: enabled ? cAccent : const Color(0xFFC5C9D0)),
                ),
              ]),
            ),
            if (isOpen)
              Container(
                margin: const EdgeInsets.fromLTRB(4, 0, 4, 4),
                padding: const EdgeInsets.fromLTRB(10, 6, 8, 8),
                decoration: BoxDecoration(
                  color: cCard,
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: const Color(0xFFE6E8EB)),
                ),
                child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                  Row(children: [
                    kit.Tap(
                      label: 'Move effect up',
                      minSize: 30,
                      radius: 7,
                      onTap: index > 0 ? () => _cmd({'op': 'fx_reorder', 'id': fxId, 'dir': -1, ...target}) : null,
                      child: Opacity(
                        opacity: index > 0 ? 1 : 0.3,
                        child: Transform.rotate(angle: math.pi, child: const Pictogram('chevron', size: 15, color: cInk2b)),
                      ),
                    ),
                    kit.Tap(
                      label: 'Move effect down',
                      minSize: 30,
                      radius: 7,
                      onTap: index + 1 < count ? () => _cmd({'op': 'fx_reorder', 'id': fxId, 'dir': 1, ...target}) : null,
                      child: Opacity(
                        opacity: index + 1 < count ? 1 : 0.3,
                        child: const Pictogram('chevron', size: 15, color: cInk2b),
                      ),
                    ),
                    const Spacer(),
                    kit.Tap(
                      label: 'Duplicate $label',
                      minSize: 30,
                      radius: 7,
                      onTap: () {
                        final st = _cmd({'op': 'fx_duplicate', 'id': fxId, ...target});
                        final newStack = _stackFromState(st, group: target.containsKey('group'));
                        if (newStack.length > index + 1) setState(() => _fxOpen = '${newStack[index + 1]['id']}');
                      },
                      child: const Pictogram('copy', size: 15, color: cAccent),
                    ),
                    const SizedBox(width: 2),
                    kit.Tap(
                      label: 'Delete $label',
                      minSize: 30,
                      radius: 7,
                      onTap: () {
                        _cmd({'op': 'fx_remove', 'id': fxId, ...target});
                        setState(() => _fxOpen = null);
                      },
                      child: const Pictogram('bin', size: 15, color: cDanger),
                    ),
                  ]),
                  const Divider(height: 8, color: Color(0xFFEEF0F2)),
                  for (final p in params) _fxParamControl(p, fxId, target),
                  _fxParamControl(mix, fxId, target, isMix: true),
                ]),
              ),
          ]),
        ),
      ),
    );
  }

  List<Map> _stackFromState(Map<String, dynamic>? st, {bool group = false}) {
    if (st == null) return const [];
    final src = group
        ? st['group'] as Map?
        : (st['adjustment'] ?? st['selected'] ?? st['picture'] ?? st['movieclip'] ?? st['textfield']) as Map?;
    return ((src?['effectStack'] as List?) ?? const []).cast<Map>();
  }

  /// Pro Effect Stack section (shared by Stickfigure, Effect Layer, Picture,
  /// Movieclip, Text, and Group pages).
  List<rw.Desc> _effectRows(Map<String, dynamic> sel, {bool group = false, bool adjustment = false, bool showBlending = true}) {
    final f = (sel['filters'] as Map<String, dynamic>?) ?? const <String, dynamic>{};
    double v(String k, double d) => (f[k] as num?)?.toDouble() ?? d;
    void filter(Map<String, Object?> m) => _cmd({'op': 'filter', ...m});
    final target = <String, Object?>{
      if (group && sel['layer'] != null) 'group': sel['layer']
      else if (sel['layer'] != null) 'layer': sel['layer'],
    };

    final stack = ((sel['effectStack'] as List?) ?? const []).cast<Map>();
    final moveList = <(String, String, bool, Map<String, Object?>)>[
      for (final k in const ['wiggle', 'swing'])
        if (!group && !adjustment && f[k] is Map) (k, k == 'wiggle' ? 'Wiggle' : 'Swing', (f[k] as Map)['shown'] != false, {'op': 'move_fx', 'kind': k, 'on': false}),
    ];
    final plugins = (group || adjustment) ? const <Map>[] : ((sel['pluginEffects'] as List?) ?? const []).cast<Map>();

    List<rw.Desc> moveSettings(String k) {
      final m = (f[k] as Map?) ?? const {};
      return [
        rw.DStep(k == 'wiggle' ? 'Distance' : 'Angle', '${((m['amount'] as num?) ?? 0).round()}${k == 'swing' ? '°' : ''}',
            onMinus: () => _cmd({'op': 'move_fx', 'kind': k, 'amount': math.max(0, ((m['amount'] as num?) ?? 0) - 2)}),
            onPlus: () => _cmd({'op': 'move_fx', 'kind': k, 'amount': ((m['amount'] as num?) ?? 0) + 2})),
        rw.DStep('Speed', ((m['speed'] as num?) ?? 1).toStringAsFixed(2),
            onMinus: () => _cmd({'op': 'move_fx', 'kind': k, 'speed': math.max(0.05, ((m['speed'] as num?) ?? 1) - 0.25)}),
            onPlus: () => _cmd({'op': 'move_fx', 'kind': k, 'speed': ((m['speed'] as num?) ?? 1) + 0.25})),
      ];
    }

    List<rw.Desc> pluginSettings(Map pe) {
      void put(Map<String, Object?> props) => _cmd({'op': 'plugin_effect', 'id': pe['id'], 'props': props});
      return [
        for (final p in ((pe['settings'] as List?) ?? const []).cast<Map>())
          switch (p['kind']) {
            'number' || 'angle' => () {
                final x = (p['extra'] as Map?) ?? const {};
                final step = ((x['step'] as num?) ?? (p['kind'] == 'angle' ? 5 : ((x['max'] as num?) != null && (x['max'] as num) <= 1 ? 0.05 : 1))).toDouble();
                final val = ((p['value'] as num?) ?? 0).toDouble();
                double c(double y) => y.clamp(((x['min'] as num?) ?? -1e9).toDouble(), ((x['max'] as num?) ?? 1e9).toDouble());
                return rw.DStep('${p['label']}', step >= 1 ? '${val.round()}' : val.toStringAsFixed(2), onMinus: () => put({p['name']: c(val - step)}), onPlus: () => put({p['name']: c(val + step)}));
              }(),
            'toggle' => rw.DTick('${p['label']}', p['value'] == true, onChanged: (val) => put({p['name']: val})),
            'colour' => rw.DWidget(_swatches(((p['value'] as num?) ?? 0xffffffff).toInt(), (c) => put({p['name']: c}))),
            'choice' => () {
                final opts = (((p['extra'] as Map?)?['options'] as List?) ?? const []).cast<String>();
                final i = opts.indexOf('${p['value']}');
                return rw.DLink('${p['label']}', '${p['value']}', onTap: opts.isEmpty ? null : () => put({p['name']: (i + 1) % opts.length}));
              }(),
            _ => rw.DInfo('${p['label']}', '${p['value']}'),
          },
        if (pe['off'] != null) rw.DHint('${pe['off']} Import the plugin again to switch it back on.'),
      ];
    }

    return [
      rw.DTitle('Effects', onAdd: _openFxBrowser, addLabel: 'Add effect'),
      if (stack.isEmpty && moveList.isEmpty && plugins.isEmpty) const rw.DHint('No effects.'),
      for (var i = 0; i < stack.length; i++) rw.DWidget(_effectStackCard(stack[i], i, stack.length, target)),
      for (final (k, label, shown, remove) in moveList) ...[
        rw.DFx(label,
            shown: shown,
            open: _fxOpen == k,
            onTap: () => setState(() => _fxOpen = _fxOpen == k ? null : k),
            onEye: () => _cmd({'op': 'move_fx', 'kind': k, 'shown': !shown}),
            onRemove: () => _cmd(remove)),
        if (_fxOpen == k) ...moveSettings(k),
      ],
      for (final pe in plugins) ...[
        rw.DFx(pe['missing'] == true ? 'Missing: ${pe['name']}' : '${pe['name']}',
            note: pe['author'] != null ? 'by ${pe['author']}' : null,
            missing: pe['missing'] == true || pe['off'] != null,
            shown: pe['enabled'] != false,
            open: _fxOpen == 'plugin:${pe['id']}',
            onTap: () => setState(() => _fxOpen = _fxOpen == 'plugin:${pe['id']}' ? null : 'plugin:${pe['id']}'),
            onEye: () => _cmd({'op': 'plugin_effect', 'id': pe['id'], 'enabled': pe['enabled'] == false}),
            onRemove: () => _cmd({'op': 'plugin_effect', 'id': pe['id'], 'remove': true})),
        if (_fxOpen == 'plugin:${pe['id']}' && pe['missing'] != true) ...pluginSettings(pe),
      ],
      if (plugins.any((pe) => pe['missing'] == true)) rw.DHint('${sel['name'] ?? 'This layer'} draws without a missing plugin until it is imported again.'),
      if (!group && !adjustment && showBlending) ...[
        if (stack.isNotEmpty) rw.DButtons('Copy effects', () => _cmd({'op': 'copy_filters'}), b: 'Paste effects', onB: _st['filtersCopied'] == true ? () => _cmd({'op': 'paste_filters'}) : null),
        if (stack.isEmpty && _st['filtersCopied'] == true) rw.DButtons('Paste effects', () => _cmd({'op': 'paste_filters'})),
        const rw.DTitle('Blending'),
        rw.DSlider('Opacity', '${(v('opacity', 1) * 100).round()}%', v('opacity', 1), onChanged: (x) => filter({'opacity': (x * 100).round() / 100})),
      ],
    ];
  }

  /// Categorized Pro Add Effect browser (Blur, Light & Glow, Color, Distort,
  /// Stylize, Motion, Plugins) with live previews, search, and Workshop link.
  Widget _effectsBrowser() {
    final q = _fxSearch.toLowerCase();
    bool match(String label) => q.isEmpty || label.toLowerCase().contains(q);
    void close() => setState(() => _fxBrowser = false);

    final isGroup = _st['group'] != null;
    final isAdj = _st['adjustment'] != null;
    final isFig = _sel != null;
    final target = <String, Object?>{
      if (isGroup) 'group': (_st['group'] as Map)['id']
      else if (_activeLayerId != null) 'layer': _activeLayerId,
    };

    void addBuiltIn(String ty) {
      // several figures: no layer named, so `each` adds one to every figure
      // (a named layer would win over each figure's turn: N copies on one)
      final multi = _st['multi'] != null && !isGroup;
      final cmd = <String, Object?>{'op': 'fx_add', 'type': ty, if (!multi) ...target};
      final st = multi ? _cmd({'op': 'each', 'label': 'Add effect', 'cmd': cmd}) : _cmd(cmd);
      final stack = _stackFromState(st, group: isGroup);
      final newId = stack.isNotEmpty ? '${stack.last['id']}' : ty;
      setState(() {
        _fxBrowser = false;
        _fxOpen = newId;
        _fxSearch = '';
      });
      if (isAdj) {
        _jumpTo('Effect Layer');
      } else if (isGroup) {
        _jumpTo('Group Tools');
      } else if (isFig) {
        _jumpTo('Stickfigure Tools');
      }
    }

    void pickRaw(String k, Map<String, Object?> cmd) {
      if (_st['multi'] != null) {
        _cmd({'op': 'each', 'label': 'Add effect', 'cmd': cmd});
      } else {
        _cmd(cmd);
      }
      setState(() {
        _fxBrowser = false;
        _fxOpen = k;
        _fxSearch = '';
      });
      if (isFig) _jumpTo('Stickfigure Tools');
    }

    return LayoutBuilder(builder: (context, box) {
      final compact = box.maxHeight < 260;
      final cols = compact && box.maxWidth >= 340 ? 4 : 3;
      final tileH = compact ? 42.0 : 54.0;
      Widget cTile(String label, Widget picture, VoidCallback? onTap) => Opacity(
            opacity: onTap == null ? 0.4 : 1,
            child: kit.Tap(
              label: label,
              minSize: 0,
              onTap: onTap,
              child: Column(mainAxisSize: MainAxisSize.min, children: [
                Container(
                  height: tileH,
                  clipBehavior: Clip.antiAlias,
                  decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(8), border: Border.all(color: cLine)),
                  child: picture,
                ),
                const SizedBox(height: 2),
                Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: cText, fontSize: compact ? 10.5 : 11.5, fontWeight: FontWeight.w500)),
              ]),
            ),
          );
      Widget cGrid(List<Widget> tiles) => GridView.count(
            crossAxisCount: cols,
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            mainAxisSpacing: compact ? 6 : 9,
            crossAxisSpacing: compact ? 6 : 8,
            childAspectRatio: compact ? 1.38 : 1.24,
            children: tiles,
          );
      Widget pic(String k) => _fxPreviews[k] == null ? const Center(child: Pictogram('fx', size: 20, color: cAccent)) : RawImage(image: _fxPreviews[k], fit: BoxFit.contain);
      Widget title(String t) => Padding(padding: EdgeInsets.only(top: compact ? 6 : 10, bottom: compact ? 3 : 5), child: Text(t, style: TextStyle(color: cTitle, fontSize: compact ? 12.5 : 13.5, fontWeight: FontWeight.w700)));

      final catalog = _effectCatalog;
      const builtInCats = ['Blur', 'Light & Glow', 'Color', 'Distort', 'Stylize'];
      final sections = <Widget>[];
      for (final cat in builtInCats) {
        if (_fxCat != 'All' && _fxCat != cat) continue;
        final tiles = [
          for (final item in catalog)
            if ('${item['category']}' == cat && match('${item['label']}'))
              cTile('${item['label']}', pic('${item['type']}'), () => addBuiltIn('${item['type']}')),
        ];
        if (tiles.isNotEmpty) sections..add(title(cat))..add(cGrid(tiles));
      }
      if ((_fxCat == 'All' || _fxCat == 'Motion') && isFig && !isGroup && !isAdj) {
        final moves = [
          for (final (k, l, icon) in _fxMovement)
            if (match(l)) cTile(l, Center(child: Pictogram(icon, size: compact ? 22 : 28, color: cAccent, stroke: 1.8)), () => pickRaw(k, {'op': 'move_fx', 'kind': k, 'on': true})),
        ];
        if (moves.isNotEmpty) sections..add(title('Motion'))..add(cGrid(moves));
      }
      if (_fxCat == 'All' || _fxCat == 'Plugins') {
        final yours = [
          for (final p in _effectPlugins)
            if (match('${p['name']}'))
              cTile('${p['name']}', pic('plugin:${p['id']}'), p['off'] != null || _st['multi'] != null ? null : () => pickRaw('plugin:${p['id']}', {'op': 'plugin_effect', 'id': p['id']})),
        ];
        if (yours.isNotEmpty) sections..add(title('Plugins'))..add(cGrid(yours));
      }

      final searchField = TextField(
        style: const TextStyle(color: cText, fontSize: 13),
        decoration: InputDecoration(
          isDense: true,
          filled: true,
          fillColor: cControl,
          hintText: 'Search effects',
          prefixIcon: const Icon(Icons.search, size: 17, color: Color(0xFF8A9099)),
          prefixIconConstraints: const BoxConstraints(minWidth: 32, minHeight: 28),
          contentPadding: EdgeInsets.symmetric(vertical: compact ? 6 : 8, horizontal: 8),
          border: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: BorderSide.none),
        ),
        onChanged: (v) => setState(() => _fxSearch = v),
      );
      final catBar = SizedBox(
        height: 30,
        child: ListView(
          scrollDirection: Axis.horizontal,
          padding: EdgeInsets.symmetric(horizontal: compact ? 8 : 14),
          children: [
            for (final cat in _fxCategories)
              Padding(
                padding: const EdgeInsets.only(right: 5),
                child: kit.Tap(
                  label: cat,
                  minSize: 0,
                  radius: 14,
                  onTap: () => setState(() => _fxCat = cat),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: _fxCat == cat ? cAccent : cControl,
                      borderRadius: BorderRadius.circular(14),
                    ),
                    child: Text(cat, style: TextStyle(color: _fxCat == cat ? cCard : cText, fontSize: 11.5, fontWeight: FontWeight.w600)),
                  ),
                ),
              ),
          ],
        ),
      );

      return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        if (compact)
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 4, 6, 4),
            child: Row(children: [
              kit.WordButton('‹ Back', size: 13, height: 30, onTap: close),
              const SizedBox(width: 4),
              Expanded(child: searchField),
              const SizedBox(width: 4),
              kit.WordButton('Workshop', size: 12.5, height: 30, onTap: () {
                close();
                _openWorkshop();
              }),
            ]),
          )
        else ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(6, 6, 8, 0),
            child: Row(children: [
              kit.WordButton('‹ Back', size: 13.5, onTap: close),
              const Spacer(),
              kit.WordButton('Workshop…', size: 13, onTap: () {
                close();
                _openWorkshop();
              }),
              kit.WordButton('Import…', size: 13, onTap: _importPlugin),
            ]),
          ),
          Padding(padding: const EdgeInsets.fromLTRB(14, 4, 14, 6), child: searchField),
        ],
        catBar,
        const SizedBox(height: 4),
        Expanded(
          child: ListView(padding: EdgeInsets.fromLTRB(compact ? 10 : 14, 0, compact ? 10 : 14, 12), children: [
            ...sections,
            if (sections.isEmpty) const Padding(padding: EdgeInsets.only(top: 18), child: Text('No effects match that filter.', textAlign: TextAlign.center, style: TextStyle(color: cSecondary, fontSize: 13))),
          ]),
        ),
      ]);
    });
  }

  /// Stickfigure Tools as rows (D15; REDESIGN §8 Stickfigure page). Every old
  /// control is kept; the pencil is "Edit segments" (the figure editor).
  List<Widget> _stickfigureTools(Map<String, dynamic> sel) {
    final scale = (sel['scale'] as num?)?.toDouble() ?? 1;
    final angle = (sel['figAngle'] as num?)?.toDouble() ?? 0;
    final stretch = _opts['stretch'] == true;
    final joining = _st['joining'] == true;
    final rows = <rw.Desc>[
      rw.DName(sel['name'] as String? ?? 'Figure', link: 'Duplicate & edit…', onLink: () => _editCopySheet(sel['name'] as String? ?? 'Figure')),
      rw.DButtons(
        'Edit figure',
        () => _cmd({'op': 'build'}),
        iconA: 'edit',
        b: 'Separate joint',
        onB: sel['root'] != true ? _separateJoint : null,
        iconB: 'separate',
      ),
      rw.DWidget(kit.PropRow('Colour', trailing: _swatches((sel['figColor'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'figure', 'color': c})))),
      ..._effectRows(sel),
      rw.DWidget(BlendModeRow(current: '${sel['blend'] ?? 'NORMAL'}', onChanged: (v) => _cmd({'op': 'blend', 'v': v}))),
      const rw.DTitle('Transform'),
      rw.DButtons('Flip sideways', () => _cmd({'op': 'flip_figure', 'h': true}), iconA: 'flipH', b: 'Flip over', onB: () => _cmd({'op': 'flip_figure', 'h': false}), iconB: 'flipV'),
      rw.DButtons('Send back', () => _cmd({'op': 'layer_order', 'forward': false}), b: 'Bring forward', onB: () => _cmd({'op': 'layer_order', 'forward': true})),
      rw.DStep('Scale', scale.toStringAsFixed(2), onMinus: scale > 0.05 ? () => _cmd({'op': 'figure', 'scale': math.max(0.05, scale - 0.05)}) : null, onPlus: () => _cmd({'op': 'figure', 'scale': scale + 0.05})),
      rw.DButtons(angle.round() == 0 ? 'Turn 45°' : 'Turn 45° (${angle.round()}°)', () => _cmd({'op': 'figure', 'turn': 45}), b: 'Center', onB: () => _cmd({'op': 'figure', 'center': true})),
      rw.DSeg(const ['Rotates', 'Stretches'], stretch ? 1 : 0, label: 'Joint drag', onSelect: (i) => _cmd({'op': 'set', 'k': 'stretch', 'v': i == 1})),
      if (joining) ...[
        rw.DSeg(
          const ['Keep offset', 'Snap to joint'],
          _st['joiningMode'] == 'absolute' ? 1 : 0,
          label: 'Attach mode',
          onSelect: (i) => _cmd({'op': 'join', 'mode': i == 1 ? 'absolute' : 'relative'}),
        ),
        rw.DButtons('Cancel', () => _cmd({'op': 'join_cancel'})),
      ] else if (sel['joinedTo'] != null) ...[
        rw.DWidget(kit.PropRow('Attached to', trailing: Text('${sel['joinedTo']}', style: const TextStyle(color: cText, fontSize: 14, fontWeight: FontWeight.w600)))),
        rw.DButtons(
          'Detach',
          () {
            final from = sel['joinedTo'];
            _cmd({'op': 'unjoin', 'all': true});
            kit.showToast(context, 'Detached ${sel['name']} from $from', light: true, icon: 'broken', iconColor: cDanger, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          },
          iconA: 'broken',
          b: _frame > 0 ? 'Detach from here' : null,
          onB: _frame > 0
              ? () {
                  final from = sel['joinedTo'];
                  _cmd({'op': 'let_go_here'});
                  kit.showToast(context, 'Detached ${sel['name']} from $from at frame ${_frame + 1}', light: true, icon: 'broken', iconColor: cDanger, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
                }
              : null,
        ),
      ] else ...[
        const rw.DTitle('Attach'),
        rw.DButtons(
          'Keep offset',
          () => _cmd({'op': 'join', 'mode': 'relative'}),
          iconA: 'chain',
          b: 'Snap to joint',
          onB: () => _cmd({'op': 'join', 'mode': 'absolute'}),
          iconB: 'attach',
        ),
      ],
      rw.DTick('Locked', sel['locked'] == true, onChanged: (v) => _cmd({'op': 'figure', 'locked': v})),
      rw.DTick('Onion skin', sel['onion'] != false, onChanged: (v) => _cmd({'op': 'figure', 'onion': v})),
      const rw.DTitle('Tweening'),
      rw.DTick('Keep in tween', sel['keepInTween'] != false, onChanged: (v) => _cmd({'op': 'figure', 'keepInTween': v})),
      rw.DTick('Blend colours', sel['tweenColors'] != false, onChanged: (v) => _cmd({'op': 'figure', 'tweenColors': v})),
      const rw.DTitle('Actions'),
      rw.DWidget(GestureDetector(
        onLongPress: _pasteLayeringPopup,
        child: rw.renderDesc(rw.DButtons('Copy figure', () => _cmd({'op': 'copy_figure', 'joined': true}), iconA: 'copy', b: 'Paste figure', onB: _st['figureCopied'] == true ? _pasteFigure : null, iconB: 'paste')),
      )),
      rw.DButtons('Copy style', () => _cmd({'op': 'copy_props'}), b: 'Paste style', onB: _st['propsCopied'] == true ? _pastePropsDialog : null),
      rw.DButtons('Convert to movieclip…', () async {
        final ok = await _ask('Convert to movieclip?', 'This figure and attached figures become a looping movieclip. You can break it apart later.', ['Cancel', 'Convert']);
        if (ok == 'Convert') _cmd({'op': 'to_movieclip'});
      }),
      rw.DWidget(GestureDetector(
        onLongPress: () => _cmd({'op': 'delete_here', 'joined': true}),
        child: rw.renderDesc(rw.DButtons('Hide here', () => _cmd({'op': 'delete_here'}), iconA: 'eye')),
      )),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  String? _textClip;

  /// Textfield Tools as rows (D15; REDESIGN §8 Text page).
  List<Widget> _textfieldTools(Map<String, dynamic>? tf) {
    final List<rw.Desc> rows;
    if (tf == null) {
      rows = [
        rw.DButtons('Add text', () => _cmd({'op': 'add_textfield'}), b: 'Paste', onB: _st['textfieldCopied'] == true ? () => _cmd({'op': 'paste_textfield'}) : null),
      ];
    } else {
      final align = {'left': 0, 'center': 1, 'right': 2}[tf['align'] as String?] ?? 1;
      final scale = (tf['scale'] as num?)?.toDouble() ?? 1;
      final opacity = ((tf['opacity'] as num?) ?? ((tf['filters'] as Map?)?['opacity'] as num?) ?? 1).toDouble();
      rows = [
        rw.DWidget(_TextFieldBox(text: tf['text'] as String? ?? '', onChanged: (v) => _cmd({'op': 'textfield', 'text': v}))),
        const rw.DWidget(SizedBox(height: 6)),
        rw.DSeg(const ['Left', 'Center', 'Right'], align, label: 'Alignment', onSelect: (i) => _cmd({'op': 'textfield', 'align': i})),
        rw.DWidget(kit.PropRow('Colour', trailing: _swatches((tf['color'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'textfield', 'color': c})))),
        rw.DStep('Scale', scale.toStringAsFixed(2), onMinus: scale > 0.05 ? () => _cmd({'op': 'textfield', 'scale': math.max(0.05, scale - 0.05)}) : null, onPlus: () => _cmd({'op': 'textfield', 'scale': scale + 0.05})),
        ..._effectRows(tf, showBlending: false),
        const rw.DTitle('Blending'),
        rw.DSlider('Opacity', '${(opacity * 100).round()}%', opacity, onChanged: (x) => _cmd({'op': 'filter', 'opacity': (x * 100).round() / 100})),
        rw.DWidget(BlendModeRow(current: '${tf['blend'] ?? 'NORMAL'}', onChanged: (v) => _cmd({'op': 'blend', 'v': v}))),
        rw.DTick('Locked', tf['locked'] == true, onChanged: (v) {
          if (v && tf['cameraLocked'] == true) _cmd({'op': 'camera_lock', 'layer': tf['layer'], 'on': false});
          _cmd({'op': 'textfield', 'locked': v});
        }),
        rw.DTick('Locked to camera', tf['cameraLocked'] == true, onChanged: (v) {
          if (v && tf['locked'] == true) _cmd({'op': 'textfield', 'locked': false});
          _cmd({'op': 'camera_lock', 'layer': tf['layer'], 'on': v});
        }),
        rw.DButtons('Copy text', () => _cmd({'op': 'copy_textfield'}), iconA: 'copy', b: 'Paste text', onB: _st['textfieldCopied'] == true ? () => _cmd({'op': 'paste_textfield'}) : null, iconB: 'paste'),
        const rw.DTitle('Font'),
        for (final w in _fontChoice(tf)) rw.DWidget(w),
        rw.DButtons('Copy words', () => setState(() => _textClip = tf['text'] as String? ?? ''), b: 'Paste words', onB: _textClip != null ? () => _cmd({'op': 'textfield', 'text': _textClip}) : null),
      ];
    }
    return [for (final d in rows) rw.renderDesc(d)];
  }

  List<Widget> _stickfigureFilters(Map<String, dynamic> sel) {
    final f = (sel['filters'] as Map<String, dynamic>?) ?? const <String, dynamic>{};
    double v(String k, double d) => (f[k] as num?)?.toDouble() ?? d;
    bool on(String k) => f[k] == true;
    int col(String k, int d) => (f[k] as num?)?.toInt() ?? d;
    void shadow({double? distance, double? angle}) => _cmd({'op': 'filter', 'shadowDistance': distance ?? v('shadowDistance', 10), 'shadowAngle': angle ?? v('shadowAngle', 135)});
    return [
      _hint('Filters show on the stage and in exports; many of them can slow the editor down.'),
      _gap,
      _stepper('Opacity', v('opacity', 1).toStringAsFixed(2), () => _cmd({'op': 'filter', 'opacity': math.max(0, v('opacity', 1) - 0.05)}), () => _cmd({'op': 'filter', 'opacity': math.min(1, v('opacity', 1) + 0.05)})),
      _gap,
      _Toggle('Invert colour', on('invert'), (x) => _cmd({'op': 'filter', 'invert': x})),
      _gap,
      _stepper('Hue shift', v('hue', 0).round().toString(), () => _cmd({'op': 'filter', 'hue': v('hue', 0) - 5}), () => _cmd({'op': 'filter', 'hue': v('hue', 0) + 5})),
      _gap,
      _stepper('Saturation', v('saturation', 1).toStringAsFixed(2), () => _cmd({'op': 'filter', 'saturation': math.max(0, v('saturation', 1) - 0.05)}), () => _cmd({'op': 'filter', 'saturation': math.min(4, v('saturation', 1) + 0.05)})),
      _gap,
      _stepper('Pixelate', v('pixelate', 1).round().toString(), () => _cmd({'op': 'filter', 'pixelate': math.max(1, v('pixelate', 1) - 1)}), () => _cmd({'op': 'filter', 'pixelate': v('pixelate', 1) + 1})),
      _gap,
      _label('Tint'),
      _swatches(col('tint', 0xffffffff), (c) => _cmd({'op': 'filter', 'tint': c})),
      _stepper('Amount', v('tintAmount', 0).toStringAsFixed(2), () => _cmd({'op': 'filter', 'tintAmount': math.max(0, v('tintAmount', 0) - 0.05)}), () => _cmd({'op': 'filter', 'tintAmount': math.min(1, v('tintAmount', 0) + 0.05)})),
      _gap,
      _label('Outline'),
      _swatches(col('outline', 0xff00ffff), (c) => _cmd({'op': 'filter', 'outline': c})),
      _stepper('Amount', v('outlineAmount', 1).toStringAsFixed(2), () => _cmd({'op': 'filter', 'outlineAmount': math.max(0, v('outlineAmount', 1) - 0.05)}), () => _cmd({'op': 'filter', 'outlineAmount': math.min(1, v('outlineAmount', 1) + 0.05)})),
      _stepper('Thickness', v('outlineThickness', 4).round().toString(), () => _cmd({'op': 'filter', 'outlineThickness': math.max(0, v('outlineThickness', 4) - 1)}), () => _cmd({'op': 'filter', 'outlineThickness': v('outlineThickness', 4) + 1})),
      _gap,
      _stepper('Blur', v('blur', 0).toStringAsFixed(1), () => _cmd({'op': 'filter', 'blur': math.max(0, v('blur', 0) - 0.5)}), () => _cmd({'op': 'filter', 'blur': v('blur', 0) + 0.5})),
      Opacity(
        opacity: v('blur', 0) > 0 ? 1 : 0.4,
        child: IgnorePointer(
          ignoring: v('blur', 0) <= 0,
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            _Toggle('Motion blur', on('motion'), (x) => _cmd({'op': 'filter', 'motion': x})),
            _stepper('Distance', v('motionDistance', 30).round().toString(), () => _cmd({'op': 'filter', 'motionDistance': math.max(0, v('motionDistance', 30) - 1)}), () => _cmd({'op': 'filter', 'motionDistance': v('motionDistance', 30) + 1})),
          ]),
        ),
      ),
      _gap,
      _label('Glow'),
      _swatches(col('glow', 0x0a84ffff), (c) => _cmd({'op': 'filter', 'glow': c})),
      _stepper('Amount', v('glowAmount', 0).toStringAsFixed(2), () => _cmd({'op': 'filter', 'glowAmount': math.max(0, v('glowAmount', 0) - 0.05)}), () => _cmd({'op': 'filter', 'glowAmount': math.min(1, v('glowAmount', 0) + 0.05)})),
      _stepper('Intensity', v('glowIntensity', 1).toStringAsFixed(2), () => _cmd({'op': 'filter', 'glowIntensity': math.max(0, v('glowIntensity', 1) - 0.1)}), () => _cmd({'op': 'filter', 'glowIntensity': v('glowIntensity', 1) + 0.1})),
      _gap,
      _label('Drop shadow'),
      _swatches(col('shadow', 0x000000ff), (c) => _cmd({'op': 'filter', 'shadow': c})),
      _stepper('Opacity', v('shadowOpacity', 0).toStringAsFixed(2), () => _cmd({'op': 'filter', 'shadowOpacity': math.max(0, v('shadowOpacity', 0) - 0.05)}), () => _cmd({'op': 'filter', 'shadowOpacity': math.min(1, v('shadowOpacity', 0) + 0.05)})),
      _stepper('Distance', v('shadowDistance', 10).round().toString(), () => shadow(distance: math.max(0, v('shadowDistance', 10) - 1)), () => shadow(distance: v('shadowDistance', 10) + 1)),
      _stepper('Angle', '${v('shadowAngle', 135).round()}°', () => shadow(angle: v('shadowAngle', 135) - 5), () => shadow(angle: v('shadowAngle', 135) + 5)),
      _stepper('Blur', v('shadowBlur', 6).toStringAsFixed(1), () => _cmd({'op': 'filter', 'shadowBlur': math.max(0, v('shadowBlur', 6) - 0.5)}), () => _cmd({'op': 'filter', 'shadowBlur': v('shadowBlur', 6) + 0.5})),
      _gap,
      _Btn('Clear all filters', onTap: () => _cmd({'op': 'clear_filters'})),
      _gap,
      _pair(_Btn('Copy filters', onTap: () => _cmd({'op': 'copy_filters'})), _Btn('Paste filters', enabled: _st['filtersCopied'] == true, onTap: () => _cmd({'op': 'paste_filters'}))),
    ];
  }

  /// Segment Tools: gradient, segment scale, the shape's own options and
  /// angle lock (not keyed: they belong to the segment, like its colour).
  /// Separate joint: extracts the selected non-root joint and its descendant
  /// branch into a new independent Stickfigure layer, snapped (Absolute Join)
  /// onto its former parent joint so visually nothing jumps — 1 undo step.
  void _separateJoint() {
    final before = _sel?['layer'];
    final st = _cmd({'op': 'separate_node'});
    final now = (st?['selected'] as Map?)?['layer'];
    if (now == null || now == before) return; // the engine explained why
    kit.showToast(context, 'Joint separated into its own figure, snapped to its parent.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
  }

  /// The selected segment's values next to how its figure came (D7b):
  /// `defaults` from the engine; figures from before D7b have none, and then
  /// only thickness (32) and length (the made-with length) compare (D7a).
  Map<String, dynamic> _segDefaults(Map<String, dynamic> sel) {
    final d = sel['defaults'];
    if (d is Map) return Map<String, dynamic>.from(d);
    final rest = (sel['restLength'] as num?)?.toDouble() ?? 0;
    return {'thickness': 32, if (rest > 0) 'length': rest};
  }

  bool _segChanged(Map<String, dynamic> sel, Map<String, dynamic> d, String k) {
    if (!d.containsKey(k)) return false;
    final a = sel[k], b = d[k];
    if (a is num && b is num) return (a - b).abs() > 0.01;
    return a != b;
  }

  /// Every value that differs from how the segment came.
  bool _segAnyChanged(Map<String, dynamic> sel) {
    if (sel['root'] == true) return false;
    final d = _segDefaults(sel);
    return d.keys.any((k) => _segChanged(sel, d, k));
  }

  /// Segment Tools as rows (D15; REDESIGN §4 Segment page). A value that
  /// differs from how the figure came shows a small dot; swiping its row (or
  /// right-clicking it) offers the reset (D11). "Reset this segment" is pinned
  /// under the card while anything differs.
  List<Widget> _segmentTools(Map<String, dynamic> sel) {
    final root = sel['root'] == true;
    if (root) {
      return [
        for (final d in <rw.Desc>[
          rw.DTick('Drag lock', sel['dragLock'] == true, onChanged: (v) => _cmd({'op': 'node', 'dragLock': v})),
        ])
          rw.renderDesc(d),
      ];
    }
    void seg(Map<String, Object?> m) => _cmd({'op': 'segment', ...m});
    final d = _segDefaults(sel);
    bool ch(String k) => _segChanged(sel, d, k);
    String onOff(Object? v) => v == true ? 'on' : 'off';
    // the reset for one value: "Reset to 32", "Reset to off"
    rw.DReset? reset(String k, String label, Map<String, Object?> cmd) => ch(k) ? rw.DReset('Reset to $label', () => _cmd(cmd)) : null;
    rw.DReset? resetSeg(String k, [String? label]) => reset(k, label ?? (d[k] is bool ? onOff(d[k]) : '${d[k]}'), {'op': 'segment', k: d[k]});
    rw.DTick tick(String label, String k) => rw.DTick(label, sel[k] == true, onChanged: (v) => seg({k: v}), changed: ch(k), reset: resetSeg(k));
    String hex(Object? c) => '#${(((c as num?)?.toInt() ?? 0) >> 8).toRadixString(16).padLeft(6, '0').toUpperCase()}';

    final thick = (sel['thickness'] as num?)?.toDouble() ?? 0;
    final len = (sel['length'] as num?)?.toDouble() ?? 0;
    final shape = sel['shape'] as String? ?? 'line';
    final grad = sel['gradient'] == true;
    final scale = (sel['segScale'] as num?)?.toDouble() ?? 1;
    final curve = (sel['curve'] as num?)?.toDouble() ?? 0;
    final sides = (sel['sides'] as num?)?.toInt() ?? 5;
    final right = (sel['rightTriangle'] as num?)?.toInt() ?? 0;
    final half = (sel['halfArc'] as num?)?.toInt() ?? 0;
    final lock = (sel['angleLock'] as num?)?.toInt() ?? 0;
    final trap = ((sel['trapRatio'] as num?) ?? 1).toDouble();
    final own = sel['ownColor'] == true;
    final rows = <rw.Desc>[
      // D35: the part's own name (the Parts list and the graph show it)
      rw.DName('${sel['partName'] ?? 'Segment'}', link: 'Rename', onLink: () => _renamePart('${sel['node']}', '${sel['partName'] ?? ''}', named: sel['partNamed'] == true)),
      rw.DButtons('Flip sideways', () => _cmd({'op': 'flip_branch', 'h': true}), iconA: 'flipH', b: 'Flip over', onB: () => _cmd({'op': 'flip_branch', 'h': false}), iconB: 'flipV'),
      const rw.DTitle('Size'),
      rw.DStep('Thickness', thick.round().toString(),
          onMinus: () => seg({'thickness': math.max(0, thick - 2)}), onPlus: () => seg({'thickness': thick + 2}), changed: ch('thickness'), reset: reset('thickness', '${(d['thickness'] as num?)?.round()}', {'op': 'segment', 'thickness': d['thickness']})),
      rw.DStep('Length', len.round().toString(),
          onMinus: () => _cmd({'op': 'node', 'length': math.max(0, len - 5)}), onPlus: () => _cmd({'op': 'node', 'length': len + 5}), changed: ch('length'), reset: reset('length', '${(d['length'] as num?)?.round()}', {'op': 'node', 'length': d['length']})),
      rw.DTick('Use segment scale', (scale - 1).abs() > 1e-3, onChanged: (v) => seg({'segScale': v ? 1.5 : 1.0}), changed: ch('segScale'), reset: resetSeg('segScale')),
      if ((scale - 1).abs() > 1e-3) rw.DStep('Segment scale', scale.toStringAsFixed(2), onMinus: () => seg({'segScale': math.max(0.05, scale - 0.05)}), onPlus: () => seg({'segScale': scale + 0.05}), changed: ch('segScale'), reset: resetSeg('segScale')),
      if (shape == 'line' || shape == 'square' || shape == 'trapezoid') rw.DStep('Curve radius', curve.round().toString(), onMinus: () => seg({'curve': curve - 5}), onPlus: () => seg({'curve': curve + 5}), changed: ch('curve'), reset: resetSeg('curve', '${(d['curve'] as num?)?.round()}')),
      if (shape == 'trapezoid') ...[
        rw.DStep('End thickness', '${(trap * 100).round()}%', onMinus: () => seg({'trapRatio': math.max(0, trap - 0.1)}), onPlus: () => seg({'trapRatio': trap + 0.1}), changed: ch('trapRatio'), reset: resetSeg('trapRatio', '${(((d['trapRatio'] as num?) ?? 1) * 100).round()}%')),
        tick('Start rounded', 'trapRoundStart'),
        tick('End rounded', 'trapRoundEnd'),
      ],
      if (shape == 'circle' || shape == 'ellipse') ...[
        rw.DTick('Half arc', half != 0, onChanged: (v) => seg({'halfArc': v ? 1 : 0}), changed: ch('halfArc'), reset: resetSeg('halfArc', d['halfArc'] == 0 ? 'off' : 'on')),
        if (half != 0) rw.DTick('Upside down', half < 0, onChanged: (v) => seg({'halfArc': v ? -1 : 1}), changed: ch('halfArc')),
      ],
      if (shape == 'triangle') ...[
        rw.DTick('Right triangle', right != 0, onChanged: (v) => seg({'rightTriangle': v ? 1 : 0}), changed: ch('rightTriangle'), reset: resetSeg('rightTriangle', d['rightTriangle'] == 0 ? 'off' : 'on')),
        if (right != 0) rw.DTick('Flipped', right < 0, onChanged: (v) => seg({'rightTriangle': v ? -1 : 1}), changed: ch('rightTriangle')),
        tick('Upside down', 'upsideDown'),
      ],
      if (shape == 'polygon') rw.DStep('Vertices', '$sides', onMinus: () => seg({'sides': math.max(3, sides - 1)}), onPlus: () => seg({'sides': sides + 1}), changed: ch('sides'), reset: resetSeg('sides')),
      const rw.DTitle('Colour'),
      tick('Own colour', 'ownColor'),
      rw.DWidget(Opacity(opacity: own ? 1 : 0.4, child: _swatches((sel['color'] as num?)?.toInt() ?? 0xff, (c) => seg({'color': c}))),
          changed: own && ch('color'), reset: own && ch('color') ? rw.DReset('Reset to ${hex(d['color'])}', () => seg({'color': d['color']})) : null),
      const rw.DHint('On: this segment keeps its own colour. Off: it follows the stickfigure colour.'),
      tick('Use gradient colour', 'gradient'),
      if (grad) ...[
        rw.DWidget(_swatches((sel['gradientColor'] as num?)?.toInt() ?? 0x808080ff, (c) => seg({'gradientColor': c})),
            changed: ch('gradientColor'), reset: ch('gradientColor') ? rw.DReset('Reset to ${hex(d['gradientColor'])}', () => seg({'gradientColor': d['gradientColor']})) : null),
        tick('Reverse gradient', 'gradientReverse'),
        if (shape == 'circle' || shape == 'ellipse' || shape == 'polygon')
          rw.DSeg(
            const ['Radial', 'Along', 'Across'],
            sel['gradientLinear'] != true ? 0 : (sel['gradientSideways'] == true ? 2 : 1),
            label: 'Gradient style',
            onSelect: (idx) => seg({'gradientLinear': idx != 0, 'gradientSideways': idx == 2}),
            changed: ch('gradientLinear') || ch('gradientSideways'),
            reset: (ch('gradientLinear') || ch('gradientSideways'))
                ? rw.DReset(
                    'Reset to ${d['gradientLinear'] != true ? 'radial' : (d['gradientSideways'] == true ? 'across' : 'along')}',
                    () => seg({'gradientLinear': d['gradientLinear'] == true, 'gradientSideways': d['gradientSideways'] == true}),
                  )
                : null,
          )
        else
          tick('Sideways gradient', 'gradientSideways'),
      ],
      if (shape == 'circle') ...[
        tick('Circle outline colour', 'outline'),
        if (sel['outline'] == true) ...[
          rw.DWidget(_swatches((sel['outlineColor'] as num?)?.toInt() ?? 0x000000ff, (c) => seg({'outlineColor': c})),
              changed: ch('outlineColor'), reset: ch('outlineColor') ? rw.DReset('Reset to ${hex(d['outlineColor'])}', () => seg({'outlineColor': d['outlineColor']})) : null),
          tick('Is hollow', 'hollow'),
        ],
      ],
      const rw.DTitle('Movement'),
      rw.DTick('Show motion path', _opts['path'] == true, onChanged: _setMotionPath),
      rw.DTick('Is stretchy', sel['stretchy'] == true, onChanged: (v) => _cmd({'op': 'node', 'stretchy': v}), changed: ch('stretchy'), reset: reset('stretchy', onOff(d['stretchy']), {'op': 'node', 'stretchy': d['stretchy']})),
      rw.DTick('Smart stretch', sel['smart'] == true, onChanged: (v) => seg({'smart': v}), changed: ch('smart'), reset: reset('smart', onOff(d['smart']), {'op': 'segment', 'smart': d['smart']})),
      if (sel['smart'] != true)
        rw.DTick('Do not smart-stretch', sel['noSmart'] == true, onChanged: (v) => seg({'noSmart': v})),
      if (_st['building'] != null || sel['stretchReset'] == true || ((sel['stretchMultiplier'] as num?) ?? 1) != 1) ...[
        rw.DTick('Reset stretch impulse', sel['stretchReset'] == true, onChanged: (v) => seg({'stretchReset': v})),
        rw.DStep('Stretch multiplier', '×${((sel['stretchMultiplier'] as num?) ?? 1).toStringAsFixed(2)}',
            onMinus: () => seg({'stretchMultiplier': ((sel['stretchMultiplier'] as num?) ?? 1) - 0.25}),
            onPlus: () => seg({'stretchMultiplier': ((sel['stretchMultiplier'] as num?) ?? 1) + 0.25})),
      ],
      if (_st['building'] != null)
        const rw.DHint('With smart stretch, the still parts under a stretchy segment stretch along with it. A connector under it makes an IK limb: pull its end and the joint between bends. Stock › Stickfigure (IK) is built this way.'),
      rw.DTick('Lock node', sel['pinned'] == true, onChanged: (v) => _cmd({'op': 'pin', 'v': v})),
      rw.DTick('Drag lock', sel['dragLock'] == true, onChanged: (v) => _cmd({'op': 'node', 'dragLock': v})),
      rw.DSeg(const ['None', 'Absolute', 'Relative'], lock, label: 'Angle lock', onSelect: (i) => seg({'angleLock': i})),
      // Separate joint: extracts this limb into its own figure, snapped to its parent
      rw.DButtons('Separate joint', _separateJoint, iconA: 'separate'),
    ];
    return [for (final r in rows) rw.renderDesc(r)];
  }


  (double, double)? _camClip;

  /// Camera Tools as rows (D15; REDESIGN §8 Camera page). The guide's four
  /// spirals share one "Spiral" choice: picking it again turns the spiral.
  List<Widget> _cameraTools() {
    final cam = _st['cam'] as Map<String, dynamic>?;
    final zoom = (cam?['zoom'] as num?)?.toDouble() ?? 1;
    final rot = (cam?['rotation'] as num?)?.toDouble() ?? 0;
    final guide = AppSettings.number('cameraGuide').clamp(0, 6);
    void setGuide(int i) => setState(() {
          AppSettings.values['cameraGuide'] = i < 3 ? i : (guide >= 3 ? 3 + (guide - 3 + 1) % 4 : 3);
          unawaited(AppSettings.save());
        });
    final auto = _st['autoCam'] as Map?;
    final camRow = ((_st['rows'] as List?) ?? const []).cast<Map>().where((r) => r['kind'] == 'camera').firstOrNull;
    final camLayerId = camRow?['id'] as String?;
    final camBlocks = ((camRow?['blocks'] as List?) ?? const []).cast<List>();
    final hasKeyHere = camBlocks.any((b) => (b[0] as num).toInt() == _frame);
    final rows = <rw.Desc>[
      if (camLayerId != null)
        rw.DButtons(
          'Add key',
          hasKeyHere ? null : () => _cmd({'op': 'new_pose', 'layer': camLayerId, 'frame': _frame}),
          iconA: 'key',
          b: 'Delete key',
          onB: hasKeyHere && _frame > 0 ? () => _cmd({'op': 'delete_pose', 'layer': camLayerId, 'frame': _frame}) : null,
          iconB: 'bin',
        ),
      rw.DButtons('Copy', () => setState(() => _camClip = (zoom, rot)), iconA: 'copy', b: 'Paste', onB: _camClip == null ? null : () => _cmd({'op': 'camera', 'zoom': _camClip!.$1, 'rotation': _camClip!.$2}), iconB: 'paste'),
      rw.DStep('Zoom', zoom.toStringAsFixed(2), onMinus: zoom > 0.05 ? () => _cmd({'op': 'camera', 'zoom': math.max(0.05, zoom - 0.05)}) : null, onPlus: () => _cmd({'op': 'camera', 'zoom': zoom + 0.05})),
      rw.DStep('Rotation', '${rot.round()}°', onMinus: () => _cmd({'op': 'camera', 'rotation': rot - 5}), onPlus: () => _cmd({'op': 'camera', 'rotation': rot + 5})),
      rw.DButtons('Center', () => _cmd({'op': 'camera', 'center': true}), b: 'Reset', onB: () => _cmd({'op': 'reset_camera'})),
      rw.DLink('Auto-camera', auto != null ? 'To frame ${(auto['end'] as num) + 1}' : 'Off', onTap: _autoCameraDialog),
      rw.DLink('Camera lock', switch (((_st['cameraLocks'] as List?) ?? const []).length) { 0 => 'None', 1 => '1 layer', final n => '$n layers' }, onTap: _cameraLockDialog),
      rw.DSeg(const ['None', 'Thirds', 'Phi', 'Spiral'], guide < 3 ? guide : 3, label: guide >= 3 ? 'Guide (spiral ${guide - 2} of 4)' : 'Guide', onSelect: setGuide),
    ];
    return [for (final d in rows) rw.renderDesc(d), rw.renderDesc(const rw.DTitle('Framing & Shake')), ..._cameraFx()];
  }

  /// Movieclip Tools as rows (D15; REDESIGN §8 Movieclip page).
  List<Widget> _movieclipTools(Map mc) {
    final speed = (mc['speed'] as num?)?.toDouble() ?? 1, off = (mc['offset'] as num?)?.toDouble() ?? 0;
    final opacity = ((mc['opacity'] as num?) ?? ((mc['filters'] as Map?)?['opacity'] as num?) ?? 1).toDouble();
    const modes = ['once', 'loop', 'pingpong'];
    final frames = (mc['frames'] as num?)?.toInt() ?? 0;
    final rows = <rw.Desc>[
      rw.DInfo('${mc['name']}', '$frames frame${frames == 1 ? '' : 's'}'),
      rw.DSeg(const ['Once', 'Loop', 'Ping-pong'], modes.indexOf('${mc['loop']}').clamp(0, 2), label: 'Playback', onSelect: (i) => _cmd({'op': 'movieclip', 'loop': modes[i]})),
      rw.DStep('Speed', speed.toStringAsFixed(2), onMinus: speed > 0 ? () => _cmd({'op': 'movieclip', 'speed': math.max(0, speed - 0.1)}) : null, onPlus: () => _cmd({'op': 'movieclip', 'speed': speed + 0.1})),
      rw.DStep('Start offset', '${off.toStringAsFixed(2)} s', onMinus: () => _cmd({'op': 'movieclip', 'offset': off - 0.1}), onPlus: () => _cmd({'op': 'movieclip', 'offset': off + 0.1})),
      ..._effectRows(mc.cast<String, dynamic>(), showBlending: false),
      const rw.DTitle('Blending'),
      rw.DSlider('Opacity', '${(opacity * 100).round()}%', opacity, onChanged: (x) => _cmd({'op': 'filter', 'opacity': (x * 100).round() / 100})),
      rw.DWidget(BlendModeRow(current: '${mc['blend'] ?? 'NORMAL'}', onChanged: (v) => _cmd({'op': 'blend', 'v': v}))),
      rw.DButtons('Edit movieclip', () => _cmd({'op': 'edit_movieclip'}), iconA: 'edit', b: 'Break apart', onB: () => _cmd({'op': 'break_apart', 'layer': mc['layer']})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// Copied wobble: XY, rotation, speed, scale with zoom.
  (double, double, double, bool)? _wobbleClip;

  /// Camera Tools: global widescreen framing and camera shake.
  List<Widget> _cameraFx() {
    final fx = (_st['camFx'] as Map?) ?? const {};
    final xy = (fx['wobbleXY'] as num?)?.toDouble() ?? 0, rot = (fx['wobbleRot'] as num?)?.toDouble() ?? 0;
    final speed = (fx['wobbleSpeed'] as num?)?.toDouble() ?? 3, zoom = fx['wobbleZoom'] != false;
    final any = xy > 0 || rot > 0;
    void fxCmd(Map<String, Object?> m) => _cmd({'op': 'camera_fx', ...m});
    final rows = <rw.Desc>[
      rw.DTick('Widescreen', fx['widescreen'] == true, onChanged: (v) => fxCmd({'widescreen': v})),
      rw.DTick('Shake position', xy > 0, onChanged: (v) => fxCmd({'wobbleXY': v ? 8 : 0})),
      if (xy > 0) rw.DStep('Distance', xy.round().toString(), onMinus: () => fxCmd({'wobbleXY': math.max(1, xy - 1)}), onPlus: () => fxCmd({'wobbleXY': xy + 1})),
      rw.DTick('Shake rotation', rot > 0, onChanged: (v) => fxCmd({'wobbleRot': v ? 0.5 : 0})),
      if (rot > 0) rw.DStep('Degrees', rot.toStringAsFixed(2), onMinus: () => fxCmd({'wobbleRot': math.max(0.1, rot - 0.1)}), onPlus: () => fxCmd({'wobbleRot': rot + 0.1})),
      rw.DStep('Shake speed', speed.toStringAsFixed(1), onMinus: any ? () => fxCmd({'wobbleSpeed': math.max(0.1, speed - 0.5)}) : null, onPlus: any ? () => fxCmd({'wobbleSpeed': speed + 0.5}) : null),
      rw.DTick('Scale with zoom', zoom, onChanged: any ? (v) => fxCmd({'wobbleZoom': v}) : null),
      rw.DButtons('Copy shake', () => setState(() => _wobbleClip = (xy, rot, speed, zoom)),
          b: 'Paste shake', onB: _wobbleClip == null ? null : () => fxCmd({'wobbleXY': _wobbleClip!.$1, 'wobbleRot': _wobbleClip!.$2, 'wobbleSpeed': _wobbleClip!.$3, 'wobbleZoom': _wobbleClip!.$4})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  static const _autoEases = ['Ease v1', 'Ease v2', 'Bounce', 'Elastic', 'Swing'];

  /// Auto-Camera Settings: the camera tweens from this frame to a later one.
  Future<void> _autoCameraDialog() async {
    if (_frame >= _frames - 1) {
      await _ask('Auto-camera', "Can't do auto-camera if this is the last frame.", ['Okay']);
      return;
    }
    final cur = _st['autoCam'] as Map?;
    var end = ((cur?['end'] as num?)?.toInt() ?? _frame + 1).clamp(_frame + 1, _frames - 1);
    final code = (cur?['ease'] as num?)?.toInt() ?? 1;
    var type = (code % 10).clamp(1, 5) - 1;
    var easeIn = (code ~/ 10) & 1 == 1, easeOut = (code ~/ 10) & 2 == 2;
    if (!mounted) return;
    final ok = await showDialog<String>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) => AlertDialog(
          backgroundColor: cPanel,
          title: const Text('Auto-camera', style: TextStyle(color: cText, fontSize: 16)),
          content: SizedBox(
            width: 400,
            child: ListView(shrinkWrap: true, children: [
              if (cur != null) ...[
                const Text('This frame already starts an auto-camera. Deleting it means the camera no longer moves and adjusts by itself as you add or remove frames.', style: TextStyle(color: cText, fontSize: 12)),
                const SizedBox(height: 6),
                _Btn('Delete auto-camera', danger: true, onTap: () => Navigator.pop(c, 'delete')),
                const SizedBox(height: 12),
              ],
              const Text('On what frame does the auto-camera end? (A frame after this one.)', style: TextStyle(color: cText, fontSize: 13)),
              const SizedBox(height: 6),
              _stepper('End frame', '${end + 1}', () => set(() => end = math.max(_frame + 1, end - 1)), () => set(() => end = math.min(_frames - 1, end + 1))),
              const SizedBox(height: 8),
              Wrap(spacing: 4, runSpacing: 4, children: [for (var i = 0; i < _autoEases.length; i++) _Chip(_autoEases[i], i == type, () => set(() => type = i))]),
              _Toggle('Ease in', easeIn, (v) => set(() => easeIn = v)),
              _Toggle('Ease out', easeOut, (v) => set(() => easeOut = v)),
              const SizedBox(height: 8),
              _hint('What is auto-camera? The camera moves, zooms and rotates by itself from this frame to the end frame: a tween for the camera across several frames. The frames in between follow it, and it adapts when frames are added or deleted.'),
            ]),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(c), child: const Text('Cancel')),
            TextButton(onPressed: () => Navigator.pop(c, 'ok'), child: const Text('Okay')),
          ],
        ),
      ),
    );
    if (ok == 'delete') {
      _cmd({'op': 'auto_camera', 'end': null});
    } else if (ok == 'ok') {
      _cmd({'op': 'auto_camera', 'end': end, 'ease': type + 1 + (easeIn ? 10 : 0) + (easeOut ? 20 : 0)});
    }
  }

  /// Lock Figure To Camera: locked figures move with the camera (a health
  /// bar, a logo); parallax and "rotate and scale with camera" per figure.
  Future<void> _cameraLockDialog() => showDialog<void>(
        context: context,
        builder: (c) => StatefulBuilder(builder: (c, set) {
          final locks = ((_st['cameraLocks'] as List?) ?? const []).cast<Map>();
          final locked = {for (final m in locks) m['layer']};
          final free = ((_st['library'] as List?) ?? const []).cast<Map>().where((m) => !locked.contains(m['layer'])).toList();
          void run(Map<String, Object?> cmd) {
            _cmd(cmd);
            set(() {});
          }

          return AlertDialog(
            backgroundColor: cPanel,
            title: const Text('Lock figure to camera', style: TextStyle(color: cText, fontSize: 16)),
            content: SizedBox(
              width: 420,
              child: ListView(shrinkWrap: true, children: [
                _hint('A locked figure is no longer moved by hand: it moves with the camera. Useful for a health bar or a logo. Joined figures cannot be locked; unjoin them first. Parallax 1.00 moves exactly with the camera, 0.10 barely, negative values the opposite way (foreground).'),
                const SizedBox(height: 12),
                _label('Currently locked figures'),
                if (locks.isEmpty) const Padding(padding: EdgeInsets.symmetric(vertical: 6), child: Text('(None)', style: TextStyle(color: cSecondary, fontSize: 13))),
                for (final m in locks) ...[
                  Row(children: [
                    Expanded(child: Text('${m['name']}', style: const TextStyle(color: cText, fontSize: 13), overflow: TextOverflow.ellipsis)),
                    SizedBox(width: 90, child: _Btn('Unlock', small: true, onTap: () => run({'op': 'camera_lock', 'layer': m['layer'], 'on': false}))),
                  ]),
                  _stepper('Parallax', ((m['parallax'] as num).toDouble()).toStringAsFixed(2), () => run({'op': 'camera_lock_set', 'layer': m['layer'], 'parallax': (m['parallax'] as num) - 0.1}), () => run({'op': 'camera_lock_set', 'layer': m['layer'], 'parallax': (m['parallax'] as num) + 0.1})),
                  _Toggle('Rotate and scale with camera?', m['rotate'] == true, (v) => run({'op': 'camera_lock_set', 'layer': m['layer'], 'rotate': v})),
                  const Divider(color: cSelected),
                ],
                const SizedBox(height: 8),
                _label('Choose figure to lock'),
                for (final m in free) Padding(padding: const EdgeInsets.only(bottom: 4), child: _Btn('${m['name']}', small: true, onTap: () => run({'op': 'camera_lock', 'layer': m['layer'], 'on': true}))),
                if ('${_st['message'] ?? ''}'.contains('Unjoin')) Text('${_st['message']}', style: const TextStyle(color: cDanger, fontSize: 12)),
              ]),
            ),
            actions: [TextButton(onPressed: () => Navigator.pop(c), child: const Text('Okay'))],
          );
        }),
      );

  /// A colour control: the current colour; a tap opens the colour picker.
  /// A colour choice as on the round 10 boards: seven presets (the current
  /// one ringed) and a rainbow "more" for the full picker (ringed when the
  /// colour is none of the presets). The alpha of the current colour is kept.
  static const _presets = [0x9B1C2E, 0x2160A8, 0x2A2B30, 0xEDB98A, 0x1E9E55, 0xF2A93B, 0xFFFFFF];
  Widget _swatches(int current, void Function(int) pick) {
    final rgb = current >> 8, alpha = current & 0xff == 0 ? 0xff : current & 0xff;
    final preset = _presets.contains(rgb);
    return Align(
      alignment: Alignment.centerRight,
      child: kit.SwatchRow(
        [for (final c in _presets) Color(0xFF000000 | c)],
        preset ? Color(0xFF000000 | rgb) : null,
        moreOn: !preset,
        onPick: (c) => pick(((c.toARGB32() & 0xffffff) << 8) | alpha),
        onMore: () => _colorPicker(current, pick),
      ),
    );
  }

  static Color _rgba(int c) => Color(((c & 0xff) << 24) | (c >> 8));
  static int _toRgba(Color c, int alpha) => ((c.toARGB32() & 0xffffff) << 8) | alpha;

  /// Colours kept with "Copy colour" (newest first; reset when the app restarts).
  static final List<int> _copiedColors = [0xff3b30ff, 0xff9500ff, 0xffcc00ff, 0x34c759ff, 0x007affff, 0xff2d92ff, 0x000000ff];

  /// The shared colour picker: copied colours, a hue / saturation square, a
  /// brightness bar and a hex field. A colour applies when the finger lifts
  /// (one undo step), so a drag through the square is not dozens of steps.
  Future<void> _colorPicker(int current, void Function(int) pick) async {
    final alpha = current & 0xff == 0 ? 0xff : current & 0xff;
    var hsv = HSVColor.fromColor(_rgba(current | 0xff));
    final hexCtl = TextEditingController(text: (current >> 8).toRadixString(16).padLeft(6, '0').toUpperCase());
    void apply(StateSetter set, HSVColor c, {bool commit = true}) {
      set(() => hsv = c);
      hexCtl.text = (_toRgba(c.toColor(), alpha) >> 8).toRadixString(16).padLeft(6, '0').toUpperCase();
      if (commit) pick(_toRgba(c.toColor(), alpha));
    }

    const sq = 200.0, bar = 28.0;
    await showDialog<void>(
      context: context,
      builder: (c) => StatefulBuilder(builder: (c, set) {
        void square(Offset p, {bool commit = false}) {
          final h = (p.dx / sq).clamp(0.0, 1.0) * 360;
          final sat = 1 - (p.dy / sq).clamp(0.0, 1.0);
          apply(set, HSVColor.fromAHSV(1, h.clamp(0, 359.99), sat, hsv.value), commit: commit);
        }

        void bright(Offset p, {bool commit = false}) => apply(set, hsv.withValue(1 - (p.dy / sq).clamp(0.0, 1.0)), commit: commit);
        return AlertDialog(
          backgroundColor: cPanel,
          contentPadding: const EdgeInsets.all(14),
          content: SizedBox(
            width: sq + bar + 60,
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              _label('Copied colours'),
              Row(children: [
                for (final k in _copiedColors)
                  Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: GestureDetector(
                      onTap: () => apply(set, HSVColor.fromColor(_rgba(k | 0xff))),
                      child: Container(width: 26, height: 26, decoration: BoxDecoration(color: _rgba(k | 0xff), borderRadius: BorderRadius.circular(4), border: Border.all(color: cSelected))),
                    ),
                  ),
              ]),
              const SizedBox(height: 10),
              Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                // hue across, saturation down
                GestureDetector(
                  onPanDown: (d) => square(d.localPosition),
                  onPanUpdate: (d) => square(d.localPosition),
                  onPanEnd: (_) => apply(set, hsv),
                  onTapUp: (d) => square(d.localPosition, commit: true),
                  child: SizedBox(
                    width: sq,
                    height: sq,
                    child: CustomPaint(painter: _HueSatPainter(hsv)),
                  ),
                ),
                const SizedBox(width: 10),
                // brightness
                GestureDetector(
                  onPanDown: (d) => bright(d.localPosition),
                  onPanUpdate: (d) => bright(d.localPosition),
                  onPanEnd: (_) => apply(set, hsv),
                  onTapUp: (d) => bright(d.localPosition, commit: true),
                  child: SizedBox(width: bar, height: sq, child: CustomPaint(painter: _BrightPainter(hsv))),
                ),
                const SizedBox(width: 10),
                Expanded(child: Container(height: sq, decoration: BoxDecoration(color: hsv.toColor(), borderRadius: BorderRadius.circular(4), border: Border.all(color: cSelected)))),
              ]),
              const SizedBox(height: 10),
              Row(children: [
                const Text('#', style: TextStyle(color: cSecondary)),
                const SizedBox(width: 4),
                Expanded(
                  child: TextField(
                    controller: hexCtl,
                    style: const TextStyle(color: cText, fontSize: 14),
                    maxLength: 6,
                    decoration: const InputDecoration(isDense: true, counterText: ''),
                    onSubmitted: (v) {
                      final n = int.tryParse(v.trim().replaceAll('#', ''), radix: 16);
                      if (n != null && v.trim().replaceAll('#', '').length == 6) apply(set, HSVColor.fromColor(Color(0xff000000 | n)));
                    },
                  ),
                ),
                IconButton(
                  tooltip: 'Use this hex colour',
                  icon: const Icon(Icons.check, color: Color(0xFF30D158)),
                  onPressed: () {
                    final t = hexCtl.text.trim().replaceAll('#', '');
                    final n = int.tryParse(t, radix: 16);
                    if (n != null && t.length == 6) apply(set, HSVColor.fromColor(Color(0xff000000 | n)));
                  },
                ),
              ]),
              const SizedBox(height: 6),
              _Btn('Copy colour', onTap: () => set(() {
                    _copiedColors.insert(0, _toRgba(hsv.toColor(), 0xff));
                    if (_copiedColors.length > 7) _copiedColors.removeLast();
                  })),
            ]),
          ),
          actions: [TextButton(onPressed: () => Navigator.pop(c), child: const Text('Done'))],
        );
      }),
    );
  }

  /// The View card (REDESIGN §8, D15): a page of rows. Nothing from the old
  /// View options panel is dropped.
  Widget _viewOptionsPanel() {
    void flag(String k, bool v) => setState(() {
          AppSettings.values[k] = v;
          unawaited(AppSettings.save());
        });
    void number(String k, int v) => setState(() {
          AppSettings.values[k] = v;
          unawaited(AppSettings.save());
        });
    final before = ((_opts['onionBefore'] as num?) ?? 1).toInt(), after = ((_opts['onionAfter'] as num?) ?? 1).toInt();
    return rw.PropertyPage([
      const rw.DTitle('Onion skin'),
      rw.DTick('Onion skin', _opts['onion'] == true, onChanged: (v) => _cmd({'op': 'set', 'k': 'onion', 'v': v})),
      if (_opts['onion'] == true) ...[
        rw.DStep('Frames before', '$before', onMinus: () => _cmd({'op': 'set', 'k': 'onion_before', 'v': before - 1}), onPlus: () => _cmd({'op': 'set', 'k': 'onion_before', 'v': before + 1})),
        rw.DStep('Frames after', '$after', onMinus: () => _cmd({'op': 'set', 'k': 'onion_after', 'v': after - 1}), onPlus: () => _cmd({'op': 'set', 'k': 'onion_after', 'v': after + 1})),
        rw.DSeg(const ['Behind', 'In front'], _opts['onionFront'] == true ? 1 : 0, onSelect: (i) => _cmd({'op': 'set', 'k': 'onion_front', 'v': i == 1})),
      ],
      const rw.DTitle('Handles'),
      rw.DTick('Only main joints', _opts['mainOnly'] == true, onChanged: (v) => _cmd({'op': 'set', 'k': 'mainonly', 'v': v})),
      if (_st['building'] != null) ...[
        rw.DTick('Show all nodes (incl. locked)', _opts['static'] == true, onChanged: (v) => _cmd({'op': 'set', 'k': 'static', 'v': v})),
        rw.DTick('Stock size figure', _opts['sizeRef'] != false, onChanged: (v) => _cmd({'op': 'set', 'k': 'sizeref', 'v': v})),
        rw.DTick('Draw order numbers', AppSettings.flag('drawOrder'), onChanged: (v) => flag('drawOrder', v)),
        rw.DSeg(const ['None', 'Polyfills', 'Connectors', 'Locks'], AppSettings.number('nodeHighlight').clamp(0, 3), label: 'Highlight', onSelect: (i) => number('nodeHighlight', i)),
      ],
      const rw.DTitle('Stage'),
      for (final (k, label) in const [
        ('showIds', 'Figure names'),
        ('neighbourLines', 'Neighbour lines'),
        ('outlineSelection', 'Outline the selection'),
        ('zoomButtons', 'Zoom buttons'),
        ('guides', 'Guides'),
        ('stageBox', 'Stage box'),
      ])
        rw.DTick(label, AppSettings.flag(k), onChanged: (v) => flag(k, v)),
      const rw.DTitle('While editing'),
      rw.DSeg(const ['None', 'No blur', 'All'], AppSettings.number('showFilters').clamp(0, 2), label: 'Effects', onSelect: (i) => number('showFilters', i)),
      rw.DTick('Motion path', _opts['path'] == true, onChanged: _setMotionPath),
      rw.DTick('Preview row', AppSettings.values['previewRow'] != false, onChanged: (v) => setState(() {
            AppSettings.values['previewRow'] = v;
            unawaited(AppSettings.save());
          })),
      rw.DTick('Section tabs', _showTabs, onChanged: (v) => setState(() => _showTabs = v)),
      const rw.DTitle('Playback'),
      rw.DSeg(const ['Full', 'Half', 'Fast'], _quality == 1 ? 0 : _quality == 0.5 ? 1 : 2, label: 'Quality', onSelect: (i) => setState(() {
            _quality = [1.0, 0.5, 0.3][i];
            _clearCache();
          })),
      const rw.DTitle('Helpers'),
      rw.DInfo('Zoom', '${_fitScale() > 0 ? (_view.s / _fitScale() * 100).round() : 100}%'),
      rw.DButtons('Fit stage', () => setState(_fit), b: 'Nudge arrows', onB: _arrows ? null : () => setState(() => _arrows = true)),
      const rw.DHint('Nudge arrows move the selected figure in small exact steps.'),
      rw.DButtons('Magnifier', _magnifier
          ? null
          : () => setState(() {
                _magnifier = true;
                _magKey = '';
              })),
      const rw.DHint('The magnifier shows the stage up close, so joints that sit close together are easy to pick.'),
      if (_engineMs > 0) ...[
        const rw.DTitle('Running slow?'),
        rw.DHint('${_engineMs.toStringAsFixed(1)} ms per picture. Lower the playback quality, or turn effects off while editing.'),
      ],
    ]);
  }

  final Map<String, ui.Image> _previews = {};



  /// "Add a figure": a window over the editor with the user's Stick Nodes
  /// figures (Barkfart Figures folder) and the built-in ones, with pictures.
  Future<void> _figurePicker({bool movieclip = false}) async {
    final e = _engine;
    if (e == null) return;
    final entries = <_PickEntry>[];
    for (final f in await figureFiles()) {
      final name = f.path.split(Platform.pathSeparator).last;
      if (movieclip && !name.toLowerCase().endsWith('.nodemc')) continue;
      final key = 'file:${f.path}';
      if (!_previews.containsKey(key)) {
        final px = e.renderFile(name, await f.readAsBytes(), 160, 160);
        if (px != null) {
          final c = Completer<ui.Image>();
          ui.decodeImageFromPixels(px, 160, 160, ui.PixelFormat.rgba8888, c.complete);
          _previews[key] = await c.future;
        }
      }
      entries.add(_PickEntry(key, name.replaceAll(RegExp(r'\.(nodes|nodemc|stknds)$', caseSensitive: false), ''), true));
    }
    // the stock figures and effects that ship with the app
    final manifest = await AssetManifest.loadFromAssetBundle(rootBundle);
    for (final path in manifest.listAssets().where((a) => a.startsWith('assets/stock/')).toList()..sort()) {
      if (movieclip && !path.toLowerCase().endsWith('.nodemc')) continue;
      final key = 'asset:$path';
      final name = Uri.decodeComponent(path.split('/').last);
      if (!_previews.containsKey(key)) {
        final bytes = (await rootBundle.load(path)).buffer.asUint8List();
        final px = e.renderFile(name, bytes, 160, 160);
        if (px != null) {
          final c = Completer<ui.Image>();
          ui.decodeImageFromPixels(px, 160, 160, ui.PixelFormat.rgba8888, c.complete);
          _previews[key] = await c.future;
        }
      }
      entries.add(_PickEntry(key, name.replaceAll(RegExp(r'\.(nodes|nodemc)$'), ''), false, effect: path.contains('/effects/')));
    }
    if (!mounted) return;
    final picked = await showDialog<List<String>>(context: context, builder: (c) => _FigurePicker(entries, _previews, movieclip: movieclip, canPaste: _st['figureCopied'] == true, editing: _st['editingClip'] != null));
    for (final k in picked ?? const <String>[]) {
      // the footer's words (round 10 board): import, paste, the project's
      // library, a new movieclip
      if (k == '#import') {
        await _importFromFiles();
      } else if (k == '#create') {
        _createNewFigure();
      } else if (k == '#paste') {
        _pasteFigure();
      } else if (k == '#library') {
        await _libraryDialog();
      } else if (k == '#newclip') {
        _cmd({'op': 'new_movieclip'});
      } else if (k.startsWith('file:')) {
        final path = k.substring(5);
        _refresh(e.importFile(path.split(Platform.pathSeparator).last, await File(path).readAsBytes()));
        _thumbsChanged('import');
      } else if (k.startsWith('asset:')) {
        final path = k.substring(6);
        _refresh(e.importFile(Uri.decodeComponent(path.split('/').last), (await rootBundle.load(path)).buffer.asUint8List()));
        _thumbsChanged('import');
      } else {
        _cmd({'op': 'add_figure', 'kind': k});
      }
    }
  }




  int _splitN = 1;
  bool _reorderBranch = false;

  /// "Modify branch": one property at a time to the selected joint and every
  /// joint below it. Each Set is its own undo step.
  Future<void> _modifyBranchDialog() async {
    final sel = _sel;
    if (sel == null) return;
    var color = (sel['color'] as num?)?.toInt() ?? 0xff;
    var thickness = ((sel['thickness'] as num?)?.toDouble() ?? 32).round();
    var shape = sel['shape'] as String? ?? 'line';
    var staticOn = sel['static'] == true;
    var stretchy = sel['stretchy'] == true;
    var scale = 1.0;
    var from = color, to = 0xff0000ff, grad = (sel['gradientColor'] as num?)?.toInt() ?? 0xffffffff;
    var ownColor = sel['ownColor'] == true, ignore0 = true;
    void apply(Map<String, Object?> change) => _cmd({'op': 'segment', 'branch': true, ...change});
    void edit(Map<String, Object?> change) => _cmd({'op': 'branch_edit', 'ignoreZero': ignore0, ...change});
    await showDialog<void>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) {
          Widget row(String label, Widget control, VoidCallback onSet) => Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                  _label(label),
                  Row(children: [
                    Expanded(child: control),
                    const SizedBox(width: 8),
                    SizedBox(width: 56, child: _Btn('Set', small: true, onTap: onSet)),
                  ]),
                ]),
              );
          Widget numberRow(String label, String value, VoidCallback minus, VoidCallback plus, VoidCallback onSet) => row(
                label,
                Row(children: [
                  Expanded(child: Text(value, style: const TextStyle(color: cText, fontSize: 13))),
                  SizedBox(width: 40, child: _Btn('−', onTap: minus)),
                  const SizedBox(width: 4),
                  SizedBox(width: 40, child: _Btn('+', onTap: plus)),
                ]),
                onSet,
              );
          return AlertDialog(
            backgroundColor: cPanel,
            title: const Text('Modify branch', style: TextStyle(color: cText, fontSize: 16)),
            content: SizedBox(
              width: 380,
              child: ListView(shrinkWrap: true, children: [
                _hint('Each Set applies one value to the selected joint and every joint below it.'),
                const SizedBox(height: 12),
                _label('Replace colour'),
                Row(children: [
                  Expanded(child: _swatches(from, (v) => set(() => from = v))),
                  const Padding(padding: EdgeInsets.symmetric(horizontal: 6), child: Text('→', style: TextStyle(color: cText))),
                  Expanded(child: _swatches(to, (v) => set(() => to = v))),
                ]),
                Row(children: [
                  Expanded(child: _Btn('Replace', small: true, onTap: () => edit({'replaceFrom': from, 'replaceTo': to}))),
                  const SizedBox(width: 8),
                  Expanded(child: _Btn('From <<< To', small: true, onTap: () => set(() => from = to))),
                ]),
                const SizedBox(height: 12),
                row('Segment colour', _swatches(color, (v) => set(() => color = v)), () => apply({'color': color})),
                row('Gradient colour', _swatches(grad, (v) => set(() => grad = v)), () => edit({'gradientColor': grad})),
                Row(children: [
                  Expanded(child: _Toggle('Use segment colour', ownColor, (v) => set(() => ownColor = v))),
                  const SizedBox(width: 8),
                  SizedBox(width: 56, child: _Btn('Set', small: true, onTap: () => edit({'ownColor': ownColor}))),
                ]),
                _label('Increment'),
                _Toggle('Ignore 0s', ignore0, (v) => set(() => ignore0 = v)),
                Row(children: [
                  const Expanded(child: Text('Thickness', style: TextStyle(color: cText, fontSize: 13))),
                  SizedBox(width: 48, child: _Btn('−2', small: true, onTap: () => edit({'addThickness': -2}))),
                  const SizedBox(width: 4),
                  SizedBox(width: 48, child: _Btn('+2', small: true, onTap: () => edit({'addThickness': 2}))),
                ]),
                const SizedBox(height: 4),
                Row(children: [
                  const Expanded(child: Text('Length', style: TextStyle(color: cText, fontSize: 13))),
                  SizedBox(width: 48, child: _Btn('−5', small: true, onTap: () => edit({'addLength': -5}))),
                  const SizedBox(width: 4),
                  SizedBox(width: 48, child: _Btn('+5', small: true, onTap: () => edit({'addLength': 5}))),
                ]),
                const SizedBox(height: 12),
                numberRow('Thickness', '$thickness', () => set(() => thickness = math.max(0, thickness - 2)), () => set(() => thickness = math.min(400, thickness + 2)), () => apply({'thickness': thickness})),
                row('Shape', Wrap(spacing: 4, runSpacing: 4, children: [
                  for (final s in const ['line', 'circle', 'square', 'ellipse', 'triangle', 'trapezoid', 'polygon'])
                    _Chip(s[0].toUpperCase() + s.substring(1), s == shape, () => set(() => shape = s)),
                ]), () => apply({'shape': shape})),
                numberRow('Scale of lengths', scale.toStringAsFixed(2), () => set(() => scale = math.max(0.1, scale - 0.1)), () => set(() => scale = math.min(10, scale + 0.1)), () => apply({'scaleLength': scale})),
                Row(children: [
                  Expanded(child: _Toggle('Static (non-moving)', staticOn, (v) => set(() => staticOn = v))),
                  const SizedBox(width: 8),
                  SizedBox(width: 56, child: _Btn('Set', small: true, onTap: () => apply({'static': staticOn}))),
                ]),
                Row(children: [
                  Expanded(child: _Toggle('Stretchy', stretchy, (v) => set(() => stretchy = v))),
                  const SizedBox(width: 8),
                  SizedBox(width: 56, child: _Btn('Set', small: true, onTap: () => apply({'stretchy': stretchy}))),
                ]),
              ]),
            ),
            actions: [TextButton(onPressed: () => Navigator.pop(c), child: const Text('Done'))],
          );
        },
      ),
    );
  }

  /// The figure editor: structure tools for the selected segment.
  /// "Edit as a new figure…" (D12, the EditCopy board): editing never
  /// changes the animated figure, so it asks first, then edits a copy.
  Future<void> _editCopySheet(String name) async {
    final go = await kit.showSheet<bool>(context, title: 'Editing makes a new figure', width: 400, children: (c) => [
          Text('$name in this animation won\u2019t change. You\u2019ll edit a copy, which you can save to your library and add to any frame.', textAlign: TextAlign.center, style: const TextStyle(color: cText, fontSize: 15, height: 1.35)),
          const SizedBox(height: 14),
          kit.ButtonPair('Cancel', () => Navigator.pop(c, false), 'Edit a copy', () => Navigator.pop(c, true), true),
        ]);
    if (go == true && mounted) {
      final sz = MediaQuery.sizeOf(context);
      if (sz.width < 900 || sz.height < 500) setState(() => _partsOpen = false);
      _cmd({'op': 'edit_copy'});
    }
  }

  bool _studioGrid = true;
  bool _studioCrosshair = true;

  /// Enters the isolated Stick Figure Creator Studio with a fresh starter figure.
  void _createNewFigure() {
    final sz = MediaQuery.maybeSizeOf(context);
    if (sz != null && (sz.width < 900 || sz.height < 500)) {
      setState(() => _partsOpen = false);
    }
    _cmd({'op': 'create_figure'});
    setState(() {
      _panelOpen = false;
      _viewOptions = false;
    });
  }

  /// Studio top bar when creating, editing, or duplicating a stick figure.
  Widget _studioHeader(EdgeInsets pad) {
    final building = (_st['building'] as Map?) ?? const {};
    final ce = _st['copyEdit'] as Map?;
    final mode = '${building['mode'] ?? ce?['mode'] ?? (ce != null ? 'copy' : 'edit')}';
    final name = '${ce?['name'] ?? building['name'] ?? _sel?['name'] ?? 'Stickfigure'}';
    final primaryLabel = mode == 'edit' ? 'Apply' : 'Add to Scene';
    final showAllNodes = _opts['static'] == true;
    return FigureStudioHeader(
      pad: pad,
      name: name,
      primaryLabel: primaryLabel,
      showAllNodes: showAllNodes,
      onToggleShowAllNodes: (all) => _cmd({'op': 'set', 'k': 'static', 'v': all}),
      showGrid: _studioGrid,
      onToggleGrid: () => setState(() => _studioGrid = !_studioGrid),
      showCrosshair: _studioCrosshair,
      onToggleCrosshair: () => setState(() => _studioCrosshair = !_studioCrosshair),
      onCancel: () {
        if (mode == 'copy') {
          _cmd({'op': 'edit_copy_end'});
        } else {
          _cmd({'op': 'build_cancel'});
        }
      },
      onRename: () async {
        final next = await _askName('Figure name', name);
        if (next == null || next.trim().isEmpty || !mounted) return;
        final layerId = building['layer'];
        if (layerId != null) {
          _cmd({'op': 'rename', 'layer': layerId, 'name': next.trim()});
        }
      },
      onSaveLibrary: mode == 'copy' ? () => unawaited(_saveCopy()) : _saveFigureFile,
      onPrimary: () => _cmd({'op': 'build_apply'}),
    );
  }

  /// Save as new figure: the copy becomes a figure file in your figures (the
  /// chooser's Yours), and the editor closes; the animation is as it was.
  Future<void> _saveCopy() async {
    final e = _engine;
    final ce = _st['copyEdit'] as Map?;
    if (e == null || ce == null) return;
    final bytes = e.figureFile();
    if (bytes == null) {
      _say('Could not save the figure: ${e.lastError}');
      return;
    }
    final dir = await figuresDir();
    final clean = '${ce['name']}'.trim().replaceAll(RegExp(r'[\\/:*?"<>|]'), '_');
    var f = File('${dir.path}/$clean.nodes');
    for (var k = 2; f.existsSync(); k++) {
      f = File('${dir.path}/$clean ($k).nodes');
    }
    await f.writeAsBytes(bytes);
    _cmd({'op': 'edit_copy_end'});
    final saved = f.path.split(Platform.pathSeparator).last.replaceAll('.nodes', '');
    if (mounted) kit.showToast(context, 'Saved $saved to your figures. Add it from + › Stick figure › Yours.', light: true);
  }

  /// Save Stickfigure As: a Stick Nodes .nodes file in the Barkfart Figures
  /// folder (it then shows in Add stickfigure... and works in Stick Nodes).
  Future<void> _saveFigureFile() async {
    final e = _engine;
    if (e == null) return;
    final b = _st['building'] as Map?;
    final name = await _askName('Save stickfigure as', '${b?['name'] ?? _sel?['name'] ?? 'Stickfigure'}');
    if (name == null || name.trim().isEmpty) return;
    final bytes = e.figureFile();
    if (bytes == null) {
      _say('Could not save the stickfigure: ${e.lastError}');
      return;
    }
    final dir = await figuresDir();
    final clean = name.trim().replaceAll(RegExp(r'[\\/:*?"<>|]'), '_');
    var f = File('${dir.path}/$clean.nodes');
    for (var k = 2; f.existsSync(); k++) {
      f = File('${dir.path}/$clean ($k).nodes');
    }
    await f.writeAsBytes(bytes);
    _say('Saved ${f.path.split(Platform.pathSeparator).last} in Barkfart Figures.', light: true);
  }

  /// Figure studio: reference tracing & studio canvas background.
  List<Widget> _traceTools() {
    final tr = _st['trace'] as Map?;
    final rows = <rw.Desc>[
      const rw.DTitle('Reference'),
      rw.DButtons('Trace figure…', _traceFigureDialog, b: 'Trace frame…', onB: _traceFrameDialog),
      if (tr != null)
        rw.DButtons(tr['frame'] != null ? 'Clear trace (f${(tr['frame'] as num) + 1})' : 'Clear trace', () => _cmd({'op': 'trace'})),
      rw.DButtons('BG image…', _loadEditorImage, b: 'Clear BG', onB: _editorImage ? () {
        _engine?.editorImage(null);
        setState(() {
          _editorImage = false;
          _drawn = '';
        });
      } : null),
      const rw.DTitle('Studio Canvas'),
      rw.DTick('Grid', _studioGrid, onChanged: (v) => setState(() => _studioGrid = v)),
      rw.DTick('Origin crosshair', _studioCrosshair, onChanged: (v) => setState(() => _studioCrosshair = v)),
      rw.DTick('Mirror axis', _opts['mirror'] == true, onChanged: (v) => _cmd({'op': 'set', 'k': 'mirror', 'v': v})),
      rw.DTick('Stock size reference', _opts['sizeRef'] != false, onChanged: (v) => _cmd({'op': 'set', 'k': 'sizeref', 'v': v})),
      rw.DWidget(kit.PropRow('Canvas', trailing: _swatches((_st['editorBg'] as num?)?.toInt() ?? 0xf5f6f8ff, (c) => _cmd({'op': 'editor_bg', 'color': c})))),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  bool _editorImage = false;

  /// Load BG image: a picture from the Barkfart Images folder, faint behind
  /// the figure being edited (view only, never saved).
  Future<void> _loadEditorImage() async {
    final e = _engine;
    if (e == null) return;
    final files = await imageFiles();
    if (!mounted) return;
    final pick = await showDialog<String>(
      context: context,
      builder: (c) => AlertDialog(
        backgroundColor: cPanel,
        title: const Text('Load a background image', style: TextStyle(color: cText, fontSize: 16)),
        content: SizedBox(
          width: 360,
          child: ListView(shrinkWrap: true, children: [
            const Text('Pictures in the Barkfart Images folder. The image shows only while editing this figure and is not saved.', style: TextStyle(color: cSecondary, fontSize: 12)),
            const SizedBox(height: 8),
            if (files.isEmpty) const Text('No pictures in the folder yet.', style: TextStyle(color: cText)),
            for (final f in files) Padding(padding: const EdgeInsets.only(bottom: 4), child: _Btn(f.path.split(Platform.pathSeparator).last, small: true, onTap: () => Navigator.pop(c, f.path))),
          ]),
        ),
        actions: [TextButton(onPressed: () => Navigator.pop(c), child: const Text('Cancel'))],
      ),
    );
    if (pick == null) return;
    final ok = e.editorImage(await File(pick).readAsBytes());
    if (!ok) _say('That picture could not be read.');
    setState(() {
      _editorImage = ok;
      _drawn = '';
    });
  }

  Future<void> _traceFrameDialog() async {
    var f = ((_st['trace'] as Map?)?['frame'] as num?)?.toInt() ?? _frame;
    final ok = await showDialog<bool>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) => AlertDialog(
          backgroundColor: cPanel,
          title: const Text('Select a frame to trace', style: TextStyle(color: cText, fontSize: 16)),
          content: SizedBox(
            width: 340,
            child: Column(mainAxisSize: MainAxisSize.min, children: [
              Text('A frame (1 to $_frames) shown faintly behind, to trace from.', style: const TextStyle(color: cSecondary, fontSize: 12)),
              const SizedBox(height: 8),
              _stepper('Frame', '${f + 1}', () => set(() => f = math.max(0, f - 1)), () => set(() => f = math.min(_frames - 1, f + 1))),
            ]),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(c, false), child: const Text('Cancel')),
            TextButton(onPressed: () => Navigator.pop(c, true), child: const Text('Trace this')),
          ],
        ),
      ),
    );
    if (ok == true) {
      _cmd({'op': 'trace', 'frame': f});
    } else if (ok == false) {
      _cmd({'op': 'trace'});
    }
  }

  Future<void> _traceFigureDialog() async {
    final figs = ((_st['library'] as List?) ?? const []).cast<Map>().where((m) => m['kind'] == 'figure').toList();
    final pick = await showDialog<String>(
      context: context,
      builder: (c) => AlertDialog(
        backgroundColor: cPanel,
        title: const Text('Select a stickfigure to trace', style: TextStyle(color: cText, fontSize: 16)),
        content: SizedBox(
          width: 340,
          child: ListView(shrinkWrap: true, children: [
            for (final m in figs) Padding(padding: const EdgeInsets.only(bottom: 4), child: _Btn('${m['name']}', small: true, onTap: () => Navigator.pop(c, '${m['layer']}'))),
          ]),
        ),
        actions: [TextButton(onPressed: () => Navigator.pop(c), child: const Text('Cancel'))],
      ),
    );
    if (pick != null) _cmd({'op': 'trace', 'layer': pick});
  }

  /// Polyfill Tools: while a polyfill is edited, taps on joints add polynodes.
  List<Widget> _polyfillTools(Map pf) {
    final own = pf['own'] == true;
    final rows = <rw.Desc>[
      const rw.DTitle('Polyfill'),
      rw.DInfo('Polynodes', '${pf['count']}'),
      rw.DWidget(kit.WordButton('Finish polyfill', primary: true, expand: true, height: 36, size: 14, onTap: () => _cmd({'op': 'polyfill_edit', 'id': null}))),
      rw.DTick('Own colour', own, onChanged: (v) => _cmd({'op': 'polyfill_color', 'own': v})),
      rw.DWidget(Opacity(opacity: own ? 1 : 0.4, child: _swatches((pf['color'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'polyfill_color', 'color': c, 'own': true})))),
      rw.DButtons('Undo point', (pf['count'] as num) > 0 ? () => _cmd({'op': 'polyfill_undo'}) : null, iconA: 'undo'),
      rw.DDanger('Delete polyfill', () => _cmd({'op': 'polyfill_delete'})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  List<Widget> _builder(Map<String, dynamic>? sel) {
    final pf = _st['polyfill'] as Map?;
    if (pf != null) return _polyfillTools(pf);
    if (_st['connecting'] == true) {
      return [
        for (final d in <rw.Desc>[
          const rw.DTitle('Connector'),
          const rw.DHint('Tap target joint.'),
          rw.DButtons('Cancel', () => _cmd({'op': 'connect_cancel'})),
        ])
          rw.renderDesc(d),
      ];
    }
    final con = sel?['connector'] as Map?;
    final connectorRows = con == null
        ? const <rw.Desc>[]
        : <rw.Desc>[
            const rw.DTitle('Connector'),
            rw.DInfo('Target joint', '${con['to']}'),
            rw.DStep('Position', '${(((con['percent'] as num).toDouble()) * 100).round()}%',
                onMinus: () => _cmd({'op': 'connector_set', 'percent': math.max(0, (con['percent'] as num) - 0.05)}),
                onPlus: () => _cmd({'op': 'connector_set', 'percent': math.min(1, (con['percent'] as num) + 0.05)})),
            rw.DTick('Reversed', con['reversed'] == true, onChanged: (v) => _cmd({'op': 'connector_set', 'reversed': v})),
            rw.DDanger('Delete connector', () => _cmd({'op': 'delete_joint'})),
          ];
    if (sel == null) {
      return [
        ..._traceTools(),
        rw.renderDesc(const rw.DTitle('Library')),
        rw.renderDesc(rw.DButtons('Save .nodes…', _saveFigureFile, iconA: 'save')),
      ];
    }
    final root = sel['root'] == true;
    final thick = (sel['thickness'] as num?)?.toDouble() ?? 0;
    final color = (sel['color'] as num?)?.toInt() ?? 0xff;
    final stretch = _opts['stretch'] == true;
    final rows = <rw.Desc>[
      ...connectorRows,
      rw.DTitle(root ? 'Root Node' : 'Joint & Branch'),
      rw.DButtons('Prev joint', () => _cmd({'op': 'cycle_joint', 'dir': -1}), iconA: 'back', b: 'Next joint', onB: () => _cmd({'op': 'cycle_joint', 'dir': 1}), iconB: 'next'),
      if (!root) rw.DInfo('Angle / Length', '${sel['angle']}° · ${sel['length']}'),
      rw.DSeg(const ['Rotate', 'Stretch'], stretch ? 1 : 0, label: 'Drag mode', onSelect: (i) => _cmd({'op': 'set', 'k': 'stretch', 'v': i == 1})),
      rw.DButtons('Add segment', () => _cmd({'op': 'add_segment'}), iconA: 'plus', b: 'Paste branch', onB: _st['branchCopied'] == true ? () => _cmd({'op': 'paste_branch'}) : null, iconB: 'paste'),
      if (!root) ...[
        rw.DButtons('Copy segment', () => _cmd({'op': 'copy_branch', 'branch': false}), iconA: 'copy', b: 'Copy branch', onB: () => _cmd({'op': 'copy_branch', 'branch': true}), iconB: 'copy'),
        rw.DButtons('Split ($_splitN)', () => _cmd({'op': 'split', 'n': _splitN + 1}), iconA: 'split', b: 'Duplicate', onB: () => _cmd({'op': 'duplicate_branch'}), iconB: 'duplicate'),
        rw.DStep('Split joints', '$_splitN', onMinus: () => setState(() => _splitN = math.max(1, _splitN - 1)), onPlus: () => setState(() => _splitN = math.min(20, _splitN + 1))),
        rw.DButtons('Flip branch', () => _cmd({'op': 'flip_branch', 'h': true}), iconA: 'flipH', b: sel['pinned'] == true ? 'Unlock' : 'Lock', onB: () => _cmd({'op': 'pin', 'v': sel['pinned'] != true}), iconB: sel['pinned'] == true ? 'unlock' : 'lock'),
        rw.DTick('Static detail', sel['static'] == true, onChanged: (v) => _cmd({'op': 'segment', 'static': v})),
        rw.DTick('Is stretchy', sel['stretchy'] == true, onChanged: (v) => _cmd({'op': 'node', 'stretchy': v})),
        rw.DTick('Smart stretch', sel['smart'] == true, onChanged: (v) => _cmd({'op': 'segment', 'smart': v})),
        rw.DStep('Thickness', thick.round().toString(), onMinus: () => _cmd({'op': 'segment', 'thickness': math.max(0, thick - 2)}), onPlus: () => _cmd({'op': 'segment', 'thickness': thick + 2})),
        rw.DWidget(kit.PropRow('Colour', trailing: _swatches(color, (c) => _cmd({'op': 'segment', 'color': c})))),
        rw.DButtons('Colour branch', () => _cmd({'op': 'segment', 'branch': true, 'color': color}), b: 'Modify branch…', onB: _modifyBranchDialog),
        const rw.DTitle('Draw Order'),
        rw.DSeg(const ['Segment', 'Branch'], _reorderBranch ? 1 : 0, onSelect: (i) => setState(() => _reorderBranch = i == 1)),
        rw.DButtons('Back', () => _cmd({'op': 'reorder', 'branch': _reorderBranch, 'to': -1}), iconA: 'back', b: 'Forward', onB: () => _cmd({'op': 'reorder', 'branch': _reorderBranch, 'to': 1}), iconB: 'next'),
        rw.DButtons('To back', () => _cmd({'op': 'reorder', 'branch': _reorderBranch, 'to': -2}), b: 'To front', onB: () => _cmd({'op': 'reorder', 'branch': _reorderBranch, 'to': 2})),
        rw.DButtons('Delete joint', () => _cmd({'op': 'delete_joint'}), b: 'Delete branch', onB: () => _cmd({'op': 'delete_segment'})),
      ] else ...[
        rw.DButtons('Modify branch…', _modifyBranchDialog),
      ],
      const rw.DTitle('Connectors & Polyfills'),
      rw.DButtons('Add connector', () => _cmd({'op': 'connect_start'}), iconA: 'chain', b: 'Add polyfill', onB: () => _cmd({'op': 'polyfill_new'}), iconB: 'plus'),
      for (final f in ((_st['polyfills'] as List?) ?? const []).cast<Map>())
        rw.DLink('Polyfill ${f['id']}', '${f['count']} pts', onTap: () => _cmd({'op': 'polyfill_edit', 'id': f['id']})),
    ];
    return [
      for (final d in rows) rw.renderDesc(d),
      ..._traceTools(),
      rw.renderDesc(const rw.DTitle('Library')),
      rw.renderDesc(rw.DButtons('Save .nodes…', _saveFigureFile, iconA: 'save')),
    ];
  }

  static const _gap = SizedBox(height: 6);

  Widget _title(String s) => Container(
        width: double.infinity,
        padding: const EdgeInsets.only(top: 8, bottom: 2),
        child: Text(s, maxLines: 1, overflow: TextOverflow.ellipsis, textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w700)),
      );

  Widget _label(String s) => Padding(padding: const EdgeInsets.only(top: 2, bottom: 4), child: Text(s, style: const TextStyle(color: cText, fontSize: 13)));

  // the row passes touches a little above and below it to its buttons (a row
  // alone turns away touches outside itself): 44 pt touches, 38 pt drawn
  Widget _pair(Widget a, Widget b) => kit.HitPad(dy: 3, child: Row(children: [Expanded(child: a), const SizedBox(width: 6), Expanded(child: b)]));

  Widget _stepper(String label, String value, VoidCallback minus, VoidCallback plus) => kit.StepRow(label, value, onMinus: minus, onPlus: plus);
}

// ---------------------------------------------------------------- stage overlay

class _HueSatPainter extends CustomPainter {
  final HSVColor c;
  _HueSatPainter(this.c);

  @override
  void paint(Canvas canvas, Size size) {
    final r = Offset.zero & size;
    final hues = [for (var i = 0; i <= 6; i++) HSVColor.fromAHSV(1, (i * 60.0) % 360, 1, c.value).toColor()];
    canvas.drawRect(r, Paint()..shader = LinearGradient(colors: hues).createShader(r));
    // toward the bottom: no saturation (grey at this brightness)
    final grey = HSVColor.fromAHSV(1, 0, 0, c.value).toColor();
    canvas.drawRect(r, Paint()..shader = LinearGradient(begin: Alignment.topCenter, end: Alignment.bottomCenter, colors: [grey.withValues(alpha: 0), grey]).createShader(r));
    final p = Offset(c.hue / 360 * size.width, (1 - c.saturation) * size.height);
    canvas.drawCircle(p, 7, Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.5
      ..color = Colors.white);
    canvas.drawCircle(p, 8.5, Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1
      ..color = Colors.black);
  }

  @override
  bool shouldRepaint(_HueSatPainter o) => o.c != c;
}

class _BrightPainter extends CustomPainter {
  final HSVColor c;
  _BrightPainter(this.c);

  @override
  void paint(Canvas canvas, Size size) {
    final r = Offset.zero & size;
    canvas.drawRect(r, Paint()..shader = LinearGradient(begin: Alignment.topCenter, end: Alignment.bottomCenter, colors: [c.withValue(1).toColor(), Colors.black]).createShader(r));
    final y = (1 - c.value) * size.height;
    canvas.drawRect(Rect.fromLTWH(-2, y - 3, size.width + 4, 6), Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..color = Colors.white);
  }

  @override
  bool shouldRepaint(_BrightPainter o) => o.c != c;
}

/// Panel help text (hidden with Settings > Fewer explanations).
Widget _hint(String text) => AppSettings.flag('nerd') ? const SizedBox.shrink() : Text(text, style: const TextStyle(color: cSecondary, fontSize: 11));

class _MagPainter extends CustomPainter {
  final ui.Image? img;
  final List handles;
  final double x0, y1, k;
  _MagPainter(this.img, this.handles, this.x0, this.y1, this.k);

  @override
  void paint(Canvas canvas, Size size) {
    final i = img;
    if (i != null) {
      paintImage(canvas: canvas, rect: Offset.zero & size, image: i, fit: BoxFit.fill, filterQuality: FilterQuality.medium);
    }
    for (final h in handles) {
      final l = h as List;
      final p = Offset(((l[0] as num).toDouble() - x0) * k, (y1 - (l[1] as num).toDouble()) * k);
      if (!(Offset.zero & size).inflate(8).contains(p)) continue;
      final sel = (l[2] as num).toInt() & 2 != 0;
      canvas.drawCircle(p, 6, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2
        ..color = sel ? const Color(0xFFE0383A) : const Color(0xFF2F6BE8));
    }
  }

  @override
  bool shouldRepaint(_MagPainter o) => o.img != img || o.handles != handles || o.x0 != x0 || o.y1 != y1 || o.k != k;
}

/// The stage's picture, drawn over the stage area it was made for (a stale
/// picture after a resize or pan stays in place and in proportion).
class _StagePicture extends CustomPainter {
  final ui.Image img;
  final Rect rect;
  final bool camera;
  final StageView v;
  final Size shot;
  _StagePicture(this.img, this.rect, this.camera, this.v, this.shot);

  @override
  void paint(Canvas canvas, Size size) {
    final Rect dest;
    if (camera) {
      // the shot fitted into the stage area
      final k = math.min(size.width / shot.width, size.height / shot.height);
      final c = size.center(Offset.zero);
      dest = Rect.fromLTRB(c.dx + rect.left * k, c.dy + rect.top * k, c.dx + rect.right * k, c.dy + rect.bottom * k);
    } else {
      dest = Rect.fromPoints(v.toScreen(rect.left, rect.top), v.toScreen(rect.right, rect.bottom));
    }
    if (dest.isEmpty || !dest.isFinite) return;
    canvas.drawImageRect(img, Rect.fromLTWH(0, 0, img.width.toDouble(), img.height.toDouble()), dest, Paint()..filterQuality = FilterQuality.low);
  }

  // the view changes in place: always repaint (one image draw)
  @override
  bool shouldRepaint(_StagePicture o) => true;
}

class _Overlay extends CustomPainter {
  final Map<String, dynamic> st;
  final StageView v;
  final bool playing;
  final bool studioGrid;
  final bool studioCrosshair;
  _Overlay(this.st, this.v, this.playing, {this.studioGrid = true, this.studioCrosshair = true});

  @override
  void paint(Canvas canvas, Size size) {
    if (st.isEmpty) return;
    if (playing) {
      // the shot fitted into the stage area; bars outside it
      final sw = (st['w'] as num).toDouble(), sh = (st['h'] as num).toDouble();
      final k = math.min(size.width / sw, size.height / sh);
      final shot = Rect.fromCenter(center: size.center(Offset.zero), width: sw * k, height: sh * k);
      final bars = Path.combine(PathOperation.difference, Path()..addRect(Offset.zero & size), Path()..addRect(shot));
      canvas.drawPath(bars, Paint()..color = cSurround);
      return;
    }
    final isStudio = st['building'] != null;
    if (isStudio) {
      final mirrorOn = (st['opts'] as Map?)?['mirror'] == true;
      paintFigureStudioGuides(
        canvas,
        size,
        toScreen: v.toScreen,
        scale: v.s,
        showGrid: studioGrid,
        showCrosshair: studioCrosshair,
        showMirrorAxis: mirrorOn,
      );
    }
    final cam = (st['camera'] as List?)?.map((x) => (x as num).toDouble()).toList();
    final cameraMode = st['mode'] == 'camera';
    if (!isStudio && cam != null && cam.length == 8 && AppSettings.flag('stageBox')) {
      // the stage: the shot itself, or in camera mode the stage at rest (the
      // camera's frame is drawn over it, Camera board)
      final Path frame;
      if (cameraMode) {
        final w = (st['w'] as num).toDouble() / 2, h = (st['h'] as num).toDouble() / 2;
        final a = v.toScreen(-w, -h), b = v.toScreen(w, h);
        frame = Path()..addRect(Rect.fromPoints(a, b));
      } else {
        frame = Path()..moveTo(v.toScreen(cam[0], cam[1]).dx, v.toScreen(cam[0], cam[1]).dy);
        for (var i = 2; i < 8; i += 2) {
          final p = v.toScreen(cam[i], cam[i + 1]);
          frame.lineTo(p.dx, p.dy);
        }
        frame.close();
      }
      // offstage: a quiet grey veil, see-through so figures standing offstage
      // still show (the user, 2026-11-24: no checkerboard); the shot reads as
      // a sheet on it, and moving the camera moves the sheet
      final outside = Path.combine(PathOperation.difference, Path()..addRect(Offset.zero & size), frame);
      canvas.drawPath(outside, Paint()..color = const Color(0xB8E2E5E9));
      canvas.drawPath(frame, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = const Color(0xFFB9BEC6));
    }
    // camera mode: the camera's frame in blue with square corner handles, and
    // the chosen guide inside it (Camera board)
    if (cam != null && cam.length == 8 && cameraMode) {
      final pts = [for (var i = 0; i < 8; i += 2) v.toScreen(cam[i], cam[i + 1])];
      canvas.drawPath(Path()..addPolygon(pts, true), Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2
        ..color = cAccent);
      _guides(canvas, cam, AppSettings.number('cameraGuide'));
      for (final p in pts) {
        final r = Rect.fromCenter(center: p, width: 10, height: 10);
        canvas.drawRect(r, Paint()..color = cCard);
        canvas.drawRect(r, Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5
          ..color = cAccent);
      }
    }
    // the motion path (round 10 boards): the selected joint on every frame,
    // pale red, numbered where its figure has poses
    final mp = st['motionPath'] as Map?;
    if (mp != null) {
      final pts = [for (final q in (mp['points'] as List)) v.toScreen(((q as List)[0] as num).toDouble(), (q[1] as num).toDouble())];
      if (pts.length > 1) {
        canvas.drawPath(Path()..addPolygon(pts, false), Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5
          ..color = const Color(0x99E04A62));
        for (final p in pts) {
          canvas.drawCircle(p, 2.2, Paint()..color = const Color(0xFFD91A3A));
        }
        // poses: a hollow ring and the frame number (frames on one spot share a label)
        final labels = <Offset, List<int>>{};
        for (final k in ((mp['keys'] as List?) ?? const []).cast<num>()) {
          final i = k.toInt();
          if (i < 0 || i >= pts.length) continue;
          final at = labels.keys.firstWhere((o) => (o - pts[i]).distance < 6, orElse: () => pts[i]);
          (labels[at] ??= []).add(i + 1);
        }
        for (final e in labels.entries) {
          canvas.drawCircle(e.key, 5.5, Paint()..color = cCard);
          canvas.drawCircle(e.key, 5.5, Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = 1.8
            ..color = const Color(0xFFD91A3A));
          final tp = TextPainter(
            text: TextSpan(text: e.value.join(', '), style: const TextStyle(color: Color(0xFFD91A3A), fontSize: 11, fontWeight: FontWeight.w700)),
            textDirection: TextDirection.ltr,
          )..layout();
          tp.paint(canvas, e.key + const Offset(8, -16));
        }
      }
    }
    // a selected textfield: a thin blue box around the text
    final tf = st['textfield'] as Map<String, dynamic>?;
    if (tf != null) {
      final r = Rect.fromPoints(
        v.toScreen((tf['x0'] as num).toDouble(), (tf['y0'] as num).toDouble()),
        v.toScreen((tf['x1'] as num).toDouble(), (tf['y1'] as num).toDouble()),
      );
      canvas.drawRect(r, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5
        ..color = cAccent);
    }
    // guides: a vertical and a horizontal line through the stage centre
    if (AppSettings.flag('guides')) {
      final c = v.toScreen(0, 0);
      canvas.drawLine(Offset(c.dx, 0), Offset(c.dx, size.height), Paint()
        ..color = const Color(0xCC30D158)
        ..strokeWidth = 1.5);
      canvas.drawLine(Offset(0, c.dy), Offset(size.width, c.dy), Paint()
        ..color = const Color(0xCCFF9F0A)
        ..strokeWidth = 1.5);
    }
    // figure names under their main joints
    if (AppSettings.flag('showIds')) {
      for (final f in (st['figures'] as List?) ?? const []) {
        final at = (f as Map)['at'] as List?;
        if (at == null) continue;
        final p = v.toScreen((at[0] as num).toDouble(), (at[1] as num).toDouble());
        final tp = TextPainter(
          text: TextSpan(text: '${f['name']}', style: const TextStyle(color: Colors.white, fontSize: 11, backgroundColor: Color(0xCC2B2E33))),
          textDirection: TextDirection.ltr,
        )..layout();
        tp.paint(canvas, p + Offset(-tp.width / 2, 12));
      }
    }
    // creation mode: the segment being drawn
    // a segment being drawn (J5, FigureEditor board): a light ghost of the
    // shape, a ring where the finger is, and its mirror image when Mirror is on
    final dl = st['drawLine'] as Map?;
    void ghost(List l) {
      final a = v.toScreen((l[0] as num).toDouble(), (l[1] as num).toDouble()), b = v.toScreen((l[2] as num).toDouble(), (l[3] as num).toDouble());
      final width = math.max(4.0, ((dl!['width'] as num?)?.toDouble() ?? 0) * v.s);
      final tint = Paint()..color = cAccent.withValues(alpha: 0.28);
      switch (dl['shape']) {
        case 'circle' || 'ellipse' || 'polygon' || 'triangle':
          // the shape spans its segment
          final r = (b - a).distance / 2;
          if (dl['shape'] == 'ellipse') {
            canvas.save();
            canvas.translate((a.dx + b.dx) / 2, (a.dy + b.dy) / 2);
            canvas.rotate((b - a).direction);
            canvas.drawOval(Rect.fromCenter(center: Offset.zero, width: r * 2, height: width), tint);
            canvas.restore();
          } else {
            canvas.drawCircle((a + b) / 2, r, tint);
          }
        default:
          canvas.drawLine(a, b, tint
            ..strokeWidth = width
            ..strokeCap = dl['shape'] == 'line' ? StrokeCap.round : StrokeCap.butt);
      }
      canvas.drawCircle(b, 15, Paint()..color = cAccent.withValues(alpha: 0.14));
      canvas.drawCircle(b, 15, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5
        ..color = cAccent.withValues(alpha: 0.35));
    }

    if (dl != null) {
      if (dl['mirror'] is List) ghost(dl['mirror'] as List);
      ghost(dl['line'] as List);
    }
    // the selected picture (Picture board): a dashed box with a grip at each
    // corner; drag inside to move it, a corner to scale it
    final pbox = (st['picture'] as Map?)?['box'] as List?;
    if (pbox != null && pbox.length == 8) {
      final pts = [for (var i = 0; i < 4; i++) v.toScreen((pbox[i * 2] as num).toDouble(), (pbox[i * 2 + 1] as num).toDouble())];
      final dash = Paint()
        ..color = cAccent
        ..strokeWidth = 1.5;
      for (var i = 0; i < 4; i++) {
        final a = pts[i], b = pts[(i + 1) % 4];
        final len = (b - a).distance;
        if (len < 1) continue;
        final d = (b - a) / len;
        for (var u = 0.0; u < len; u += 9) {
          canvas.drawLine(a + d * u, a + d * math.min(len, u + 5), dash);
        }
      }
      for (final c in pts) {
        final r = Rect.fromCenter(center: c, width: 9, height: 9);
        canvas.drawRect(r, Paint()..color = Colors.white);
        canvas.drawRect(r, Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5
          ..color = cAccent);
      }
    }
    // the figure editor's size reference: the engine draws the stock figure
    // faintly; its label goes under its feet
    final ref = st['sizeRef'] as List?;
    if (ref != null) {
      final tp = TextPainter(
        text: const TextSpan(text: 'Stock size', style: TextStyle(color: Color(0xFF9AA0A8), fontSize: 11, fontFamily: kit.kFont)),
        textDirection: TextDirection.ltr,
      )..layout();
      tp.paint(canvas, v.toScreen((ref[0] as num).toDouble(), (ref[1] as num).toDouble()) - Offset(tp.width / 2, tp.height / 2));
    }
    final hs = (st['handles'] as List?) ?? const [];
    // selected joint red, the joints connected to it purple with a line to
    // each, everything else blue rings (after the original's look)
    const purple = Color(0xFFA534D0), red = Color(0xFFD91A3A), blue = Color(0xFF2D63E0);
    Offset? selAt;
    final near = <Offset>[];
    for (final h in hs) {
      final l = h as List;
      final f = (l[2] as num).toInt();
      final p = v.toScreen((l[0] as num).toDouble(), (l[1] as num).toDouble());
      if (f & 2 != 0) selAt = p;
      if (f & 8 != 0) near.add(p);
    }
    if (selAt != null && AppSettings.flag('neighbourLines')) {
      final line = Paint()
        ..color = purple
        ..strokeWidth = 2.5
        ..strokeCap = StrokeCap.round;
      for (final p in near) {
        canvas.drawLine(selAt, p, line);
      }
    }
    final hasActiveSelection = (st['selected'] != null || st['textfield'] != null || st['picture'] != null || st['movieclip'] != null || st['adjustment'] != null) && st['building'] == null;
    for (final h in hs) {
      final l = h as List;
      final p = v.toScreen((l[0] as num).toDouble(), (l[1] as num).toDouble());
      final f = (l[2] as num).toInt();
      // the selected picture's main handle: its box and grips stand in for it
      if (pbox != null && f & 3 == 3) continue;
      final root = f & 1 != 0, sel = f & 2 != 0, inFig = f & 4 != 0, nb = f & 8 != 0;
      final quiet = hasActiveSelection && !inFig && !sel;
      final rawColor = sel ? red : nb ? purple : (root ? cRootNode : blue);
      final color = quiet ? rawColor.withValues(alpha: 0.52) : rawColor;
      // Settings: node style (0 legacy squares, 1 modern rings, 2 solid dots)
      // and line width (1 thin ... 5 thick)
      final style = AppSettings.number('nodeStyle'), k = (0.6 + 0.2 * AppSettings.number('lineWidth').clamp(1, 5)) * (quiet ? 0.86 : 1.0);
      final stroke = Paint()
        ..style = style == 2 ? PaintingStyle.fill : PaintingStyle.stroke
        ..strokeWidth = (sel || nb ? 2.5 : 2) * k
        ..color = color;
      final halo = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = (quiet ? 3.5 : 4.5) * k
        ..color = quiet ? const Color(0x99FFFFFF) : const Color(0xCCFFFFFF);
      // figure editor View options: Node highlight (a filled ring on the
      // polynodes, connectors or locked joints)
      final hl = AppSettings.number('nodeHighlight');
      final lit = (hl == 1 && f & 16 != 0) || (hl == 2 && f & 32 != 0) || (hl == 3 && f & 64 != 0);
      if (lit) canvas.drawCircle(p, 9, Paint()..color = const Color(0xAAFFB020));
      if (sel) {
        canvas.drawCircle(p, 12.5 * k, Paint()..color = red.withValues(alpha: 0.16));
      }
      // shape-coded (FigureEditor board): the main node a dark square, the
      // selected joint a red dot, connectors diamonds and polyfill anchors
      // triangles in the figure editor, every other joint a ring
      final fill = Paint()..color = const Color(0xE6FFFFFF);
      final edge = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5 * k
        ..color = Colors.white;
      if (root) {
        final r = RRect.fromRectAndRadius(Rect.fromCenter(center: p, width: 10 * k, height: 10 * k), Radius.circular(1.5 * k));
        canvas.drawRRect(r, Paint()..color = sel ? red : cRootNode);
        canvas.drawRRect(r, edge);
      } else if (sel && style != 0) {
        canvas.drawCircle(p, 7.5 * k, Paint()..color = red);
        canvas.drawCircle(p, 7.5 * k, edge..strokeWidth = 2 * k);
      } else if (f & 32 != 0 || f & 128 != 0) {
        final d = (f & 32 != 0 ? 6.5 : 7) * k;
        final path = f & 32 != 0
            ? (Path()
              ..moveTo(p.dx, p.dy - d)
              ..lineTo(p.dx + d, p.dy)
              ..lineTo(p.dx, p.dy + d)
              ..lineTo(p.dx - d, p.dy)
              ..close())
            : (Path()
              ..moveTo(p.dx, p.dy - d)
              ..lineTo(p.dx + d, p.dy + d * 0.75)
              ..lineTo(p.dx - d, p.dy + d * 0.75)
              ..close());
        final pen = Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2 * k
          ..strokeJoin = StrokeJoin.round
          ..color = nb ? purple : (f & 32 != 0 ? const Color(0xFF2D9A8C) : const Color(0xFFD9821E));
        canvas.drawPath(path, halo);
        canvas.drawPath(path, fill);
        canvas.drawPath(path, pen);
      } else if (style == 0) {
        final r = Rect.fromCenter(center: p, width: 8 * k, height: 8 * k);
        canvas.drawRect(r, halo);
        canvas.drawRect(r, stroke);
      } else {
        canvas.drawCircle(p, 5.5 * k, halo);
        if (style != 2) canvas.drawCircle(p, 5.5 * k, fill);
        canvas.drawCircle(p, 5.5 * k, stroke);
      }
      // Show draw order: each joint's place in the drawing order
      if (l.length > 3 && AppSettings.flag('drawOrder')) {
        final tp = TextPainter(
          text: TextSpan(text: '${l[3]}', style: const TextStyle(color: Color(0xFF111111), fontSize: 11, fontWeight: FontWeight.w600, backgroundColor: Color(0xCCFFFFFF))),
          textDirection: TextDirection.ltr,
        )..layout();
        tp.paint(canvas, p + const Offset(7, -16));
      }
    }
    // the drawing readout, over everything: "Length N, A°"
    if (dl != null) {
      final l = dl['line'] as List;
      final b = v.toScreen((l[2] as num).toDouble(), (l[3] as num).toDouble());
      final tp = TextPainter(
        text: TextSpan(text: 'Length ${(dl['length'] as num).round()}, ${(dl['angle'] as num).round()}\u00b0', style: const TextStyle(color: Colors.white, fontSize: 11.5, fontWeight: FontWeight.w600, fontFamily: kit.kFont)),
        textDirection: TextDirection.ltr,
      )..layout();
      // right of the finger as on the board, or left of it where a floating
      // panel may cover the stage's right side
      final bw = tp.width + 22, room = size.width - (size.width > 700 ? 380 : 0);
      final left = b.dx + 16 + bw > room ? b.dx - 16 - bw : b.dx + 16;
      final box = RRect.fromRectAndRadius(Rect.fromLTWH(left, math.max(4, b.dy - 26), bw, 22), const Radius.circular(11));
      canvas.drawRRect(box, Paint()..color = const Color(0xDB222428));
      tp.paint(canvas, Offset(box.left + 11, box.top + (22 - tp.height) / 2));
    }
  }

  /// Default (centre lines, diagonals, centre circle), rule of thirds, phi
  /// grid, or a golden spiral in one of four turns, in the shot's own
  /// corners (so they follow its zoom and rotation).
  void _guides(Canvas canvas, List<double> cam, int mode) {
    Offset at(double u, double w) {
      final x = cam[0] + (cam[2] - cam[0]) * u + (cam[6] - cam[0]) * w;
      final y = cam[1] + (cam[3] - cam[1]) * u + (cam[7] - cam[1]) * w;
      return v.toScreen(x, y);
    }

    Paint pen(Color c) => Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5
      ..color = c;
    void line(double u0, double w0, double u1, double w1, Color c) => canvas.drawLine(at(u0, w0), at(u1, w1), pen(c));
    void dashed(double u0, double w0, double u1, double w1) {
      final a = at(u0, w0), b = at(u1, w1);
      final len = (b - a).distance;
      if (len < 1) return;
      final d = (b - a) / len;
      final p = pen(const Color(0xFFA9C8EC))..strokeWidth = 1.2;
      for (var t = 0.0; t < len; t += 9) {
        canvas.drawLine(a + d * t, a + d * math.min(len, t + 5), p);
      }
    }

    switch (mode) {
      case 0:
        // None (Camera board): no lines
        return;
      case 1 || 2:
        final a = mode == 1 ? 1 / 3 : 0.382;
        for (final k in [a, 1 - a]) {
          dashed(k, 0, k, 1);
          dashed(0, k, 1, k);
        }
      case >= 3 && <= 6:
        // golden spiral: r = e^(b θ), b = ln(phi) / (pi / 2), around the phi point
        final flipU = mode == 4 || mode == 6, flipW = mode == 5 || mode == 6;
        final path = Path();
        const b = 0.30634896253;
        for (var i = 0; i <= 240; i++) {
          final th = -i / 240 * 4 * math.pi;
          final r = 0.62 * math.exp(b * th);
          var u = 0.618 + r * math.cos(th) * 0.62, w = 0.382 + r * math.sin(th) * 0.62;
          if (flipU) u = 1 - u;
          if (flipW) w = 1 - w;
          final p = at(u.clamp(0, 1), w.clamp(0, 1));
          i == 0 ? path.moveTo(p.dx, p.dy) : path.lineTo(p.dx, p.dy);
        }
        canvas.drawPath(path, pen(const Color(0xCCFFB020)));
      default:
        line(0, 0.5, 1, 0.5, const Color(0xCCE0383A));
        line(0.5, 0, 0.5, 1, const Color(0xCC2F6BE8));
        line(0, 0, 1, 1, const Color(0x9934C759));
        line(0, 1, 1, 0, const Color(0x99BF5AF2));
        canvas.drawCircle(at(0.5, 0.5), 10, pen(const Color(0xCCFFFFFF)));
    }
  }

  @override
  bool shouldRepaint(_Overlay o) => true;
}

// ---------------------------------------------------------------- controls

/// The textfield's text: edits apply live, and a rebuild does not reset the
/// cursor while it has focus.
class _TextFieldBox extends StatefulWidget {
  final String text;
  final ValueChanged<String> onChanged;
  final bool oneLine;
  const _TextFieldBox({required this.text, required this.onChanged, this.oneLine = false});

  @override
  State<_TextFieldBox> createState() => _TextFieldBoxState();
}

class _TextFieldBoxState extends State<_TextFieldBox> {
  late final TextEditingController _ctl = TextEditingController(text: widget.text);
  final FocusNode _focus = FocusNode();

  @override
  void didUpdateWidget(_TextFieldBox old) {
    super.didUpdateWidget(old);
    if (!_focus.hasFocus && _ctl.text != widget.text) _ctl.text = widget.text;
  }

  @override
  void dispose() {
    _ctl.dispose();
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: _ctl,
      focusNode: _focus,
      maxLines: widget.oneLine ? 1 : 4,
      minLines: widget.oneLine ? 1 : 2,
      style: const TextStyle(color: cText, fontSize: 14),
      decoration: InputDecoration(
        hintText: 'Text',
        hintStyle: const TextStyle(color: cSecondary),
        filled: true,
        fillColor: cControl,
        isDense: true,
        border: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: BorderSide.none),
        contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      ),
      onChanged: widget.onChanged,
    );
  }
}

class _Btn extends StatelessWidget {
  final String label;
  final VoidCallback? onTap;
  final bool enabled, accent, danger, small;
  final IconData? icon;
  const _Btn(this.label, {this.onTap, this.enabled = true, this.accent = false, this.danger = false, this.small = false, this.icon});

  @override
  Widget build(BuildContext context) {
    // REDESIGN §3: blue words on field grey; the one main action is blue; red for destructive
    final on = enabled && onTap != null;
    final fg = !on ? cTag : accent ? cCard : danger ? cDanger : cAccent;
    final bg = accent ? (on ? cAccent : cTag) : danger ? const Color(0xFFFDECEE) : cControl;
    // drawn 38 (or 30) tall; touches 44 tall
    return kit.HitPad(
      dy: small ? 7 : 3,
      child: kit.Tap(
      onTap: on ? onTap : null,
      minSize: 0,
      child: Container(
        height: small ? 30 : 38,
        decoration: BoxDecoration(color: bg, borderRadius: BorderRadius.circular(10)),
        child: Center(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 6),
            child: icon != null
                ? Icon(icon, size: 18, color: fg)
                : Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: fg, fontSize: small ? 12 : 13, fontWeight: FontWeight.w700)),
          ),
        ),
      ),
      ),
    );
  }
}

/// A setting that is on or off: the big square tick box (REDESIGN §3).
class _Toggle extends StatelessWidget {
  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;
  const _Toggle(this.label, this.value, this.onChanged);

  @override
  Widget build(BuildContext context) => GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => onChanged(!value),
        child: kit.PropRow(label, trailing: kit.TickBox(value, onChanged: onChanged)),
      );
}

class _Chip extends StatelessWidget {
  final String label;
  final bool selected;
  final VoidCallback onTap;
  const _Chip(this.label, this.selected, this.onTap);

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
        decoration: BoxDecoration(color: selected ? cCard : cControl, borderRadius: BorderRadius.circular(9), border: selected ? Border.all(color: cAccent, width: 2) : null),
        child: Text(label, style: TextStyle(color: cAccent, fontSize: 12, fontWeight: selected ? FontWeight.w700 : FontWeight.w400)),
      ),
    );
  }
}

/// "Today, 07:04", "Yesterday, 18:30", "23 Nov, 09:12" or "23 Nov 2025, …".
String _friendlyTime(DateTime t, {bool seconds = false}) {
  final now = DateTime.now();
  String two(int v) => v.toString().padLeft(2, '0');
  final time = '${two(t.hour)}:${two(t.minute)}${seconds ? ':${two(t.second)}' : ''}';
  final day = DateTime(t.year, t.month, t.day);
  final today = DateTime(now.year, now.month, now.day);
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  final d = day == today
      ? 'Today'
      : day == today.subtract(const Duration(days: 1))
          ? 'Yesterday'
          : '${t.day} ${months[t.month - 1]}${t.year == now.year ? '' : ' ${t.year}'}';
  return '$d, $time';
}

/// The States board's "Start here" bubble beside + (an empty animation).
class _StartHere extends StatelessWidget {
  const _StartHere();
  @override
  Widget build(BuildContext context) => Row(mainAxisSize: MainAxisSize.min, children: [
        CustomPaint(size: const Size(6, 12), painter: _NubLeft()),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
          decoration: BoxDecoration(color: cHint, borderRadius: BorderRadius.circular(8)),
          child: const Text('Start here', style: TextStyle(color: cText, fontSize: 13)),
        ),
      ]);
}

class _NubLeft extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) => canvas.drawPath(Path()..moveTo(size.width, 0)..lineTo(0, size.height / 2)..lineTo(size.width, size.height)..close(), Paint()..color = cHint);
  @override
  bool shouldRepaint(_NubLeft o) => false;
}

/// The figure editor's shape glyphs (FigureEditor board): on the shelf, in
/// the Parts list and on the Shape page. Drawn in a 40 x 26 box.
class ShapeGlyph extends CustomPainter {
  ShapeGlyph(this.kind, this.color);
  final String kind;
  final Color color;
  @override
  void paint(Canvas canvas, Size size) {
    final (w, h) = (size.width, size.height);
    final cy = h / 2, x0 = w * 0.18, x1 = w * 0.82, t = h * 0.36;
    final fill = Paint()..color = color;
    switch (kind) {
      case 'select':
        canvas.drawPath(
            Path()
              ..moveTo(w / 2 - 6, cy - 10)
              ..relativeLineTo(12, 9)
              ..relativeLineTo(-5.2, 1.2)
              ..relativeLineTo(3, 6)
              ..relativeLineTo(-2.6, 1.2)
              ..relativeLineTo(-3, -6)
              ..relativeLineTo(-4.2, 3.4)
              ..close(),
            fill);
      case 'segment':
        canvas.drawRect(Rect.fromLTWH(x0, cy - t / 2, x1 - x0, t), fill);
      case 'rounded':
        canvas.drawLine(Offset(x0 + t / 2, cy), Offset(x1 - t / 2, cy), Paint()
          ..color = color
          ..strokeWidth = t
          ..strokeCap = StrokeCap.round);
      case 'circle':
        canvas.drawCircle(Offset(w / 2, cy), h * 0.36, fill);
      case 'ellipse':
        canvas.drawOval(Rect.fromCenter(center: Offset(w / 2, cy), width: w * 0.6, height: h * 0.52), fill);
      case 'triangle':
        canvas.drawPath(Path()..moveTo(w / 2, cy - h * 0.34)..lineTo(w / 2 + h * 0.4, cy + h * 0.3)..lineTo(w / 2 - h * 0.4, cy + h * 0.3)..close(), fill);
      case 'trapezoid':
        canvas.drawPath(Path()..moveTo(x0, cy - t * 0.7)..lineTo(x1, cy - t * 0.3)..lineTo(x1, cy + t * 0.3)..lineTo(x0, cy + t * 0.7)..close(), fill);
      case 'polygon':
        final p = Path();
        for (var i = 0; i < 5; i++) {
          final a = -math.pi / 2 + i * 2 * math.pi / 5;
          final q = Offset(w / 2 + math.cos(a) * h * 0.38, cy + math.sin(a) * h * 0.38);
          i == 0 ? p.moveTo(q.dx, q.dy) : p.lineTo(q.dx, q.dy);
        }
        canvas.drawPath(p..close(), fill);
      case 'polyfill':
        canvas.drawPath(
            Path()
              ..moveTo(w * 0.22, cy + h * 0.28)
              ..quadraticBezierTo(w * 0.3, cy - h * 0.4, w * 0.55, cy - h * 0.3)
              ..quadraticBezierTo(w * 0.8, cy - h * 0.2, w * 0.8, cy + h * 0.2)
              ..close(),
            Paint()..color = color.withValues(alpha: 0.85));
        for (final o in [Offset(w * 0.22, cy + h * 0.28), Offset(w * 0.8, cy + h * 0.2)]) {
          canvas.drawCircle(o, 2, Paint()..color = cCard);
          canvas.drawCircle(o, 2, Paint()
            ..color = color
            ..style = PaintingStyle.stroke);
        }
      case 'connector':
        canvas.drawPath(Path()..moveTo(w / 2, cy - 6)..lineTo(w / 2 + 6, cy)..lineTo(w / 2, cy + 6)..lineTo(w / 2 - 6, cy)..close(), Paint()
          ..color = color
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2);
      case 'mirror':
        final dash = Paint()
          ..color = color
          ..strokeWidth = 1.6;
        for (var y = cy - h * 0.42; y < cy + h * 0.42; y += 5) {
          canvas.drawLine(Offset(w / 2, y), Offset(w / 2, math.min(y + 2.5, cy + h * 0.42)), dash);
        }
        final arm = Paint()
          ..color = color
          ..strokeWidth = t * 0.7
          ..strokeCap = StrokeCap.round;
        canvas.drawLine(Offset(w / 2 - 4, cy - 7), Offset(w / 2 - 14, cy + 6), arm);
        canvas.drawLine(Offset(w / 2 + 4, cy - 7), Offset(w / 2 + 14, cy + 6), arm);
    }
  }

  @override
  bool shouldRepaint(ShapeGlyph o) => o.kind != kind || o.color != color;
}

/// A sound clip's waveform (M5): rounded bars, as on the round 10 board.
class _WavePeaks extends CustomPainter {
  _WavePeaks(this.peaks);
  final List<double> peaks;
  @override
  void paint(Canvas canvas, Size size) {
    final pen = Paint()..color = const Color(0xFF86AFC8);
    // its own loudest moment fills the height, so a quiet sound still shows its shape
    final top = peaks.fold<double>(0, math.max);
    for (var i = 0; i < peaks.length; i++) {
      final a = math.max(0.12, top > 0 ? (peaks[i] / top).clamp(0.0, 1.0) : 0.0);
      final h = a * (size.height - 8);
      canvas.drawRRect(RRect.fromRectAndRadius(Rect.fromLTWH(6 + i * 4.0, (size.height - h) / 2, 2.2, h), const Radius.circular(1)), pen);
    }
  }

  @override
  bool shouldRepaint(_WavePeaks o) => o.peaks != peaks;
}

/// The timeline's selection box or lasso, as it is drawn (G5).
class _BoxPainter extends CustomPainter {
  _BoxPainter(this.path, this.lasso);
  final List<Offset> path;
  final bool lasso;
  @override
  void paint(Canvas canvas, Size size) {
    if (path.length < 2) return;
    final fill = Paint()..color = cAccent.withValues(alpha: 0.08);
    final line = Paint()
      ..color = cAccent
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    if (lasso) {
      final p = Path()..addPolygon(path, true);
      canvas.drawPath(p, fill);
      canvas.drawPath(p, line);
    } else {
      final r = RRect.fromRectAndRadius(Rect.fromPoints(path.first, path.last), const Radius.circular(4));
      canvas.drawRRect(r, fill);
      canvas.drawRRect(r, line);
    }
  }

  @override
  bool shouldRepaint(_BoxPainter old) => true;
}

/// A block or keyframe being dragged on the timeline (User list #1, #2, #3):
/// - `block_end` (edge 0): right grip (changes block length both ways)
/// - `block_move` / `pose_key` (edge 1): held block / keyframe moved freely
/// - `block_start` (edge 2): left grip (moves start both ways within neighbour bounds)
/// - `prop_key`: animated property keyframe dragged along its row
class _BlockDrag {
  _BlockDrag(
    this.layer,
    this.start,
    this.length,
    this.cell, {
    this.edge = 0,
    this.kind = 'block_end',
    this.run,
    this.property,
    this.minStart,
    this.maxStart,
    this.multiPoses,
    this.multiProps,
    this.scaleAnchor,
    this.scaleBaseSpan = 1,
    this.scaleFromLeft = false,
    this.block,
  });
  final String layer;
  final int start, length, edge;
  final String kind;
  final double cell;
  final int? minStart, maxStart;
  /// K2c: the script run being dragged (null: a pose or property key).
  final String? run;
  /// User list #3: the animated property id when dragging a property key.
  final String? property;
  /// Round 3: snapshot of multi-selected keys & proportional stretch anchor.
  final Set<(String, int)>? multiPoses;
  final Set<(String, int)>? multiProps;
  final int? scaleAnchor;
  final int scaleBaseSpan;
  final bool scaleFromLeft;
  /// The pose block's index in its row, for a grip drag.
  final int? block;
  double dx = 0;
  double? rawStartX;
  double startScrollX = 0;
  int? lastSent;
  double? lastFactor;
  bool live = false;

  /// Where the block/key would be now: (first frame, length).
  (int, int) preview({bool allowStart0 = false}) {
    final by = (dx / cell).round();
    final lo = minStart ?? (allowStart0 ? 0 : 1);
    if (kind == 'prop_key') return (math.max(0, start + by), 1);
    return switch (edge) {
      0 => (start, math.max(1, length + by)),
      1 => (math.max(lo, start + by), length),
      _ => () {
          final hi = maxStart ?? (start + length - 1);
          final ns = (start + by).clamp(lo, math.max(lo, hi)).toInt();
          return (ns, math.max(1, start + length - ns));
        }(),
    };
  }
}

/// Subtle visual cue along the bottom of multi-frame pose blocks:
/// directional forward chevrons (`›`) for tweening blocks vs a calm flat hold
/// bar with an end tick for held poses.
class _BlockSpanCuePainter extends CustomPainter {
  const _BlockSpanCuePainter({required this.tween, required this.active});
  final bool tween, active;

  @override
  void paint(Canvas canvas, Size size) {
    if (size.width < 6) return;
    final c = active ? cAccent.withValues(alpha: 0.72) : (tween ? const Color(0xFF6B8FB8) : const Color(0xFF9EA6B0));
    final y = size.height / 2;
    if (tween) {
      final pen = Paint()
        ..color = c
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.25
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round;
      canvas.drawLine(Offset(0, y), Offset(size.width, y), Paint()..color = c.withValues(alpha: 0.35)..strokeWidth = 1.0);
      for (var x = 6.0; x < size.width - 2; x += 10) {
        final p = Path()
          ..moveTo(x - 1.6, y - 1.6)
          ..lineTo(x + 0.8, y)
          ..lineTo(x - 1.6, y + 1.6);
        canvas.drawPath(p, pen);
      }
    } else {
      final holdPen = Paint()
        ..color = c.withValues(alpha: active ? 0.5 : 0.55)
        ..strokeWidth = 1.4
        ..strokeCap = StrokeCap.round;
      canvas.drawLine(Offset(0, y), Offset(size.width, y), holdPen);
      canvas.drawLine(Offset(size.width, y - 1.6), Offset(size.width, y + 1.6), holdPen);
    }
  }

  @override
  bool shouldRepaint(_BlockSpanCuePainter old) => old.tween != tween || old.active != active;
}

/// A script run's trace in its block: what the script does over the run (its
/// first changed value), scaled to the block; dashed when it can't run.
class _TracePainter extends CustomPainter {
  _TracePainter(this.values, this.color, {this.missing = false});
  final List<double?> values;
  final Color color;
  final bool missing;

  @override
  void paint(Canvas canvas, Size size) {
    final pen = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.6
      ..strokeCap = StrokeCap.round
      ..color = color;
    final ys = values.whereType<double>().toList();
    if (missing || ys.length < 2) {
      // nothing worked out: a flat dashed line
      for (var x = 8.0; x < size.width - 8; x += 8) {
        canvas.drawLine(Offset(x, size.height / 2), Offset(math.min(x + 4, size.width - 8), size.height / 2), pen);
      }
      return;
    }
    final lo = ys.reduce(math.min), hi = ys.reduce(math.max);
    final span = hi - lo < 1e-6 ? 1.0 : hi - lo;
    final path = Path();
    var started = false;
    for (var i = 0; i < values.length; i++) {
      final v = values[i];
      if (v == null) {
        started = false;
        continue;
      }
      final x = 8 + (size.width - 16) * (values.length == 1 ? 0.5 : i / (values.length - 1));
      final y = size.height - 6 - (size.height - 12) * ((v - lo) / span);
      if (started) {
        path.lineTo(x, y);
      } else {
        path.moveTo(x, y);
        started = true;
      }
    }
    canvas.drawPath(path, pen);
  }

  @override
  bool shouldRepaint(_TracePainter o) => o.values != values || o.color != color || o.missing != missing;
}

/// The ruler, the merged Preview row, the layer rows' grid, and the trailing runway.
class _TimelinePainter extends CustomPainter {
  _TimelinePainter({
    required this.frames,
    int? displayCells,
    required this.cell,
    required this.left,
    required this.fps,
    required this.cur,
    required this.rulerH,
    required this.stripH,
    required this.lines,
    required this.bands,
    required this.aspect,
    required this.thumbs,
    this.groupStrips = const [],
    this.loopStart,
    this.loopEnd,
    this.drawHeader = true,
    this.drawBody = true,
  }) : displayCells = displayCells ?? frames;

  /// K: folded groups' rows: (top, height, pictures by frame)
  final List<(double, double, Map<int, ui.Image>)> groupStrips;
  final int frames, displayCells, fps, cur;
  final int? loopStart, loopEnd;
  final double cell, left, rulerH, stripH, aspect;
  final List<double> lines;
  final List<(double, double)> bands;
  final Map<int, ui.Image> thumbs;
  final bool drawHeader, drawBody;

  static int rulerStep(int fps, double cell) {
    final safeFps = math.max(1, fps);
    if (cell < 18) return math.max(2, safeFps * 2);
    if (cell < 28) return math.max(2, safeFps);
    final raw = (cell >= 40 ? safeFps / 4 : safeFps / 2).ceil().clamp(2, safeFps);
    for (var d = raw; d <= safeFps; d++) {
      if (safeFps % d == 0) return d;
    }
    return raw;
  }

  static bool showRulerNumber(int f, int fps, int step, double cell) {
    if (f == 1) return true;
    final sec = fps > 0 && f % fps == 0;
    if (sec) return cell >= 18;
    if (f % step != 0) return false;
    if (cell < 64) {
      if (f == 2) return false;
      if (fps > 2 && (f % fps == 1 || f % fps == fps - 1)) return false;
    }
    return true;
  }

  @override
  void paint(Canvas canvas, Size size) {
    final endX = left + frames * cell;
    final step = rulerStep(fps, cell);

    if (drawHeader) {
      // Subtle runway shading on the header past `frames`
      if (displayCells > frames && endX < size.width) {
        canvas.drawRect(Rect.fromLTRB(endX, 0, size.width, size.height), Paint()..color = const Color(0xFFF7F8FA));
      }
      // Visual loop range indicator on the ruler when "Loop to" is configured
      if (loopStart != null && loopEnd != null && loopEnd! > loopStart!) {
        final lx0 = left + loopStart!.clamp(0, frames - 1) * cell;
        final lx1 = left + loopEnd!.clamp(1, frames) * cell;
        canvas.drawRect(Rect.fromLTRB(lx0, 0, lx1, rulerH), Paint()..color = const Color(0x1F1577D6));
        final bar = Paint()..color = cAccent;
        canvas.drawRect(Rect.fromLTRB(lx0, 0, lx1, 2), bar);
        canvas.drawRect(Rect.fromLTRB(lx0, 0, lx0 + 2, 7), bar);
        canvas.drawRect(Rect.fromLTRB(lx1 - 2, 0, lx1, 7), bar);
      }
      // Ruler ticks & frame numbers across both active frames AND trailing runway!
      for (var i = 0; i < displayCells; i++) {
        final inRunway = i >= frames;
        final f = i + 1, sec = f % fps == 0, label = showRulerNumber(f, fps, step, cell);
        final x = left + i * cell + cell / 2;
        final h = sec ? 8.0 : label ? 6.0 : 4.0;
        final tickColor = inRunway
            ? (sec ? const Color(0xFFB8BEC7) : const Color(0xFFD6DAE0))
            : (sec ? cInk2b : label ? const Color(0xFF9AA0A8) : const Color(0xFFC4C8CE));
        canvas.drawRect(Rect.fromLTWH(x - 0.5, rulerH - h, 1, h), Paint()..color = tickColor);
        if (label && i != cur) {
          final tp = TextPainter(
            text: TextSpan(
              text: '$f',
              style: TextStyle(
                color: inRunway ? const Color(0xFFAEB4BD) : (sec ? cText : cSecondary),
                fontSize: 11,
                fontWeight: sec && !inRunway ? FontWeight.w700 : FontWeight.w400,
                fontFamily: kit.kFont,
                fontFeatures: const [ui.FontFeature.tabularFigures()],
              ),
            ),
            textDirection: TextDirection.ltr,
          )..layout();
          tp.paint(canvas, Offset(x - tp.width / 2, 3));
        }
      }
      canvas.drawRect(Rect.fromLTWH(0, rulerH - 1, size.width, 1), Paint()..color = const Color(0xFFE6E8EB));
      if (stripH > 0) {
        _strip(canvas, rulerH + 2, stripH - 4, thumbs);
        canvas.drawRect(Rect.fromLTWH(0, size.height - 1, size.width, 1), Paint()..color = const Color(0xFFDADDE2));
      }
      if (displayCells > frames && endX < size.width) {
        canvas.drawRect(Rect.fromLTWH(endX, 0, 1, size.height), Paint()..color = const Color(0xFFDADDE2));
      }
    }

    if (drawBody) {
      // Subtle runway background past `frames` so users see open scrollable space to extend into
      if (displayCells > frames && endX < size.width) {
        canvas.drawRect(Rect.fromLTRB(endX, 0, size.width, size.height), Paint()..color = const Color(0xFFF8F9FB));
      }
      // Property rows paler band
      for (final (y, h) in bands) {
        canvas.drawRect(Rect.fromLTWH(0, y, size.width, h), Paint()..color = const Color(0xFFFAFBFC));
      }
      // Subtle current-playhead column highlight across all layer rows
      final maxRowY = lines.isEmpty ? size.height : lines.last;
      if (cur >= 0 && cur < displayCells) {
        canvas.drawRect(
          Rect.fromLTWH(left + cur * cell, 0, cell, maxRowY),
          Paint()..color = cAccent.withValues(alpha: 0.055),
        );
      }
      // Subtle vertical cell grid hairlines so empty cells read clearly per frame
      final gridStep = cell >= 24 ? 1 : step;
      final gridPaint = Paint()..color = const Color(0xFFF1F3F6);
      final secPaint = Paint()..color = const Color(0xFFE7EAEE);
      for (var i = 1; i < displayCells; i += gridStep) {
        final gx = left + i * cell;
        canvas.drawRect(Rect.fromLTWH(gx, 0, 1, maxRowY), i % fps == 0 ? secPaint : gridPaint);
      }
      // Folded group preview strips
      for (final (y, h, pics) in groupStrips) {
        _strip(canvas, y, h, pics);
      }
      // Horizontal row dividers
      for (final y in lines) {
        canvas.drawRect(Rect.fromLTWH(0, y - 1, size.width, 1), Paint()..color = const Color(0xFFEEF0F2));
      }
      // End-of-animation boundary line
      if (displayCells > frames && endX < size.width) {
        canvas.drawRect(Rect.fromLTWH(endX, 0, 1, maxRowY), Paint()..color = const Color(0xFFDADDE2));
      }
    }
  }

  /// One film strip at `top`, `th` tall: tiles at the picture's shape, each
  /// showing the moment under its middle.
  void _strip(Canvas canvas, double top, double th, Map<int, ui.Image> thumbs) {
    {
      final r = RRect.fromRectAndRadius(Rect.fromLTWH(left, top, frames * cell, th), const Radius.circular(5));
      canvas.save();
      canvas.clipRRect(r);
      canvas.drawRRect(r, Paint()..color = cCard);
      final tw = math.max(8.0, (th * aspect).roundToDouble());
      for (var x = 0.0; x < frames * cell; x += tw) {
        final i = ((x + tw / 2) / cell).floor().clamp(0, frames - 1);
        final img = thumbs[i];
        final dst = Rect.fromLTWH(left + x, top, tw - 1, th);
        if (img != null) {
          // cover: the picture's middle, cropped to the tile
          final iw = img.width.toDouble(), ih = img.height.toDouble();
          final k = math.max(dst.width / iw, dst.height / ih);
          final sw = dst.width / k, sh = dst.height / k;
          canvas.drawImageRect(img, Rect.fromLTWH((iw - sw) / 2, (ih - sh) / 2, sw, sh), dst, Paint()..filterQuality = FilterQuality.medium);
        }
        canvas.drawRect(Rect.fromLTWH(left + x + tw - 1, top, 1, th), Paint()..color = cCard);
      }
      canvas.restore();
      canvas.drawRRect(r, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = cLine);
    }
  }

  @override
  bool shouldRepaint(_TimelinePainter o) => true;
}

class _PickEntry {
  final String id, name;
  final bool mine, effect;
  const _PickEntry(this.id, this.name, this.mine, {this.effect = false});
}

class _FigurePicker extends StatefulWidget {
  final List<_PickEntry> entries;
  final Map<String, ui.Image> previews;
  final bool movieclip, canPaste, editing;
  const _FigurePicker(this.entries, this.previews, {this.movieclip = false, this.canPaste = false, this.editing = false});

  @override
  State<_FigurePicker> createState() => _FigurePickerState();
}

class _FigurePickerState extends State<_FigurePicker> {
  final TextEditingController _searchCtrl = TextEditingController();
  final List<String> _picked = [];
  String _filter = '';
  int _tab = 0; // Stock (figures, then effect figures), Yours
  int _cat = 0; // 0 All, 1 Figures, 2 Effects

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Widget _tile(_PickEntry f) {
    final on = _picked.contains(f.id);
    final img = widget.previews[f.id];
    // a tap adds it (round 10 board); holding picks several, added together
    return GestureDetector(
      onTap: () => _picked.isEmpty ? Navigator.pop(context, [f.id]) : setState(() => on ? _picked.remove(f.id) : _picked.add(f.id)),
      onLongPress: () => setState(() => on ? _picked.remove(f.id) : _picked.add(f.id)),
      onSecondaryTap: () => setState(() => on ? _picked.remove(f.id) : _picked.add(f.id)),
      child: Column(children: [
        Expanded(
          child: Stack(fit: StackFit.expand, children: [
            Container(
              decoration: BoxDecoration(color: on ? const Color(0xFFF2F7FD) : cCard, borderRadius: BorderRadius.circular(10), border: Border.all(color: on ? cAccent : cLine, width: on ? 2 : 1)),
              padding: const EdgeInsets.all(6),
              child: img == null ? const Center(child: Text('?', style: TextStyle(color: cSecondary))) : RawImage(image: img, fit: BoxFit.contain),
            ),
            if (on)
              Positioned(
                right: 5,
                top: 5,
                child: Container(
                  width: 18,
                  height: 18,
                  alignment: Alignment.center,
                  decoration: const BoxDecoration(color: cAccent, shape: BoxShape.circle),
                  child: Text('${_picked.indexOf(f.id) + 1}', style: const TextStyle(color: cCard, fontSize: 10.5, fontWeight: FontWeight.w700)),
                ),
              ),
          ]),
        ),
        const SizedBox(height: 3),
        Text(f.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: on ? cAccent : cText, fontSize: 12, fontWeight: on ? FontWeight.w700 : FontWeight.w400)),
      ]),
    );
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    final compact = size.height < 500;
    final match = widget.entries.where((f) => f.name.toLowerCase().contains(_filter.toLowerCase())).toList();
    final hasEffects = match.any((f) => !f.mine && f.effect) && match.any((f) => !f.mine && !f.effect);
    final stockList = switch (_cat) {
      1 => match.where((f) => !f.mine && !f.effect).toList(),
      2 => match.where((f) => !f.mine && f.effect).toList(),
      _ => [...match.where((f) => !f.mine && !f.effect), ...match.where((f) => !f.mine && f.effect)],
    };
    final lists = [stockList, match.where((f) => f.mine).toList()];
    final list = lists[_tab];
    return Dialog(
      backgroundColor: cCard,
      insetPadding: EdgeInsets.all(compact ? 8 : 12),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
      child: SizedBox(
        width: math.min(760, size.width * 0.94),
        height: math.min(580, size.height * (compact ? 0.94 : 0.9)),
        child: Padding(
          padding: EdgeInsets.fromLTRB(compact ? 14 : 20, compact ? 8 : 12, compact ? 14 : 20, compact ? 10 : 14),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            SizedBox(
              height: compact ? 32 : 36,
              child: Stack(alignment: Alignment.center, children: [
                Text(widget.movieclip ? 'Add a movieclip' : 'Add a stick figure', textAlign: TextAlign.center, style: TextStyle(color: cTitle, fontSize: compact ? 15.5 : 17, fontWeight: FontWeight.w700)),
                if (!compact && _picked.isNotEmpty)
                  Positioned(
                    left: 0,
                    child: Text('${_picked.length} selected', style: const TextStyle(color: cAccent, fontSize: 12, fontWeight: FontWeight.w600)),
                  ),
                Positioned(
                  right: -6,
                  child: kit.Tap(label: 'Close', minSize: 36, onTap: () => Navigator.pop(context), child: const Pictogram('x', size: 18, color: cSecondary)),
                ),
              ]),
            ),
            SizedBox(height: compact ? 4 : 6),
            Row(children: [
              SizedBox(width: compact ? 190 : 220, child: kit.Segmented(const ['Stock', 'Yours'], _tab, onSelect: (i) => setState(() => _tab = i))),
              if (_tab == 0 && hasEffects && size.width >= 620) ...[
                const SizedBox(width: 8),
                SizedBox(width: 210, child: kit.Segmented(const ['All', 'Figures', 'FX'], _cat, onSelect: (i) => setState(() => _cat = i))),
              ],
              const SizedBox(width: 10),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(bottom: 6),
                  child: Container(
                    height: compact ? 32 : 36,
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(10)),
                    child: Row(children: [
                      const Pictogram('search', size: 16, color: cSecondary),
                      const SizedBox(width: 8),
                      Expanded(
                        child: TextField(
                          controller: _searchCtrl,
                          style: const TextStyle(color: cText, fontSize: 13.5),
                          decoration: InputDecoration(
                            isDense: true,
                            border: InputBorder.none,
                            hintText: widget.movieclip ? 'Search movieclips…' : 'Search figures…',
                            hintStyle: const TextStyle(color: cSecondary, fontSize: 13),
                          ),
                          onChanged: (v) => setState(() => _filter = v),
                        ),
                      ),
                      if (_filter.isNotEmpty)
                        kit.Tap(
                          label: 'Clear search',
                          minSize: 28,
                          onTap: () {
                            _searchCtrl.clear();
                            setState(() => _filter = '');
                          },
                          child: const Pictogram('x', size: 14, color: cSecondary),
                        ),
                    ]),
                  ),
                ),
              ),
            ]),
            SizedBox(height: compact ? 4 : 6),
            Expanded(
              child: list.isEmpty
                  ? Center(
                      child: Text(
                        _tab == 1
                            ? (widget.movieclip
                                ? 'Put movieclip files (.nodemc) in the "Barkfart Figures" folder, or tap "New movieclip" below to make one.'
                                : 'Put figure files (.nodes, .nodemc, .stknds) in the "Barkfart Figures" folder (Files app) and they show up here.')
                            : 'Nothing matches "$_filter".',
                        textAlign: TextAlign.center,
                        style: const TextStyle(color: cSecondary, fontSize: 13),
                      ),
                    )
                  : RawScrollbar(
                      thumbVisibility: true,
                      thickness: 4,
                      radius: const Radius.circular(2),
                      thumbColor: const Color(0x666B7078),
                      child: GridView.builder(
                        gridDelegate: SliverGridDelegateWithMaxCrossAxisExtent(
                          maxCrossAxisExtent: compact ? 118 : 136,
                          mainAxisSpacing: compact ? 8 : 12,
                          crossAxisSpacing: compact ? 8 : 10,
                          childAspectRatio: compact ? 1.05 : 0.95,
                        ),
                        itemCount: list.length,
                        itemBuilder: (c, i) => _tile(list[i]),
                      ),
                    ),
            ),
            Container(
              padding: EdgeInsets.only(top: compact ? 6 : 10),
              decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
              // the board's footer: Import from Files… and Paste figure (figures)
              // or New movieclip (movieclips); the project's library beside them
              child: Row(children: [
                Expanded(
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Row(children: [
                      kit.WordButton('Import from Files…', size: 13.5, height: compact ? 32 : 36, onTap: () => Navigator.pop(context, ['#import'])),
                      kit.WordButton('This project\u2019s library…', size: 13.5, height: compact ? 32 : 36, onTap: () => Navigator.pop(context, ['#library'])),
                    ]),
                  ),
                ),
                if (_picked.isNotEmpty) ...[
                  kit.WordButton('Clear', size: 13.5, height: compact ? 32 : 36, onTap: () => setState(_picked.clear)),
                  const SizedBox(width: 6),
                  kit.WordButton('Add ${_picked.length}', size: 14, height: compact ? 32 : 36, primary: true, onTap: () => Navigator.pop(context, List<String>.from(_picked))),
                ] else if (widget.movieclip)
                  kit.WordButton('New movieclip', size: 14, height: compact ? 32 : 36, primary: true, onTap: widget.editing ? null : () => Navigator.pop(context, ['#newclip']))
                else ...[
                  if (widget.canPaste) ...[
                    kit.WordButton('Paste', icon: 'paste', size: 13.5, height: compact ? 32 : 36, onTap: () => Navigator.pop(context, ['#paste'])),
                    const SizedBox(width: 6),
                  ],
                  kit.WordButton('Create stick figure', icon: 'createFigure', size: 13.5, height: compact ? 32 : 36, primary: true, onTap: () => Navigator.pop(context, ['#create'])),
                ],
              ]),
            ),
          ]),
        ),
      ),
    );
  }
}

/// Open: saved animations with a filter and sort, Open / Delete / Cancel.
class _ExportDialog extends StatefulWidget {
  final double w, h;
  final int frames;
  final String name;
  // the watermark and "Make vertical" (Project Tools before the redesign)
  final Map? watermark;
  final void Function(Map<String, Object?>) cmd;
  final Widget Function(int, void Function(int)) swatches;
  final bool vertical;
  final ValueChanged<bool> onVertical;
  final double fps;
  const _ExportDialog(this.w, this.h, this.frames, this.name, {this.watermark, required this.cmd, required this.swatches, this.vertical = false, required this.onVertical, this.fps = 30});

  @override
  State<_ExportDialog> createState() => _ExportDialogState();
}

class _ExportDialogState extends State<_ExportDialog> {
  int _format = 0;
  int _size = 1;
  int _start = 1;
  int _end = 0;
  bool _transparent = false, _sound = true;
  late final TextEditingController _name = TextEditingController(text: widget.name);
  // mirrors the engine's watermark setting (its defaults: ":)" in red)
  late Map _wm = widget.watermark == null ? const {} : Map.of(widget.watermark!);
  late bool _wmOn = widget.watermark != null;
  late bool _vertical = widget.vertical;

  static const _widths = [320, 640, 1280, 1920];

  String _seconds(int frames) {
    final s = frames / (widget.fps > 0 ? widget.fps : 30);
    final t = s == s.roundToDouble() ? '${s.round()}' : s.toStringAsFixed(1);
    return '$t second${t == '1' ? '' : 's'}';
  }

  // shown in the redesign's order (REDESIGN §8); the engine's codes stay 0 GIF, 1 PNG frames, 2 video
  static const _order = [2, 0, 1];

  late final ScrollController _scroll = ScrollController();

  @override
  void dispose() {
    _scroll.dispose();
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final screenH = MediaQuery.sizeOf(context).height;
    final w = _widths[_size], h = (w * widget.h / widget.w).round();
    final last = _end == 0 ? widget.frames : _end;
    final count = last - _start + 1;
    final canTransparent = _format == 0 || _format == 1;
    return Dialog(
      backgroundColor: cCard,
      insetPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
      clipBehavior: Clip.antiAlias,
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: 460, maxHeight: math.max(240.0, screenH - 20)),
        child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Container(
            padding: const EdgeInsets.fromLTRB(20, 12, 20, 10),
            decoration: const BoxDecoration(border: Border(bottom: BorderSide(color: Color(0xFFEEF0F2)))),
            child: const Text('Export', textAlign: TextAlign.center, style: TextStyle(color: cTitle, fontSize: 17, fontWeight: FontWeight.w700)),
          ),
          Flexible(
            child: RawScrollbar(
              controller: _scroll,
              thumbVisibility: true,
              thickness: 3.5,
              radius: const Radius.circular(2),
              thumbColor: const Color(0x556B7078),
              padding: const EdgeInsets.only(top: 4, bottom: 4, right: 2),
              child: SingleChildScrollView(
                controller: _scroll,
                padding: const EdgeInsets.fromLTRB(20, 10, 20, 12),
                child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                  const kit.PropRow('File name', height: 24),
                  TextField(
                    controller: _name,
                    style: const TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w600),
                    decoration: InputDecoration(
                      isDense: true,
                      filled: true,
                      fillColor: cCard,
                      hintText: 'File name',
                      suffixIcon: const Icon(Icons.edit_outlined, size: 16, color: cSecondary),
                      suffixIconConstraints: const BoxConstraints(minWidth: 32, minHeight: 32),
                      contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
                      enabledBorder: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: const BorderSide(color: cLine)),
                      focusedBorder: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: const BorderSide(color: cAccent, width: 1.5)),
                    ),
                  ),
                  const SizedBox(height: 8),
                  const kit.PropRow('Format', height: 26),
                  kit.Segmented(const ['Video', 'GIF', 'PNG frames'], _order.indexOf(_format), onSelect: (i) => setState(() => _format = _order[i])),
                  const kit.PropRow('Size', height: 26),
                  kit.Segmented(const ['320', '640', '1280', '1920'], _size, onSelect: (i) => setState(() => _size = i)),
                  kit.PropRow('Picture size', trailing: Text('$w × $h', style: const TextStyle(color: cSecondary, fontSize: 13))),
                  kit.StepRow('From frame', '$_start', onMinus: _start > 1 ? () => setState(() => _start--) : null, onPlus: _start < last ? () => setState(() => _start++) : null),
                  kit.StepRow('To frame', '$last', onMinus: last > _start ? () => setState(() => _end = last - 1) : null, onPlus: last < widget.frames ? () => setState(() => _end = last + 1 >= widget.frames ? 0 : last + 1) : null),
                  if (_format == 2)
                    kit.TickRow('Include sound', _sound, onChanged: (v) => setState(() => _sound = v)),
                  if (canTransparent)
                    kit.TickRow('Transparent background', _transparent, onChanged: (v) => setState(() => _transparent = v)),
                  kit.TickRow('Show watermark', _wmOn, onChanged: (v) {
                    widget.cmd({'op': 'watermark', 'on': v});
                    setState(() => _wmOn = v);
                  }),
                  if (_wmOn) ...[
                    const kit.Hint('Drawn small at the top right of the exported pictures. Saved with the animation.'),
                    _TextFieldBox(oneLine: true, text: _wm['text'] as String? ?? ':)', onChanged: (v) {
                      widget.cmd({'op': 'watermark', 'text': v});
                      _wm = {..._wm, 'text': v};
                    }),
                    const SizedBox(height: 6),
                    widget.swatches((_wm['color'] as num?)?.toInt() ?? 0xff3b30ff, (c) {
                      widget.cmd({'op': 'watermark', 'color': c});
                      setState(() => _wm = {..._wm, 'color': c});
                    }),
                    const SizedBox(height: 4),
                  ],
                  kit.TickRow('Make vertical (phone video)', _vertical, onChanged: (v) {
                    widget.onVertical(v);
                    setState(() => _vertical = v);
                  }),
                  if (_vertical) ...[
                    const kit.Hint('Turns the camera of every frame a quarter turn, so the right edge becomes the top of the video. It changes the animation; Undo turns them back.'),
                    kit.WordButton('Turn the cameras 90°', filled: true, expand: true, height: 36, onTap: () => widget.cmd({'op': 'rotate_cameras'})),
                    const SizedBox(height: 4),
                  ],
                  kit.Hint('${_seconds(count)} at ${widget.fps == widget.fps.roundToDouble() ? widget.fps.round() : widget.fps} fps. ${_format == 2 ? 'Videos have a solid background; switch to GIF or PNG frames for transparency.' : _format == 1 ? '$count PNG pictures${_transparent ? ' (transparent)' : ''}, with the soundtrack as a WAV when there is sound.' : 'An animated GIF${_transparent ? ' (transparent)' : ''} in Barkfart Exports, with the soundtrack as a WAV when there is sound.'}'),
                ]),
              ),
            ),
          ),
          Container(
            padding: const EdgeInsets.fromLTRB(20, 10, 20, 12),
            decoration: const BoxDecoration(
              color: cCard,
              border: Border(top: BorderSide(color: Color(0xFFE0E3E8))),
              boxShadow: [BoxShadow(color: Color(0x0A000000), blurRadius: 6, offset: Offset(0, -2))],
            ),
            child: Row(children: [
              Expanded(child: kit.WordButton('Cancel', onTap: () => Navigator.pop(context), filled: true, expand: true, height: 40)),
              const SizedBox(width: 8),
              Expanded(
                child: kit.WordButton(
                  _format == 0 ? 'Export GIF' : _format == 2 ? 'Export video' : 'Export $count PNGs',
                  primary: true,
                  expand: true,
                  height: 40,
                  onTap: () => Navigator.pop(context, <String, Object>{
                    'format': _format,
                    'w': w,
                    'h': h,
                    'start': _start - 1,
                    'count': count,
                    'transparent': canTransparent && _transparent,
                    // video: the engine's flag argument means "without sound"
                    'silent': _format == 2 && !_sound,
                    'name': _name.text,
                  }),
                ),
              ),
            ]),
          ),
        ]),
      ),
    );
  }
}


/// D34: the graph's mapping from frames and values to the picture, shared by
/// the painter and the drags. The selected value's unit sets the scale; a
/// value in another unit is scaled to fit (drawn dashed).
class _CurveGeom {
  _CurveGeom({required this.size, required this.frames, required this.cell, required this.scroll, required this.channels, required this.selected, required this.frozen}) {
    final sel = channels.where((c) => c['id'] == selected).firstOrNull ?? channels.where((c) => c['unit'] != 'colour').firstOrNull;
    unit = '${sel?['unit'] ?? 'deg'}';
    final own = <String, (double, double)>{};
    for (final c in channels) {
      if (c['unit'] == 'colour') continue;
      final vs = <double>[];
      for (final k in (c['keys'] as List).cast<List>()) {
        vs.add((k[1] as num).toDouble());
      }
      for (final q in ((c['result'] as List?) ?? const []).cast<List>()) {
        vs.add((q[1] as num).toDouble());
      }
      for (final sg in (c['segs'] as List).cast<List>()) {
        if (sg[0] == 'c') vs.addAll([(sg[2] as num).toDouble(), (sg[4] as num).toDouble()]);
        if (sg[0] == 's') vs.addAll([for (final q in (sg[1] as List).cast<List>()) (q[1] as num).toDouble()]);
      }
      if (vs.isEmpty) continue;
      own['${c['id']}'] = (vs.reduce(math.min), vs.reduce(math.max));
    }
    // the shared scale: every value in the selected one's unit
    var lo = double.infinity, hi = -double.infinity;
    for (final c in channels) {
      final r = own['${c['id']}'];
      if (r == null || c['unit'] != unit) continue;
      lo = math.min(lo, r.$1);
      hi = math.max(hi, r.$2);
    }
    for (final c in channels) {
      final id = '${c['id']}';
      final r = frozen?[id] ?? (c['unit'] == unit && lo.isFinite ? (lo, hi) : own[id]);
      if (r == null) continue;
      var (a, b) = r;
      if (frozen?[id] == null) {
        final pad = math.max((b - a) * 0.12, unit == 'deg' ? 5.0 : 0.05);
        a -= pad;
        b += pad;
      }
      ranges_[id] = (a, b);
    }
  }

  static const rulerH = 22.0, left = 8.0, padY = 14.0, bandH = 14.0;
  final Size size;
  final int frames;
  final double cell, scroll;
  final List<Map> channels;
  final String? selected;
  final Map<String, (double, double)>? frozen;
  late final String unit;
  final Map<String, (double, double)> ranges_ = {};

  bool get hasBand => channels.any((c) => c['unit'] == 'colour');
  double get top => rulerH + padY;
  double get bottom => size.height - padY - (hasBand ? bandH + 10 : 0);
  double xOf(double frame) => left + frame * cell + cell / 2 - scroll;
  int frameAt(double x) => ((x + scroll - left) / cell).floor().clamp(0, frames - 1);
  Map<String, (double, double)> ranges() => Map.of(ranges_);
  (double, double) range(Map c) => ranges_['${c['id']}'] ?? (0, 1);
  double yOf(Map c, double v) {
    final (a, b) = range(c);
    final span = (b - a).abs() < 1e-9 ? 1.0 : b - a;
    return top + (b - v) / span * (bottom - top);
  }

  double valuePerPx(Map c) {
    final (a, b) = range(c);
    return ((b - a).abs() < 1e-9 ? 1.0 : b - a) / math.max(1.0, bottom - top);
  }

  bool fitted(Map c) => c['unit'] != unit;

  /// The selected key's handle ends (left, right) on the picture.
  (Offset, Offset)? handles(String? id, double? frame) {
    final c = channels.where((x) => x['id'] == id).firstOrNull;
    if (c == null || frame == null || c['unit'] == 'colour') return null;
    final k = (c['keys'] as List).cast<List>().where((k) => (k[0] as num).toDouble() == frame).firstOrNull;
    if (k == null || k.length < 8) return null;
    final v = (k[1] as num).toDouble();
    Offset at(num df, num dv) => Offset(xOf(frame + df.toDouble()), yOf(c, v + dv.toDouble()));
    return (at(k[4] as num, k[5] as num), at(k[6] as num, k[7] as num));
  }
}

class _CurvesPainter extends CustomPainter {
  _CurvesPainter(this.g, this.colours, this.cur, this.keyFrame, {required this.fps});
  final _CurveGeom g;
  final Map<String, Color> colours;
  final int cur, fps;
  final double? keyFrame;

  @override
  void paint(Canvas canvas, Size size) {
    final w = size.width;
    // ruler: ticks every frame, numbers like the timeline's, the playhead pill
    canvas.drawRect(Rect.fromLTWH(0, _CurveGeom.rulerH - 1, w, 1), Paint()..color = const Color(0xFFE6E8EB));
    final step = _TimelinePainter.rulerStep(fps, g.cell);
    for (var f = 0; f < g.frames; f++) {
      final x = g.xOf(f.toDouble());
      if (x < -20 || x > w + 20) continue;
      final fn = f + 1;
      final sec = fps > 0 && fn % fps == 0;
      final big = _TimelinePainter.showRulerNumber(fn, fps, step, g.cell);
      canvas.drawRect(
        Rect.fromLTWH(x, _CurveGeom.rulerH - (sec ? 8 : big ? 6 : 4), 1, sec ? 8 : big ? 6 : 4),
        Paint()..color = sec ? cInk2b : (big ? const Color(0xFF9AA0A8) : const Color(0xFFC4C8CE)),
      );
      if (big && f != cur) _text(canvas, '$fn', Offset(x, 3), sec ? cText : const Color(0xFF6B7078), 11, centre: true, bold: sec);
    }
    // the selected value's held stretches, shaded
    final sel = g.channels.where((c) => c['id'] == g.selected).firstOrNull;
    if (sel != null && sel['unit'] != 'colour') {
      final ks = (sel['keys'] as List).cast<List>();
      final segs = (sel['segs'] as List).cast<List>();
      for (var i = 0; i < segs.length && i + 1 < ks.length; i++) {
        if (segs[i][0] != 'h') continue;
        final a = g.xOf((ks[i][0] as num).toDouble()), b = g.xOf((ks[i + 1][0] as num).toDouble());
        canvas.drawRect(Rect.fromLTRB(a, _CurveGeom.rulerH, b, size.height), Paint()..color = const Color(0xFFF6F7F9));
      }
    }
    // value lines in the selected one's unit, labelled at the left
    if (sel != null && sel['unit'] != 'colour') {
      final (a, b) = g.range(sel);
      final nice = _niceStep((b - a) / 4);
      if (nice > 0) {
        for (var v = (a / nice).ceil() * nice; v <= b; v += nice) {
          final y = g.yOf(sel, v);
          canvas.drawRect(Rect.fromLTWH(0, y, w, 1), Paint()..color = const Color(0xFFEEF0F2));
          final label = g.unit == 'deg' ? '${v.round()}°' : (nice >= 1 ? '${v.round()}' : v.toStringAsFixed(2));
          _text(canvas, label, Offset(4, y - 13), const Color(0xFF9AA0A8), 10.5);
        }
      }
      if (g.channels.any((c) => c['unit'] != 'colour' && g.fitted(c))) _text(canvas, 'dashed: scaled to fit', Offset(w - 6, _CurveGeom.rulerH + 3), const Color(0xFF9AA0A8), 11, end: true);
    }
    // the playhead
    final px = g.xOf(cur.toDouble());
    canvas.drawRect(Rect.fromLTWH(px - 0.75, _CurveGeom.rulerH - 2, 1.5, size.height), Paint()..color = const Color(0xFF8DB8E8));
    final pill = RRect.fromRectAndRadius(Rect.fromCenter(center: Offset(px, 11), width: math.max(30, 12 + 7.0 * '${cur + 1}'.length), height: 18), const Radius.circular(9));
    canvas.drawRRect(pill, Paint()..color = const Color(0xFFDCEAF9));
    _text(canvas, '${cur + 1}', Offset(px, 3), cAccent, 11, centre: true, bold: true);
    // curves: the selected one on top and stronger
    final order = [...g.channels.where((c) => c['unit'] != 'colour')]..sort((a, b) => (a['id'] == g.selected ? 1 : 0) - (b['id'] == g.selected ? 1 : 0));
    for (final c in order) {
      final colour = colours['${c['id']}'] ?? cInk2b;
      final picked = c['id'] == g.selected;
      final path = _path(c, w);
      final pen = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = picked ? 2.6 : 1.8
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round
        ..color = picked ? colour : colour.withValues(alpha: 0.55);
      final result = ((c['result'] as List?) ?? const []).cast<List>();
      if (result.isNotEmpty) {
        // D36: the keys' own curve faint and dashed (the keys stay on it),
        // what the modifiers make of it solid
        canvas.drawPath(_dashed(path), Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.6
          ..color = colour.withValues(alpha: 0.38));
        final r = Path();
        var lastY = 0.0;
        for (final (j, q) in result.indexed) {
          final o = Offset(g.xOf((q[0] as num).toDouble()), g.yOf(c, (q[1] as num).toDouble()));
          j == 0 ? r.moveTo(math.min(0, o.dx), o.dy) : r.lineTo(o.dx, o.dy);
          if (j == 0) r.lineTo(o.dx, o.dy);
          lastY = o.dy;
        }
        // after the last frame it holds, as the keys do
        r.lineTo(w, lastY);
        canvas.drawPath(r, pen);
      } else {
        canvas.drawPath(g.fitted(c) && !picked ? _dashed(path) : path, pen);
      }
      for (final k in (c['keys'] as List).cast<List>()) {
        final f = (k[0] as num).toDouble();
        if (picked && f == keyFrame) continue;
        final o = Offset(g.xOf(f), g.yOf(c, (k[1] as num).toDouble()));
        canvas.drawCircle(o, picked ? 5 : 4, Paint()..color = cCard);
        canvas.drawCircle(o, picked ? 5 : 4, Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2
          ..color = picked ? colour : colour.withValues(alpha: 0.6));
      }
    }
    // the colour values: a band along the foot, its keys as dots
    final bands = g.channels.where((c) => c['unit'] == 'colour').toList();
    for (final (i, c) in bands.indexed) {
      final ks = (c['keys'] as List).cast<List>();
      if (ks.isEmpty) continue;
      final y = size.height - 10 - _CurveGeom.bandH - i * (_CurveGeom.bandH + 4);
      final x0 = g.xOf(0), x1 = g.xOf((g.frames - 1).toDouble());
      final r = RRect.fromRectAndRadius(Rect.fromLTRB(x0, y, x1, y + 12), const Radius.circular(6));
      final stops = <double>[], cols = <Color>[];
      for (final k in ks) {
        stops.add(((g.xOf((k[0] as num).toDouble()) - x0) / math.max(1, x1 - x0)).clamp(0.0, 1.0));
        cols.add(_EditorPageState._rgba((k[1] as num).toInt()));
      }
      if (cols.length == 1) {
        cols.add(cols.first);
        stops.add(1);
      }
      canvas.drawRRect(r, Paint()..shader = ui.Gradient.linear(Offset(x0, 0), Offset(x1, 0), cols, stops));
      for (final k in ks) {
        final o = Offset(g.xOf((k[0] as num).toDouble()), y + 6);
        canvas.drawCircle(o, 4.5, Paint()..color = cCard);
        canvas.drawCircle(o, 4.5, Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.6
          ..color = cInk2b);
      }
    }
    // the selected key: filled, its handles where they shape a curve, the readout
    if (sel != null && keyFrame != null && sel['unit'] != 'colour') {
      final k = (sel['keys'] as List).cast<List>().where((k) => (k[0] as num).toDouble() == keyFrame).firstOrNull;
      if (k != null) {
        final o = Offset(g.xOf(keyFrame!), g.yOf(sel, (k[1] as num).toDouble()));
        final hs = g.handles('${sel['id']}', keyFrame);
        if (hs != null) {
          final line = Paint()
            ..color = cText
            ..strokeWidth = 1.4;
          for (final h in [hs.$1, hs.$2]) {
            if ((h - o).distance < 1) continue;
            canvas.drawLine(o, h, line);
            canvas.drawCircle(h, 5, Paint()..color = cCard);
            canvas.drawCircle(h, 5, Paint()
              ..style = PaintingStyle.stroke
              ..strokeWidth = 1.6
              ..color = cText);
          }
        }
        canvas.drawCircle(o, 6.5, Paint()..color = cAccent);
        canvas.drawCircle(o, 6.5, Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2
          ..color = Colors.white);
        final v = (k[1] as num).toDouble();
        final words = 'Frame ${keyFrame!.round() + 1}, ${sel['unit'] == 'deg' ? '${v.round()}°' : v.abs() >= 10 ? v.round() : v.toStringAsFixed(2)}';
        final tp = TextPainter(text: TextSpan(text: words, style: const TextStyle(color: Colors.white, fontSize: 11.5, fontWeight: FontWeight.w600, fontFamily: kit.kFont)), textDirection: TextDirection.ltr)..layout();
        final bw = tp.width + 22;
        final at = Offset(o.dx + 14 + bw > size.width ? o.dx - 14 - bw : o.dx + 14, math.min(o.dy + 14, size.height - 26));
        canvas.drawRRect(RRect.fromRectAndRadius(Rect.fromLTWH(at.dx, at.dy, bw, 22), const Radius.circular(11)), Paint()..color = const Color(0xDB222428));
        tp.paint(canvas, Offset(at.dx + 11, at.dy + (22 - tp.height) / 2));
      }
    }
  }

  /// One value's curve: flat before its first key and after its last, and
  /// each stretch as the engine shapes it.
  Path _path(Map c, double w) {
    final ks = (c['keys'] as List).cast<List>();
    final segs = (c['segs'] as List).cast<List>();
    final p = Path();
    if (ks.isEmpty) return p;
    Offset at(num f, num v) => Offset(g.xOf(f.toDouble()), g.yOf(c, v.toDouble()));
    final first = at(ks[0][0] as num, ks[0][1] as num);
    p.moveTo(math.min(0, first.dx), first.dy);
    p.lineTo(first.dx, first.dy);
    for (var i = 0; i + 1 < ks.length; i++) {
      final b = at(ks[i + 1][0] as num, ks[i + 1][1] as num);
      final sg = i < segs.length ? segs[i] : const ['l'];
      switch (sg[0]) {
        case 'h':
          final a = at(ks[i][0] as num, ks[i][1] as num);
          p.lineTo(b.dx, a.dy);
          p.lineTo(b.dx, b.dy);
        case 'c':
          final c1 = at(sg[1] as num, sg[2] as num), c2 = at(sg[3] as num, sg[4] as num);
          p.cubicTo(c1.dx, c1.dy, c2.dx, c2.dy, b.dx, b.dy);
        case 's':
          for (final q in (sg[1] as List).cast<List>()) {
            final o = at(q[0] as num, q[1] as num);
            p.lineTo(o.dx, o.dy);
          }
          p.lineTo(b.dx, b.dy);
        default:
          p.lineTo(b.dx, b.dy);
      }
    }
    final last = at(ks.last[0] as num, ks.last[1] as num);
    p.lineTo(math.max(w, last.dx), last.dy);
    return p;
  }

  static Path _dashed(Path src) {
    final out = Path();
    for (final m in src.computeMetrics()) {
      for (var d = 0.0; d < m.length; d += 12) {
        out.addPath(m.extractPath(d, math.min(m.length, d + 7)), Offset.zero);
      }
    }
    return out;
  }

  static double _niceStep(double raw) {
    if (!(raw > 0)) return 0;
    final p = math.pow(10, (math.log(raw) / math.ln10).floor()).toDouble();
    for (final m in const [1.0, 2.0, 5.0, 10.0]) {
      if (m * p >= raw) return m * p;
    }
    return 10 * p;
  }

  static void _text(Canvas canvas, String s, Offset at, Color colour, double size, {bool centre = false, bool end = false, bool bold = false}) {
    final tp = TextPainter(text: TextSpan(text: s, style: TextStyle(color: colour, fontSize: size, fontWeight: bold ? FontWeight.w700 : FontWeight.w400, fontFamily: kit.kFont)), textDirection: TextDirection.ltr)..layout();
    tp.paint(canvas, Offset(centre ? at.dx - tp.width / 2 : end ? at.dx - tp.width : at.dx, at.dy));
  }

  @override
  bool shouldRepaint(_CurvesPainter o) => true;
}
