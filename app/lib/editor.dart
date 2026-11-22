// The editor: sidebar left, frame strip on top, stage in the middle
// (docs/barkfart/UI-SPEC.md). All editing happens in the Rust session; this
// file draws its state and turns touches into commands.
import 'dart:async';
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
      _magTick();
    })..start();
    _start();
  }

  Future<void> _start() async {
    final e = _engine;
    if (e == null) return;
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
      }
    }
    await AppSettings.load();
    e.command({'op': 'set', 'k': 'axis', 'v': AppSettings.flag('axisLock')});
    _refresh(e.command({'op': 'state'}));
    // draw every frame picture for the project just opened
    _thumbsChanged('open');
    _reminder = Timer.periodic(const Duration(seconds: 30), (_) => _remindToSave());
    final script = Platform.environment['BARKFART_SCRIPT'];
    if (script != null) Timer(const Duration(milliseconds: 1500), () => _runScript(script));
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
    _savedAt = DateTime.now();
    _file = f;
    await Recovery.clear();
    _refresh(e.command({'op': 'state'}));
    if (!quiet) _say('Saved ${f.name}', light: true);
    return true;
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
          // scripted: this frame plays the first imported sound
          final l = (_st['soundList'] as List?) ?? const [];
          if (l.isNotEmpty) _cmd({'op': 'frame_sound', 'asset': (l.first as Map)['id']});
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
    if (_st['building'] != null) return const ['Figure editor'];
    if (_st['mode'] == 'camera') return const ['Camera Tools'];
    if (_frameSelected) return const ['Frame Tools'];
    if (_st['textfield'] != null) return const ['Textfield Tools'];
    if (_st['movieclip'] != null) return const ['Movieclip Tools'];
    final sel = _sel;
    if (sel != null) return const ['Stickfigure Tools', 'Stickfigure Filters', 'Segment Tools'];
    if (_st['editingClip'] != null) return const ['Movieclip editor', 'Textfield Tools'];
    return const ['Project Tools', 'Textfield Tools'];
  }

  List<Widget> _sectionBody(String name) {
    switch (name) {
      case 'App Tools':
        return _appTools();
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
    }
    return const [];
  }

  String _tab = '';

  void _jumpTo(String name) {
    setState(() => _tab = name);
    final c = _sectionKey(name).currentContext;
    if (c != null) Scrollable.ensureVisible(c, duration: const Duration(milliseconds: 200), alignment: 0);
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
    final stripH = big ? 112.0 : 92.0;
    final leftHand = AppSettings.flag('leftHand');
    return Scaffold(
      backgroundColor: cSurround,
      body: RepaintBoundary(
        key: _shot,
        child: Column(children: [
        if (!_fsNow) _appBar(pad),
        Expanded(child: Row(textDirection: leftHand ? TextDirection.rtl : TextDirection.ltr, children: [
          if (!_fsNow) _leftRail(pad, leftHand),
          Expanded(
            child: Column(children: [
              if (!_fsNow && !_hidePanels && _st['building'] != null)
              Container(
                height: stripH,
                color: cPanel,
                padding: EdgeInsets.only(right: leftHand ? 0 : pad.right, left: leftHand ? pad.left : 0),
                child: _buildBar(),
              ),
              Expanded(
                child: Padding(
                  padding: EdgeInsets.only(right: leftHand ? 0 : pad.right, left: leftHand ? pad.left : 0, bottom: pad.bottom),
                  child: Stack(textDirection: TextDirection.ltr, children: [
                    Positioned.fill(child: _stage()),
                    if (_arrows && !_playing) Positioned(left: _arrowsAt.dx, top: _arrowsAt.dy, child: _arrowsWindow()),
                    if (_magnifier && !_playing) Positioned(left: _magAt.dx, top: _magAt.dy, child: _magnifierWindow()),
                    if (!_fsNow && _panelOpen && !_hidePanels)
                      // tablets and big windows dock it as a full-height column (D20)
                      dock
                          ? Positioned(top: 0, right: 0, bottom: 0, width: 360, child: _propertiesCard(docked: true))
                          : Positioned(top: 8, right: 8, bottom: 8, width: big ? 360 : 320, child: _propertiesCard()),
                    if (_viewOptions) Positioned(top: 8, left: 8, bottom: 8, width: 330, child: _viewOptionsPanel()),
                  ]),
                ),
              ),
              if (!_fsNow && !_hidePanels) _controlBar(),
              if (!_fsNow && !_hidePanels && _st['building'] == null) _previewRow(),
              if (!_fsNow && !_hidePanels) _framesBar(),
            ]),
          ),
          if (!_fsNow) _rightRail(pad, leftHand),
        ])),
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
          kit.WordButton('View', onTap: () => setState(() => _viewOptions = !_viewOptions)),
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
              for (final (i, label) in const [(0, 'New animation'), (1, 'Open…'), (2, 'Save'), (3, 'Save as…'), (4, 'Settings')])
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
        await _openDialog();
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
            if (_st['mode'] == 'camera') IgnorePointer(child: Container(decoration: BoxDecoration(border: Border.all(color: cAccent, width: 2)))),
          ]),
        ),
      );
    });
  }

  static const _drawTools = [
    ('none', 'None', Icons.near_me_outlined),
    ('square', 'Segment', Icons.horizontal_rule),
    ('line', 'Rounded', Icons.remove),
    ('circle', 'Circle', Icons.circle_outlined),
    ('ellipse', 'Ellipse', Icons.egg_outlined),
    ('triangle', 'Triangle', Icons.change_history),
    ('trapezoid', 'Trapezoid', Icons.filter_list),
    ('polygon', 'Polygon', Icons.pentagon_outlined),
    ('polyfill', 'Polyfill', Icons.format_shapes),
  ];

  Widget _buildBar() {
    final building = _st['building'] as Map<String, dynamic>;
    final tool = _st['drawTool'] as String? ?? 'none';
    // phones: a narrower title without the hint, so the tools keep room
    return LayoutBuilder(builder: (context, box) {
    final narrow = box.maxWidth < 560;
    return Row(children: [
      SizedBox(width: narrow ? 8 : 12),
      SizedBox(
        width: narrow ? 84 : 170,
        child: Column(mainAxisAlignment: MainAxisAlignment.center, crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(narrow ? '${building['name']}' : 'Editing ${building['name']}', maxLines: narrow ? 2 : 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: cText, fontSize: narrow ? 12 : 15, fontWeight: FontWeight.w600)),
          if (!narrow) _hint('Changes apply on every frame. Pick a draw tool, then drag from a joint.'),
        ]),
      ),
      const SizedBox(width: 8),
      // Draw Tools: one at a time; a tool stays picked for the next drag
      Expanded(
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(children: [
            for (final (id, label, icon) in _drawTools)
              Padding(
                padding: const EdgeInsets.only(right: 4),
                child: Material(
                  color: (id == 'polyfill' ? _st['polyfill'] != null : tool == id && _st['polyfill'] == null) ? cAccent : cControl,
                  borderRadius: BorderRadius.circular(4),
                  child: InkWell(
                    // Polyfill: a new polyfill anchored at the selected joint (Polyfill Tools)
                    onTap: () => id == 'polyfill'
                        ? (_sel == null ? _say('Tap a joint first: the polyfill is drawn right before it.') : _cmd({'op': 'polyfill_new'}))
                        : _cmd({'op': 'draw_tool', 'shape': id}),
                    child: SizedBox(
                      width: 62,
                      height: 52,
                      child: Column(mainAxisAlignment: MainAxisAlignment.center, children: [
                        Icon(icon, size: 20, color: cText),
                        Text(label, style: const TextStyle(color: cText, fontSize: 10)),
                      ]),
                    ),
                  ),
                ),
              ),
          ]),
        ),
      ),
      SizedBox(width: narrow ? 60 : 96, child: _Btn('Done', accent: true, onTap: () => _cmd({'op': 'build_done'}))),
      const SizedBox(width: 8),
    ]);
    });
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
    final opts = await showDialog<Map<String, Object>>(context: context, builder: (c) => _ExportDialog(sw, sh, _frames, _file?.name ?? 'Animation'));
    if (opts == null) return;
    await _runExport(opts['format'] as int, opts['w'] as int, opts['h'] as int, opts['start'] as int, opts['count'] as int, opts['transparent'] as bool || opts['silent'] == true, opts['name'] as String);
  }

  final ScrollController _stripScroll = ScrollController();

  /// The ruler and the Preview row (REDESIGN §4, D16): one cell per frame in a
  /// continuous strip; numbers follow the frame rate (every quarter second,
  /// whole seconds bold); the current frame is a pale pill. Per-frame marks
  /// stay, small and quiet (D17); hold a cell to copy frames from it.
  Widget _previewRow() {
    const cell = 56.0, rulerH = 22.0, thumbH = 32.0;
    final fps = _fps.round().clamp(1, 240);
    final step = math.max(1, (fps / 4).round());
    final cur = _playing ? (_playT * _fps).floor() : _frame;
    Widget mark(String icon, Color c) => Padding(padding: const EdgeInsets.only(right: 1), child: Icon(_markIcons[icon], size: 10, color: c));
    return Container(
      height: rulerH + thumbH + 10,
      color: cCard,
      child: ListView.builder(
        controller: _stripScroll,
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: 8),
        itemCount: _frames,
        itemBuilder: (c, i) {
          final f = i + 1, isCur = i == cur, sec = f % fps == 0, label = f == 1 || sec || f % step == 0;
          final marks = (_st['frameMarks'] as List?)?.elementAtOrNull(i) as List?;
          final noTween = (_st['noTweenFrames'] as List?)?.elementAtOrNull(i) == true;
          final img = _thumbs[i];
          return GestureDetector(
            onTap: () {
              if (_playing) _togglePlay();
              setState(() => _frameSelected = true);
              _cmd({'op': 'frame', 'i': i});
              if (!AppSettings.flag('keepSelection')) _cmd({'op': 'select'});
            },
            onLongPress: _playing ? null : () => _copyPopup(i),
            child: SizedBox(
              width: cell,
              child: Column(children: [
                SizedBox(
                  width: cell,
                  height: rulerH,
                  child: Stack(alignment: Alignment.bottomCenter, clipBehavior: Clip.none, children: [
                    Container(width: 1, height: sec ? 8 : label ? 6 : 4, color: sec ? cInk2b : label ? const Color(0xFF9AA0A8) : const Color(0xFFC4C8CE)),
                    if (isCur)
                      Positioned(
                        top: 2,
                        child: Container(
                          padding: const EdgeInsets.symmetric(horizontal: 8),
                          height: 17,
                          alignment: Alignment.center,
                          decoration: BoxDecoration(color: const Color(0xFFDCEAF9), borderRadius: BorderRadius.circular(9)),
                          child: Text('$f', style: const TextStyle(color: cAccent, fontSize: 11, fontWeight: FontWeight.w700)),
                        ),
                      )
                    else if (label)
                      Positioned(top: 2, child: Text('$f', style: TextStyle(color: sec ? cText : cSecondary, fontSize: 11, fontWeight: sec ? FontWeight.w700 : FontWeight.w400, fontFeatures: const [ui.FontFeature.tabularFigures()]))),
                  ]),
                ),
                Container(
                  height: thumbH,
                  margin: const EdgeInsets.only(right: 1),
                  decoration: BoxDecoration(
                    color: cCard,
                    border: Border.all(color: isCur ? cAccent : cLine, width: isCur ? 2 : 1),
                    borderRadius: BorderRadius.horizontal(left: Radius.circular(i == 0 ? 5 : 0), right: Radius.circular(i == _frames - 1 ? 5 : 0)),
                  ),
                  child: Stack(fit: StackFit.expand, children: [
                    if (img != null) RawImage(image: img, fit: BoxFit.cover),
                    Positioned(
                      left: 2,
                      bottom: 1,
                      child: Row(children: [
                        if (marks?.first == true) mark('pause', const Color(0xFFD9821E)),
                        if (marks?.elementAtOrNull(1) == true) mark('repeat', cConnected),
                        if (marks?.elementAtOrNull(2) == true) mark('silence', cDanger),
                        if (marks?.elementAtOrNull(3) == true) mark('slow', cConnected),
                        if (marks?.elementAtOrNull(4) == true) mark('camera', const Color(0xFFD94F94)),
                        if (_st['tweening'] == true && noTween && i < _frames - 1) mark('notween', cDanger),
                      ]),
                    ),
                  ]),
                ),
              ]),
            ),
          );
        },
      ),
    );
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
          const Spacer(),
          kit.WordButton('$_fpsLabel fps', key: _fpsKey, size: 13, onTap: _playing ? null : _animationPopover),
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
    final editing = _st['editingClip'] != null;
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
        _figurePicker();
      }),
      ('plus', 'New movieclip', editing ? null : () => _cmd({'op': 'new_movieclip'})),
      ('folder', 'Import from Files…', _importFromFiles),
      ('figure', 'Paste stick figure', _st['figureCopied'] == true ? _pasteFigure : null),
      ('parts', 'This project\u2019s library…', _libraryDialog),
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
        const Spacer(),
        kit.WordButton('Add frame', size: 14, onTap: _playing ? null : () => _cmd({'op': 'insert_frame'})),
        kit.WordButton('Copy frame', size: 14, onTap: _playing ? null : () => _cmd({'op': 'copy_frame'})),
        kit.WordButton('Paste frame', size: 14, onTap: _playing || _st['frameCopied'] != true ? null : () => _cmd({'op': 'paste_frame'})),
        kit.WordButton('Delete frame', size: 14, danger: true, onTap: _playing || _frames <= 1 ? null : () => _cmd({'op': 'delete_frame'})),
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
      padding: EdgeInsets.only(left: leftHand ? 0 : pad.left, right: leftHand ? pad.right : 0, top: 6, bottom: pad.bottom),
      decoration: BoxDecoration(color: cPanel, border: Border(right: leftHand ? BorderSide.none : const BorderSide(color: cLine), left: leftHand ? const BorderSide(color: cLine) : BorderSide.none)),
      child: Column(children: [
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

  /// The right rail (REDESIGN §4): undo, redo, Properties, delete; hide the
  /// panels at the bottom.
  Widget _rightRail(EdgeInsets pad, bool leftHand) {
    return Container(
      width: 64 + (leftHand ? pad.left : pad.right),
      padding: EdgeInsets.only(right: leftHand ? 0 : pad.right, left: leftHand ? pad.left : 0, top: 6, bottom: 6 + pad.bottom),
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

  static String _tabLabel(String n) => n == 'Stickfigure Filters' ? 'Filters' : n.replaceAll(' Tools', '');

  /// The Properties card (REDESIGN §4): today's sections, tabbed by what is
  /// selected. It floats over the stage on the right.
  Widget _propertiesCard({bool docked = false}) {
    final names = _sections();
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
                    for (final n in names) ...[
                      Container(key: _sectionKey(n), width: double.infinity, padding: const EdgeInsets.only(top: 10, bottom: 4), child: Text(n, textAlign: TextAlign.center, style: const TextStyle(color: cTitle, fontSize: 15, fontWeight: FontWeight.w700))),
                      ..._sectionBody(n),
                      const SizedBox(height: 10),
                    ],
                  ]),
                ),
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

  List<Widget> _appTools() => [
        _pair(_Btn('New project…', onTap: _newProject), _Btn('Open…', onTap: _openDialog)),
        _gap,
        _Btn('Settings…', onTap: _settingsDialog),
      ];

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
        _gap,
        _Toggle('Show watermark', _st['watermark'] != null, (v) => _cmd({'op': 'watermark', 'on': v})),
        if (_st['watermark'] != null) ...[
          _hint('Drawn small at the top right of exported pictures.'),
          _TextFieldBox(text: (_st['watermark'] as Map)['text'] as String? ?? '', onChanged: (v) => _cmd({'op': 'watermark', 'text': v})),
          const SizedBox(height: 4),
          _swatches(((_st['watermark'] as Map)['color'] as num?)?.toInt() ?? 0xff3b30ff, (c) => _cmd({'op': 'watermark', 'color': c})),
        ],
        _gap,
        _Toggle('Make vertical (phone video)', _vertical, (v) => setState(() => _vertical = v)),
        if (_vertical) ...[
          _hint('Turns the camera of every frame a quarter turn; the right edge becomes the top of the video.'),
          _Btn('Rotate cameras 90°', onTap: () => _cmd({'op': 'rotate_cameras'})),
        ],
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
              ? '$n in-between pictures per frame when playing and exporting (${rate.round()} a second). The last frame tweens into the first when looping. Turning effects off while playing helps when playback lags; exports always have them.'
              : 'Each frame shows for 1/${_fps.round()} s. Turning effects off while playing helps when playback lags; exports always have them.'),
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
      _label('Font'),
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
    return [
      Text('Frame ${_frame + 1} of $_frames · Nodes: ${_st['nodeCount'] ?? 0}', style: const TextStyle(color: cSecondary, fontSize: 12)),
      _gap,
      GestureDetector(
        onLongPress: last ? null : _tweenCountPopup,
        child: _Btn('Add tweened frame ($label)', enabled: !last, onTap: () => _cmd({'op': 'tween_add', 'at': at})),
      ),
      if (last) const Padding(padding: EdgeInsets.only(top: 4), child: Text('Tweened frames go between two frames.', style: TextStyle(color: cSecondary, fontSize: 11))),
      _gap,
      Row(children: [
        Expanded(
          child: _Segmented(const ['50', '33/67', '10/90'], ['50', '33/67', '10/90'].indexOf(_tweenAt), (i) => setState(() {
                final v = ['50', '33/67', '10/90'][i];
                // tapping the chosen pair again: both, then the first, then the second
                _tweenPick = v == _tweenAt && v != '50' ? (_tweenPick + 1) % 3 : 0;
                _tweenAt = v;
              })),
        ),
        const SizedBox(width: 4),
        SizedBox(width: 36, child: _Btn('?', onTap: _tweenHelp)),
      ]),
      if (_tweenAt != '50') _hint('Tap ${_tweenAt.replaceAll('/', ' / ')} again to add only one of the two frames.'),
      _gap,
      _hint('Hold the button to add several evenly spaced frames.'),
      _gap,
      _label('Background colour'),
      _swatches(bgColor, (c) => _cmd({'op': 'frame_bg', 'color': c})),
      _gap,
      _Toggle('Use gradient', bgGradient, (v) => _cmd({'op': 'frame_bg', 'useGradient': v})),
      if (bgGradient) ...[
        _label('Gradient colour (top)'),
        _swatches(bgTop, (c) => _cmd({'op': 'frame_bg', 'gradient': c})),
        _Toggle('Expand gradient', (_st['frameBg'] as Map?)?['expand'] == true, (v) => _cmd({'op': 'frame_bg', 'expand': v})),
        _hint('Stretches the gradient taller: most of the stage is the gradient colour.'),
      ],
      _gap,
      _Toggle('Do not tween this frame', _st['frameNoTween'] == true, (v) => _cmd({'op': 'no_tween', 'on': v})),
      // Slow Motion Tweening: only while the project tweens
      Opacity(
        opacity: _st['tweening'] == true ? 1 : 0.4,
        child: IgnorePointer(
          ignoring: _st['tweening'] != true,
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            _Toggle('Slow motion tweening', slow > 0, (v) => _cmd({'op': 'frame_slow', 'n': v ? 10 : 0})),
            if (slow > 0) ...[
              _hint('This overrides the number of tweened frames for this one frame: more in-between pictures, so it plays slower.'),
              _stepper('Tweened frames', '$slow', () => _cmd({'op': 'frame_slow', 'n': math.max(1, slow - 1)}), () => _cmd({'op': 'frame_slow', 'n': slow + 1})),
            ],
          ]),
        ),
      ),
      _gap,
      _Toggle('Pause on this frame', delay > 0, (v) => _cmd({'op': 'frame_delay', 'seconds': v ? 0.5 : 0})),
      if (delay > 0) _stepper('Seconds', delay.toStringAsFixed(2), () => _cmd({'op': 'frame_delay', 'seconds': math.max(0.01, delay - 0.05)}), () => _cmd({'op': 'frame_delay', 'seconds': delay + 0.05})),
      _gap,
      if (_frame > 0) _Toggle('Repeat frames before this one', back > 0, (v) => _cmd({'op': 'frame_repeat', 'back': v ? 1 : 0, 'times': 1})),
      if (back > 0) ...[
        _hint('After this frame, playback jumps back and plays the series again: a short loop without a movieclip.'),
        _stepper('Back', '$back frames', () => _cmd({'op': 'frame_repeat', 'back': math.max(1, back - 1), 'times': times}), () => _cmd({'op': 'frame_repeat', 'back': math.min(_frame, back + 1), 'times': times})),
        _stepper('Repeat', '$times times', () => _cmd({'op': 'frame_repeat', 'back': back, 'times': math.max(1, times - 1)}), () => _cmd({'op': 'frame_repeat', 'back': back, 'times': times + 1})),
        _Toggle('Use frame tweening during repeat', (_st['frameRepeat'] as Map?)?['tween'] == true, (v) => _cmd({'op': 'frame_repeat', 'back': back, 'times': times, 'tween': v})),
        _hint('With tweening on: this frame tweens into the frame it jumps back to. Off: it holds until the jump.'),
      ],
      _gap,
      _Toggle('Silence all sounds', _st['frameSilence'] == true, (v) => _cmd({'op': 'frame_silence', 'on': v})),
      _hint('Sounds still playing stop when this frame starts.'),
      _gap,
      ..._soundsBlock(),
      _gap,
      _Btn('Clear frame', danger: true, onTap: () async {
        final ok = await _ask('Clear this frame?', 'Every figure and textfield on this frame is taken off it. The frame itself stays. Undo brings them back.', ['Cancel', 'Clear frame']);
        if (ok == 'Clear frame') _cmd({'op': 'clear_frame'});
      }),
    ];
  }


  /// Frame Tools > Sounds: the sound this frame starts, and its settings.
  /// The sound this frame plays, as a page of rows (D15; REDESIGN §8 Sound page).
  List<Widget> _soundsBlock() {
    final list = ((_st['soundList'] as List?) ?? const []).cast<Map<String, dynamic>>();
    final snd = _st['frameSound'] as Map<String, dynamic>?;
    final has = snd != null;
    double v(String k, double d) => (snd?[k] as num?)?.toDouble() ?? d;
    void set(Map<String, Object?> m) => _cmd({'op': 'frame_sound', ...m});
    VoidCallback? when(bool ok, VoidCallback f) => ok ? f : null;
    final vol = v('volume', 1), pan = v('pan', 0), pitch = v('pitch', 1);
    final rows = <rw.Desc>[
      const rw.DTitle('Sound Tools'),
      rw.DLink('Sound', has ? snd['name'] as String : 'None', onTap: list.isEmpty ? null : () => _pickFrameSound(list, snd?['asset'] as String?)),
      rw.DStep('Volume', '${(vol * 100).round()}%', onMinus: when(has && vol > 0, () => set({'volume': math.max(0, vol - 0.05)})), onPlus: when(has && vol < 1, () => set({'volume': math.min(1, vol + 0.05)}))),
      rw.DStep('Pan', pan.abs() < 0.05 ? 'Centre' : pan < 0 ? 'L ${(-pan * 100).round()}' : 'R ${(pan * 100).round()}', onMinus: when(has && pan > -1, () => set({'pan': math.max(-1, pan - 0.1)})), onPlus: when(has && pan < 1, () => set({'pan': math.min(1, pan + 0.1)}))),
      rw.DStep('Pitch', pitch.toStringAsFixed(2), onMinus: when(has && pitch > 0.25, () => set({'pitch': math.max(0.25, pitch - 0.05)})), onPlus: when(has && pitch < 4, () => set({'pitch': math.min(4, pitch + 0.05)}))),
      rw.DButtons('Copy sound', when(has, () => _cmd({'op': 'copy_sound'})), b: 'Paste sound', onB: when(_st['soundCopied'] == true, () => _cmd({'op': 'paste_sound'}))),
      rw.DButtons('Import a sound…', _importSounds),
      if (has) rw.DDanger('Clear sound', () => set({'asset': null})),
    ];
    return [
      Container(key: _sectionKey('Sounds')),
      for (final d in rows) rw.renderDesc(d),
    ];
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

  static const _easings = [('linear', 'Linear'), ('smooth', 'Ease'), ('in', 'Ease in'), ('out', 'Ease out'), ('hold', 'None')];
  int _easing = 0;

  List<Widget> _stickfigureTools(Map<String, dynamic> sel) {
    final scale = (sel['scale'] as num?)?.toDouble() ?? 1;
    final angle = (sel['figAngle'] as num?)?.toDouble() ?? 0;
    final pos = sel['layerPos'], count = sel['figureCount'];
    final stretch = _opts['stretch'] == true;
    return [
      Text(sel['name'] as String? ?? 'Figure', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 13)),
      _gap,
      Row(children: [
        Expanded(
          child: GestureDetector(
            onLongPress: () => _cmd({'op': 'delete_here', 'joined': true}),
            child: _Btn('Delete stickfigure', danger: true, onTap: () => _cmd({'op': 'delete_here'})),
          ),
        ),
        const SizedBox(width: 6),
        SizedBox(width: 44, child: _Btn('✎', onTap: () => _cmd({'op': 'build'}))),
      ]),
      const Padding(padding: EdgeInsets.only(top: 3), child: Text('Removes it from this frame. Hold: with the figures joined to it. ✎ edits its segments.', style: TextStyle(color: cSecondary, fontSize: 11))),
      _gap,
      _pair(_Btn('Previous', onTap: () => _cmd({'op': 'cycle_figure', 'dir': -1})), _Btn('Next', onTap: () => _cmd({'op': 'cycle_figure', 'dir': 1}))),
      _gap,
      _label('Stickfigure'),
      _pair(_Btn('Copy', onTap: () => _cmd({'op': 'copy_figure', 'joined': true})), GestureDetector(onLongPress: _pasteLayeringPopup, child: _Btn('Paste', enabled: _st['figureCopied'] == true, onTap: _pasteFigure))),
      _gap,
      _label('All properties'),
      _pair(_Btn('Copy', onTap: () => _cmd({'op': 'copy_props'})), _Btn('Paste', enabled: _st['propsCopied'] == true, onTap: _pastePropsDialog)),
      _gap,
      _label('Pose'),
      _pair(_Btn('Copy', onTap: () => _cmd({'op': 'copy_pose'})), _Btn('Paste', enabled: _st['pose'] == true, onTap: () => _cmd({'op': 'paste_pose'}))),
      _gap,
      _pair(_Btn('Flip sideways', onTap: () => _cmd({'op': 'flip_figure', 'h': true})), _Btn('Flip over', onTap: () => _cmd({'op': 'flip_figure', 'h': false}))),
      _gap,
      Row(children: [
        Expanded(child: _Btn('Layer down', onTap: () => _cmd({'op': 'layer_order', 'forward': false}))),
        Padding(padding: const EdgeInsets.symmetric(horizontal: 6), child: Text('$pos / $count', style: const TextStyle(color: cSecondary, fontSize: 12))),
        Expanded(child: _Btn('Layer up', onTap: () => _cmd({'op': 'layer_order', 'forward': true}))),
      ]),
      _gap,
      _Btn('Center stickfigure', onTap: () => _cmd({'op': 'figure', 'center': true})),
      _gap,
      _Btn('Convert to movieclip…', onTap: () async {
        final ok = await _ask('Convert to movieclip?', 'This figure and the figures joined to it become a movieclip that plays their animation in a loop, in their place. Library Tools can break it apart again.', ['Cancel', 'Convert']);
        if (ok == 'Convert') _cmd({'op': 'to_movieclip'});
      }),
      _gap,
      _label('Stickfigure colour'),
      _swatches((sel['figColor'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'figure', 'color': c})),
      _gap,
      _stepper('Scale', scale.toStringAsFixed(2), () => _cmd({'op': 'figure', 'scale': math.max(0.05, scale - 0.05)}), () => _cmd({'op': 'figure', 'scale': scale + 0.05})),
      _gap,
      Row(children: [
        Expanded(child: Text('Angle  ${angle.round()}°', style: const TextStyle(color: cText, fontSize: 13))),
        SizedBox(width: 60, child: _Btn('+45°', onTap: () => _cmd({'op': 'figure', 'turn': 45}))),
      ]),
      _gap,
      _Toggle('Locked', sel['locked'] == true, (v) => _cmd({'op': 'figure', 'locked': v})),
      _Toggle('Visible in onion skin', sel['onion'] != false, (v) => _cmd({'op': 'figure', 'onion': v})),
      _gap,
      _label('Drag'),
      _Segmented(const ['Rotate', 'Stretch'], stretch ? 1 : 0, (i) => _cmd({'op': 'set', 'k': 'stretch', 'v': i == 1})),
      _gap,
      if (_st['joining'] == true) ...[
        Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(border: Border.all(color: cAccent), borderRadius: BorderRadius.circular(4)),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            const Text('Tap a joint on another figure to join this one to it.', style: TextStyle(color: cText, fontSize: 12)),
            _gap,
            _Btn('Cancel', onTap: () => _cmd({'op': 'join_cancel'})),
          ]),
        ),
      ] else if (sel['joinedTo'] != null) ...[
        Text('Joined to ${sel['joinedTo']}', style: const TextStyle(color: cSecondary, fontSize: 12)),
        _gap,
        _Btn('Unjoin', onTap: () => _cmd({'op': 'unjoin'})),
      ] else
        _Btn('Join to…', onTap: () => _cmd({'op': 'join'})),
      _gap,
      _label('Tween to the next frame'),
      _Btn('Tween: ${_easings[_easing].$2}', onTap: () {
        setState(() => _easing = (_easing + 1) % _easings.length);
        _cmd({'op': 'easing', 'v': _easings[_easing].$1});
      }),
      _Toggle('Keep stickfigure during tween', sel['keepInTween'] != false, (v) => _cmd({'op': 'figure', 'keepInTween': v})),
      _Toggle('Tween stickfigure colours', sel['tweenColors'] != false, (v) => _cmd({'op': 'figure', 'tweenColors': v})),
      _hint('Tweening options only apply if the animation has tweening on. Keep: the figure stays during the in-betweens before a frame it is not on. Colours: they blend between frames instead of changing at the next one.'),
    ];
  }

  String? _textClip;

  List<Widget> _textfieldTools(Map<String, dynamic>? tf) {
    if (tf == null) {
      return [
        _Btn('Add textfield', accent: true, onTap: () => _cmd({'op': 'add_textfield'})),
        _gap,
        _Btn('Paste', enabled: _st['textfieldCopied'] == true, onTap: () => _cmd({'op': 'paste_textfield'})),
        _gap,
        _hint('A textfield sits on this frame only. Tap its box to select it, then drag to move it.'),
      ];
    }
    final align = {'left': 0, 'center': 1, 'right': 2}[tf['align'] as String?] ?? 1;
    final scale = (tf['scale'] as num?)?.toDouble() ?? 1;
    return [
      Text(tf['name'] as String? ?? 'Textfield', maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 13)),
      _gap,
      _TextFieldBox(text: tf['text'] as String? ?? '', onChanged: (v) => _cmd({'op': 'textfield', 'text': v})),
      const SizedBox(height: 4),
      // the words alone, between textfields
      _pair(
        _Btn('Copy text', small: true, onTap: () => setState(() => _textClip = tf['text'] as String? ?? '')),
        _Btn('Paste text', small: true, enabled: _textClip != null, onTap: () => _cmd({'op': 'textfield', 'text': _textClip})),
      ),
      _gap,
      ..._fontChoice(tf),
      _gap,
      _label('Alignment'),
      _Segmented(const ['Left', 'Center', 'Right'], align, (i) => _cmd({'op': 'textfield', 'align': i})),
      _gap,
      _label('Text colour'),
      _swatches((tf['color'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'textfield', 'color': c})),
      _gap,
      _stepper('Scale', scale.toStringAsFixed(2), () => _cmd({'op': 'textfield', 'scale': math.max(0.05, scale - 0.05)}), () => _cmd({'op': 'textfield', 'scale': scale + 0.05})),
      _gap,
      _Toggle('Locked', tf['locked'] == true, (v) {
        // the two locks exclude each other
        if (v && tf['cameraLocked'] == true) _cmd({'op': 'camera_lock', 'layer': tf['layer'], 'on': false});
        _cmd({'op': 'textfield', 'locked': v});
      }),
      _Toggle('Locked to camera', tf['cameraLocked'] == true, (v) {
        if (v && tf['locked'] == true) _cmd({'op': 'textfield', 'locked': false});
        _cmd({'op': 'camera_lock', 'layer': tf['layer'], 'on': v});
      }),
      _hint('Locked to camera: the text stays put in the shot however the camera moves (a title, a caption).'),
      _gap,
      _pair(_Btn('Copy', onTap: () => _cmd({'op': 'copy_textfield'})), _Btn('Paste', enabled: _st['textfieldCopied'] == true, onTap: () => _cmd({'op': 'paste_textfield'}))),
      _gap,
      _Btn('Add textfield', onTap: () => _cmd({'op': 'add_textfield'})),
      _gap,
      _Btn('Delete textfield', danger: true, onTap: () => _cmd({'op': 'delete_textfield'})),
      const Padding(padding: EdgeInsets.only(top: 3), child: Text('Removes it from this frame.', style: TextStyle(color: cSecondary, fontSize: 11))),
    ];
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
  List<Widget> _segmentLook(Map<String, dynamic> sel) {
    void seg(Map<String, Object?> m) => _cmd({'op': 'segment', ...m});
    final shape = sel['shape'] as String? ?? 'line';
    final grad = sel['gradient'] == true;
    final scale = (sel['segScale'] as num?)?.toDouble() ?? 1;
    final curve = (sel['curve'] as num?)?.toDouble() ?? 0;
    final sides = (sel['sides'] as num?)?.toInt() ?? 5;
    final right = (sel['rightTriangle'] as num?)?.toInt() ?? 0;
    final half = (sel['halfArc'] as num?)?.toInt() ?? 0;
    final lock = (sel['angleLock'] as num?)?.toInt() ?? 0;
    return [
      _Toggle('Use gradient colour', grad, (v) => seg({'gradient': v})),
      if (grad) ...[
        _swatches((sel['gradientColor'] as num?)?.toInt() ?? 0x808080ff, (c) => seg({'gradientColor': c})),
        _Toggle('Reverse gradient', sel['gradientReverse'] == true, (v) => seg({'gradientReverse': v})),
        _Toggle('Sideways gradient', sel['gradientSideways'] == true, (v) => seg({'gradientSideways': v})),
      ],
      _gap,
      _Toggle('Use segment scale', (scale - 1).abs() > 1e-3, (v) => seg({'segScale': v ? 1.5 : 1.0})),
      if ((scale - 1).abs() > 1e-3) _stepper('Segment scale', scale.toStringAsFixed(2), () => seg({'segScale': math.max(0.05, scale - 0.05)}), () => seg({'segScale': scale + 0.05})),
      _gap,
      if (shape == 'line' || shape == 'square' || shape == 'trapezoid')
        _stepper('Curve radius', curve.round().toString(), () => seg({'curve': curve - 5}), () => seg({'curve': curve + 5})),
      if (shape == 'trapezoid') ...[
        _stepper('End thickness', '${(((sel['trapRatio'] as num?) ?? 1) * 100).round()}%', () => seg({'trapRatio': math.max(0, ((sel['trapRatio'] as num?) ?? 1) - 0.1)}), () => seg({'trapRatio': ((sel['trapRatio'] as num?) ?? 1) + 0.1})),
        _Toggle('Start rounded', sel['trapRoundStart'] == true, (v) => seg({'trapRoundStart': v})),
        _Toggle('End rounded', sel['trapRoundEnd'] == true, (v) => seg({'trapRoundEnd': v})),
      ],
      if (shape == 'circle' || shape == 'ellipse') ...[
        _Toggle('Half arc', half != 0, (v) => seg({'halfArc': v ? 1 : 0})),
        if (half != 0) _Toggle('Upside down', half < 0, (v) => seg({'halfArc': v ? -1 : 1})),
      ],
      if (shape == 'circle') ...[
        _Toggle('Circle outline colour', sel['outline'] == true, (v) => seg({'outline': v})),
        if (sel['outline'] == true) ...[
          _swatches((sel['outlineColor'] as num?)?.toInt() ?? 0x000000ff, (c) => seg({'outlineColor': c})),
          _Toggle('Is hollow', sel['hollow'] == true, (v) => seg({'hollow': v})),
        ],
      ],
      if (shape == 'triangle') ...[
        _Toggle('Right triangle', right != 0, (v) => seg({'rightTriangle': v ? 1 : 0})),
        if (right != 0) _Toggle('Flipped', right < 0, (v) => seg({'rightTriangle': v ? -1 : 1})),
        _Toggle('Upside down', sel['upsideDown'] == true, (v) => seg({'upsideDown': v})),
      ],
      if (shape == 'polygon') _stepper('Vertices', '$sides', () => seg({'sides': math.max(3, sides - 1)}), () => seg({'sides': sides + 1})),
      _gap,
      _label('Angle lock'),
      _Segmented(const ['None', 'Absolute', 'Relative'], lock, (i) => seg({'angleLock': i})),
      _gap,
    ];
  }

  List<Widget> _segmentTools(Map<String, dynamic> sel) {
    final root = sel['root'] == true;
    if (root) {
      return [
        const Text('The main node moves the whole figure. Tap a joint for its segment.', style: TextStyle(color: cSecondary, fontSize: 12)),
        _gap,
        _Toggle('Drag lock', sel['dragLock'] == true, (v) => _cmd({'op': 'node', 'dragLock': v})),
      ];
    }
    final thick = (sel['thickness'] as num?)?.toDouble() ?? 0;
    final len = (sel['length'] as num?)?.toDouble() ?? 0;
    // the length the segment was made with
    final rest = (sel['restLength'] as num?)?.toDouble() ?? 0;
    return [
      _pair(_Btn('Flip sideways', onTap: () => _cmd({'op': 'flip_branch', 'h': true})), _Btn('Flip over', onTap: () => _cmd({'op': 'flip_branch', 'h': false}))),
      _gap,
      _Toggle('Use segment colour', sel['ownColor'] == true, (v) => _cmd({'op': 'segment', 'ownColor': v})),
      _hint('On: this segment keeps its own colour. Off: it follows the stickfigure colour.'),
      Opacity(
        opacity: sel['ownColor'] == true ? 1 : 0.4,
        child: _swatches((sel['color'] as num?)?.toInt() ?? 0xff, (c) => _cmd({'op': 'segment', 'color': c})),
      ),
      _gap,
      _stepper('Thickness', thick.round().toString(), () => _cmd({'op': 'segment', 'thickness': math.max(0, thick - 2)}), () => _cmd({'op': 'segment', 'thickness': thick + 2})),
      _Btn('Reset thickness', small: true, enabled: thick != 32, onTap: () => _cmd({'op': 'segment', 'thickness': 32})),
      _gap,
      _stepper('Length', len.round().toString(), () => _cmd({'op': 'node', 'length': math.max(0, len - 5)}), () => _cmd({'op': 'node', 'length': len + 5})),
      _Btn('Reset length', small: true, enabled: rest > 0 && (len - rest).abs() > 0.01, onTap: () => _cmd({'op': 'node', 'length': rest})),
      _gap,
      ..._segmentLook(sel),
      _Toggle('Is stretchy', sel['stretchy'] == true, (v) => _cmd({'op': 'node', 'stretchy': v})),
      _Toggle('Lock node', sel['pinned'] == true, (v) => _cmd({'op': 'pin', 'v': v})),
      _Toggle('Drag lock', sel['dragLock'] == true, (v) => _cmd({'op': 'node', 'dragLock': v})),
    ];
  }

  (double, double)? _camClip;

  List<Widget> _cameraTools() {
    final cam = _st['cam'] as Map<String, dynamic>?;
    final zoom = (cam?['zoom'] as num?)?.toDouble() ?? 1;
    final rot = (cam?['rotation'] as num?)?.toDouble() ?? 0;
    return [
      const Text('Drag on the stage to move the camera on this frame.', style: TextStyle(color: cSecondary, fontSize: 12)),
      _gap,
      // this frame's zoom and rotation onto another frame
      _pair(
        _Btn('Copy camera', onTap: () => setState(() => _camClip = (zoom, rot))),
        _Btn('Paste camera', enabled: _camClip != null, onTap: () => _cmd({'op': 'camera', 'zoom': _camClip!.$1, 'rotation': _camClip!.$2})),
      ),
      _gap,
      _stepper('Zoom', zoom.toStringAsFixed(2), () => _cmd({'op': 'camera', 'zoom': math.max(0.05, zoom - 0.05)}), () => _cmd({'op': 'camera', 'zoom': zoom + 0.05})),
      _gap,
      _stepper('Rotation', '${rot.round()}°', () => _cmd({'op': 'camera', 'rotation': rot - 5}), () => _cmd({'op': 'camera', 'rotation': rot + 5})),
      _gap,
      _pair(_Btn('Center camera', onTap: () => _cmd({'op': 'camera', 'center': true})), _Btn('Reset', onTap: () => _cmd({'op': 'reset_camera'}))),
      _gap,
      _hint("Use auto-camera to have this frame's camera tween across multiple frames automatically."),
      _Btn(_st['autoCam'] != null ? 'Auto-camera… (to frame ${((_st['autoCam'] as Map)['end'] as num) + 1})' : 'Auto-camera…', onTap: _autoCameraDialog),
      _gap,
      _Btn('Lock figure to camera…', onTap: _cameraLockDialog),
      _gap,
      _Btn('Mode: ${const ['Default', 'Rule of thirds', 'Phi grid', 'Fib spiral 1', 'Fib spiral 2', 'Fib spiral 3', 'Fib spiral 4'][AppSettings.number('cameraGuide').clamp(0, 6)]}', onTap: () => setState(() {
            AppSettings.values['cameraGuide'] = (AppSettings.number('cameraGuide') + 1) % 7;
            unawaited(AppSettings.save());
          })),
      _gap,
      ..._cameraFx(),
    ];
  }

  /// Movieclip Tools: the selected movieclip's playback.
  /// Movieclip Tools as rows (D15; REDESIGN §8 Movieclip page).
  List<Widget> _movieclipTools(Map mc) {
    final speed = (mc['speed'] as num?)?.toDouble() ?? 1, off = (mc['offset'] as num?)?.toDouble() ?? 0;
    const modes = ['once', 'loop', 'pingpong'];
    final frames = (mc['frames'] as num?)?.toInt() ?? 0;
    final rows = <rw.Desc>[
      rw.DName('${mc['name']}', link: 'Edit movieclip ›', onLink: () => _cmd({'op': 'edit_movieclip'})),
      rw.DInfo('Length', '$frames frame${frames == 1 ? '' : 's'}'),
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
    return [
      _Toggle('Widescreen effect', fx['widescreen'] == true, (v) => fxCmd({'widescreen': v})),
      _gap,
      _Toggle('Wobble camera XY', xy > 0, (v) => fxCmd({'wobbleXY': v ? 8 : 0})),
      if (xy > 0) _stepper('Amount', xy.round().toString(), () => fxCmd({'wobbleXY': math.max(1, xy - 1)}), () => fxCmd({'wobbleXY': xy + 1})),
      _Toggle('Wobble camera rotation', rot > 0, (v) => fxCmd({'wobbleRot': v ? 0.5 : 0})),
      if (rot > 0) _stepper('Degrees', rot.toStringAsFixed(2), () => fxCmd({'wobbleRot': math.max(0.1, rot - 0.1)}), () => fxCmd({'wobbleRot': rot + 0.1})),
      Opacity(
        opacity: any ? 1 : 0.4,
        child: IgnorePointer(
          ignoring: !any,
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            _stepper('Wobble speed', speed.toStringAsFixed(2), () => fxCmd({'wobbleSpeed': math.max(0.1, speed - 0.5)}), () => fxCmd({'wobbleSpeed': speed + 0.5})),
            _Toggle('Scale wobble with zoom', zoom, (v) => fxCmd({'wobbleZoom': v})),
          ]),
        ),
      ),
      _pair(
        _Btn('Copy wobble', onTap: () => setState(() => _wobbleClip = (xy, rot, speed, zoom))),
        _Btn('Paste wobble', enabled: _wobbleClip != null, onTap: () => fxCmd({'wobbleXY': _wobbleClip!.$1, 'wobbleRot': _wobbleClip!.$2, 'wobbleSpeed': _wobbleClip!.$3, 'wobbleZoom': _wobbleClip!.$4})),
      ),
      _hint('Widescreen and wobble belong to this frame and show when playing and in exports, not while editing.'),
    ];
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
  Widget _swatches(int current, void Function(int) pick) {
    final hex = '#${(current >> 8).toRadixString(16).padLeft(6, '0').toUpperCase()}';
    return GestureDetector(
      onTap: () => _colorPicker(current, pick),
      child: Row(children: [
        Container(
          width: 56,
          height: 28,
          decoration: BoxDecoration(color: _rgba(current), borderRadius: BorderRadius.circular(4), border: Border.all(color: cSelected)),
        ),
        const SizedBox(width: 8),
        Text(hex, style: const TextStyle(color: cSecondary, fontSize: 12, fontFeatures: [ui.FontFeature.tabularFigures()])),
        const Spacer(),
        const Icon(Icons.palette_outlined, size: 18, color: cSecondary),
      ]),
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

  /// Open: a window listing saved animations (filter, sort, open, delete).
  Future<void> _openDialog() async {
    if (!await _leaveUnsaved()) return;
    final files = await ProjectFile.all();
    final past = await ProjectFile.pastSaves();
    if (!mounted) return;
    final chosen = await showDialog<ProjectFile>(context: context, builder: (c) => _OpenDialog(files, _file?.path, _ask, past));
    if (chosen != null) await _openFile(chosen);
  }

  final Map<String, ui.Image> _previews = {};



  /// "Add a figure": a window over the editor with the user's Stick Nodes
  /// figures (Barkfart Figures folder) and the built-in ones, with pictures.
  Future<void> _figurePicker() async {
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
    final picked = await showDialog<List<String>>(context: context, builder: (c) => _FigurePicker(entries, _previews));
    for (final k in picked ?? const <String>[]) {
      if (k.startsWith('file:')) {
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
    if (cam != null && cam.length == 8 && AppSettings.flag('stageBox')) {
      final frame = Path()..moveTo(v.toScreen(cam[0], cam[1]).dx, v.toScreen(cam[0], cam[1]).dy);
      for (var i = 2; i < 8; i += 2) {
        final p = v.toScreen(cam[i], cam[i + 1]);
        frame.lineTo(p.dx, p.dy);
      }
      frame.close();
      // offstage is the checkerboard (REDESIGN §3); slightly see-through so
      // figures standing offstage still show
      final outside = Path.combine(PathOperation.difference, Path()..addRect(Offset.zero & size), frame);
      canvas.drawPath(outside, Paint()..shader = _checker());
      canvas.drawPath(frame, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = const Color(0x66000000));
    }
    // Camera Tools "Mode": composition guides inside the shot (camera mode only)
    if (cam != null && cam.length == 8 && st['mode'] == 'camera') _guides(canvas, cam, AppSettings.number('cameraGuide'));
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
    final hs = (st['handles'] as List?) ?? const [];
    // selected joint red, the joints connected to it purple with a line to
    // each, everything else blue rings (after the original's look)
    const purple = Color(0xFFA43BE0), red = Color(0xFFE0383A), blue = Color(0xFF2F6BE8);
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
      final color = sel ? red : nb ? purple : (root ? cRoot : blue);
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
      if (root || style == 0) {
        final r = Rect.fromCenter(center: p, width: (root ? 10 : 8) * k, height: (root ? 10 : 8) * k);
        canvas.drawRect(r, halo);
        canvas.drawRect(r, stroke);
      } else {
        canvas.drawCircle(p, 5.5 * k, halo);
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
    switch (mode) {
      case 1 || 2:
        final a = mode == 1 ? 1 / 3 : 0.382;
        for (final k in [a, 1 - a]) {
          line(k, 0, k, 1, const Color(0xCC2F6BE8));
          line(0, k, 1, k, const Color(0xCCE0383A));
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
  const _TextFieldBox({required this.text, required this.onChanged});

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
      maxLines: 4,
      minLines: 2,
      style: const TextStyle(color: cText, fontSize: 13),
      decoration: const InputDecoration(
        hintText: 'Text',
        hintStyle: TextStyle(color: cSecondary),
        filled: true,
        fillColor: cControl,
        isDense: true,
        border: OutlineInputBorder(borderSide: BorderSide(color: cSelected)),
        contentPadding: EdgeInsets.all(8),
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

class _FileRow extends StatelessWidget {
  final String name, when;
  final bool current;
  final VoidCallback onTap;
  const _FileRow(this.name, this.when, this.current, this.onTap);

  @override
  Widget build(BuildContext context) {
    return Material(
      color: current ? cSelected : cControl,
      borderRadius: BorderRadius.circular(4),
      child: InkWell(
        borderRadius: BorderRadius.circular(4),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(name, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 13)),
            Text(when, style: const TextStyle(color: cSecondary, fontSize: 11)),
          ]),
        ),
      ),
    );
  }
}

class _PickEntry {
  final String id, name;
  final bool mine, effect;
  const _PickEntry(this.id, this.name, this.mine, {this.effect = false});
}

class _FigurePicker extends StatefulWidget {
  final List<_PickEntry> entries;
  final Map<String, ui.Image> previews;
  const _FigurePicker(this.entries, this.previews);

  @override
  State<_FigurePicker> createState() => _FigurePickerState();
}

class _FigurePickerState extends State<_FigurePicker> {
  final List<String> _picked = [];
  String _filter = '';
  int _tab = 0; // Stock, Yours, Effects

  Widget _tile(_PickEntry f) {
    final on = _picked.contains(f.id);
    final img = widget.previews[f.id];
    return GestureDetector(
      onTap: () => setState(() => on ? _picked.remove(f.id) : _picked.add(f.id)),
      onDoubleTap: () => Navigator.pop(context, [f.id]),
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
    final lists = [match.where((f) => !f.mine && !f.effect).toList(), match.where((f) => f.mine).toList(), match.where((f) => f.effect).toList()];
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
            const Text('Add a stick figure', textAlign: TextAlign.center, style: TextStyle(color: cTitle, fontSize: 17, fontWeight: FontWeight.w700)),
            const SizedBox(height: 10),
            Row(children: [
              SizedBox(width: 300, child: kit.Segmented(const ['Stock', 'Yours', 'Effects'], _tab, onSelect: (i) => setState(() => _tab = i))),
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
                      hintText: 'Search',
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
              child: Row(children: [
                Text(_picked.isEmpty ? 'Tap figures to pick them; double-tap adds one.' : '${_picked.length} picked', style: const TextStyle(color: cSecondary, fontSize: 12)),
                const Spacer(),
                SizedBox(width: 110, child: kit.WordButton('Cancel', filled: true, expand: true, height: 38, onTap: () => Navigator.pop(context))),
                const SizedBox(width: 8),
                SizedBox(
                  width: 120,
                  child: kit.WordButton(_picked.isEmpty ? 'Add' : 'Add (${_picked.length})', primary: true, expand: true, height: 38, onTap: _picked.isEmpty ? null : () => Navigator.pop(context, List<String>.from(_picked))),
                ),
              ]),
            ),
          ]),
        ),
      ),
    );
  }
}

/// Open: saved animations with a filter and sort, Open / Delete / Cancel.
class _OpenDialog extends StatefulWidget {
  final List<ProjectFile> files;
  final String? current;
  final Future<String?> Function(String, String, List<String>) ask;
  final List<ProjectFile> past;
  const _OpenDialog(this.files, this.current, this.ask, this.past);

  @override
  State<_OpenDialog> createState() => _OpenDialogState();
}

class _OpenDialogState extends State<_OpenDialog> {
  late List<ProjectFile> _files = List.of(widget.files);
  bool _past = false;
  String _filter = '';
  int _sort = 0;
  ProjectFile? _picked;

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    final list = _files.where((f) => f.name.toLowerCase().contains(_filter.toLowerCase())).toList();
    if (_sort == 0) {
      list.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
    } else {
      list.sort((a, b) => File(b.path).lastModifiedSync().compareTo(File(a.path).lastModifiedSync()) * (_sort == 1 ? 1 : -1));
    }
    return Dialog(
      backgroundColor: cPanel,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      child: SizedBox(
        width: math.min(560, size.width * 0.8),
        height: math.min(520, size.height * 0.85),
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            Row(children: [
              Expanded(child: Text(_past ? 'Past saves' : 'Open an animation', style: const TextStyle(color: cText, fontSize: 17, fontWeight: FontWeight.w600))),
              // every save keeps the version it replaced
              SizedBox(
                width: 150,
                child: _Btn(_past ? '‹ Animations' : 'Past saves ›', onTap: () => setState(() {
                      _past = !_past;
                      _files = List.of(_past ? widget.past : widget.files);
                      _picked = null;
                    })),
              ),
            ]),
            if (_past) _hint('Earlier versions, kept each time an animation was saved over. One opens as an unsaved copy.'),
            const SizedBox(height: 10),
            Row(children: [
              Expanded(
                child: TextField(
                  style: const TextStyle(color: cText),
                  decoration: const InputDecoration(hintText: 'Filter', isDense: true, prefixIcon: Icon(Icons.search, size: 18)),
                  onChanged: (v) => setState(() => _filter = v),
                ),
              ),
              const SizedBox(width: 10),
              SizedBox(width: 210, child: _Segmented(const ['A–Z', 'Newest', 'Oldest'], _sort, (i) => setState(() => _sort = i))),
            ]),
            const SizedBox(height: 10),
            Expanded(
              child: list.isEmpty
                  ? Center(child: Text(_past ? 'No past saves yet.' : 'Nothing saved yet.', style: const TextStyle(color: cSecondary)))
                  : ListView(children: [
                      for (final f in list)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 6),
                          child: _FileRow(f.name, f.when + (f.path == widget.current ? '  ·  open now' : ''), identical(f, _picked), () => setState(() => _picked = f)),
                        ),
                    ]),
            ),
            const SizedBox(height: 10),
            Row(children: [
              SizedBox(
                width: 150,
                child: _Btn('Delete from device', danger: true, enabled: _picked != null, onTap: () async {
                  final f = _picked!;
                  final ok = await widget.ask('Delete "${f.name}"?', 'The file is removed from this device. This cannot be undone.', ['Cancel', 'Delete']);
                  if (ok == 'Delete') {
                    await File(f.path).delete();
                    setState(() {
                      _files.remove(f);
                      _picked = null;
                    });
                  }
                }),
              ),
              const Spacer(),
              SizedBox(width: 100, child: _Btn('Cancel', onTap: () => Navigator.pop(context))),
              const SizedBox(width: 8),
              SizedBox(width: 100, child: _Btn('Open', accent: true, enabled: _picked != null, onTap: () => Navigator.pop(context, _picked))),
            ]),
          ]),
        ),
      ),
    );
  }
}

class _ExportDialog extends StatefulWidget {
  final double w, h;
  final int frames;
  final String name;
  const _ExportDialog(this.w, this.h, this.frames, this.name);

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

  static const _widths = [320, 640, 1280, 1920];

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
            kit.Hint(_format == 2
                ? 'A video (H.264, with the sound) that Photos and video players open. Videos can’t be transparent.'
                : _format == 1
                    ? '$count pictures, with the soundtrack as a WAV when there is sound.'
                    : 'A GIF, with the soundtrack as a WAV when there is sound. Saved in the Barkfart Exports folder.'),
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
