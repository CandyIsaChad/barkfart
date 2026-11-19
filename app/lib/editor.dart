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

import 'engine.dart';
import 'files.dart';

// Colours (UI-SPEC "Visual language").
const cPanel = Color(0xFF1E2024);
const cControl = Color(0xFF2B2E33);
const cPressed = Color(0xFF363A40);
const cSelected = Color(0xFF40454C);
const cSurround = Color(0xFF17191C);
const cText = Color(0xFFE6E6E6);
const cSecondary = Color(0xFF9AA0A6);
const cAccent = Color(0xFF0A84FF);
const cDanger = Color(0xFFFF453A);
const cRoot = Color(0xFFFF9F0A);

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
  Timer? _statusTimer;

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
    _ticker = createTicker((_) => _tick())..start();
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
        _say('Restored unsaved changes');
      } else if (last != null && e.load(await last.read()) == null) {
        _file = last;
      } else {
        e.newProject();
      }
    }
    _refresh(e.command({'op': 'state'}));
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
    return st;
  }

  void _say(String s) {
    _statusTimer?.cancel();
    setState(() => _status = s);
    _statusTimer = Timer(const Duration(seconds: 4), () {
      if (mounted) setState(() => _status = '');
    });
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
      final dur = _frames / _fps;
      final secs = e.playbackTime ?? _clockOffset + _clock.elapsedMicroseconds / 1e6;
      t = dur > 0 ? secs % dur : 0;
      // draw on frame boundaries only (the engine draws one picture per frame)
      t = (t * _fps).floor() / _fps;
      if (t != _playT) setState(() => _playT = t);
    } else {
      t = _frame / _fps;
    }
    final scale = math.min(_dpr, 1600 / _view.size.longestSide) * (_playing ? _quality : 1.0);
    final w = (_view.size.width * scale).round(), h = (_view.size.height * scale).round();
    if (w <= 0 || h <= 0) return;
    final camera = _playing;
    final onion = !_playing && _opts['onion'] == true && _frames > 1;
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
    final key = '${_st['rev']} $t $w $h $x0 $y0 $x1 $y1 $camera $onion';
    if (key == _drawn) return;
    final fi = (t * _fps).round();
    if (camera) {
      final ck = '${_st['rev']} $w $h';
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
    final px = e.renderView(t, w, h, x0, y0, x1, y1, camera: camera, onion: onion);
    if (px == null) {
      _busy = false;
      return;
    }
    final ms = e.lastMicros / 1000.0;
    ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, (img) {
      if (!mounted) return;
      final keep = camera && _cacheBytes + w * h * 4 <= _cacheBudget && '${_st['rev']} $w $h' == _cacheKey;
      if (keep) {
        _frameCache[fi] = img;
        _cacheBytes += w * h * 4;
      }
      setState(() {
        _setImage(img, owned: !keep);
        _engineMs = _engineMs == 0 ? ms : _engineMs * 0.9 + ms * 0.1;
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

  double get _hitRadius => 22 / _view.s;

  void _down(PointerDownEvent ev) {
    if (_playing) {
      _togglePlay();
      return;
    }
    _pointers[ev.pointer] = ev.localPosition;
    if (_pointers.length == 1) {
      _downAt = ev.localPosition;
      _moved = false;
      final p = _view.toStage(ev.localPosition);
      final st = _cmd({'op': 'down', 'x': p.dx, 'y': p.dy, 'r': _hitRadius});
      if (st?['hit'] == true) {
        _oneFinger = 1;
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

  void _up(PointerEvent ev) {
    if (!_pointers.containsKey(ev.pointer)) return;
    _pointers.remove(ev.pointer);
    if (_pointers.isEmpty) {
      if (_oneFinger == 1) _cmd({'op': ev is PointerCancelEvent ? 'cancel' : 'up'});
      if (_oneFinger == 2 && !_moved) {
        final now = DateTime.now();
        if (now.difference(_lastTapUp).inMilliseconds < 300) {
          setState(_fit);
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

  void _togglePlay() {
    final e = _engine;
    if (e == null) return;
    setState(() {
      if (_playing) {
        _playing = false;
        e.stop();
        _clock.stop();
      } else {
        _playing = true;
        final start = _frame / _fps;
        _clock
          ..reset()
          ..start();
        // the clock starts at the current frame
        _clockOffset = start;
        e.play(start, loopStart: 0, loopEnd: _frames / _fps);
      }
    });
  }

  double _clockOffset = 0;

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
    _file = f;
    await Recovery.clear();
    _refresh(e.command({'op': 'state'}));
    if (!quiet) _say('Saved ${f.name}');
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
    _engine?.newProject();
    _file = null;
    _fitted = false;
    _refresh(_engine?.command({'op': 'state'}));
    _say('New animation');
  }

  Future<void> _openSheet() async {
    if (!await _leaveUnsaved()) return;
    final files = await ProjectFile.all();
    if (!mounted) return;
    final chosen = await showModalBottomSheet<ProjectFile>(
      context: context,
      backgroundColor: cPanel,
      builder: (c) => ListView(children: [
        const Padding(padding: EdgeInsets.all(16), child: Text('Animations', style: TextStyle(color: cText, fontSize: 17, fontWeight: FontWeight.w600))),
        if (files.isEmpty) const Padding(padding: EdgeInsets.all(16), child: Text('Nothing saved yet.', style: TextStyle(color: cSecondary))),
        for (final f in files)
          ListTile(
            title: Text(f.name, style: const TextStyle(color: cText)),
            subtitle: Text(f.when, style: const TextStyle(color: cSecondary)),
            selected: f.path == _file?.path,
            onTap: () => Navigator.pop(c, f),
          ),
      ]),
    );
    if (chosen == null) return;
    final err = _engine?.load(await chosen.read());
    if (err != null) {
      _say('Could not open ${chosen.name}: $err');
      return;
    }
    _file = chosen;
    _fitted = false;
    _refresh(_engine?.command({'op': 'state'}));
  }

  ExportJob? _export;

  Future<void> _exportGif() async {
    final e = _engine;
    if (e == null || _export != null) return;
    final dir = await exportsDir();
    final name = (_file?.name ?? 'Animation').replaceAll(RegExp(r'[^A-Za-z0-9 _-]'), '');
    final stamp = DateTime.now().toIso8601String().replaceAll(':', '-').split('.').first;
    final sw = (_st['w'] as num).toDouble(), sh = (_st['h'] as num).toDouble();
    final w = 480, h = (480 * sh / sw).round();
    final job = e.export(0, '${dir.path}/$name $stamp.gif', w, h);
    if (job == null) {
      _say('Export failed: ${e.lastError}');
      return;
    }
    e.exportSoundtrack('${dir.path}/$name $stamp.wav');
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
            _say('Exported to Barkfart Exports (Files app)');
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
        case 'savebutton':
          unawaited(_save());
        case 'picker':
          unawaited(_figurePicker());
        case 'back':
          if (mounted) Navigator.of(context).maybePop();
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

  @override
  Widget build(BuildContext context) {
    _dpr = MediaQuery.devicePixelRatioOf(context);
    if (_fatal.isNotEmpty) {
      return Scaffold(backgroundColor: cSurround, body: Center(child: Text(_fatal, style: const TextStyle(color: cText))));
    }
    final big = MediaQuery.sizeOf(context).shortestSide > 600;
    final pad = MediaQuery.paddingOf(context);
    return Scaffold(
      backgroundColor: cSurround,
      body: RepaintBoundary(
        key: _shot,
        child: Row(children: [
          Container(
            width: (big ? 200 : 168) + pad.left,
            color: cPanel,
            padding: EdgeInsets.only(left: pad.left, top: pad.top, bottom: pad.bottom),
            child: _sidebar(),
          ),
          Expanded(
            child: Column(children: [
              Container(
                height: (big ? 76 : 64) + pad.top,
                color: cPanel,
                padding: EdgeInsets.only(top: pad.top, right: pad.right),
                child: _strip(big),
              ),
              Expanded(child: Padding(padding: EdgeInsets.only(right: pad.right, bottom: pad.bottom), child: _stage())),
            ]),
          ),
        ]),
      ),
    );
  }

  Widget _stage() {
    return LayoutBuilder(builder: (context, c) {
      if (_view.size != c.biggest) {
        _view.size = c.biggest;
        if (!_fitted) _fit();
      }
      return Listener(
        onPointerDown: _down,
        onPointerMove: _move,
        onPointerUp: _up,
        onPointerCancel: _up,
        onPointerSignal: _scroll,
        child: ClipRect(
          child: Stack(fit: StackFit.expand, children: [
            if (_image != null) RawImage(image: _image, fit: BoxFit.fill, filterQuality: FilterQuality.low),
            CustomPaint(painter: _Overlay(_st, _view, _playing)),
            if (_st['mode'] == 'camera') IgnorePointer(child: Container(decoration: BoxDecoration(border: Border.all(color: cAccent, width: 2)))),
          ]),
        ),
      );
    });
  }

  Widget _strip(bool big) {
    final building = _st['building'] as Map<String, dynamic>?;
    if (building != null) {
      return Row(children: [
        const SizedBox(width: 12),
        Expanded(
          child: Column(mainAxisAlignment: MainAxisAlignment.center, crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text('Editing ${building['name']}', style: const TextStyle(color: cText, fontSize: 15, fontWeight: FontWeight.w600)),
            const Text('Segment changes apply to this figure on every frame.', style: TextStyle(color: cSecondary, fontSize: 11)),
          ]),
        ),
        SizedBox(width: 96, child: _Btn('Done', accent: true, onTap: () => _cmd({'op': 'build_done'}))),
        const SizedBox(width: 8),
      ]);
    }
    final cell = big ? 64.0 : 52.0;
    final keyed = (_st['keyed'] as List?) ?? const [];
    return Row(children: [
      Expanded(
        child: ListView.builder(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 6),
          itemCount: _frames,
          itemBuilder: (c, i) {
            final cur = _playing ? (_playT * _fps).round() == i : i == _frame;
            final key = i == 0 || (i < keyed.length && keyed[i] == true);
            return GestureDetector(
              onTap: () {
                if (_playing) _togglePlay();
                _cmd({'op': 'frame', 'i': i});
              },
              child: Container(
                width: key ? cell : cell * 0.6,
                margin: const EdgeInsets.only(right: 4),
                decoration: BoxDecoration(
                  color: cur ? cSelected : (key ? cControl : const Color(0xFF24272B)),
                  border: Border.all(color: cur ? cAccent : const Color(0x00000000), width: 2),
                ),
                child: Stack(children: [
                  Positioned(left: 5, top: 3, child: Text('${i + 1}', style: TextStyle(color: cur ? cText : (key ? cSecondary : const Color(0xFF5F646B)), fontSize: 12, fontFeatures: const [ui.FontFeature.tabularFigures()]))),
                  if (key)
                    Positioned(left: 0, right: 0, bottom: 6, child: Center(child: Transform.rotate(angle: math.pi / 4, child: Container(width: 7, height: 7, color: cur ? cAccent : cSecondary))))
                  else
                    const Positioned(left: 0, right: 0, bottom: 4, child: Center(child: Text('~', style: TextStyle(color: Color(0xFF5F646B), fontSize: 12)))),
                ]),
              ),
            );
          },
        ),
      ),
      _iconButton(Icons.add, 'Add frame', () => _cmd({'op': 'insert_frame'})),
      const SizedBox(width: 4),
      _iconButton(_playing ? Icons.stop : Icons.play_arrow, _playing ? 'Stop' : 'Play', _togglePlay, accent: true),
      const SizedBox(width: 8),
    ]);
  }

  Widget _iconButton(IconData icon, String tip, VoidCallback onTap, {bool accent = false}) {
    return Tooltip(
      message: tip,
      child: Material(
        color: accent ? cAccent : cControl,
        borderRadius: BorderRadius.circular(4),
        child: InkWell(
          borderRadius: BorderRadius.circular(4),
          onTap: onTap,
          child: SizedBox(width: 44, height: 44, child: Icon(icon, color: cText)),
        ),
      ),
    );
  }

  // ---------------------------------------------------------------- sidebar

  Widget _sidebar() {
    final sel = _sel;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(10, 10, 10, 6),
        child: _pair(
          _Btn('Undo', enabled: _st['canUndo'] == true, onTap: () => _cmd({'op': 'undo'})),
          _Btn('Redo', enabled: _st['canRedo'] == true, onTap: () => _cmd({'op': 'redo'})),
        ),
      ),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10),
        child: _Segmented(
          ['Figure', 'All', 'Camera'],
          ['figure', 'all', 'camera'].indexOf(_st['mode'] as String? ?? 'figure'),
          (i) {
            if (i == 2 && _st['hasCamera'] != true) {
              _cmd({'op': 'add_camera'});
            } else {
              _cmd({'op': 'mode', 'v': ['figure', 'all', 'camera'][i]});
            }
          },
        ),
      ),
      const SizedBox(height: 6),
      Expanded(
        child: ListView(padding: const EdgeInsets.fromLTRB(10, 4, 10, 10), children: _st['building'] != null ? _builder(sel) : (sel == null ? _nothingSelected() : _selected(sel))),
      ),
      Container(
        color: cControl,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
        child: Text(
          _status.isNotEmpty ? _status : 'Frame ${(_playing ? (_playT * _fps).round() : _frame) + 1} / $_frames · ${(_frame / _fps).toStringAsFixed(2)} s${_st['dirty'] == true ? ' · not saved' : ''}',
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(color: cSecondary, fontSize: 11, fontFeatures: [ui.FontFeature.tabularFigures()]),
        ),
      ),
    ]);
  }

  List<Widget> _nothingSelected() {
    final onion = _opts['onion'] == true;
    return [
      _title('Animation'),
      _Btn('Add figure…', accent: true, onTap: _figurePicker),
      _gap,
      _Toggle('Onion skin', onion, (v) => _cmd({'op': 'set', 'k': 'onion', 'v': v})),
      _gap,
      _stepper('FPS', _fps.round().toString(), () => _cmd({'op': 'fps', 'v': _fps - 1}), () => _cmd({'op': 'fps', 'v': _fps + 1})),
      ..._frameTools(),
      _title('Camera'),
      if (_st['hasCamera'] != true) _Btn('Add camera', onTap: () => _cmd({'op': 'add_camera'})) else _Btn('Reset camera', onTap: () => _cmd({'op': 'reset_camera'})),
      _title('View'),
      _Toggle('Joints of other figures', _opts['mainOnly'] != true, (v) => _cmd({'op': 'set', 'k': 'mainonly', 'v': !v})),
      _gap,
      _label('Playback quality'),
      _Segmented(const ['Full', 'Half', 'Fast'], _quality == 1 ? 0 : _quality == 0.5 ? 1 : 2, (i) => setState(() {
            _quality = [1.0, 0.5, 0.3][i];
            _clearCache();
          })),
      _title('File'),
      _pair(_Btn('New', onTap: _newProject), _Btn('Open', onTap: _openSheet)),
      _gap,
      _pair(_Btn('Save…', onTap: _save), _Btn('Export GIF', onTap: _exportGif)),
      _gap,
      Text(_file?.name ?? 'Not saved yet', style: const TextStyle(color: cSecondary, fontSize: 11)),
      if (_engineMs > 0) Text('${_engineMs.toStringAsFixed(1)} ms per picture', style: const TextStyle(color: cSecondary, fontSize: 11)),
    ];
  }

  /// Frame tools: frames, in-betweens, easing (also under a selected figure).
  List<Widget> _frameTools() {
    return [
      _title('Frame ${_frame + 1}'),
      _pair(_Btn('New frame', onTap: () => _cmd({'op': 'insert_frame'})), _Btn('Delete', danger: true, enabled: _frames > 1, onTap: () => _cmd({'op': 'delete_frame'}))),
      _gap,
      _label('Add in-betweens after this frame'),
      Row(children: [
        for (final n in const [1, 2, 3, 5]) ...[
          Expanded(child: _Btn('+$n', onTap: () => _cmd({'op': 'inbetween', 'n': n}))),
          if (n != 5) const SizedBox(width: 4),
        ],
      ]),
      _gap,
      _label('Easing to the next key frame'),
      Wrap(spacing: 4, runSpacing: 4, children: [
        for (final e in const [('linear', 'Even'), ('smooth', 'Smooth'), ('in', 'Speed up'), ('out', 'Slow down'), ('hold', 'No tween')])
          _Chip(e.$2, false, () => _cmd({'op': 'easing', 'v': e.$1})),
      ]),
    ];
  }

  Future<void> _figurePicker() async {
    final list = ((_st['builtins'] as List?) ?? const []).cast<Map<String, dynamic>>();
    final kind = await showModalBottomSheet<String>(
      context: context,
      backgroundColor: cPanel,
      builder: (c) => ListView(children: [
        const Padding(padding: EdgeInsets.all(16), child: Text('Add a figure', style: TextStyle(color: cText, fontSize: 17, fontWeight: FontWeight.w600))),
        for (final f in list)
          ListTile(
            title: Text(f['name'] as String, style: const TextStyle(color: cText)),
            onTap: () => Navigator.pop(c, f['id'] as String),
          ),
      ]),
    );
    if (kind != null) _cmd({'op': 'add_figure', 'kind': kind});
  }

  /// A figure is selected: posing and figure tools only (segments live in the figure editor).
  List<Widget> _selected(Map<String, dynamic> sel) {
    final stretch = _opts['stretch'] == true;
    final scope = _opts['scope'] as String? ?? 'figure';
    final snap = (_opts['snap'] as num?)?.toDouble() ?? 0;
    return [
      _title(sel['name'] as String? ?? 'Figure'),
      _pair(_Btn('◀ Figure', onTap: () => _cmd({'op': 'cycle_figure', 'dir': -1})), _Btn('Figure ▶', onTap: () => _cmd({'op': 'cycle_figure', 'dir': 1}))),
      _gap,
      _Btn('Edit figure…', accent: true, onTap: () => _cmd({'op': 'build'})),
      _gap,
      _pair(_Btn('Copy pose', onTap: () => _cmd({'op': 'copy_pose'})), _Btn('Paste pose', enabled: _st['pose'] == true, onTap: () => _cmd({'op': 'paste_pose'}))),
      _gap,
      _label('Drag'),
      _Segmented(const ['Rotate', 'Stretch'], stretch ? 1 : 0, (i) => _cmd({'op': 'set', 'k': 'stretch', 'v': i == 1})),
      _gap,
      _label('Keys'),
      _Segmented(const ['Joint', 'Pose'], scope == 'joint' ? 0 : 1, (i) => _cmd({'op': 'set', 'k': 'scope', 'v': i == 0 ? 'joint' : 'figure'})),
      _gap,
      _label('Angle snap'),
      _Segmented(const ['Off', '15°', '45°'], snap == 15 ? 1 : snap == 45 ? 2 : 0, (i) => _cmd({'op': 'set', 'k': 'snap', 'v': [0, 15, 45][i]})),
      _gap,
      _pair(_Btn('Turn -15°', onTap: () => _cmd({'op': 'rotate_figure', 'deg': 15})), _Btn('Turn +15°', onTap: () => _cmd({'op': 'rotate_figure', 'deg': -15}))),
      _gap,
      _pair(_Btn('Flip', onTap: () => _cmd({'op': 'flip_figure', 'h': true})), _Btn('Duplicate', onTap: () => _cmd({'op': 'duplicate_figure'}))),
      _gap,
      _pair(_Btn('To back', onTap: () => _cmd({'op': 'layer_order', 'forward': false})), _Btn('To front', onTap: () => _cmd({'op': 'layer_order', 'forward': true}))),
      _gap,
      _pair(_Btn('Cut here', enabled: _frame > 0, onTap: () => _cmd({'op': 'cut'})), _Btn('Delete here', danger: true, onTap: () => _cmd({'op': 'delete_here'}))),
      _gap,
      const Text('Cut: the figure becomes two separate figures from this frame. Delete here: gone on this frame only.', style: TextStyle(color: cSecondary, fontSize: 11)),
      _gap,
      _Btn('Delete on every frame', danger: true, onTap: () => _cmd({'op': 'delete_everywhere'})),
      ..._frameTools(),
      _gap,
      _Btn('Done', onTap: () => _cmd({'op': 'select'})),
    ];
  }

  /// The figure editor: structure tools for the selected segment.
  List<Widget> _builder(Map<String, dynamic>? sel) {
    if (sel == null) return [_title('Figure editor'), const Text('Tap a joint of this figure.', style: TextStyle(color: cSecondary, fontSize: 13))];
    final root = sel['root'] == true;
    final shape = sel['shape'] as String? ?? 'line';
    final thick = (sel['thickness'] as num?)?.toDouble() ?? 0;
    final color = (sel['color'] as num?)?.toInt() ?? 0xff;
    final stretch = _opts['stretch'] == true;
    return [
      _title(root ? 'Main node' : 'Segment'),
      _pair(_Btn('◀ Joint', onTap: () => _cmd({'op': 'cycle_joint', 'dir': -1})), _Btn('Joint ▶', onTap: () => _cmd({'op': 'cycle_joint', 'dir': 1}))),
      _gap,
      if (!root) Text('Angle ${sel['angle']}° · length ${sel['length']}', style: const TextStyle(color: cSecondary, fontSize: 11)),
      if (!root) _gap,
      _label('Drag'),
      _Segmented(const ['Rotate', 'Stretch'], stretch ? 1 : 0, (i) => _cmd({'op': 'set', 'k': 'stretch', 'v': i == 1})),
      _gap,
      _pair(_Btn('Add segment', onTap: () => _cmd({'op': 'add_segment'})), _Btn('Delete', danger: true, enabled: !root, onTap: () => _cmd({'op': 'delete_segment'}))),
      _gap,
      if (!root) ...[
        _pair(_Btn('Split', onTap: () => _cmd({'op': 'split', 'n': 2})), _Btn('Copy branch', onTap: () => _cmd({'op': 'duplicate_branch'}))),
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
        Wrap(spacing: 6, runSpacing: 6, children: [
          for (final c in const [0x000000ff, 0xffffffff, 0xff453aff, 0xff9f0aff, 0xffd60aff, 0x30d158ff, 0x0a84ffff, 0xbf5af2ff, 0x8e8e93ff, 0x7a4a2aff])
            GestureDetector(
              onTap: () => _cmd({'op': 'segment', 'color': c}),
              child: Container(
                width: 26,
                height: 26,
                decoration: BoxDecoration(
                  color: Color(((c & 0xff) << 24) | (c >> 8)),
                  borderRadius: BorderRadius.circular(4),
                  border: Border.all(color: c == color ? cAccent : cSelected, width: c == color ? 3 : 1),
                ),
              ),
            ),
        ]),
        _gap,
        _Btn('Colour whole branch', onTap: () => _cmd({'op': 'segment', 'branch': true, 'color': color})),
      ],
      _gap,
      _Btn('Done', accent: true, onTap: () => _cmd({'op': 'build_done'})),
    ];
  }

  static const _gap = SizedBox(height: 6);

  Widget _title(String s) => Padding(
        padding: const EdgeInsets.only(top: 12, bottom: 6),
        child: Text(s, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(color: cText, fontSize: 13, fontWeight: FontWeight.w600)),
      );

  Widget _label(String s) => Padding(padding: const EdgeInsets.only(bottom: 4), child: Text(s, style: const TextStyle(color: cSecondary, fontSize: 11)));

  Widget _pair(Widget a, Widget b) => Row(children: [Expanded(child: a), const SizedBox(width: 6), Expanded(child: b)]);

  Widget _stepper(String label, String value, VoidCallback minus, VoidCallback plus) => Row(children: [
        Expanded(child: Text('$label  $value', style: const TextStyle(color: cText, fontSize: 13))),
        SizedBox(width: 40, child: _Btn('−', onTap: minus)),
        const SizedBox(width: 4),
        SizedBox(width: 40, child: _Btn('+', onTap: plus)),
      ]);
}

// ---------------------------------------------------------------- stage overlay

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
    if (cam != null && cam.length == 8) {
      final frame = Path()..moveTo(v.toScreen(cam[0], cam[1]).dx, v.toScreen(cam[0], cam[1]).dy);
      for (var i = 2; i < 8; i += 2) {
        final p = v.toScreen(cam[i], cam[i + 1]);
        frame.lineTo(p.dx, p.dy);
      }
      frame.close();
      // the surround outside the export frame is dimmed
      final outside = Path.combine(PathOperation.difference, Path()..addRect(Offset.zero & size), frame);
      canvas.drawPath(outside, Paint()..color = const Color(0x8C17191C));
      canvas.drawPath(frame, Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = const Color(0x66000000));
    }
    final hs = (st['handles'] as List?) ?? const [];
    final ring = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5
      ..color = const Color(0x99000000);
    for (final h in hs) {
      final l = h as List;
      final p = v.toScreen((l[0] as num).toDouble(), (l[1] as num).toDouble());
      final flags = (l[2] as num).toInt();
      final root = flags & 1 != 0, sel = flags & 2 != 0, fig = flags & 4 != 0;
      if (sel) canvas.drawCircle(p, 11, Paint()..color = const Color(0x550A84FF));
      if (root) {
        final r = Rect.fromCenter(center: p, width: 10, height: 10);
        canvas.drawRect(r, Paint()..color = sel ? cAccent : cRoot);
        canvas.drawRect(r, ring);
      } else {
        canvas.drawCircle(p, 4.5, Paint()..color = sel ? cAccent : (fig ? const Color(0xFFFFFFFF) : const Color(0xDDFFFFFF)));
        canvas.drawCircle(p, 4.5, ring);
      }
    }
  }

  @override
  bool shouldRepaint(_Overlay o) => true;
}

// ---------------------------------------------------------------- controls

class _Btn extends StatelessWidget {
  final String label;
  final VoidCallback? onTap;
  final bool enabled, accent, danger;
  const _Btn(this.label, {this.onTap, this.enabled = true, this.accent = false, this.danger = false});

  @override
  Widget build(BuildContext context) {
    return Opacity(
      opacity: enabled ? 1 : 0.4,
      child: Material(
        color: cControl,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(4), side: accent ? const BorderSide(color: cAccent) : BorderSide.none),
        child: InkWell(
          borderRadius: BorderRadius.circular(4),
          onTap: enabled ? onTap : null,
          child: SizedBox(
            height: 40,
            child: Center(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4),
                child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis, style: TextStyle(color: danger ? cDanger : cText, fontSize: 13)),
              ),
            ),
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
  Widget build(BuildContext context) {
    return Container(
      height: 40,
      decoration: BoxDecoration(color: cControl, borderRadius: BorderRadius.circular(4)),
      child: Row(children: [
        for (var i = 0; i < labels.length; i++)
          Expanded(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () => onTap(i),
              child: Container(
                margin: const EdgeInsets.all(2),
                decoration: BoxDecoration(
                  color: i == selected ? cSelected : const Color(0x00000000),
                  borderRadius: BorderRadius.circular(3),
                  border: i == selected ? Border.all(color: const Color(0xFF5A6068)) : null,
                ),
                child: Center(child: Text(labels[i], maxLines: 1, style: TextStyle(color: i == selected ? cText : cSecondary, fontSize: 12))),
              ),
            ),
          ),
      ]),
    );
  }
}

class _Toggle extends StatelessWidget {
  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;
  const _Toggle(this.label, this.value, this.onChanged);

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () => onChanged(!value),
      child: SizedBox(
        height: 40,
        child: Row(children: [
          Expanded(child: Text(label, style: const TextStyle(color: cText, fontSize: 13))),
          Container(
            width: 40,
            height: 24,
            decoration: BoxDecoration(color: value ? cAccent : cControl, borderRadius: BorderRadius.circular(12)),
            alignment: value ? Alignment.centerRight : Alignment.centerLeft,
            padding: const EdgeInsets.all(3),
            child: Container(width: 18, height: 18, decoration: const BoxDecoration(color: cText, shape: BoxShape.circle)),
          ),
        ]),
      ),
    );
  }
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
        decoration: BoxDecoration(color: selected ? cSelected : cControl, borderRadius: BorderRadius.circular(4), border: selected ? Border.all(color: const Color(0xFF5A6068)) : null),
        child: Text(label, style: TextStyle(color: selected ? cText : cSecondary, fontSize: 12)),
      ),
    );
  }
}
