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
import 'ui/kit.dart' as kit;
import 'ui/pictograms.dart';
import 'ui/rows.dart' as rw;
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

  ui.Image? _image;
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
    await AppSettings.load();
    e.command({'op': 'set', 'k': 'axis', 'v': AppSettings.flag('axisLock')});
    e.command({'op': 'set', 'k': 'path', 'v': AppSettings.values['motionPath'] != false});
    _refresh(e.command({'op': 'state'}));
    // draw every frame picture for the project just opened
    _thumbsChanged('open');
    _reminder = Timer.periodic(const Duration(seconds: 30), (_) => _remindToSave());
    final script = Platform.environment['BARKFART_SCRIPT'];
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
    _clearCache();
    if (_imageOwned) _image?.dispose();
    _engine?.dispose();
    super.dispose();
  }

  // ---------------------------------------------------------------- state

  void _refresh(Map<String, dynamic>? st) {
    if (st == null) return;
    if (_soundPage && (st['selected']?['layer'] != _st['selected']?['layer'] || st['frame'] != _st['frame'] || st['frameSound'] == null)) _soundPage = false;
    setState(() => _st = st);
    final m = st['message'] as String? ?? '';
    if (m.isNotEmpty) _say(m);
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
  }

  void _tick() {
    final e = _engine;
    if (e == null || _busy || _view.size.isEmpty || _st.isEmpty) return;
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
      if (t != _playT) setState(() => _playT = t);
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
        if (!identical(cached, _image)) setState(() => _setImage(cached, owned: false));
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
      if (!mounted) return;
      _pictureSt = drawnFrom;
      final keep = camera && _cacheBytes + w * h * 4 <= _cacheBudget && '${_st['rev']} $w $h' == _cacheKey;
      if (keep) {
        _frameCache[fi] = img;
        _cacheBytes += w * h * 4;
      }
      setState(() {
        _setImage(img, owned: !keep);
        _engineMs = ms;
      });
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
    if (_frameSelected) setState(() => _frameSelected = false);
    if (_pointers.length == 1) {
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
      // a second finger: whatever the first was doing becomes a pinch
      if (_oneFinger == 1) _cmd({'op': 'cancel'});
      _oneFinger = 0;
      final ps = _pointers.values.toList();
      _pinchDist = (ps[0] - ps[1]).distance;
      _pinchMid = (ps[0] + ps[1]) / 2;
    }
  }

  void _move(PointerMoveEvent ev) {
    if (!_pointers.containsKey(ev.pointer)) return;
    _pointers[ev.pointer] = ev.localPosition;
    if ((ev.localPosition - _downAt).distance > 6) _moved = true;
    if (_pointers.length == 1) {
      if (_oneFinger == 1) {
        final p = _view.toStage(ev.localPosition);
        _cmd({'op': 'move', 'x': p.dx, 'y': p.dy});
      } else if (_oneFinger == 2) {
        final d = ev.localPosition - _panFrom;
        _panFrom = ev.localPosition;
        setState(() {
          _view.cx -= d.dx / _view.s;
          _view.cy += d.dy / _view.s;
        });
      }
    } else if (_pointers.length == 2) {
      final ps = _pointers.values.toList();
      final dist = (ps[0] - ps[1]).distance;
      final mid = (ps[0] + ps[1]) / 2;
      setState(() {
        // keep the stage point under the old midpoint under the new one
        final anchor = _view.toStage(_pinchMid);
        if (_pinchDist > 1 && dist > 1) _view.s = (_view.s * dist / _pinchDist).clamp(0.02, 40.0);
        final now = _view.toStage(mid);
        _view.cx += anchor.dx - now.dx;
        _view.cy += anchor.dy - now.dy;
      });
      _pinchDist = dist;
      _pinchMid = mid;
    }
  }

  Object? _lastFigTap;
  DateTime _lastFigTapAt = DateTime(0);

  void _up(PointerEvent ev) {
    if (!_pointers.containsKey(ev.pointer)) return;
    _pointers.remove(ev.pointer);
    if (_pointers.isEmpty) {
      if (_oneFinger == 1) {
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
      if (_oneFinger == 2 && !_moved) {
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
    } else if (_pointers.length == 1) {
      // back to one finger after a pinch: it pans
      _oneFinger = 2;
      _panFrom = _pointers.values.first;
    }
  }

  void _scroll(PointerSignalEvent ev) {
    if (ev is PointerScrollEvent) {
      setState(() {
        final anchor = _view.toStage(ev.localPosition);
        _view.s = (_view.s * math.pow(0.998, ev.scrollDelta.dy)).clamp(0.02, 40.0);
        final now = _view.toStage(ev.localPosition);
        _view.cx += anchor.dx - now.dx;
        _view.cy += anchor.dy - now.dy;
      });
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
    final play = await showDialog<bool>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) => AlertDialog(
          backgroundColor: cPanel,
          title: const Text('Loop to', style: TextStyle(color: cText, fontSize: 16)),
          content: SizedBox(
            width: 300,
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              _hint('After the end frame, playback goes back to:'),
              const SizedBox(height: 6),
              _Segmented(const ['Frame 1', 'Start frame'], _loopToStart ? 1 : 0, (i) => set(() => _loopToStart = i == 1)),
              const SizedBox(height: 10),
              _stepper('End frame', _loopEnd == 0 ? 'last' : '$_loopEnd', () => set(() => _loopEnd = math.max(0, _loopEnd - 1)), () => set(() => _loopEnd = math.min(_frames, _loopEnd + 1))),
              const SizedBox(height: 10),
              _Btn('Reset all', onTap: () => set(() {
                    _loopToStart = false;
                    _loopEnd = 0;
                  })),
            ]),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(c, false), child: const Text('Close')),
            TextButton(onPressed: () => Navigator.pop(c, true), child: const Text('Play')),
          ],
        ),
      ),
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

  Future<String?> _ask(String title, String body, List<String> buttons) {
    return showDialog<String>(
      context: context,
      builder: (c) => AlertDialog(
        backgroundColor: cPanel,
        title: Text(title, style: const TextStyle(color: cText, fontSize: 17)),
        content: Text(body, style: const TextStyle(color: cSecondary)),
        actions: [for (final b in buttons) TextButton(onPressed: () => Navigator.pop(c, b), child: Text(b))],
      ),
    );
  }

  Future<String?> _askName(String title, String initial) {
    final ctl = TextEditingController(text: initial)..selection = TextSelection(baseOffset: 0, extentOffset: initial.length);
    return showDialog<String>(
      context: context,
      builder: (c) => AlertDialog(
        backgroundColor: cPanel,
        title: Text(title, style: const TextStyle(color: cText, fontSize: 17)),
        content: TextField(
          controller: ctl,
          autofocus: true,
          style: const TextStyle(color: cText),
          decoration: const InputDecoration(labelText: 'Name'),
          onSubmitted: (v) => Navigator.pop(c, v.trim().isEmpty ? null : v.trim()),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.pop(c), child: const Text('Cancel')),
          TextButton(onPressed: () => Navigator.pop(c, ctl.text.trim().isEmpty ? null : ctl.text.trim()), child: const Text('Save')),
        ],
      ),
    );
  }

  Future<void> _newProject() async {
    if (!await _leaveUnsaved()) return;
    await _freshProject();
    _file = null;
    _fitted = false;
    _frameSelected = false;
    _refresh(_engine?.command({'op': 'state'}));
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
          _cmd({'op': 'select', 'layer': r['id'], if (r['kind'] == 'picture' || r['kind'] == 'movieclip') 'node': ''});
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
            setState(() => _blockDrag = _BlockDrag('${_sel!['layer']}', (pose['start'] as num).toInt(), (pose['length'] as num).toInt(), _cellW, edge: w.length > 2 ? (w[2] == 'move' ? 1 : w[2] == 'start' ? 2 : 0) : 0)..dx = double.parse(w[1]));
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
        case 'quit':
          exit(0);
      }
      await Future<void>.delayed(const Duration(milliseconds: 120));
    }
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
    if (_frameSelected) return const ['Frame Tools'];
    if (_st['textfield'] != null) return const ['Textfield Tools'];
    if (_st['movieclip'] != null) return const ['Movieclip Tools'];
    if (_st['picture'] != null) return const ['Picture Tools'];
    final sel = _sel;
    if (_st['multi'] != null) return const ['Selection'];
    if (sel != null) return const ['Stickfigure Tools', 'Segment Tools', 'Pose'];
    if (_st['editingClip'] != null) return const ['Movieclip editor', 'Textfield Tools'];
    // nothing selected: Save, Export and the add menu live in the top bar and
    // the timeline (§9); the card says how to fill it
    return const ['Nothing selected'];
  }

  List<Widget> _sectionBody(String name) {
    switch (name) {
      case 'Sound Tools':
        return _soundsBlock();
      case 'Picture Tools':
        return _pictureTools(_st['picture'] as Map);
      case 'Nothing selected':
        return [
          for (final d in <rw.Desc>[
            const rw.DHint('Tap a figure for its properties, a block for its pose, or the ruler for the whole frame.'),
            if (_st['figureCopied'] == true || _st['textfieldCopied'] == true)
              rw.DButtons(_st['figureCopied'] == true ? 'Paste figure' : 'Paste text', _st['figureCopied'] == true ? _pasteFigure : () => _cmd({'op': 'paste_textfield'}),
                  b: _st['figureCopied'] == true && _st['textfieldCopied'] == true ? 'Paste text' : null, onB: () => _cmd({'op': 'paste_textfield'})),
          ])
            rw.renderDesc(d),
        ];
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

  @override
  Widget build(BuildContext context) {
    _dpr = MediaQuery.devicePixelRatioOf(context);
    if (_fatal.isNotEmpty) {
      return Scaffold(backgroundColor: cSurround, body: Center(child: Text(_fatal, style: const TextStyle(color: cText))));
    }
    final big = MediaQuery.sizeOf(context).shortestSide > 600;
    final screen = MediaQuery.sizeOf(context);
    // BARKFART_DOCK=1 forces it, for checks on small screens
    final dock = (screen.width >= 1200 && screen.height >= 900) || Platform.environment['BARKFART_DOCK'] == '1';
    // screen padding from the settings (notches, rounded corners)
    final pad = MediaQuery.paddingOf(context) + EdgeInsets.only(left: AppSettings.number('padLeft').toDouble(), right: AppSettings.number('padRight').toDouble());
    final leftHand = AppSettings.flag('leftHand');
    return Scaffold(
      backgroundColor: cSurround,
      body: RepaintBoundary(
        key: _shot,
        child: Column(children: [
        if (!_fsNow) (_st['copyEdit'] is Map ? _copyHeader(pad, _st['copyEdit'] as Map) : _appBar(pad)),
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
                    // floating over the stage; docked, it is a column beside it (below)
                    if (!_fsNow && _panelOpen && !_hidePanels && !dock) Positioned(top: 8, right: 8, bottom: 8, width: big ? 360 : 320, child: _propertiesCard()),
                    if (_st['building'] != null && _partsOpen && !_viewOptions && !_hidePanels) Positioned(top: 8, left: 8, bottom: 8, width: 220, child: _partsPanel()),
                    if (_viewOptions) Positioned(top: 8, left: 8, bottom: 8, width: 330, child: _viewOptionsPanel()),
                  ]),
                ),
              ),
            ]),
          ),
          // tablets and big windows dock Properties as a full-height column
          // beside the stage, which centres in what is left (D20, Tablet board)
          if (!_fsNow && _panelOpen && !_hidePanels && dock) SizedBox(width: 360, child: _propertiesCard(docked: true)),
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
                : Column(children: [
                    _controlBar(),
                    _previewRow(),
                    if (_layersOpen) _framesBar(),
                  ]),
          ),
        ]),
      ),
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
        ]),
        IgnorePointer(
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 320),
              child: Text(_file?.name ?? 'Untitled', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w700)),
            ),
            if (dirty) Container(width: 7, height: 7, margin: const EdgeInsets.only(left: 7), decoration: const BoxDecoration(color: Color(0xFFF2A93B), shape: BoxShape.circle)),
          ]),
        ),
      ]),
    );
  }

  final GlobalKey _projectsKey = GlobalKey();

  /// "Projects": the animation's file actions, until the Projects screen (E2).
  Future<void> _projectsMenu() async {
    final box = _projectsKey.currentContext?.findRenderObject() as RenderBox?;
    final at = box == null ? const Offset(8, 44) : box.localToGlobal(Offset(0, box.size.height + 4));
    final pick = await showGeneralDialog<int>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Projects',
      barrierColor: const Color(0x00000000),
      transitionDuration: Duration.zero,
      pageBuilder: (c, a1, a2) => Stack(children: [
        Positioned(
          left: at.dx,
          top: at.dy,
          child: kit.PanelCard(
            width: 220,
            padding: const EdgeInsets.all(6),
            child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, mainAxisSize: MainAxisSize.min, children: [
              for (final (i, label) in const [(0, 'New animation'), (1, 'Your animations…'), (2, 'Save'), (3, 'Save as…'), (4, 'Settings')])
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
    }
  }

  Widget _stage() {
    return LayoutBuilder(builder: (context, c) {
      if (_view.size != c.biggest) {
        _view.size = c.biggest;
        if (!_fitted) _fit();
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
            if (_image != null) RawImage(image: _image, fit: BoxFit.fill, filterQuality: FilterQuality.low),
            CustomPaint(painter: _Overlay(_pictureSt ?? _st, _view, _playing)),
            if (AppSettings.flag('zoomButtons') && !_playing)
              Positioned(
                right: 10,
                bottom: 10,
                child: Column(children: [
                  for (final (icon, k) in [(Icons.zoom_in, 1.2), (Icons.zoom_out, 1 / 1.2)])
                    Padding(
                      padding: const EdgeInsets.only(top: 6),
                      child: Material(
                        color: cControl,
                        shape: const CircleBorder(),
                        child: InkWell(
                          customBorder: const CircleBorder(),
                          onTap: () => setState(() {
                            // zoom about the middle of the view
                            final c = _view.toStage(_view.size.center(Offset.zero));
                            _view.s = (_view.s * k).clamp(0.02, 40.0);
                            final c2 = _view.toStage(_view.size.center(Offset.zero));
                            _view.cx += c.dx - c2.dx;
                            _view.cy += c.dy - c2.dy;
                          }),
                          child: Padding(padding: const EdgeInsets.all(8), child: Icon(icon, color: cText)),
                        ),
                      ),
                    ),
                ]),
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
      final label = kind == 'connector' ? 'Connector ${p['n']}' : kind == 'polyfill' ? 'Polyfill ${p['n']}' : '${words[p['shape']] ?? 'Segment'} ${p['n']}';
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
        // polyfills follow their joint; segments drag to a new place
        child: kind == 'polyfill' ? item : ReorderableDelayedDragStartListener(index: i, child: item),
      );
    }

    return kit.PanelCard(
      padding: EdgeInsets.zero,
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const Padding(padding: EdgeInsets.only(top: 12), child: Text('Parts', textAlign: TextAlign.center, style: TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w600))),
        const Text('Front at the top', textAlign: TextAlign.center, style: TextStyle(color: cSecondary, fontSize: 12)),
        const SizedBox(height: 6),
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

  /// The figure editor's shape shelf (J2, FigureEditor board): what a drag
  /// from a joint draws, one picked at a time; Select draws nothing.
  Widget _shapeShelf() {
    final tool = _st['drawTool'] as String? ?? 'none';
    const tools = [
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
    Widget tile(String id, String label, String glyph) {
      final on = id == 'polyfill' ? _st['polyfill'] != null : tool == id && _st['polyfill'] == null;
      return kit.Tap(
        label: label,
        minSize: 0,
        onTap: () => id == 'polyfill'
            ? (_sel == null ? _say('Tap a joint first: the polyfill is drawn right before it.') : _cmd({'op': 'polyfill_new'}))
            : _cmd({'op': 'draw_tool', 'shape': id}),
        child: Container(
          width: 74,
          height: 64,
          decoration: BoxDecoration(color: on ? const Color(0xFFE8F0FA) : null, borderRadius: BorderRadius.circular(12)),
          child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
            CustomPaint(size: const Size(40, 26), painter: ShapeGlyph(glyph, on ? cAccent : cInk2b)),
            const SizedBox(height: 4),
            Text(label, style: TextStyle(color: on ? cAccent : cSecondary, fontSize: 12, fontWeight: on ? FontWeight.w600 : FontWeight.w400)),
          ]),
        ),
      );
    }

    return Container(
      height: 76,
      decoration: const BoxDecoration(color: cCard, border: Border(top: BorderSide(color: cLine))),
      child: Row(children: [
        Expanded(
          child: Center(
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(mainAxisSize: MainAxisSize.min, children: [for (final (id, label, glyph) in tools) Padding(padding: const EdgeInsets.symmetric(horizontal: 2), child: tile(id, label, glyph))]),
            ),
          ),
        ),
        // editing in place (scripts and tests) ends with Done; a copy ends in the header
        if (_st['copyEdit'] == null) Padding(padding: const EdgeInsets.only(right: 10), child: kit.WordButton('Done', strong: true, onTap: () => _cmd({'op': 'build_done'}))),
      ]),
    );
  }

  // ---------------------------------------------------------------- filmstrip and grid

  final Map<int, ui.Image> _thumbs = {};
  final Set<int> _thumbDirty = {};
  int _thumbRev = -1;

  /// After a change: during a drag only the current frame's picture is stale.
  void _thumbsChanged(String op) {
    final rev = (_st['rev'] as num?)?.toInt() ?? 0;
    // a project just opened, made or imported into always redraws everything
    final whole = op == 'open' || op == 'new' || op == 'import';
    if (rev == _thumbRev && !whole) return;
    _thumbRev = rev;
    if (op == 'move' || op == 'down') return;
    if (op == 'up') {
      _thumbDirty.add(_frame);
      return;
    }
    for (var i = 0; i < _frames; i++) {
      _thumbDirty.add(i);
    }
    _thumbs.keys.where((k) => k >= _frames).toList().forEach((k) => _thumbs.remove(k)?.dispose());
  }

  // M4: the pictures inside timeline blocks, per layer and frame ("id@frame"):
  // a tween block shows the pose on every frame, a hold block once. Redrawn a
  // few at a time after a change; the old picture shows until then.
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

  void _rowPicTick() {
    final e = _engine;
    if (e == null || _playing || _oneFinger == 1 || _st.isEmpty || !_layersOpen) return;
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
    if (_rowPicDirty.isEmpty) return;
    // nearest the playhead first, a few per tick
    final order = _rowPicDirty.toList()..sort((a, b) => (int.parse(a.split('@').last) - _frame).abs().compareTo((int.parse(b.split('@').last) - _frame).abs()));
    for (final k in order.take(3)) {
      _rowPicDirty.remove(k);
      final at = k.lastIndexOf('@');
      final id = k.substring(0, at), f = int.parse(k.substring(at + 1));
      final px = (44 * math.min(_dpr, 2.0)).round();
      final pic = e.renderLayer(id, f / _fps, px, px);
      if (pic == null) continue;
      ui.decodeImageFromPixels(pic, px, px, ui.PixelFormat.rgba8888, (img) {
        if (!mounted) return;
        setState(() {
          _rowPics.remove(k)?.dispose();
          _rowPics[k] = img;
        });
      });
    }
  }

  /// Draws one stale frame picture (nearest the current frame first).
  void _thumbTick() {
    final e = _engine;
    if (e == null || _thumbDirty.isEmpty || _playing || _oneFinger == 1 || _st.isEmpty) return;
    final i = _thumbDirty.reduce((a, b) => (a - _frame).abs() <= (b - _frame).abs() ? a : b);
    _thumbDirty.remove(i);
    if (i >= _frames) return;
    final sw = (_st['w'] as num).toDouble(), sh = (_st['h'] as num).toDouble();
    final w = (120 * math.min(_dpr, 2.0)).round(), h = (w * sh / sw).round();
    final px = e.renderView(i / _fps, w, h, -sw / 2, -sh / 2, sw / 2, sh / 2, camera: true);
    if (px == null) return;
    ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, (img) {
      if (!mounted) return;
      setState(() {
        _thumbs.remove(i)?.dispose();
        _thumbs[i] = img;
      });
    });
  }

  int _copyCount = 1;

  /// The copy popup on a frame picture: copy it alone, or with N frames after it.
  Future<void> _copyPopup(int frame) async {
    if (_playing) return;
    setState(() => _frameSelected = true);
    _cmd({'op': 'frame', 'i': frame});
    if (!AppSettings.flag('keepSelection')) _cmd({'op': 'select'});
    await showDialog<void>(
      context: context,
      builder: (c) => StatefulBuilder(
        builder: (c, set) {
          final maxN = math.max(0, _frames - 1 - frame);
          final n = math.min(_copyCount, maxN);
          return AlertDialog(
            backgroundColor: cPanel,
            content: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              _pair(
                _Btn('Copy this frame', onTap: () {
                  _cmd({'op': 'copy_frame', 'n': 0});
                  Navigator.pop(c);
                }),
                _Btn('Paste after it', enabled: _st['frameCopied'] == true, onTap: () {
                  _cmd({'op': 'paste_frame'});
                  Navigator.pop(c);
                }),
              ),
              const SizedBox(height: 12),
              const Text('Or copy this frame and the frames after it:', style: TextStyle(color: cSecondary, fontSize: 12)),
              const SizedBox(height: 6),
              Row(children: [
                SizedBox(width: 44, child: _Btn('−', enabled: n > 1, onTap: () => set(() => _copyCount = math.max(1, n - 1)))),
                const SizedBox(width: 6),
                Expanded(
                  child: _Btn('Copy +$n', accent: true, enabled: maxN > 0, onTap: () {
                    _cmd({'op': 'copy_frame', 'n': n});
                    Navigator.pop(c);
                  }),
                ),
                const SizedBox(width: 6),
                SizedBox(width: 44, child: _Btn('+', enabled: n < maxN, onTap: () => set(() => _copyCount = math.min(maxN, n + 1)))),
              ]),
              const SizedBox(height: 4),
              Text(maxN > 0 ? 'Frames ${frame + 1} to ${frame + 1 + n}.' : 'This is the last frame.', style: const TextStyle(color: cSecondary, fontSize: 11)),
            ]),
          );
        },
      ),
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

  /// The timeline (REDESIGN §4, D16, D27): a names column, then the ruler in
  /// frames, the Preview row (every layer merged, tiled like a film strip at
  /// the picture's shape, not a box per frame) and, unfolded, one row per
  /// layer with its poses as quiet blocks. The playhead is a pale line with a
  /// pill on the ruler. Per-frame marks stay small and quiet (D17).
  Widget _previewRow() {
    const rulerH = 22.0, stripH = 28.0, gap = 4.0, rowH = 32.0, left = 8.0;
    final cell = _cellW;
    final fps = _fps.round().clamp(1, 240);
    final cur = _playing ? (_playT * _fps).floor().clamp(0, _frames - 1) : _frame;
    final rows = _layersOpen && !_playing ? ((_st['rows'] as List?) ?? const []).cast<Map>() : const <Map>[];
    final selLayer = _sel?['layer'];
    final showPreview = AppSettings.values['previewRow'] != false;
    final top = rulerH + (showPreview ? stripH + gap : 0);
    // H3: a layer's animated values as thin rows under it, when opened
    const propH = 24.0;
    final ys = <double>[];
    final props = <(Map, Map, double)>[]; // layer, property, y
    var yy = top;
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
    // the States board: an animation with nothing in it says how to start
    final empty = _layersOpen && !_playing && !rows.any((r) => r['kind'] != 'camera');
    if (empty) yy += 2 * rowH;
    final contentH = yy + (rows.isEmpty && !empty ? 6 : 0);
    final width = left * 2 + _frames * cell;
    final aspect = ((_st['w'] as num?) ?? 16) / ((_st['h'] as num?) ?? 9);
    int frameAt(double x) => ((x - left) / cell).floor().clamp(0, _frames - 1);
    void frameTap(int i) {
      if (_playing) _togglePlay();
      setState(() => _frameSelected = true);
      _cmd({'op': 'frame', 'i': i});
      if (!AppSettings.flag('keepSelection')) _cmd({'op': 'select'});
    }

    Widget mark(String icon, Color c) => Padding(padding: const EdgeInsets.only(right: 1), child: Icon(_markIcons[icon], size: 10, color: c));
    final markList = (_st['frameMarks'] as List?) ?? const [];
    final noTween = (_st['noTweenFrames'] as List?) ?? const [];
    final content = SizedBox(
      width: width,
      height: contentH,
      child: Stack(clipBehavior: Clip.none, children: [
        Positioned.fill(
          child: CustomPaint(
            painter: _TimelinePainter(
              frames: _frames, cell: cell, left: left, fps: fps, cur: cur, rulerH: rulerH,
              stripH: showPreview ? stripH : 0,
              lines: [for (final y in ys) y + rowH, for (final (_, _, y) in props) y + propH],
              bands: [for (final (_, _, y) in props) (y, propH)],
              aspect: aspect.toDouble(), thumbs: _thumbs,
            ),
          ),
        ),
        // the ruler and the Preview row: the whole moment (Frame page); hold to copy
        Positioned(
          left: 0,
          top: 0,
          width: width,
          height: top,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapUp: (d) => frameTap(frameAt(d.localPosition.dx)),
            onLongPressStart: _playing ? null : (d) => _copyPopup(frameAt(d.localPosition.dx)),
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
        // the Frame page is about this moment: its cell outlined in the strip
        if (_frameSelected && showPreview && !_playing)
          Positioned(
            left: left + cur * cell - 1,
            top: rulerH,
            width: cell + 2,
            height: stripH,
            child: IgnorePointer(child: Container(decoration: BoxDecoration(border: Border.all(color: cAccent, width: 2), borderRadius: BorderRadius.circular(5)))),
          ),
        // the rows' blocks: one per pose
        for (var k = 0; k < rows.length; k++) ..._rowBlocks(rows[k], ys[k], rowH, cell, left, rows[k]['id'] == selLayer),
        // grey diamonds where an animated value changes
        for (final (_, pr, y) in props)
          for (final f in ((pr['keys'] as List?) ?? const []).cast<num>())
            Positioned(
              left: left + f * cell + cell / 2 - 5,
              top: y + propH / 2 - 5,
              child: IgnorePointer(child: Transform.rotate(angle: math.pi / 4, child: Container(width: 8, height: 8, decoration: BoxDecoration(color: const Color(0xFF8A9099), borderRadius: BorderRadius.circular(1.5))))),
            ),
        // the playhead: a pale line and the pill
        Positioned(left: left + cur * cell + cell / 2 - 0.75, top: rulerH - 2, width: 1.5, height: contentH - rulerH + 2, child: const IgnorePointer(child: ColoredBox(color: Color(0xFF8DB8E8)))),
        Positioned(
          left: left + cur * cell + cell / 2 - 16,
          top: 2,
          child: IgnorePointer(
            child: Container(
              constraints: const BoxConstraints(minWidth: 32),
              height: 18,
              padding: const EdgeInsets.symmetric(horizontal: 6),
              alignment: Alignment.center,
              decoration: BoxDecoration(color: const Color(0xFFDCEAF9), borderRadius: BorderRadius.circular(9)),
              child: Text('${cur + 1}', style: const TextStyle(color: cAccent, fontSize: 11, fontWeight: FontWeight.w700)),
            ),
          ),
        ),
      ]),
    );
    final scroller = SingleChildScrollView(
      controller: _stripScroll,
      scrollDirection: Axis.horizontal,
      physics: _boxPath != null ? const NeverScrollableScrollPhysics() : null,
      child: content,
    );
    // G3: pinch (or Ctrl + scroll wheel) to zoom the frames
    void zoom(double k) => setState(() => _cellW = (_cellW * k).clamp(14.0, 112.0));
    Widget area = Listener(
      onPointerSignal: (e) {
        if (e is PointerScrollEvent && HardwareKeyboard.instance.isControlPressed) {
          zoom(e.scrollDelta.dy > 0 ? 1 / 1.15 : 1.15);
        }
      },
      child: GestureDetector(
        onScaleStart: (_) => _pinchFrom = _cellW,
        onScaleUpdate: (d) {
          if (d.pointerCount >= 2) setState(() => _cellW = (_pinchFrom * d.horizontalScale).clamp(14.0, 112.0));
        },
        child: scroller,
      ),
    );
    // G5: double-tap and drag for a selection box, triple-tap and drag for a
    // lasso (counted from raw touches, so a plain tap stays instant)
    if (rows.isNotEmpty) {
      area = Listener(
        behavior: HitTestBehavior.translucent,
        onPointerDown: (e) {
          final now = DateTime.now();
          final quick = now.difference(_lastDown) < const Duration(milliseconds: 350) && (e.localPosition - _lastDownAt).distance < 24;
          _taps = quick ? _taps + 1 : 1;
          _lastDown = now;
          _lastDownAt = e.localPosition;
          if (_taps >= 2 && !_playing && e.localPosition.dy > top) {
            setState(() {
              _boxPath = [e.localPosition];
              _lasso = _taps >= 3;
            });
          }
        },
        onPointerMove: (e) {
          final path = _boxPath;
          if (path == null) return;
          setState(() {
            if (_lasso) {
              path.add(e.localPosition);
            } else {
              _boxPath = [path.first, e.localPosition];
            }
          });
        },
        onPointerUp: (_) => _finishBox(rows, ys, cell, rowH),
        onPointerCancel: (_) => setState(() => _boxPath = null),
        child: Stack(children: [
          area,
          if (_boxPath != null) Positioned.fill(child: IgnorePointer(child: CustomPaint(painter: _BoxPainter(_boxPath!, _lasso)))),
        ]),
      );
    }
    // the names column: a blank cell by the ruler, "Preview" with its eye,
    // then each layer front first with its icon, name and eye
    Widget name(Map r) {
      final on = r['visible'] != false, selected = r['id'] == selLayer || ((_st['multi'] as List?)?.any((m) => (m as Map)['id'] == r['id']) ?? false);
      const icons = {'figure': 'figure', 'picture': 'picture', 'text': 'text', 'movieclip': 'clip', 'camera': 'camera', 'sound': 'sound'};
      final row = Container(
        height: rowH,
        padding: const EdgeInsets.symmetric(horizontal: 8),
        decoration: BoxDecoration(color: selected ? cSelected : null, border: const Border(bottom: BorderSide(color: Color(0xFFEEF0F2)))),
        child: Row(children: [
          if (((r['props'] as List?) ?? const []).isNotEmpty)
            kit.Tap(
              label: _openRows.contains(r['id']) ? 'Hide ${r['name']}\u2019s animated values' : 'Show ${r['name']}\u2019s animated values',
              minSize: 20,
              onTap: () => setState(() => _openRows.contains(r['id']) ? _openRows.remove(r['id']) : _openRows.add('${r['id']}')),
              child: Transform.rotate(angle: _openRows.contains(r['id']) ? 0 : -math.pi / 2, child: const Pictogram('chevron', size: 14, color: cSecondary)),
            )
          else
            const SizedBox(width: 14),
          const SizedBox(width: 4),
          Pictogram(icons[r['kind']] ?? 'figure', size: 20, color: selected ? cAccent : cInk2b),
          const SizedBox(width: 6),
          const SizedBox(width: 4),
          Expanded(child: Text('${r['name']}', maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: selected ? cAccent : on ? cText : cSecondary, fontSize: 15, fontWeight: selected ? FontWeight.w700 : FontWeight.w400))),
          Pictogram('eye', size: 18, color: on ? const Color(0xFF9AA0A8) : const Color(0xFFD5D8DD)),
        ]),
      );
      if (r['kind'] != 'figure' || _playing) return row;
      // G4: swipe a row (right-click on Linux) for Duplicate and Delete
      void pick() => _cmd({'op': 'select', 'layer': r['id']});
      return rw.SwipeActions(
        key: ValueKey('row-${r['id']}-$_rowActionsOpen'),
        initiallyOpen: _rowActionsOpen && r['id'] == selLayer,
        actionWidth: 70,
        actions: [
          rw.SwipeAction('Duplicate', () {
            pick();
            _cmd({'op': 'duplicate_figure'});
          }),
          rw.SwipeAction('Delete', () {
            pick();
            _cmd({'op': 'delete_everywhere'});
            kit.showToast(context, '${r['name']} is gone from every frame.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          }, danger: true),
        ],
        child: GestureDetector(onTap: pick, child: row),
      );
    }

    final names = Container(
      width: MediaQuery.sizeOf(context).width >= 900 ? 210 : 140,
      decoration: const BoxDecoration(border: Border(right: BorderSide(color: Color(0xFFE6E8EB)))),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const SizedBox(height: rulerH),
        if (showPreview)
          SizedBox(
            height: stripH + gap,
            child: Padding(
              padding: const EdgeInsets.only(left: 36, right: 4),
              child: Row(children: [
                const Expanded(child: Text('Preview', style: TextStyle(color: cSecondary, fontSize: 12))),
                kit.Tap(
                  label: 'Hide the preview',
                  minSize: 28,
                  onTap: () => setState(() {
                    AppSettings.values['previewRow'] = false;
                    unawaited(AppSettings.save());
                  }),
                  child: const Pictogram('eye', size: 16, color: Color(0xFF9AA0A8)),
                ),
              ]),
            ),
          ),
        for (final r in rows) ...[
          name(r),
          if (_openRows.contains(r['id']))
            for (final pr in ((r['props'] as List?) ?? const []).cast<Map>())
              Container(
                height: propH,
                padding: const EdgeInsets.only(left: 30, right: 2),
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
    return Container(
      color: cCard,
      constraints: BoxConstraints(maxHeight: math.min(contentH, MediaQuery.sizeOf(context).height * 0.38)),
      child: SingleChildScrollView(
        child: SizedBox(
          height: contentH,
          child: Stack(children: [
            Row(crossAxisAlignment: CrossAxisAlignment.start, children: [names, Expanded(child: area)]),
            if (empty)
              Positioned(
                left: MediaQuery.sizeOf(context).width >= 900 ? 210 : 140,
                right: 0,
                top: yy - 2 * rowH,
                height: 2 * rowH,
                child: const IgnorePointer(child: Center(child: Text('Nothing here yet. Tap + to add a stick figure.', style: TextStyle(color: cSecondary, fontSize: 14)))),
              ),
          ]),
        ),
      ),
    );
  }

  /// A layer row's poses as quiet blocks (G2): one rounded bar per pose, no
  /// markers. The selected figure's pose under the playhead is pale blue with
  /// a blue edge and grips at both ends: the right one sets its length, the
  /// left one where it starts (G3). Holding any later pose and sliding moves
  /// it. Tapping selects the figure on that frame and opens its pose.
  List<Widget> _rowBlocks(Map r, double y, double rowH, double cell, double left, bool selectedRow) {
    final on = (r['on'] as List?) ?? const [];
    final out = <Widget>[];
    // M5: a sound row is its clips, with the waveform
    if (r['kind'] == 'sound') {
      for (final c in ((r['clips'] as List?) ?? const []).cast<List>()) {
        // drawn up to the end of the animation (a longer sound plays on past it)
        final x = left + (c[0] as num).toDouble() * cell + 1;
        final w = math.max(8.0, math.min((c[1] as num).toDouble() * cell, (_frames - (c[0] as num).toDouble()) * cell) - 2);
        final n = math.max(1, ((w - 12) / 4).floor());
        final key = '${c[2]}@${c[3]}@${c[4]}@$n';
        final peaks = _peaks[key] ??= _engine?.soundPeaks('${c[2]}', (c[3] as num).toDouble(), (c[4] as num).toDouble(), n) ?? const <double>[];
        out.add(Positioned(
          left: x,
          top: y + (rowH - 26) / 2,
          child: GestureDetector(
            onTap: _playing
                ? null
                : () {
                    _cmd({'op': 'frame', 'i': (c[0] as num).floor()});
                    setState(() {
                      _soundPage = true;
                      _panelOpen = true;
                    });
                  },
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
    final drag = _blockDrag != null && _blockDrag!.layer == r['id'] ? _blockDrag : null;
    Widget bar(double x, double w, {bool strong = false, bool ghost = false}) => Container(
          width: w,
          height: 26,
          decoration: BoxDecoration(
            color: strong ? const Color(0xFFE4EEFA) : const Color(0xFFEEF0F3),
            borderRadius: BorderRadius.circular(5),
            border: Border.all(color: strong ? cAccent : const Color(0xFFDCDFE4), width: strong ? 2 : 1),
          ),
          foregroundDecoration: ghost ? BoxDecoration(color: cCard.withValues(alpha: 0.3), borderRadius: BorderRadius.circular(5)) : null,
        );
    for (final b in (r['blocks'] as List? ?? const [])) {
      final st = (b[0] as num).toInt(), len = (b[1] as num).toInt();
      if (drag != null && drag.start == st) continue; // drawn where it is going
      // only the frames the layer is on
      var a = st, z = st + len - 1;
      while (a <= z && on.elementAtOrNull(a) == false) {
        a++;
      }
      while (z >= a && on.elementAtOrNull(z) == false) {
        z--;
      }
      if (a > z) continue;
      final strong = selectedRow && _frame >= st && _frame < st + len;
      final x = left + a * cell + 1, w = (z - a + 1) * cell - 2;
      void select(double dx) {
        _cmd({'op': 'frame', 'i': (a + dx ~/ cell).clamp(0, _frames - 1)});
        if (r['kind'] == 'camera') {
          _setMode('camera');
          return;
        }
        if (r['kind'] == 'picture' || r['kind'] == 'movieclip') {
          _cmd({'op': 'select', 'layer': r['id'], 'node': ''});
          setState(() => _panelOpen = true);
          return;
        }
        if (r['kind'] == 'figure') {
          _cmd({'op': 'select', 'layer': r['id']});
          setState(() => _panelOpen = true);
          _jumpTo('Pose');
        }
      }

      Widget grip(bool atStart) => Positioned(
            left: atStart ? 0 : null,
            right: atStart ? null : 0,
            top: 0,
            bottom: 0,
            width: 14,
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onHorizontalDragStart: (_) => setState(() => _blockDrag = _BlockDrag('${r['id']}', st, len, cell, edge: atStart ? 2 : 0)),
              onHorizontalDragUpdate: (d) {
                if (_blockDrag != null) setState(() => _blockDrag!.dx += d.delta.dx);
              },
              onHorizontalDragEnd: (_) => _endBlockDrag(),
              child: MouseRegion(
                cursor: SystemMouseCursors.resizeLeftRight,
                child: Center(child: Container(width: 4, height: 14, decoration: BoxDecoration(color: cAccent, borderRadius: BorderRadius.circular(2)))),
              ),
            ),
          );
      final figure = r['kind'] == 'figure';
      // the pose's pictures: every frame of a tween, the first of a hold (a
      // camera row shows the camera's glyph)
      final tween = !(b[2] == true) && st + len < _frames;
      // a joined figure's first block carries the chain (M5): the pictures move over
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
                _say('${r['name']} is joined to ${r['joinedTo']}. Let go on its Stickfigure page.', light: true);
              },
              child: const Pictogram('chain', size: 13, color: Color(0xFF5D636B), stroke: 2.2),
            ),
          ),
      ];
      out.add(Positioned(
        left: x,
        top: y + (rowH - 26) / 2,
        child: _playing
            ? Stack(children: [bar(x, w), ...pics])
            : GestureDetector(
                onTapUp: (d) => select(d.localPosition.dx),
                onLongPressStart: figure && st > 0
                    ? (d) {
                        select(d.localPosition.dx);
                        setState(() => _blockDrag = _BlockDrag('${r['id']}', st, len, cell, edge: 1));
                      }
                    : null,
                onLongPressMoveUpdate: (d) {
                  if (_blockDrag != null) setState(() => _blockDrag!.dx = d.offsetFromOrigin.dx);
                },
                onLongPressEnd: (_) => _endBlockDrag(),
                child: Stack(children: [
                  bar(x, w, strong: strong),
                  ...pics,
                  if (strong && figure && st > 0) grip(true),
                  if (strong && figure) grip(false),
                ]),
              ),
      ));
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
    if (drag != null) {
      final (st, len) = drag.preview();
      out.add(Positioned(left: left + st * cell + 1, top: y + (rowH - 26) / 2, child: IgnorePointer(child: bar(0, len * cell - 2, strong: true, ghost: true))));
    }
    return out;
  }

  /// A selection box or lasso let go: the figures whose poses it covers (a
  /// plain double tap: the row under it). One figure is selected; several
  /// open the multi-select page.
  void _finishBox(List<Map> rows, List<double> ys, double cell, double rowH) {
    final path = _boxPath;
    if (path == null) return;
    setState(() => _boxPath = null);
    final off = _stripScroll.hasClients ? _stripScroll.offset : 0.0;
    final hit = <String>[];
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
      if (r['kind'] != 'figure') continue;
      final y0 = ys[k], cy = y0 + rowH / 2;
      if (point) {
        if (path.first.dy >= y0 && path.first.dy < y0 + rowH) hit.add('${r['id']}');
        continue;
      }
      final on = (r['on'] as List?) ?? const [];
      for (var i = 0; i < _frames; i++) {
        if (on.elementAtOrNull(i) == false) continue;
        final c = Offset(8 + i * cell + cell / 2 - off, cy);
        if (_lasso ? inLasso(c) : rect.inflate(cell / 2).contains(c) && rect.top <= y0 + rowH && rect.bottom >= y0) {
          hit.add('${r['id']}');
          break;
        }
      }
    }
    if (hit.isEmpty) return;
    if (hit.length == 1) {
      _cmd({'op': 'select', 'layer': hit.first});
    } else {
      _cmd({'op': 'select_many', 'layers': hit});
    }
    setState(() => _panelOpen = true);
  }

  /// A chain marker tapped (I2): what it is, and Let go / Join elsewhere (a
  /// join) or Join again (a let-go). Each change is one step with an Undo toast.
  Future<void> _joinPopover(Map r, int f, String? to, String? from, GlobalKey key) async {
    final box = key.currentContext?.findRenderObject() as RenderBox?;
    final at = box == null ? const Offset(200, 300) : box.localToGlobal(Offset.zero);
    final screen = MediaQuery.sizeOf(context);
    final name = '${r['name']}';
    final pick = await showGeneralDialog<int>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Join',
      barrierColor: const Color(0x00000000),
      transitionDuration: Duration.zero,
      pageBuilder: (c, a1, a2) => Stack(children: [
        Positioned(
          left: math.min(at.dx, screen.width - 290),
          bottom: screen.height - at.dy + 6,
          child: kit.PanelCard(
            width: 280,
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              Text(to != null ? '$name is joined to $to from frame ${f + 1}.' : '$name lets go of ${from ?? 'its figure'} at frame ${f + 1}.', style: const TextStyle(color: cText, fontSize: 14)),
              const SizedBox(height: 8),
              if (to != null)
                kit.ButtonPair('Let go', () => Navigator.pop(c, 1), 'Join elsewhere', () => Navigator.pop(c, 2))
              else
                kit.ButtonPair('Join again', from == null ? null : () => Navigator.pop(c, 3), 'Close', () => Navigator.pop(c)),
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
      kit.showToast(context, '$name lets go of $to at frame ${at + 1}', light: true, icon: 'broken', iconColor: cDanger, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
    } else if (pick == 2) {
      _cmd({'op': 'frame', 'i': f});
      _cmd({'op': 'select', 'layer': r['id']});
      _cmd({'op': 'join'});
      _say('Tap a joint on another figure: $name joins it from frame ${f + 1}.', light: true);
    } else if (pick == 3) {
      final target = ((_st['rows'] as List?) ?? const []).cast<Map>().where((x) => x['name'] == from).firstOrNull;
      if (target == null) return;
      _cmd({'op': 'frame', 'i': f});
      _cmd({'op': 'select', 'layer': r['id']});
      _cmd({'op': 'join_here', 'layer': target['id']});
      if (!mounted) return;
      kit.showToast(context, '$name joins $from again at frame ${f + 1}', light: true, icon: 'chain', action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
    }
  }

  /// A grip or a held block let go: the new length, start, or place.
  void _endBlockDrag() {
    final d = _blockDrag;
    if (d == null) return;
    setState(() => _blockDrag = null);
    final (st, len) = d.preview();
    _cmd({'op': 'frame', 'i': d.start});
    if (d.edge == 0) {
      if (len != d.length) _cmd({'op': 'block_length', 'frames': len});
    } else if (st != d.start) {
      _cmd({'op': 'move_block', 'to': st});
    }
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
    _cmd({'op': 'frame', 'i': i.clamp(0, _frames - 1)});
    if (!AppSettings.flag('keepSelection')) _cmd({'op': 'select'});
  }

  /// The timeline's control toolbar (REDESIGN §4): frame steps on the left,
  /// onion skin, transport in the middle, the frame rate on the right.
  Widget _controlBar() {
    return Container(
      height: 40,
      decoration: const BoxDecoration(color: cCard, border: Border(top: BorderSide(color: cLine), bottom: BorderSide(color: Color(0xFFE6E8EB)))),
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
          const Spacer(),
          kit.Tap(
            key: _fpsKey,
            label: 'Animation: $_fpsLabel frames per second',
            onTap: _playing ? null : _animationPopover,
            radius: 8,
            child: Padding(padding: const EdgeInsets.symmetric(horizontal: 8), child: Text('$_fpsLabel fps', style: const TextStyle(color: cSecondary, fontSize: 13))),
          ),
          // G1: unfold the layer rows (folded: the ruler and Preview row only)
          Transform.rotate(
            angle: _layersOpen ? 0 : math.pi,
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
    final box = _addKey.currentContext?.findRenderObject() as RenderBox?;
    final screen = MediaQuery.sizeOf(context);
    final at = box == null ? Offset(8, screen.height - 48) : box.localToGlobal(Offset.zero);
    final items = <(String, String, VoidCallback?)>[
      ('figure', 'Stick figure', () {
        setState(() => _libType = 0);
        _figurePicker();
      }),
      ('picture', 'Picture', _importImages),
      ('text', 'Text', () => _cmd({'op': 'add_textfield'})),
      ('sound', 'Sound', _importSounds),
      ('clip', 'Movieclip', () {
        setState(() => _libType = 1);
        _figurePicker(movieclip: true);
      }),
      // planned (K): shown as on the board, not yet usable
      ('folder', 'Group', null),
    ];
    final pick = await showGeneralDialog<int>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Add',
      barrierColor: const Color(0x00000000),
      transitionDuration: Duration.zero,
      pageBuilder: (c, a1, a2) => Stack(children: [
        Positioned(
          left: at.dx,
          bottom: screen.height - at.dy + 6,
          child: kit.PanelCard(
            width: 250,
            padding: const EdgeInsets.all(6),
            child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              for (var i = 0; i < items.length; i++) ...[
                if (i == 5) Container(height: 1, margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 4), color: const Color(0xFFE6E8EB)),
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

  /// The timeline's bottom bar: "+" alone on the left, frame words on the right.
  Widget _framesBar() {
    return Container(
      height: 40,
      color: cCard,
      padding: const EdgeInsets.symmetric(horizontal: 6),
      child: Row(children: [
        kit.IconTool('plus', key: _addKey, label: 'Add', color: cAccent, onTap: _playing ? null : _addMenu),
        if (!_playing && !((_st['rows'] as List?) ?? const []).any((r) => (r as Map)['kind'] != 'camera'))
          const Padding(padding: EdgeInsets.only(left: 4, right: 4), child: _StartHere()),
        // H3 (D5): keying. Off, a change to an effect or the opacity is the
        // same on every frame; on, it is a key on this frame
        kit.IconTool('key', label: _opts['keying'] == true ? 'Keying: changes make keys' : 'Keying off: changes apply to every frame', on: _opts['keying'] == true, onTap: _playing ? null : () => _cmd({'op': 'set', 'k': 'keying', 'v': _opts['keying'] != true})),
        const Spacer(),
        // §4: the Stick Nodes bridge and the two pose words; copy, paste and
        // delete frame are on the Frame page (tap the ruler)
        kit.WordButton('Add frame', size: 14, onTap: _playing ? null : () => _cmd({'op': 'insert_frame'})),
        kit.WordButton('New pose', size: 14, onTap: _playing || _sel == null || (_sel!['pose'] as Map?)?['here'] == true ? null : () => _cmd({'op': 'new_pose'})),
        kit.WordButton('Hold longer', size: 14, onTap: _playing || _sel == null ? null : () => _cmd({'op': 'hold_longer'})),
      ]),
    );
  }


  // ---------------------------------------------------------------- left panel

  /// Switch between posing figures, moving everything and the camera.
  void _setMode(String m) {
    setState(() => _frameSelected = false);
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

  /// What the rail's Delete removes: the selected text, figure, or frame.
  VoidCallback? get _deleteSelection {
    if (_playing) return null;
    if (_st['textfield'] != null) return () => _cmd({'op': 'delete_textfield'});
    if (_sel != null && _st['building'] == null) return () => _cmd({'op': 'delete_here'});
    if (_frameSelected && _frames > 1) return () => _cmd({'op': 'delete_frame'});
    return null;
  }

  bool _panelOpen = true, _hidePanels = false;
  bool _partsOpen = true;
  bool _layersOpen = true, _rowActionsOpen = false;
  // the Sound page (a sound block tapped); any other pick leaves it
  bool _soundPage = false;
  final Set<String> _openRows = {};
  // G3: the timeline's zoom (a frame's width), a pinch's start, a block drag
  double _cellW = 46, _pinchFrom = 46;
  _BlockDrag? _blockDrag;
  // G5: quick taps counted for the box (2) and lasso (3); the shape being drawn
  int _taps = 0;
  DateTime _lastDown = DateTime(0);
  Offset _lastDownAt = Offset.zero;
  List<Offset>? _boxPath;
  bool _lasso = false;

  /// The right rail (REDESIGN §4): undo, redo, Properties, delete; hide the
  /// panels at the bottom.
  Widget _rightRail(EdgeInsets pad, bool leftHand) {
    return Container(
      width: 64 + (leftHand ? pad.left : pad.right),
      padding: EdgeInsets.only(right: leftHand ? 0 : pad.right, left: leftHand ? pad.left : 0, top: 6, bottom: 6 + (_fsNow || _hidePanels ? pad.bottom : 0)),
      decoration: BoxDecoration(color: cPanel, border: Border(left: leftHand ? BorderSide.none : const BorderSide(color: cLine), right: leftHand ? const BorderSide(color: cLine) : BorderSide.none)),
      child: Column(children: [
        kit.RailButton('undo', label: 'Undo', onTap: !_playing && _st['canUndo'] == true ? () => _cmd({'op': 'undo'}) : null),
        const SizedBox(height: 4),
        kit.RailButton('redo', label: 'Redo', onTap: !_playing && _st['canRedo'] == true ? () => _cmd({'op': 'redo'}) : null),
        const SizedBox(height: 10),
        kit.RailButton('props', label: 'Properties', pressed: _panelOpen && !_hidePanels, onTap: () => setState(() {
              _hidePanels = false;
              _panelOpen = !_panelOpen;
            })),
        const SizedBox(height: 4),
        kit.RailButton('bin', label: 'Delete', onTap: _deleteSelection),
        const Spacer(),
        kit.RailButton('full', label: _hidePanels ? 'Show the panels' : 'Hide the panels', pressed: _hidePanels, onTap: () => setState(() => _hidePanels = !_hidePanels)),
      ]),
    );
  }

  static String _tabLabel(String n) => n == 'Stickfigure Filters' ? 'Filters' : n == 'Figure editor' ? 'Figure' : n.replaceAll(' Tools', '');

  /// The Properties card (REDESIGN §4): today's sections, tabbed by what is
  /// selected. It floats over the stage on the right.
  Widget _propertiesCard({bool docked = false}) {
    if (_fxBrowser && (_sel != null || _st['multi'] != null) && !_playing) return kit.PanelCard(docked: docked, padding: EdgeInsets.zero, child: _effectsBrowser());
    final names = _sections();
    if (names.length == 1 && names.first == 'Nothing selected' && !docked) {
      return Align(
        alignment: Alignment.topCenter,
        child: kit.PanelCard(child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: _sectionBody('Nothing selected'))),
      );
    }
    final current = names.contains(_tab) ? names.indexOf(_tab) : 0;
    return kit.PanelCard(
      docked: docked,
      padding: EdgeInsets.zero,
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        if (!_playing && _showTabs && names.length > 1)
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 10, 10, 0),
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
        Expanded(
          child: _playing
              ? ListView(padding: const EdgeInsets.fromLTRB(14, 10, 14, 10), children: _playbackPanel())
              : SingleChildScrollView(
                  controller: _panelScroll,
                  padding: const EdgeInsets.fromLTRB(14, 4, 14, 40),
                  child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                    // one page per tab (round 10 boards); without tabs, the pages stack
                    for (final n in (_showTabs && names.length > 1 ? [names[current]] : names)) ...[
                      if (_pageTitle(n).isNotEmpty)
                        Container(key: _sectionKey(n), width: double.infinity, padding: const EdgeInsets.only(top: 10, bottom: 4), child: Text(_pageTitle(n), textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w600)))
                      else
                        SizedBox(key: _sectionKey(n), height: 6),
                      ..._sectionBody(n),
                      const SizedBox(height: 10),
                    ],
                  ]),
                ),
        ),
        // the page's pinned button (round 10 boards): Delete stickfigure,
        // Reset this segment, Delete this pose, Clear frame, ...
        if (_pageFoot(_playing ? '' : names[current]) case (final String label, final VoidCallback onTap, final VoidCallback? onHold))
          Container(
            decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
            padding: const EdgeInsets.fromLTRB(14, 8, 14, 8),
            child: GestureDetector(onLongPress: onHold, child: kit.WordButton(label, onTap: onTap, filled: true, danger: true, expand: true, height: 36)),
          ),
        // export progress (until the Export sheet shows it, B5)
        if (_status.isNotEmpty)
          Container(
            color: cControl,
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
            child: Text(_status, maxLines: 2, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cSecondary, fontSize: 12)),
          ),
      ]),
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
        'Nothing selected' => '',
        _ => section,
      };

  /// A page's pinned button: (label, tap, hold).
  (String, VoidCallback, VoidCallback?)? _pageFoot(String section) {
    final sel = _sel;
    switch (section) {
      case 'Stickfigure Tools' when sel != null:
        // hold: with the figures joined to it
        return ('Delete stickfigure', () => _cmd({'op': 'delete_here'}), () => _cmd({'op': 'delete_here', 'joined': true}));
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
        return ('Delete picture', () => _cmd({'op': 'delete_here'}), null);
      case 'Sound Tools' when _st['frameSound'] != null:
        return ('Delete sound', () => _cmd({'op': 'frame_sound', 'asset': null}), null);
      case 'Textfield Tools' when _st['textfield'] != null:
        return ('Delete text', () => _cmd({'op': 'delete_textfield'}), null);
      case 'Movieclip Tools' when _st['movieclip'] != null:
        return ('Delete movieclip', () => _cmd({'op': 'delete_here'}), null);
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
          number('saveReminder', 'Save reminder (minutes, 0: off)', 0, 120, 1),
        ]);
        final middle = card([
          const rw.DTitle('Editing'),
          ...toggle('axisLock', 'Axis locking', 'Drags of a figure or the camera keep to a straight line when they nearly follow one.'),
          ...toggle('keepSelection', 'Keep the selection when changing frames', 'Picking another frame leaves the selected figure selected.'),
          ...toggle('stylus', 'Precise touches (stylus)', 'Joints have to be touched more exactly: for a stylus rather than a finger.'),
          ...toggle('haptics', 'Haptic feedback', 'A light tick when you grab a joint, on devices that have it.'),
          ...toggle('doubleTapMovieclip', 'Double-tap a figure to make a movieclip', 'Two quick taps on a joint ask whether to convert the figure.'),
          ...toggle('doubleTapNone', 'Double-tap the stage for Select (figure editor)', 'Instead of fitting the view, while editing a figure.'),
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
    final box = _fpsKey.currentContext?.findRenderObject() as RenderBox?;
    final screen = MediaQuery.sizeOf(context);
    final at = box == null ? Offset(screen.width - 12, screen.height - 90) : box.localToGlobal(Offset(box.size.width, 0));
    await showGeneralDialog<void>(
      context: context,
      barrierDismissible: true,
      barrierLabel: 'Animation',
      barrierColor: const Color(0x00000000),
      transitionDuration: Duration.zero,
      pageBuilder: (c, a1, a2) => StatefulBuilder(builder: (c, set) {
        void cmd(Map<String, Object?> m) {
          _cmd(m);
          set(() {});
        }

        final tween = _st['tweening'] == true;
        final n = (_st['tweenedFrames'] as num?)?.toInt() ?? 5;
        final rate = (_st['rate'] as num?)?.toDouble() ?? _fps;
        final rows = <rw.Desc>[
          const rw.DTitle('Animation'),
          rw.DStep('Frames per second', _fpsLabel, onMinus: _fps > 1 ? () => cmd({'op': 'fps', 'v': _fps - 1}) : null, onPlus: () => cmd({'op': 'fps', 'v': _fps + 1})),
          rw.DTick('Tweening', tween, onChanged: (v) => cmd({'op': 'animation', 'tweening': v})),
          rw.DStep('Extra smoothness', '$n', onMinus: tween && n > 0 ? () => cmd({'op': 'animation', 'tweened': n - 1}) : null, onPlus: tween ? () => cmd({'op': 'animation', 'tweened': n + 1}) : null),
          rw.DTick('Loop back to the start', _st['loopTween'] == true, onChanged: tween ? (v) => cmd({'op': 'animation', 'loop': v}) : null),
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
        return Stack(children: [
          Positioned(
            right: screen.width - at.dx,
            bottom: screen.height - at.dy + 6,
            child: kit.PanelCard(
              width: 320,
              child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [for (final d in rows) rw.renderDesc(d)]),
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
    ];
    // a frame's sound has its own page (the Sound board): tap its block
    final snd = _st['frameSound'] as Map?;
    return [
      for (final d in rows) rw.renderDesc(d),
      rw.renderDesc(const rw.DTitle('Sound')),
      rw.renderDesc(snd != null
          ? rw.DLink('This frame plays', '${snd['name']}', onTap: () => setState(() => _soundPage = true))
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
      setState(() {
        _soundPage = true;
        _panelOpen = true;
      });
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
      rw.DButtons('New pose here', () => each('New pose', {'op': 'new_pose'}), b: 'Hold longer', onB: () => each('Hold longer', {'op': 'hold_longer'})),
      rw.DButtons('Flip sideways', () => each('Flip figures', {'op': 'flip_figure', 'h': true}), b: 'Flip over', onB: () => each('Flip figures', {'op': 'flip_figure', 'h': false})),
      rw.DTick('Visible in onion skin', same('onion'), onChanged: (v) => each('Onion skin', {'op': 'figure', 'onion': v})),
      rw.DButtons('Duplicate', () => each('Duplicate figures', {'op': 'duplicate_figure'}), b: 'Pick one', onB: () => _cmd({'op': 'select', 'layer': multi.first['id']})),
      rw.DDanger('Delete from this frame', () => each('Delete figures', {'op': 'delete_here'})),
      const rw.DHint('Double-tap the timeline and drag for a box; triple-tap and drag to draw a lasso.'),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// The Picture page (round 10 Picture board).
  List<Widget> _pictureTools(Map pic) {
    void set(Map<String, Object?> m) => _cmd({'op': 'picture', ...m});
    final scale = ((pic['scale'] as num?) ?? 1).toDouble(), opacity = ((pic['opacity'] as num?) ?? 1).toDouble();
    const blends = ['NORMAL', 'MULTIPLY', 'SCREEN', 'ADD'];
    const blendNames = ['Normal', 'Multiply', 'Screen', 'Add'];
    final blend = math.max(0, blends.indexOf('${pic['blend'] ?? 'NORMAL'}'));
    final rows = <rw.Desc>[
      rw.DInfo('Picture', '${pic['file'] ?? pic['name']}'),
      rw.DStep('Scale', scale.toStringAsFixed(2), onMinus: scale > 0.05 ? () => set({'scale': math.max(0.05, scale - 0.05)}) : null, onPlus: () => set({'scale': scale + 0.05})),
      rw.DButtons('Flip sideways', () => set({'flipH': true}), b: 'Flip over', onB: () => set({'flipV': true})),
      const rw.DTitle('Blending'),
      rw.DSlider('Opacity', '${(opacity * 100).round()}%', opacity, onChanged: (x) => set({'opacity': (x * 100).round() / 100})),
      rw.DLink('Blend', blendNames[blend], onTap: () => _cmd({'op': 'blend', 'v': blends[(blend + 1) % blends.length]})),
      rw.DTick('Locked', pic['locked'] == true, onChanged: (v) => set({'locked': v})),
      rw.DTick('Locked to camera', pic['cameraLocked'] == true, onChanged: (v) => _cmd({'op': 'camera_lock', 'layer': pic['layer'], 'on': v})),
      const rw.DHint('Move a picture with "Move everything" (the second mode on the left), or lock it to the camera for a backdrop.'),
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
    // the board's four: even, slow end, slow both ends, bounce ("in": slow
    // start, from older files, shows as even)
    const eases = ['linear', 'out', 'smooth', 'bounce'];
    final rows = <rw.Desc>[
      rw.DInfo(sel['name'] as String? ?? 'Figure', len == 1 ? 'frame ${start + 1}' : last ? 'frame ${start + 1} to the end' : 'frames ${start + 1} to ${start + len}'),
      rw.DSeg(const ['Hold until the next pose', 'Tween into it'], hold ? 0 : 1, onSelect: (i) => easing(i == 0 ? 'hold' : 'linear')),
      rw.DEase(math.max(0, eases.indexOf(ease)), enabled: !hold, onSelect: (i) => easing(eases[i])),
      rw.DStep('Length in frames', '$len',
          onMinus: !last && len > 1 ? () => _cmd({'op': 'block_length', 'frames': len - 1}) : null,
          onPlus: () => _cmd({'op': 'block_length', 'frames': len + 1})),
      rw.DButtons('Copy pose', () => _cmd({'op': 'copy_pose'}), b: 'Paste pose', onB: _st['pose'] == true ? () => _cmd({'op': 'paste_pose'}) : null),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }



  // ---------------------------------------------------------------- effects

  /// The effects a figure can have (REDESIGN §4 Effects browser, H2): the
  /// filter each is, the command that turns it on with a useful start, and the
  /// one that turns it off. Movement effects are modifiers (`move_fx`).
  static const _fxLook = [
    ('blur', 'Blur', {'blur': 6.0}, {'blur': 0.0}),
    ('motion', 'Motion blur', {'motion': true, 'motionDistance': 30.0}, {'motion': false}),
    ('glow', 'Glow', {'glowAmount': 0.7, 'glowIntensity': 1.2}, {'glowAmount': 0.0}),
    ('adjust', 'Colour', {'saturation': 0.2}, {'hue': 0.0, 'saturation': 1.0}),
    ('tint', 'Tint', {'tint': 0xC0392BFF, 'tintAmount': 0.5}, {'tintAmount': 0.0}),
    ('invert', 'Invert', {'invert': true}, {'invert': false}),
    ('pixelate', 'Pixelate', {'pixelate': 8.0}, {'pixelate': 1.0}),
  ];
  static const _fxEdges = [
    ('outline', 'Outline', {'outline': 0x222428FF, 'outlineAmount': 1.0, 'outlineThickness': 6.0}, {'outlineAmount': 0.0}),
    ('shadow', 'Shadow', {'shadowOpacity': 0.5}, {'shadowOpacity': 0.0}),
  ];
  static const _fxMovement = [('wiggle', 'Wiggle', 'wiggle'), ('swing', 'Swing', 'swing'), ('loop', 'Loop', 'loopmod'), ('follow', 'Follow', 'follow'), ('link', 'Link', 'link')];

  bool _fxBrowser = false;
  String? _fxOpen;
  String _fxSearch = '';
  final Map<String, ui.Image> _fxPreviews = {};

  /// Opens the browser: the tiles' pictures are the selected figure, on this
  /// frame, with each effect (drawn by the engine on a copy).
  Future<void> _openFxBrowser() async {
    final e = _engine;
    if (e == null) return;
    for (final img in _fxPreviews.values) {
      img.dispose();
    }
    _fxPreviews.clear();
    final w = (106 * math.min(_dpr, 2.0)).round(), h = (58 * math.min(_dpr, 2.0)).round();
    // effects are sized in stage units and a tile shows the figure about a
    // quarter size, so the pictures use stronger settings than a new effect
    const strong = {
      'blur': {'blur': 26.0},
      'motion': {'motion': true, 'motionDistance': 160.0},
      'glow': {'glow': 0xFFC83DFF, 'glowAmount': 1.0, 'glowIntensity': 2.5},
      'pixelate': {'pixelate': 30.0},
      'outline': {'outline': 0x222428FF, 'outlineAmount': 1.0, 'outlineThickness': 22.0},
      'shadow': {'shadowOpacity': 0.6, 'shadowDistance': 40.0, 'shadowAngle': 135.0, 'shadowBlur': 12.0},
    };
    for (final (k, _, on, _) in [..._fxLook, ..._fxEdges]) {
      final px = e.renderEffectPreview(jsonEncode(strong[k] ?? on), w, h);
      if (px == null) continue;
      final c = Completer<ui.Image>();
      ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, c.complete);
      _fxPreviews[k] = await c.future;
    }
    if (mounted) setState(() => _fxBrowser = true);
  }

  /// The Effects section of the Stickfigure page (D10): each effect the
  /// figure has, with an eye; tapping one opens its settings under it, a
  /// swipe removes it. Then Blending (opacity).
  List<rw.Desc> _effectRows(Map<String, dynamic> sel) {
    final f = (sel['filters'] as Map<String, dynamic>?) ?? const <String, dynamic>{};
    final on = (f['on'] as Map?) ?? const {};
    final hidden = ((f['hidden'] as List?) ?? const []).toSet();
    double v(String k, double d) => (f[k] as num?)?.toDouble() ?? d;
    int col(String k, int d) => (f[k] as num?)?.toInt() ?? d;
    void filter(Map<String, Object?> m) => _cmd({'op': 'filter', ...m});
    final list = <(String, String, bool, Map<String, Object?>)>[
      for (final (k, label, _, off) in [..._fxLook, ..._fxEdges])
        if (on[k] == true) (k, label, !hidden.contains(k), {'op': 'filter', ...off}),
      for (final k in const ['wiggle', 'swing'])
        if (f[k] is Map) (k, k == 'wiggle' ? 'Wiggle' : 'Swing', (f[k] as Map)['shown'] != false, {'op': 'move_fx', 'kind': k, 'on': false}),
    ];
    List<rw.Desc> settings(String k) {
      Map m(String key) => (f[key] as Map?) ?? const {};
      return switch (k) {
        'blur' => [rw.DStep('Radius', v('blur', 0).toStringAsFixed(1), onMinus: () => filter({'blur': math.max(0.5, v('blur', 0) - 0.5)}), onPlus: () => filter({'blur': v('blur', 0) + 0.5}))],
        'motion' => [rw.DStep('Distance', v('motionDistance', 30).round().toString(), onMinus: () => filter({'motionDistance': math.max(0, v('motionDistance', 30) - 2)}), onPlus: () => filter({'motionDistance': v('motionDistance', 30) + 2}))],
        'glow' => [
            rw.DWidget(_swatches(col('glow', 0x0a84ffff), (c) => filter({'glow': c}))),
            rw.DStep('Amount', v('glowAmount', 0).toStringAsFixed(2), onMinus: () => filter({'glowAmount': math.max(0.05, v('glowAmount', 0) - 0.05)}), onPlus: () => filter({'glowAmount': math.min(1, v('glowAmount', 0) + 0.05)})),
            rw.DStep('Strength', v('glowIntensity', 1).toStringAsFixed(2), onMinus: () => filter({'glowIntensity': math.max(0, v('glowIntensity', 1) - 0.1)}), onPlus: () => filter({'glowIntensity': v('glowIntensity', 1) + 0.1})),
          ],
        'adjust' => [
            rw.DStep('Hue shift', v('hue', 0).round().toString(), onMinus: () => filter({'hue': v('hue', 0) - 5}), onPlus: () => filter({'hue': v('hue', 0) + 5})),
            rw.DStep('Saturation', v('saturation', 1).toStringAsFixed(2), onMinus: () => filter({'saturation': math.max(0, v('saturation', 1) - 0.05)}), onPlus: () => filter({'saturation': math.min(4, v('saturation', 1) + 0.05)})),
          ],
        'tint' => [
            rw.DWidget(_swatches(col('tint', 0xffffffff), (c) => filter({'tint': c}))),
            rw.DStep('Amount', v('tintAmount', 0).toStringAsFixed(2), onMinus: () => filter({'tintAmount': math.max(0.05, v('tintAmount', 0) - 0.05)}), onPlus: () => filter({'tintAmount': math.min(1, v('tintAmount', 0) + 0.05)})),
          ],
        'pixelate' => [rw.DStep('Size', v('pixelate', 1).round().toString(), onMinus: () => filter({'pixelate': math.max(2, v('pixelate', 1) - 1)}), onPlus: () => filter({'pixelate': v('pixelate', 1) + 1}))],
        'outline' => [
            rw.DWidget(_swatches(col('outline', 0xff00ffff), (c) => filter({'outline': c}))),
            rw.DStep('Thickness', v('outlineThickness', 4).round().toString(), onMinus: () => filter({'outlineThickness': math.max(0, v('outlineThickness', 4) - 1)}), onPlus: () => filter({'outlineThickness': v('outlineThickness', 4) + 1})),
            rw.DStep('Amount', v('outlineAmount', 1).toStringAsFixed(2), onMinus: () => filter({'outlineAmount': math.max(0.05, v('outlineAmount', 1) - 0.05)}), onPlus: () => filter({'outlineAmount': math.min(1, v('outlineAmount', 1) + 0.05)})),
          ],
        'shadow' => [
            rw.DWidget(_swatches(col('shadow', 0x000000ff), (c) => filter({'shadow': c}))),
            rw.DStep('Opacity', v('shadowOpacity', 0).toStringAsFixed(2), onMinus: () => filter({'shadowOpacity': math.max(0.05, v('shadowOpacity', 0) - 0.05)}), onPlus: () => filter({'shadowOpacity': math.min(1, v('shadowOpacity', 0) + 0.05)})),
            rw.DStep('Distance', v('shadowDistance', 10).round().toString(), onMinus: () => filter({'shadowDistance': math.max(0, v('shadowDistance', 10) - 1), 'shadowAngle': v('shadowAngle', 135)}), onPlus: () => filter({'shadowDistance': v('shadowDistance', 10) + 1, 'shadowAngle': v('shadowAngle', 135)})),
            rw.DStep('Angle', '${v('shadowAngle', 135).round()}°', onMinus: () => filter({'shadowDistance': v('shadowDistance', 10), 'shadowAngle': v('shadowAngle', 135) - 5}), onPlus: () => filter({'shadowDistance': v('shadowDistance', 10), 'shadowAngle': v('shadowAngle', 135) + 5})),
            rw.DStep('Blur', v('shadowBlur', 6).toStringAsFixed(1), onMinus: () => filter({'shadowBlur': math.max(0, v('shadowBlur', 6) - 0.5)}), onPlus: () => filter({'shadowBlur': v('shadowBlur', 6) + 0.5})),
          ],
        'wiggle' || 'swing' => [
            rw.DStep(k == 'wiggle' ? 'Distance' : 'Angle', '${((m(k)['amount'] as num?) ?? 0).round()}${k == 'swing' ? '°' : ''}',
                onMinus: () => _cmd({'op': 'move_fx', 'kind': k, 'amount': math.max(0, ((m(k)['amount'] as num?) ?? 0) - 2)}), onPlus: () => _cmd({'op': 'move_fx', 'kind': k, 'amount': ((m(k)['amount'] as num?) ?? 0) + 2})),
            rw.DStep('Speed', ((m(k)['speed'] as num?) ?? 1).toStringAsFixed(2),
                onMinus: () => _cmd({'op': 'move_fx', 'kind': k, 'speed': math.max(0.05, ((m(k)['speed'] as num?) ?? 1) - 0.25)}), onPlus: () => _cmd({'op': 'move_fx', 'kind': k, 'speed': ((m(k)['speed'] as num?) ?? 1) + 0.25})),
          ],
        _ => const [],
      };
    }

    return [
      rw.DTitle('Effects', onAdd: _openFxBrowser, addLabel: 'Add an effect'),
      if (list.isEmpty) const rw.DHint('No effects yet. They show while playing and in exports; FX in the timeline shows them while editing.'),
      for (final (k, label, shown, remove) in list) ...[
        rw.DFx(label,
            tag: _fxAnimated(sel, label),
            shown: shown,
            open: _fxOpen == k,
            onTap: () => setState(() => _fxOpen = _fxOpen == k ? null : k),
            onEye: () => k == 'wiggle' || k == 'swing' ? _cmd({'op': 'move_fx', 'kind': k, 'shown': !shown}) : _cmd({'op': 'filter', 'shown': {k: !shown}}),
            onRemove: () => _cmd(remove)),
        if (_fxOpen == k) ...settings(k),
      ],
      if (list.isNotEmpty) rw.DButtons('Copy effects', () => _cmd({'op': 'copy_filters'}), b: 'Paste effects', onB: _st['filtersCopied'] == true ? () => _cmd({'op': 'paste_filters'}) : null),
      if (list.isEmpty && _st['filtersCopied'] == true) rw.DButtons('Paste effects', () => _cmd({'op': 'paste_filters'})),
      const rw.DTitle('Blending'),
      rw.DSlider('Opacity', '${(v('opacity', 1) * 100).round()}%', v('opacity', 1), onChanged: (x) => filter({'opacity': (x * 100).round() / 100})),
    ];
  }

  /// "strength animated": what of an effect changes over time (the selected
  /// figure's row in the timeline lists its animated values).
  String? _fxAnimated(Map<String, dynamic> sel, String label) {
    final row = ((_st['rows'] as List?) ?? const []).cast<Map>().where((r) => r['id'] == sel['layer']).firstOrNull;
    final parts = [
      for (final p in ((row?['props'] as List?) ?? const []).cast<Map>())
        if ('${p['name']}'.startsWith('$label ')) '${p['name']}'.substring(label.length + 1),
    ];
    return parts.isEmpty ? null : '${parts.join(', ')} animated';
  }

  /// The effects browser (REDESIGN §4): inside Properties, "‹ Properties"
  /// back, a search, then the groups in tiles of three. Look and Edges tiles
  /// show the selected figure with the effect; picking one adds it (one
  /// step) and opens its settings.
  Widget _effectsBrowser() {
    final q = _fxSearch.toLowerCase();
    bool match(String label) => q.isEmpty || label.toLowerCase().contains(q);
    void close() => setState(() => _fxBrowser = false);
    void pick(String k, Map<String, Object?> cmd) {
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
      _jumpTo('Stickfigure Tools');
    }

    Widget tile(String label, Widget picture, VoidCallback? onTap) => Opacity(
          opacity: onTap == null ? 0.4 : 1,
          child: kit.Tap(
            label: label,
            minSize: 0,
            onTap: onTap,
            child: Column(mainAxisSize: MainAxisSize.min, children: [
              Container(
                height: 58,
                clipBehavior: Clip.antiAlias,
                decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(8), border: Border.all(color: cLine)),
                child: picture,
              ),
              const SizedBox(height: 3),
              Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 12)),
            ]),
          ),
        );
    Widget grid(List<Widget> tiles) => GridView.count(
          crossAxisCount: 3,
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          mainAxisSpacing: 10,
          crossAxisSpacing: 8,
          childAspectRatio: 1.25,
          children: tiles,
        );
    Widget pic(String k) => _fxPreviews[k] == null ? const SizedBox.expand() : RawImage(image: _fxPreviews[k], fit: BoxFit.contain);
    final look = [for (final (k, l, on, _) in _fxLook) if (match(l)) tile(l, pic(k), () => pick(k, {'op': 'filter', ...on}))];
    final edges = [for (final (k, l, on, _) in _fxEdges) if (match(l)) tile(l, pic(k), () => pick(k, {'op': 'filter', ...on}))];
    final moves = [
      for (final (k, l, icon) in _fxMovement)
        if (match(l))
          tile(l, Center(child: Pictogram(icon, size: 30, color: cAccent, stroke: 1.8)), k == 'wiggle' || k == 'swing' ? () => pick(k, {'op': 'move_fx', 'kind': k, 'on': true}) : null),
    ];
    Widget title(String t) => Padding(padding: const EdgeInsets.only(top: 12, bottom: 6), child: Text(t, textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w700)));
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(6, 8, 10, 0),
        child: Row(children: [kit.WordButton('‹ Properties', onTap: close), const Spacer()]),
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(14, 4, 14, 0),
        child: TextField(
          style: const TextStyle(color: cText, fontSize: 14),
          decoration: InputDecoration(
            isDense: true,
            filled: true,
            fillColor: cControl,
            hintText: 'Search effects',
            prefixIcon: const Icon(Icons.search, size: 18, color: Color(0xFF8A9099)),
            contentPadding: const EdgeInsets.symmetric(vertical: 10),
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: BorderSide.none),
          ),
          onChanged: (v) => setState(() => _fxSearch = v),
        ),
      ),
      Expanded(
        child: ListView(padding: const EdgeInsets.fromLTRB(14, 0, 14, 16), children: [
          if (look.isNotEmpty) ...[title('Look'), grid(look)],
          if (moves.isNotEmpty) ...[title('Movement'), grid(moves), if (q.isEmpty) const kit.Hint('Loop, Follow and Link come later.')],
          if (edges.isNotEmpty) ...[title('Edges'), grid(edges)],
          if (look.isEmpty && moves.isEmpty && edges.isEmpty) const Padding(padding: EdgeInsets.only(top: 24), child: Text('No effect by that name.', textAlign: TextAlign.center, style: TextStyle(color: cSecondary))),
        ]),
      ),
    ]);
  }

  /// Stickfigure Tools as rows (D15; REDESIGN §8 Stickfigure page). Every old
  /// control is kept; the pencil is "Edit segments" (the figure editor).
  List<Widget> _stickfigureTools(Map<String, dynamic> sel) {
    final scale = (sel['scale'] as num?)?.toDouble() ?? 1;
    final angle = (sel['figAngle'] as num?)?.toDouble() ?? 0;
    final stretch = _opts['stretch'] == true;
    final joining = _st['joining'] == true;
    const blends = ['NORMAL', 'MULTIPLY', 'SCREEN', 'ADD'];
    const blendNames = ['Normal', 'Multiply', 'Screen', 'Add'];
    final blend = math.max(0, blends.indexOf('${sel['blend'] ?? 'NORMAL'}'));
    final rows = <rw.Desc>[
      // the round 10 board, top to bottom (the figure editor link stays "Edit
      // segments" until editing makes a new figure, J1 / D12)
      rw.DName(sel['name'] as String? ?? 'Figure', link: 'Edit as a new figure…', onLink: () => _editCopySheet(sel['name'] as String? ?? 'The figure')),
      rw.DWidget(kit.PropRow('Colour', trailing: _swatches((sel['figColor'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'figure', 'color': c})))),
      ..._effectRows(sel),
      rw.DLink('Blend', blendNames[blend], onTap: () => _cmd({'op': 'blend', 'v': blends[(blend + 1) % blends.length]})),
      const rw.DTitle('Stickfigure Tools'),
      rw.DButtons('Flip sideways', () => _cmd({'op': 'flip_figure', 'h': true}), b: 'Flip over', onB: () => _cmd({'op': 'flip_figure', 'h': false})),
      rw.DButtons('Send back', () => _cmd({'op': 'layer_order', 'forward': false}), b: 'Bring forward', onB: () => _cmd({'op': 'layer_order', 'forward': true})),
      rw.DStep('Scale', scale.toStringAsFixed(2), onMinus: scale > 0.05 ? () => _cmd({'op': 'figure', 'scale': math.max(0.05, scale - 0.05)}) : null, onPlus: () => _cmd({'op': 'figure', 'scale': scale + 0.05})),
      rw.DButtons(angle.round() == 0 ? 'Turn 45°' : 'Turn 45° (now ${angle.round()}°)', () => _cmd({'op': 'figure', 'turn': 45}), b: 'Center on stage', onB: () => _cmd({'op': 'figure', 'center': true})),
      rw.DSeg(const ['Rotates', 'Stretches'], stretch ? 1 : 0, label: 'Dragging a joint', onSelect: (i) => _cmd({'op': 'set', 'k': 'stretch', 'v': i == 1})),
      if (joining) ...[
        const rw.DHint('Tap a joint on another figure to join this one to it.'),
        rw.DButtons('Cancel joining', () => _cmd({'op': 'join_cancel'})),
      ] else if (sel['joinedTo'] != null)
        rw.DWidget(kit.PropRow('Joined to', trailing: Row(mainAxisSize: MainAxisSize.min, children: [
          Text('${sel['joinedTo']}', style: const TextStyle(color: cText, fontSize: 14, fontWeight: FontWeight.w600)),
          kit.WordButton('Let go', size: 13, onTap: () {
            final from = sel['joinedTo'];
            _cmd({'op': 'unjoin'});
            kit.showToast(context, _frame > 0 ? '${sel['name']} lets go of $from at frame ${_frame + 1}' : '${sel['name']} lets go of $from', light: true, icon: 'broken', iconColor: cDanger, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
          }),
        ])))
      else
        rw.DLink('Joined to', 'Nothing', onTap: () => _cmd({'op': 'join'})),
      rw.DTick('Locked', sel['locked'] == true, onChanged: (v) => _cmd({'op': 'figure', 'locked': v})),
      rw.DTick('Visible in onion skin', sel['onion'] != false, onChanged: (v) => _cmd({'op': 'figure', 'onion': v})),
      const rw.DTitle('Tweening'),
      rw.DTick('Stay on stage between poses', sel['keepInTween'] != false, onChanged: (v) => _cmd({'op': 'figure', 'keepInTween': v})),
      rw.DTick('Blend colours between poses', sel['tweenColors'] != false, onChanged: (v) => _cmd({'op': 'figure', 'tweenColors': v})),
      const rw.DTitle('Copy and paste'),
      rw.DWidget(GestureDetector(
        onLongPress: _pasteLayeringPopup,
        child: rw.renderDesc(rw.DButtons('Copy figure', () => _cmd({'op': 'copy_figure', 'joined': true}), b: 'Paste figure', onB: _st['figureCopied'] == true ? _pasteFigure : null)),
      )),
      rw.DButtons('Copy properties', () => _cmd({'op': 'copy_props'}), b: 'Paste properties', onB: _st['propsCopied'] == true ? _pastePropsDialog : null),
      rw.DButtons('Convert to movieclip…', () async {
        final ok = await _ask('Convert to movieclip?', 'This figure and the figures joined to it become a movieclip that plays their animation in a loop, in their place. It can be broken apart again.', ['Cancel', 'Convert']);
        if (ok == 'Convert') _cmd({'op': 'to_movieclip'});
      }),
      const rw.DHint('Delete stickfigure (below) removes it from this frame; hold it to take the figures joined to it too.'),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  String? _textClip;

  /// Textfield Tools as rows (D15; REDESIGN §8 Text page).
  List<Widget> _textfieldTools(Map<String, dynamic>? tf) {
    final List<rw.Desc> rows;
    if (tf == null) {
      rows = [
        rw.DButtons('Add textfield', () => _cmd({'op': 'add_textfield'}), b: 'Paste', onB: _st['textfieldCopied'] == true ? () => _cmd({'op': 'paste_textfield'}) : null),
        const rw.DHint('A textfield sits on this frame only. Tap its box to select it, then drag to move it.'),
      ];
    } else {
      final align = {'left': 0, 'center': 1, 'right': 2}[tf['align'] as String?] ?? 1;
      final scale = (tf['scale'] as num?)?.toDouble() ?? 1;
      // the Text board's order; fonts and copying the words alone follow
      rows = [
        rw.DWidget(_TextFieldBox(text: tf['text'] as String? ?? '', onChanged: (v) => _cmd({'op': 'textfield', 'text': v}))),
        const rw.DWidget(SizedBox(height: 6)),
        rw.DSeg(const ['Left', 'Center', 'Right'], align, label: 'Alignment', onSelect: (i) => _cmd({'op': 'textfield', 'align': i})),
        rw.DWidget(kit.PropRow('Colour', trailing: _swatches((tf['color'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'textfield', 'color': c})))),
        rw.DStep('Scale', scale.toStringAsFixed(2), onMinus: scale > 0.05 ? () => _cmd({'op': 'textfield', 'scale': math.max(0.05, scale - 0.05)}) : null, onPlus: () => _cmd({'op': 'textfield', 'scale': scale + 0.05})),
        rw.DTick('Locked', tf['locked'] == true, onChanged: (v) {
          // the two locks exclude each other
          if (v && tf['cameraLocked'] == true) _cmd({'op': 'camera_lock', 'layer': tf['layer'], 'on': false});
          _cmd({'op': 'textfield', 'locked': v});
        }),
        rw.DTick('Locked to camera', tf['cameraLocked'] == true, onChanged: (v) {
          if (v && tf['locked'] == true) _cmd({'op': 'textfield', 'locked': false});
          _cmd({'op': 'camera_lock', 'layer': tf['layer'], 'on': v});
        }),
        const rw.DHint('Locked to camera: it stays put in the shot however the camera moves, for titles and captions.'),
        rw.DButtons('Copy text', () => _cmd({'op': 'copy_textfield'}), b: 'Paste text', onB: _st['textfieldCopied'] == true ? () => _cmd({'op': 'paste_textfield'}) : null),
        const rw.DTitle('Font'),
        for (final w in _fontChoice(tf)) rw.DWidget(w),
        // the words alone, between textfields
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
  /// Break off (D3): the selected joint and everything below it become a
  /// figure of their own, joined where they were. Nothing moves.
  void _breakOff() {
    final before = _sel?['layer'];
    final st = _cmd({'op': 'break_off'});
    final now = (st?['selected'] as Map?)?['layer'];
    if (now == null || now == before) return; // the engine said why
    kit.showToast(context, 'That limb is its own figure now, joined where it was.', light: true, action: 'Undo', onAction: () => _cmd({'op': 'undo'}));
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
          const rw.DHint('The main node moves the whole figure. Tap a joint for its segment.'),
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
      rw.DButtons('Flip sideways', () => _cmd({'op': 'flip_branch', 'h': true}), b: 'Flip over', onB: () => _cmd({'op': 'flip_branch', 'h': false})),
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
      rw.DTick('Lock node', sel['pinned'] == true, onChanged: (v) => _cmd({'op': 'pin', 'v': v})),
      rw.DTick('Drag lock', sel['dragLock'] == true, onChanged: (v) => _cmd({'op': 'node', 'dragLock': v})),
      rw.DSeg(const ['None', 'Absolute', 'Relative'], lock, label: 'Angle lock', onSelect: (i) => seg({'angleLock': i})),
      // D3 / F1: the limb becomes its own figure, joined where it was
      rw.DButtons('Break off as a figure', _breakOff),
      if (_segAnyChanged(sel)) const rw.DHint('A dot marks a value that differs from how the figure came. Swipe its row (or right-click it) to reset it.'),
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
    final rows = <rw.Desc>[
      const rw.DHint('Drag on the stage to move the camera on this frame.'),
      // this frame's zoom and rotation onto another frame
      rw.DButtons('Copy camera', () => setState(() => _camClip = (zoom, rot)), b: 'Paste camera', onB: _camClip == null ? null : () => _cmd({'op': 'camera', 'zoom': _camClip!.$1, 'rotation': _camClip!.$2})),
      rw.DStep('Zoom', zoom.toStringAsFixed(2), onMinus: zoom > 0.05 ? () => _cmd({'op': 'camera', 'zoom': math.max(0.05, zoom - 0.05)}) : null, onPlus: () => _cmd({'op': 'camera', 'zoom': zoom + 0.05})),
      rw.DStep('Rotation', '${rot.round()}°', onMinus: () => _cmd({'op': 'camera', 'rotation': rot - 5}), onPlus: () => _cmd({'op': 'camera', 'rotation': rot + 5})),
      rw.DButtons('Center camera', () => _cmd({'op': 'camera', 'center': true}), b: 'Reset', onB: () => _cmd({'op': 'reset_camera'})),
      rw.DLink('Auto-camera', auto != null ? 'To frame ${(auto['end'] as num) + 1}' : 'Off', onTap: _autoCameraDialog),
      const rw.DHint("Auto-camera tweens this frame's camera across several frames."),
      rw.DLink('Lock a figure to the camera', switch (((_st['cameraLocks'] as List?) ?? const []).length) { 0 => 'None', 1 => '1 figure', final n => '$n figures' }, onTap: _cameraLockDialog),
      rw.DSeg(const ['None', 'Thirds', 'Phi', 'Spiral'], guide < 3 ? guide : 3, label: guide >= 3 ? 'Guide (spiral ${guide - 2} of 4)' : 'Guide', onSelect: setGuide),
    ];
    return [for (final d in rows) rw.renderDesc(d), rw.renderDesc(const rw.DTitle('Effects on this frame')), ..._cameraFx()];
  }

  /// Movieclip Tools: the selected movieclip's playback.
  /// Movieclip Tools as rows (D15; REDESIGN §8 Movieclip page).
  List<Widget> _movieclipTools(Map mc) {
    final speed = (mc['speed'] as num?)?.toDouble() ?? 1, off = (mc['offset'] as num?)?.toDouble() ?? 0;
    const modes = ['once', 'loop', 'pingpong'];
    final frames = (mc['frames'] as num?)?.toInt() ?? 0;
    final rows = <rw.Desc>[
      rw.DInfo('${mc['name']}', '$frames frame${frames == 1 ? '' : 's'} long'),
      rw.DSeg(const ['Once', 'Loop', 'Ping-pong'], modes.indexOf('${mc['loop']}').clamp(0, 2), label: 'Plays', onSelect: (i) => _cmd({'op': 'movieclip', 'loop': modes[i]})),
      rw.DStep('Speed', speed.toStringAsFixed(2), onMinus: speed > 0 ? () => _cmd({'op': 'movieclip', 'speed': math.max(0, speed - 0.1)}) : null, onPlus: () => _cmd({'op': 'movieclip', 'speed': speed + 0.1})),
      rw.DStep('Start offset', '${off.toStringAsFixed(2)} s', onMinus: () => _cmd({'op': 'movieclip', 'offset': off - 0.1}), onPlus: () => _cmd({'op': 'movieclip', 'offset': off + 0.1})),
      const rw.DHint('Drag the box on the stage to move it.'),
      rw.DButtons('Edit movieclip', () => _cmd({'op': 'edit_movieclip'}), b: 'Break apart', onB: () => _cmd({'op': 'break_apart', 'layer': mc['layer']})),
    ];
    return [for (final d in rows) rw.renderDesc(d)];
  }

  /// Copied wobble: XY, rotation, speed, scale with zoom.
  (double, double, double, bool)? _wobbleClip;

  /// Camera Tools: this frame's widescreen bars and wobble (playback and
  /// export only).
  List<Widget> _cameraFx() {
    final fx = (_st['camFx'] as Map?) ?? const {};
    final xy = (fx['wobbleXY'] as num?)?.toDouble() ?? 0, rot = (fx['wobbleRot'] as num?)?.toDouble() ?? 0;
    final speed = (fx['wobbleSpeed'] as num?)?.toDouble() ?? 3, zoom = fx['wobbleZoom'] != false;
    final any = xy > 0 || rot > 0;
    void fxCmd(Map<String, Object?> m) => _cmd({'op': 'camera_fx', ...m});
    // the Camera board's words; the amounts show under a wobble that is on
    final rows = <rw.Desc>[
      rw.DTick('Widescreen', fx['widescreen'] == true, onChanged: (v) => fxCmd({'widescreen': v})),
      rw.DTick('Wobble position', xy > 0, onChanged: (v) => fxCmd({'wobbleXY': v ? 8 : 0})),
      if (xy > 0) rw.DStep('Distance', xy.round().toString(), onMinus: () => fxCmd({'wobbleXY': math.max(1, xy - 1)}), onPlus: () => fxCmd({'wobbleXY': xy + 1})),
      rw.DTick('Wobble rotation', rot > 0, onChanged: (v) => fxCmd({'wobbleRot': v ? 0.5 : 0})),
      if (rot > 0) rw.DStep('Degrees', rot.toStringAsFixed(2), onMinus: () => fxCmd({'wobbleRot': math.max(0.1, rot - 0.1)}), onPlus: () => fxCmd({'wobbleRot': rot + 0.1})),
      rw.DStep('Wobble speed', speed.toStringAsFixed(1), onMinus: any ? () => fxCmd({'wobbleSpeed': math.max(0.1, speed - 0.5)}) : null, onPlus: any ? () => fxCmd({'wobbleSpeed': speed + 0.5}) : null),
      rw.DTick('Scale wobble with zoom', zoom, onChanged: any ? (v) => fxCmd({'wobbleZoom': v}) : null),
      rw.DButtons('Copy wobble', () => setState(() => _wobbleClip = (xy, rot, speed, zoom)),
          b: 'Paste wobble', onB: _wobbleClip == null ? null : () => fxCmd({'wobbleXY': _wobbleClip!.$1, 'wobbleRot': _wobbleClip!.$2, 'wobbleSpeed': _wobbleClip!.$3, 'wobbleZoom': _wobbleClip!.$4})),
      const rw.DHint('Widescreen and wobble belong to this frame and show when playing and in exports, not while editing.'),
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
        rw.DTick('Static joints', _opts['static'] != false, onChanged: (v) => _cmd({'op': 'set', 'k': 'static', 'v': v})),
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
    if (go == true) _cmd({'op': 'edit_copy'});
  }

  /// The figure editor's header (EditCopy / FigureEditor boards): Cancel, the
  /// copy's name over "… stays as it is", Save as new figure.
  Widget _copyHeader(EdgeInsets pad, Map ce) {
    return Container(
      height: 44 + pad.top,
      padding: EdgeInsets.only(top: pad.top, left: pad.left + 8, right: pad.right + 8),
      decoration: const BoxDecoration(color: cCard, border: Border(bottom: BorderSide(color: cLine))),
      child: Stack(alignment: Alignment.center, children: [
        Row(children: [
          kit.WordButton('Cancel', onTap: () => _cmd({'op': 'edit_copy_end'})),
          const Spacer(),
          kit.WordButton('Save as new figure', strong: true, onTap: _saveCopy),
        ]),
        IgnorePointer(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Text('${ce['name']}', style: const TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w700)),
            Text('${ce['of']} in your animation stays as it is', style: const TextStyle(color: cSecondary, fontSize: 12)),
          ]),
        ),
      ]),
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

  /// Figure editor: trace over another frame or figure (shown faintly behind).
  List<Widget> _traceTools() {
    final tr = _st['trace'] as Map?;
    return [
      _hint('Load a stickfigure or frame to be the background, and trace over the top of it.'),
      _pair(_Btn('Trace stickfigure…', small: true, onTap: _traceFigureDialog), _Btn('Trace frame…', small: true, onTap: _traceFrameDialog)),
      if (tr != null) ...[
        const SizedBox(height: 4),
        _Btn(tr['frame'] != null ? 'Clear trace (frame ${(tr['frame'] as num) + 1})' : 'Clear trace', small: true, onTap: () => _cmd({'op': 'trace'})),
      ],
      _hint('Load a background image to trace from.'),
      _pair(_Btn('Load BG image…', small: true, onTap: _loadEditorImage), _Btn('Clear BG image', small: true, enabled: _editorImage, onTap: () {
        _engine?.editorImage(null);
        setState(() {
          _editorImage = false;
          _drawn = '';
        });
      })),
      _gap,
      _label('Background colour (while editing)'),
      _swatches((_st['editorBg'] as num?)?.toInt() ?? 0xffffffff, (c) => _cmd({'op': 'editor_bg', 'color': c})),
    ];
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
    return [
      _title('Polyfill Tools'),
      _hint('Use polyfills to fill a large area by connecting joints. Tap joints of this figure one after another, clockwise, to add polynodes; the fill appears as you go. A polyfill is drawn right before its anchor joint: move that joint back or forward in the draw order to move the fill.'),
      _gap,
      Text('Polynode count: ${pf['count']}', style: const TextStyle(color: cText, fontSize: 13)),
      _gap,
      _Btn('Finish polyfill', accent: true, onTap: () => _cmd({'op': 'polyfill_edit', 'id': null})),
      _gap,
      _Toggle('Use polyfill colour', own, (v) => _cmd({'op': 'polyfill_color', 'own': v})),
      Opacity(opacity: own ? 1 : 0.4, child: _swatches((pf['color'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'polyfill_color', 'color': c, 'own': true}))),
      _gap,
      _Btn('Remove last polynode', enabled: (pf['count'] as num) > 0, onTap: () => _cmd({'op': 'polyfill_undo'})),
      _gap,
      _Btn('Delete polyfill', danger: true, onTap: () => _cmd({'op': 'polyfill_delete'})),
    ];
  }

  List<Widget> _builder(Map<String, dynamic>? sel) {
    final pf = _st['polyfill'] as Map?;
    if (pf != null) return _polyfillTools(pf);
    if (_st['connecting'] == true) {
      return [
        _title('Add connector'),
        _hint('Tap the joint this connector ends on. The connector is a new joint on the straight line from the selected joint to it, and stays on that line however the two move.'),
        _gap,
        _Btn('Cancel', onTap: () => _cmd({'op': 'connect_cancel'})),
      ];
    }
    final con = sel?['connector'] as Map?;
    final connectorTools = con == null
        ? const <Widget>[]
        : <Widget>[
            _title('Connector Tools'),
            Text('Ends on joint ${con['to']}', style: const TextStyle(color: cSecondary, fontSize: 12)),
            _stepper('Percent', ((con['percent'] as num).toDouble()).toStringAsFixed(2), () => _cmd({'op': 'connector_set', 'percent': math.max(0, (con['percent'] as num) - 0.05)}), () => _cmd({'op': 'connector_set', 'percent': math.min(1, (con['percent'] as num) + 0.05)})),
            _Toggle('Reversed', con['reversed'] == true, (v) => _cmd({'op': 'connector_set', 'reversed': v})),
            _Btn('Delete connector', danger: true, onTap: () => _cmd({'op': 'delete_joint'})),
            _gap,
          ];
    if (sel == null) return [_title('Figure editor'), const Text('Tap a joint of this figure.', style: TextStyle(color: cSecondary, fontSize: 13)), _gap, ..._traceTools()];
    final root = sel['root'] == true;
    final shape = sel['shape'] as String? ?? 'line';
    final thick = (sel['thickness'] as num?)?.toDouble() ?? 0;
    final color = (sel['color'] as num?)?.toInt() ?? 0xff;
    final stretch = _opts['stretch'] == true;
    return [
      ...connectorTools,
      _title(root ? 'Main node' : 'Segment'),
      _pair(_Btn('◀ Joint', onTap: () => _cmd({'op': 'cycle_joint', 'dir': -1})), _Btn('Joint ▶', onTap: () => _cmd({'op': 'cycle_joint', 'dir': 1}))),
      _gap,
      if (!root) Text('Angle ${sel['angle']}° · length ${sel['length']}', style: const TextStyle(color: cSecondary, fontSize: 11)),
      if (!root) _gap,
      _label('Drag'),
      _Segmented(const ['Rotate', 'Stretch'], stretch ? 1 : 0, (i) => _cmd({'op': 'set', 'k': 'stretch', 'v': i == 1})),
      _gap,
      _pair(_Btn('Add segment', onTap: () => _cmd({'op': 'add_segment'})), _Btn('Delete branch', danger: true, enabled: !root, onTap: () => _cmd({'op': 'delete_segment'}))),
      _gap,
      // paste a copied segment / branch onto this joint (the main node too)
      if (!root) _Btn('Delete segment (keep what hangs off it)', danger: true, small: true, onTap: () => _cmd({'op': 'delete_joint'})),
      if (!root) _gap,
      _Btn('Paste segment here', enabled: _st['branchCopied'] == true, onTap: () => _cmd({'op': 'paste_branch'})),
      _gap,
      if (!root) ...[
        _pair(_Btn('Copy segment', onTap: () => _cmd({'op': 'copy_branch', 'branch': false})), _Btn('Copy branch', onTap: () => _cmd({'op': 'copy_branch', 'branch': true}))),
        _gap,
        _pair(_Btn('Split ($_splitN)', onTap: () => _cmd({'op': 'split', 'n': _splitN + 1})), _Btn('Duplicate branch', onTap: () => _cmd({'op': 'duplicate_branch'}))),
        _stepper('New joints', '$_splitN', () => setState(() => _splitN = math.max(1, _splitN - 1)), () => setState(() => _splitN = math.min(20, _splitN + 1))),
        _gap,
        _label('Draw order'),
        _Segmented(const ['Segment', 'Branch'], _reorderBranch ? 1 : 0, (i) => setState(() => _reorderBranch = i == 1)),
        const SizedBox(height: 4),
        Row(children: [
          for (final (icon, to, tip) in const [(Icons.first_page, -2, 'To the back'), (Icons.chevron_left, -1, 'One back'), (Icons.chevron_right, 1, 'One forward'), (Icons.last_page, 2, 'To the front')])
            Expanded(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 2),
                child: Tooltip(message: tip, child: _Btn('', onTap: () => _cmd({'op': 'reorder', 'branch': _reorderBranch, 'to': to}), icon: icon)),
              ),
            ),
        ]),
        _gap,
        _pair(_Btn('Flip branch', onTap: () => _cmd({'op': 'flip_branch', 'h': true})), _Btn(sel['pinned'] == true ? 'Unlock' : 'Lock', onTap: () => _cmd({'op': 'pin', 'v': sel['pinned'] != true}))),
        _gap,
        _Toggle('Static (detail)', sel['static'] == true, (v) => _cmd({'op': 'segment', 'static': v})),
        _gap,
        _label('Shape'),
        Wrap(spacing: 4, runSpacing: 4, children: [
          for (final s in const ['line', 'circle', 'square', 'ellipse', 'triangle', 'trapezoid', 'polygon'])
            _Chip(s[0].toUpperCase() + s.substring(1), s == shape, () => _cmd({'op': 'segment', 'shape': s})),
        ]),
        _gap,
        _stepper('Thickness', thick.round().toString(), () => _cmd({'op': 'segment', 'thickness': math.max(0, thick - 2)}), () => _cmd({'op': 'segment', 'thickness': thick + 2})),
        _gap,
        _label('Colour'),
        _swatches(color, (c) => _cmd({'op': 'segment', 'color': c})),
        _gap,
        _Btn('Colour whole branch', onTap: () => _cmd({'op': 'segment', 'branch': true, 'color': color})),
      ],
      _gap,
      _Btn('Modify branch…', onTap: _modifyBranchDialog),
      _gap,
      ..._traceTools(),
      _gap,
      _Btn('Save stickfigure…', onTap: _saveFigureFile),
      _gap,
      _Btn('Add connector…', onTap: () => _cmd({'op': 'connect_start'})),
      _gap,
      _Btn('Add polyfill…', onTap: () => _cmd({'op': 'polyfill_new'})),
      for (final f in ((_st['polyfills'] as List?) ?? const []).cast<Map>())
        Padding(
          padding: const EdgeInsets.only(top: 4),
          child: _Btn('Edit polyfill ${f['id']} (${f['count']} polynodes)', small: true, onTap: () => _cmd({'op': 'polyfill_edit', 'id': f['id']})),
        ),
      _gap,
      _Btn('Done', accent: true, onTap: () => _cmd({'op': 'build_done'})),
    ];
  }

  static const _gap = SizedBox(height: 6);

  Widget _title(String s) => Container(
        width: double.infinity,
        padding: const EdgeInsets.only(top: 8, bottom: 2),
        child: Text(s, maxLines: 1, overflow: TextOverflow.ellipsis, textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w700)),
      );

  Widget _label(String s) => Padding(padding: const EdgeInsets.only(top: 2, bottom: 4), child: Text(s, style: const TextStyle(color: cText, fontSize: 13)));

  Widget _pair(Widget a, Widget b) => Row(children: [Expanded(child: a), const SizedBox(width: 6), Expanded(child: b)]);

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

ui.Image? _checkerTile;

/// The offstage checkerboard as a repeating shader (one 16 x 16 tile, made once).
Shader _checker() {
  if (_checkerTile == null) {
    final rec = ui.PictureRecorder();
    final c = Canvas(rec);
    const a = Color(0xE66C6F74), b = Color(0xE6606368);
    c.drawRect(const Rect.fromLTWH(0, 0, 16, 16), Paint()..color = a);
    c.drawRect(const Rect.fromLTWH(0, 0, 8, 8), Paint()..color = b);
    c.drawRect(const Rect.fromLTWH(8, 8, 8, 8), Paint()..color = b);
    _checkerTile = rec.endRecording().toImageSync(16, 16);
  }
  return ImageShader(_checkerTile!, TileMode.repeated, TileMode.repeated, Matrix4.identity().storage);
}

class _Overlay extends CustomPainter {
  final Map<String, dynamic> st;
  final StageView v;
  final bool playing;
  _Overlay(this.st, this.v, this.playing);

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
    final cam = (st['camera'] as List?)?.map((x) => (x as num).toDouble()).toList();
    final cameraMode = st['mode'] == 'camera';
    if (cam != null && cam.length == 8 && AppSettings.flag('stageBox')) {
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
      // offstage is the checkerboard (REDESIGN §3); slightly see-through so
      // figures standing offstage still show
      final outside = Path.combine(PathOperation.difference, Path()..addRect(Offset.zero & size), frame);
      canvas.drawPath(outside, Paint()..shader = _checker());
      canvas.drawPath(frame, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = const Color(0x66000000));
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
    final dl = (st['drawLine'] as List?)?.map((x) => (x as num).toDouble()).toList();
    if (dl != null && dl.length == 4) {
      canvas.drawLine(v.toScreen(dl[0], dl[1]), v.toScreen(dl[2], dl[3]), Paint()
        ..color = const Color(0xCC1A1A1A)
        ..strokeWidth = 6
        ..strokeCap = StrokeCap.round);
      canvas.drawCircle(v.toScreen(dl[2], dl[3]), 7, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2
        ..color = cAccent);
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
    for (final h in hs) {
      final l = h as List;
      final p = v.toScreen((l[0] as num).toDouble(), (l[1] as num).toDouble());
      final f = (l[2] as num).toInt();
      final root = f & 1 != 0, sel = f & 2 != 0, nb = f & 8 != 0;
      final color = sel ? red : nb ? purple : (root ? cRootNode : blue);
      // Settings: node style (0 legacy squares, 1 modern rings, 2 solid dots)
      // and line width (1 thin ... 5 thick)
      final style = AppSettings.number('nodeStyle'), k = 0.6 + 0.2 * AppSettings.number('lineWidth').clamp(1, 5);
      final stroke = Paint()
        ..style = style == 2 ? PaintingStyle.fill : PaintingStyle.stroke
        ..strokeWidth = (sel || nb ? 2.5 : 2) * k
        ..color = color;
      final halo = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 4.5 * k
        ..color = const Color(0xCCFFFFFF);
      // figure editor View options: Node highlight (a filled ring on the
      // polynodes, connectors or locked joints)
      final hl = AppSettings.number('nodeHighlight');
      final lit = (hl == 1 && f & 16 != 0) || (hl == 2 && f & 32 != 0) || (hl == 3 && f & 64 != 0);
      if (lit) canvas.drawCircle(p, 9, Paint()..color = const Color(0xAAFFB020));
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
    return kit.Tap(
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
    );
  }
}

class _Segmented extends StatelessWidget {
  final List<String> labels;
  final int selected;
  final ValueChanged<int> onTap;
  const _Segmented(this.labels, this.selected, this.onTap);

  @override
  Widget build(BuildContext context) => kit.Segmented(labels, selected, onSelect: onTap);
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

/// A block being changed (G3): its end grip (edge 0), held and slid (1), or
/// its start grip (2), which moves where it starts and keeps where it ends.
class _BlockDrag {
  _BlockDrag(this.layer, this.start, this.length, this.cell, {this.edge = 0});
  final String layer;
  final int start, length, edge;
  final double cell;
  double dx = 0;

  /// Where the pose would be now: (first frame, length).
  (int, int) preview() {
    final by = (dx / cell).round();
    return switch (edge) {
      0 => (start, math.max(1, length + by)),
      1 => (math.max(1, start + by), length),
      _ => (math.max(1, math.min(start + length - 1, start + by)), start + length - math.max(1, math.min(start + length - 1, start + by))),
    };
  }
}

/// The ruler, the merged Preview row and the rows' lines (REDESIGN §4, D16).
class _TimelinePainter extends CustomPainter {
  _TimelinePainter({required this.frames, required this.cell, required this.left, required this.fps, required this.cur, required this.rulerH, required this.stripH, required this.lines, required this.bands, required this.aspect, required this.thumbs});
  final int frames, fps, cur;
  final double cell, left, rulerH, stripH, aspect;
  final List<double> lines;
  final List<(double, double)> bands;
  final Map<int, ui.Image> thumbs;

  @override
  void paint(Canvas canvas, Size size) {
    // ruler: ticks every frame, labels every quarter second (fewer zoomed out),
    // whole seconds bold and taller; the current frame's label is the pill
    final step = math.max(1, (cell >= 40 ? fps / 4 : cell >= 28 ? fps / 2 : cell >= 18 ? fps : fps * 2).round());
    for (var i = 0; i < frames; i++) {
      final f = i + 1, sec = f % fps == 0, label = f == 1 || f % step == 0 || (sec && cell >= 18);
      final x = left + i * cell + cell / 2;
      final h = sec ? 8.0 : label ? 6.0 : 4.0;
      canvas.drawRect(Rect.fromLTWH(x - 0.5, rulerH - h, 1, h), Paint()..color = sec ? cInk2b : label ? const Color(0xFF9AA0A8) : const Color(0xFFC4C8CE));
      if (label && i != cur) {
        final tp = TextPainter(
          text: TextSpan(text: '$f', style: TextStyle(color: sec ? cText : cSecondary, fontSize: 11, fontWeight: sec ? FontWeight.w700 : FontWeight.w400, fontFamily: kit.kFont, fontFeatures: const [ui.FontFeature.tabularFigures()])),
          textDirection: TextDirection.ltr,
        )..layout();
        tp.paint(canvas, Offset(x - tp.width / 2, 3));
      }
    }
    // the Preview row: one strip, tiles at the picture's shape, each showing
    // the moment under its middle
    if (stripH > 0) {
      final r = RRect.fromRectAndRadius(Rect.fromLTWH(left, rulerH + 2, frames * cell, stripH - 4), const Radius.circular(5));
      canvas.save();
      canvas.clipRRect(r);
      canvas.drawRRect(r, Paint()..color = cCard);
      final th = stripH - 4, tw = math.max(8.0, (th * aspect).roundToDouble());
      for (var x = 0.0; x < frames * cell; x += tw) {
        final i = ((x + tw / 2) / cell).floor().clamp(0, frames - 1);
        final img = thumbs[i];
        final dst = Rect.fromLTWH(left + x, rulerH + 2, tw - 1, th);
        if (img != null) {
          // cover: the picture's middle, cropped to the tile
          final iw = img.width.toDouble(), ih = img.height.toDouble();
          final k = math.max(dst.width / iw, dst.height / ih);
          final sw = dst.width / k, sh = dst.height / k;
          canvas.drawImageRect(img, Rect.fromLTWH((iw - sw) / 2, (ih - sh) / 2, sw, sh), dst, Paint()..filterQuality = FilterQuality.medium);
        }
        canvas.drawRect(Rect.fromLTWH(left + x + tw - 1, rulerH + 2, 1, th), Paint()..color = cCard);
      }
      canvas.restore();
      canvas.drawRRect(r, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = cLine);
    }
    // property rows: a paler band; every row: a faint line under it
    for (final (y, h) in bands) {
      canvas.drawRect(Rect.fromLTWH(0, y, size.width, h), Paint()..color = const Color(0xFFFAFBFC));
    }
    for (final y in lines) {
      canvas.drawRect(Rect.fromLTWH(0, y - 1, size.width, 1), Paint()..color = const Color(0xFFEEF0F2));
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
  final List<String> _picked = [];
  String _filter = '';
  int _tab = 0; // Stock (figures, then effect figures), Yours

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
          child: Container(
            decoration: BoxDecoration(color: cCard, borderRadius: BorderRadius.circular(10), border: Border.all(color: on ? cAccent : cLine, width: on ? 2 : 1)),
            padding: const EdgeInsets.all(6),
            child: img == null ? const Center(child: Text('?', style: TextStyle(color: cSecondary))) : RawImage(image: img, fit: BoxFit.contain),
          ),
        ),
        const SizedBox(height: 4),
        Text(f.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: on ? cAccent : cText, fontSize: 12, fontWeight: on ? FontWeight.w700 : FontWeight.w400)),
      ]),
    );
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    final match = widget.entries.where((f) => f.name.toLowerCase().contains(_filter.toLowerCase())).toList();
    final lists = [[...match.where((f) => !f.mine && !f.effect), ...match.where((f) => !f.mine && f.effect)], match.where((f) => f.mine).toList()];
    final list = lists[_tab];
    return Dialog(
      backgroundColor: cCard,
      insetPadding: const EdgeInsets.all(12),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
      child: SizedBox(
        width: math.min(760, size.width * 0.9),
        height: math.min(560, size.height * 0.9),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 20, 16),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Text(widget.movieclip ? 'Add a movieclip' : 'Add a stick figure', textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 17, fontWeight: FontWeight.w600)),
            const SizedBox(height: 10),
            Row(children: [
              SizedBox(width: 240, child: kit.Segmented(const ['Stock', 'Yours'], _tab, onSelect: (i) => setState(() => _tab = i))),
              const SizedBox(width: 12),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(bottom: 6),
                  child: TextField(
                    style: const TextStyle(color: cText, fontSize: 14),
                    decoration: InputDecoration(
                      isDense: true,
                      filled: true,
                      fillColor: cControl,
                      hintText: widget.movieclip ? 'Search' : 'Search figures',
                      prefixIcon: const Icon(Icons.search, size: 18, color: Color(0xFF8A9099)),
                      contentPadding: const EdgeInsets.symmetric(vertical: 10),
                      border: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: BorderSide.none),
                    ),
                    onChanged: (v) => setState(() => _filter = v),
                  ),
                ),
              ),
            ]),
            const SizedBox(height: 8),
            Expanded(
              child: list.isEmpty
                  ? Center(
                      child: Text(
                        _tab == 1 ? 'Put figure files (.nodes, .nodemc, .stknds) in the "Barkfart Figures" folder (Files app) and they show up here.' : 'Nothing matches.',
                        textAlign: TextAlign.center,
                        style: const TextStyle(color: cSecondary, fontSize: 13),
                      ),
                    )
                  : GridView.builder(
                      gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(maxCrossAxisExtent: 140, mainAxisSpacing: 12, crossAxisSpacing: 10, childAspectRatio: 0.95),
                      itemCount: list.length,
                      itemBuilder: (c, i) => _tile(list[i]),
                    ),
            ),
            Container(
              padding: const EdgeInsets.only(top: 10),
              decoration: const BoxDecoration(border: Border(top: BorderSide(color: Color(0xFFE6E8EB)))),
              // the board's footer: Import from Files… and Paste figure; the
              // project's library and New movieclip beside them
              child: Row(children: [
                kit.WordButton('Import from Files…', size: 14, onTap: () => Navigator.pop(context, ['#import'])),
                kit.WordButton('This project\u2019s library…', size: 14, onTap: () => Navigator.pop(context, ['#library'])),
                if (widget.movieclip) kit.WordButton('New movieclip', size: 14, onTap: widget.editing ? null : () => Navigator.pop(context, ['#newclip'])),
                const Spacer(),
                if (_picked.isNotEmpty)
                  kit.WordButton('Add ${_picked.length}', size: 14, primary: true, onTap: () => Navigator.pop(context, List<String>.from(_picked)))
                else
                  kit.WordButton('Paste figure', size: 14, onTap: widget.canPaste ? () => Navigator.pop(context, ['#paste']) : null),
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

  @override
  Widget build(BuildContext context) {
    final w = _widths[_size], h = (w * widget.h / widget.w).round();
    final last = _end == 0 ? widget.frames : _end;
    final count = last - _start + 1;
    return Dialog(
      backgroundColor: cCard,
      insetPadding: const EdgeInsets.all(12),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(18)),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460),
        child: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(20, 14, 20, 16),
          child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            const Padding(padding: EdgeInsets.only(bottom: 8), child: Text('Export', textAlign: TextAlign.center, style: TextStyle(color: cTitle, fontSize: 17, fontWeight: FontWeight.w700))),
            TextField(
              controller: _name,
              style: const TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w700),
              decoration: InputDecoration(
                isDense: true,
                filled: true,
                fillColor: cControl,
                hintText: 'Name',
                contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                border: OutlineInputBorder(borderRadius: BorderRadius.circular(10), borderSide: BorderSide.none),
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
            kit.TickRow('Include sound', _format == 2 ? _sound : false, onChanged: _format == 2 ? (v) => setState(() => _sound = v) : null),
            kit.TickRow('Transparent background', _format == 1 ? _transparent : false, onChanged: _format == 1 ? (v) => setState(() => _transparent = v) : null),
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
            // the board's summary: how long, then what the format can't do
            kit.Hint('${_seconds(count)} at ${widget.fps == widget.fps.roundToDouble() ? widget.fps.round() : widget.fps} fps. ${_format == 2 ? 'Videos can’t be transparent; GIF and PNG frames can.' : _format == 1 ? '$count pictures, with the soundtrack as a WAV when there is sound.' : 'A GIF in the Barkfart Exports folder, with the soundtrack as a WAV when there is sound.'}'),
            Row(children: [
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
                    'transparent': _format == 1 && _transparent,
                    // video: the engine's flag argument means "without sound"
                    'silent': _format == 2 && !_sound,
                    'name': _name.text,
                  }),
                ),
              ),
            ]),
          ]),
        ),
      ),
    );
  }
}
