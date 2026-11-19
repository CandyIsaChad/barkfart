// Barkfart: Flutter interface over the Rust engine. This first version only
// proves the link: the engine draws a waving figure every frame.
import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/scheduler.dart';

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
  late final Ticker _ticker;
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
    if (shot != null) Timer(const Duration(seconds: 6), () => _saveShot(shot));
  }

  Future<void> _loadDemo() async {
    final e = _engine;
    if (e == null) return;
    final bytes = (await rootBundle.load('assets/demo.bark')).buffer.asUint8List();
    final err = e.load(bytes);
    setState(() {
      if (err != null) {
        _status = 'Demo project failed to load: $err';
      } else {
        _info = e.info;
        _clock.reset();
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
    _busy = true;
    final sw = Stopwatch()..start();
    final secs = _clock.elapsedMicroseconds / 1e6;
    final t = info.duration > 0 ? secs % info.duration : 0.0;
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
      if (!mounted) return;
      if (_frames == 0) debugPrint('first image decoded');
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
              child: LayoutBuilder(builder: (context, c) {
                _size = c.biggest;
                return SizedBox.expand(
                  child: _frame == null ? const ColoredBox(color: Colors.white) : RawImage(image: _frame, fit: BoxFit.fill, filterQuality: FilterQuality.none),
                );
              }),
            ),
            Container(
              width: double.infinity,
              color: const Color(0xFF222222),
              padding: const EdgeInsets.all(8),
              child: Text(
                '$_status · ${_info == null ? 'no project' : 'v1 kitchen sink ${_info!.width}x${_info!.height}'} · frame $_frames · ${_ms.toStringAsFixed(1)} ms per frame in the engine',
                style: const TextStyle(color: Colors.white, fontSize: 13),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
