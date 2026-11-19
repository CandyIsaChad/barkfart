// Barkfart: Flutter interface over the Rust engine. This first version only
// proves the link: the engine draws a waving figure every frame.
import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';

import 'engine.dart';

void main() => runApp(const BarkfartApp());

class BarkfartApp extends StatelessWidget {
  const BarkfartApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Barkfart',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(colorSchemeSeed: const Color(0xFFE8792B), useMaterial3: true),
      home: const SpikePage(),
    );
  }
}

class SpikePage extends StatefulWidget {
  const SpikePage({super.key});

  @override
  State<SpikePage> createState() => _SpikePageState();
}

class _SpikePageState extends State<SpikePage> with SingleTickerProviderStateMixin {
  Engine? _engine;
  String _status = 'Starting the engine…';
  ProjectInfo? _info;
  ui.Image? _frame;
  /// Display-synced (paused by Flutter in the background); drawing happens
  /// only when the project frame changes.
  late final Ticker _ticker;
  int _lastFrame = -1;
  Size _lastSize = Size.zero;
  /// Frame-rate cap; null plays at the project's own fps (options menu later).
  double? maxFps;
  bool _busy = false;
  Size _size = Size.zero;
  double _dpr = 1;
  int _frames = 0;
  final Stopwatch _clock = Stopwatch()..start();
  double _ms = 0;
  final GlobalKey _shot = GlobalKey();

  @override
  void initState() {
    super.initState();
    try {
      _engine = Engine.open();
      _status = _engine == null ? 'No GPU device for the engine' : 'Engine: ${_engine!.adapter}';
    } catch (e) {
      _status = 'Engine failed to load: $e';
    }
    _loadDemo();
    _ticker = createTicker((_) => _tick())..start();
    final shot = Platform.environment['BARKFART_SCREENSHOT'];
    if (shot != null) Timer(Duration(seconds: int.tryParse(Platform.environment['BARKFART_SHOT_AFTER'] ?? '') ?? 6), () => _saveShot(shot));
  }

  static const _demos = ['assets/sound-demo.bark', 'assets/demo.bark'];
  int _demo = 0;

  Future<void> _loadDemo() async {
    final e = _engine;
    if (e == null) return;
    final open = Platform.environment['BARKFART_OPEN'];
    final bytes = open != null && open.isNotEmpty
        ? File(open).readAsBytesSync()
        : (await rootBundle.load(_demos[_demo % _demos.length])).buffer.asUint8List();
    final err = e.load(bytes);
    setState(() {
      if (err != null) {
        _status = 'Demo project failed to load: $err';
      } else {
        _info = e.info;
        _clock.reset();
        _lastFrame = -1;
        final d = _info?.duration ?? 0;
        e.play(0, loopStart: 0, loopEnd: d);
      }
    });
  }

  void _tick() {
    final e = _engine;
    final info = _info;
    if (e == null || info == null || _busy || _size.isEmpty) return;
    // render at device pixels, capped for this first version
    final scale = (_dpr * _size.longestSide).clamp(1, 1600) / _size.longestSide;
    final w = (_size.width * scale).round(), h = (_size.height * scale).round();
    if (w <= 0 || h <= 0) return;
    final sw = Stopwatch()..start();
    // the picture follows the sound when there is sound output, else its own clock
    final secs = e.playbackTime ?? _clock.elapsedMicroseconds / 1e6;
    final fps = (maxFps ?? info.fps).clamp(1.0, 240.0);
    final frame = (secs * fps).floor();
    if (frame == _lastFrame && _size == _lastSize) return;
    if (Platform.environment['BARKFART_TRACE'] != null) debugPrint('tick frame=$frame secs=${secs.toStringAsFixed(3)} sound=${e.playbackTime != null}');
    _lastFrame = frame;
    _lastSize = _size;
    _busy = true;   // only once we are really drawing (an early return must not leave it set)
    final t = info.duration > 0 ? (frame / fps) % info.duration : 0.0;
    final px = e.render(t, w, h);
    final renderMs = e.lastMicros / 1000.0;
    sw.stop();
    if (px == null) {
      _busy = false;
      if (_frames == 0) debugPrint('render failed at ${w}x$h');
      return;
    }
    if (_frames == 0) debugPrint('rendered ${w}x$h in ${renderMs.toStringAsFixed(1)} ms');
    ui.decodeImageFromPixels(px, w, h, ui.PixelFormat.rgba8888, (img) {
      if (Platform.environment['BARKFART_TRACE'] != null) debugPrint('decoded engine=${renderMs.toStringAsFixed(1)}ms total=${sw.elapsedMilliseconds}ms');
      if (!mounted) return;
      setState(() {
        _frame?.dispose();
        _frame = img;
        _frames++;
        _ms = _ms == 0 ? renderMs : _ms * 0.9 + renderMs * 0.1;
      });
      _busy = false;
    });
  }

  Future<void> _saveShot(String path) async {
    final b = _shot.currentContext?.findRenderObject() as RenderRepaintBoundary?;
    if (b != null) {
      final img = await b.toImage(pixelRatio: 1);
      final data = await img.toByteData(format: ui.ImageByteFormat.png);
      if (data != null) File(path).writeAsBytesSync(data.buffer.asUint8List());
    }
    exit(0);
  }

  @override
  void dispose() {
    _ticker.dispose();
    _frame?.dispose();
    _engine?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    _dpr = MediaQuery.devicePixelRatioOf(context);
    return Scaffold(
      body: RepaintBoundary(
        key: _shot,
        child: Column(
          children: [
            Expanded(
              child: GestureDetector(
                onTap: () {
                  _demo++;
                  _loadDemo();
                },
                child: LayoutBuilder(builder: (context, c) {
                _size = c.biggest;
                return SizedBox.expand(
                  child: _frame == null ? const ColoredBox(color: Colors.white) : RawImage(image: _frame, fit: BoxFit.fill, filterQuality: FilterQuality.none),
                );
              }),
              ),
            ),
            Container(
              width: double.infinity,
              color: const Color(0xFF222222),
              padding: const EdgeInsets.all(8),
              child: Text(
                '$_status · sound: ${_engine?.audioDevice ?? '-'} · ${_info == null ? 'no project' : '${_info!.width}x${_info!.height} at ${(maxFps ?? _info!.fps).round()} fps'} · frame $_frames · ${_ms.toStringAsFixed(1)} ms per frame in the engine',
                style: const TextStyle(color: Colors.white, fontSize: 13),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
