// Pictograms: the redesign's icons (REDESIGN.md §3), drawn on a 24-unit grid
// with one stroke weight. The shapes are the mockups' own (generated from
// docs/barkfart/mockups), so the app and the boards draw the same icons.
import 'dart:math' as math;
import 'package:flutter/widgets.dart';

/// A stroked pictogram ('pose', 'undo', 'eye', ...) or a filled transport
/// glyph ('play', 'pause', 'stepBack', ...).
class Pictogram extends StatelessWidget {
  const Pictogram(this.name, {super.key, this.size = 24, this.color = const Color(0xFF1577D6), this.stroke = 2});

  final String name;
  final double size;
  final Color color;
  final double stroke;

  static bool has(String name) => _strokeIcons.containsKey(name) || _fillIcons.containsKey(name);

  @override
  Widget build(BuildContext context) => SizedBox(width: size, height: size, child: CustomPaint(painter: _PictogramPainter(name, color, stroke)));
}

class _PictogramPainter extends CustomPainter {
  _PictogramPainter(this.name, this.color, this.stroke);
  final String name;
  final Color color;
  final double stroke;

  @override
  void paint(Canvas canvas, Size size) {
    final k = size.shortestSide / 24;
    canvas.save();
    canvas.scale(k);
    final fill = _fillIcons[name];
    if (fill != null) {
      final p = Paint()..color = color..isAntiAlias = true;
      for (final s in fill) {
        canvas.drawPath(s.path, p);
      }
    } else {
      final p = Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = stroke
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round
        ..isAntiAlias = true;
      for (final s in _strokeIcons[name] ?? const <_Shape>[]) {
        canvas.drawPath(s.dashed ? _dash(s.path, 2.2, 2.2) : s.path, p);
      }
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(_PictogramPainter old) => old.name != name || old.color != color || old.stroke != stroke;
}

Path _dash(Path src, double on, double off) {
  final out = Path();
  for (final m in src.computeMetrics()) {
    var d = 0.0;
    while (d < m.length) {
      out.addPath(m.extractPath(d, math.min(d + on, m.length)), Offset.zero);
      d += on + off;
    }
  }
  return out;
}

abstract class _Shape {
  const _Shape({required this.dashed});
  final bool dashed;
  Path get path;
}

class _P extends _Shape {
  const _P(this.d, {required super.dashed});
  final String d;
  @override
  Path get path => _cache.putIfAbsent(d, () => svgPath(d));
  static final Map<String, Path> _cache = {};
}

class _C extends _Shape {
  const _C(this.cx, this.cy, this.r, {required super.dashed});
  final double cx, cy, r;
  @override
  Path get path => Path()..addOval(Rect.fromCircle(center: Offset(cx, cy), radius: r));
}

class _R extends _Shape {
  const _R(this.x, this.y, this.w, this.h, this.rx, {required super.dashed});
  final double x, y, w, h, rx;
  @override
  Path get path => Path()..addRRect(RRect.fromRectAndRadius(Rect.fromLTWH(x, y, w, h), Radius.circular(rx)));
}

/// Reads SVG path data (M L H V C S Q T A Z, absolute and relative).
Path svgPath(String d) {
  final path = Path();
  final tokens = RegExp(r'[MmLlHhVvCcSsQqTtAaZz]|-?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?').allMatches(d).map((m) => m.group(0)!).toList();
  var i = 0;
  var cmd = 'M';
  var x = 0.0, y = 0.0, sx = 0.0, sy = 0.0;
  double? cx2, cy2; // last control point, for S and T
  String prev = '';
  bool isCmd(String t) => RegExp(r'^[A-Za-z]$').hasMatch(t);
  double n() => double.parse(tokens[i++]);
  while (i < tokens.length) {
    if (isCmd(tokens[i])) cmd = tokens[i++];
    final rel = cmd == cmd.toLowerCase();
    final ox = rel ? x : 0.0, oy = rel ? y : 0.0;
    switch (cmd.toUpperCase()) {
      case 'M':
        x = ox + n();
        y = oy + n();
        path.moveTo(x, y);
        sx = x;
        sy = y;
        cmd = rel ? 'l' : 'L'; // further pairs are lines
      case 'L':
        x = ox + n();
        y = oy + n();
        path.lineTo(x, y);
      case 'H':
        x = ox + n();
        path.lineTo(x, y);
      case 'V':
        y = (rel ? y : 0) + n();
        path.lineTo(x, y);
      case 'C':
        final x1 = ox + n(), y1 = oy + n(), x2 = ox + n(), y2 = oy + n();
        x = ox + n();
        y = oy + n();
        path.cubicTo(x1, y1, x2, y2, x, y);
        cx2 = x2;
        cy2 = y2;
      case 'S':
        final rx1 = (prev == 'C' || prev == 'S') && cx2 != null ? 2 * x - cx2 : x;
        final ry1 = (prev == 'C' || prev == 'S') && cy2 != null ? 2 * y - cy2 : y;
        final x2 = ox + n(), y2 = oy + n();
        x = ox + n();
        y = oy + n();
        path.cubicTo(rx1, ry1, x2, y2, x, y);
        cx2 = x2;
        cy2 = y2;
      case 'Q':
        final x1 = ox + n(), y1 = oy + n();
        x = ox + n();
        y = oy + n();
        path.quadraticBezierTo(x1, y1, x, y);
        cx2 = x1;
        cy2 = y1;
      case 'T':
        final x1 = (prev == 'Q' || prev == 'T') && cx2 != null ? 2 * x - cx2 : x;
        final y1 = (prev == 'Q' || prev == 'T') && cy2 != null ? 2 * y - cy2 : y;
        x = ox + n();
        y = oy + n();
        path.quadraticBezierTo(x1, y1, x, y);
        cx2 = x1;
        cy2 = y1;
      case 'A':
        final rx = n(), ry = n(), rot = n(), large = n() != 0, sweep = n() != 0;
        x = ox + n();
        y = oy + n();
        path.arcToPoint(Offset(x, y), radius: Radius.elliptical(rx, ry), rotation: rot, largeArc: large, clockwise: sweep);
      case 'Z':
        path.close();
        x = sx;
        y = sy;
    }
    prev = cmd.toUpperCase();
  }
  return path;
}

const _strokeIcons = <String, List<_Shape>>{
  'all': [_C(7.6, 5.6, 2.1, dashed: false), _P(r'M7.6 7.9v6M7.6 10.2 4.4 12M7.6 10.2l3.2 1.8M7.6 13.9 5.2 19.4M7.6 13.9l2.4 5.5', dashed: false), _C(16.4, 5.6, 2.1, dashed: false), _P(r'M16.4 7.9v6M16.4 10.2 13.2 12M16.4 10.2l3.2 1.8M16.4 13.9 14 19.4M16.4 13.9l2.4 5.5', dashed: false)],
  'back': [_P(r'm15 5-7 7 7 7', dashed: false)],
  'bin': [_P(r'M4.5 7h15M9.5 7V4.8h5V7M6.5 7l.9 12.2a1 1 0 0 0 1 .8h7.2a1 1 0 0 0 1-.8L17.5 7M10.2 10.5v6M13.8 10.5v6', dashed: false)],
  'broken': [_P(r'M10.5 6 12 4.5a3.5 3.5 0 0 1 5 5L15.5 11', dashed: false), _P(r'M13.5 18 12 19.5a3.5 3.5 0 0 1-5-5L8.5 13', dashed: false), _P(r'M4.5 8.5H7M8.5 4.5V7M19.5 15.5H17M15.5 19.5V17', dashed: false)],
  'camera': [_P(r'M3 8V5.5A1.5 1.5 0 0 1 4.5 4H7M17 4h2.5A1.5 1.5 0 0 1 21 5.5V8M21 16v2.5a1.5 1.5 0 0 1-1.5 1.5H17M7 20H4.5A1.5 1.5 0 0 1 3 18.5V16', dashed: false), _C(12.0, 9.2, 1.7, dashed: false), _P(r'M12 10.9v3.4M12 12l-2.2 1M12 12l2.2 1M12 14.3l-1.6 2.7M12 14.3l1.6 2.7', dashed: false)],
  'chain': [_P(r'M10 13.5a3.5 3.5 0 0 0 5 0l3-3a3.5 3.5 0 0 0-5-5l-1 1', dashed: false), _P(r'M14 10.5a3.5 3.5 0 0 0-5 0l-3 3a3.5 3.5 0 0 0 5 5l1-1', dashed: false)],
  'chevron': [_P(r'm6 9 6 6 6-6', dashed: false)],
  'clip': [_R(3.5, 4.0, 17.0, 16.0, 2.0, dashed: false), _P(r'M7.5 4v16M16.5 4v16', dashed: false), _C(12.0, 8.4, 1.4, dashed: false), _P(r'M12 9.8v3.6M12 11l-1.7 1M12 11l1.7 1M12 13.4l-1.3 2.6M12 13.4l1.3 2.6', dashed: false)],
  // effects: a small burst (the FX toggle)
  'fx': [_P(r'M12 3.5v4M12 16.5v4M3.5 12h4M16.5 12h4M6 6l2.6 2.6M15.4 15.4 18 18M18 6l-2.6 2.6M8.6 15.4 6 18', dashed: false)],
  'eye': [_P(r'M2.5 12S6 5.8 12 5.8 21.5 12 21.5 12 18 18.2 12 18.2 2.5 12 2.5 12z', dashed: false), _C(12.0, 12.0, 2.8, dashed: false)],
  'figure': [_C(12.0, 4.6, 2.4, dashed: false), _P(r'M12 7v7.2M12 9.4l-4.4 3M12 9.4l4.4 3M12 14.2l-3.4 6.4M12 14.2l3.4 6.4', dashed: false)],
  'folder': [_P(r'M3.5 7.5A1.5 1.5 0 0 1 5 6h4l2 2h8a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 19 19H5a1.5 1.5 0 0 1-1.5-1.5z', dashed: false)],
  'follow': [_C(17.0, 8.0, 2.6, dashed: false), _C(7.0, 16.0, 2.6, dashed: true), _P(r'M9.3 14.3 14.6 9.7', dashed: false), _P(r'm11.5 9.5 3.1.2-.2 3.1', dashed: false)],
  'full': [_P(r'M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5', dashed: false)],
  'handles': [_R(5.0, 5.0, 14.0, 14.0, 1.5, dashed: true), _C(5.0, 5.0, 2.0, dashed: false), _C(19.0, 19.0, 2.0, dashed: false)],
  'hat': [_P(r'M7.5 15.5V9a2.5 2.5 0 0 1 2.5-2.5h4A2.5 2.5 0 0 1 16.5 9v6.5', dashed: false), _P(r'M3.5 16h17', dashed: false), _P(r'M7.5 12.5h9', dashed: false)],
  'headonly': [_C(12.0, 12.0, 5.5, dashed: false)],
  'key': [_P(r'M12 4.8 19.2 12 12 19.2 4.8 12z', dashed: false)],
  'link': [_P(r'M10 14a3.5 3.5 0 0 0 5 0l3-3a3.5 3.5 0 0 0-5-5l-1 1', dashed: false), _P(r'M14 10a3.5 3.5 0 0 0-5 0l-3 3a3.5 3.5 0 0 0 5 5l1-1', dashed: false)],
  'loop': [_P(r'M17.5 8A6.5 6.5 0 0 0 6 9', dashed: false), _P(r'M6.5 16a6.5 6.5 0 0 0 11.5-1', dashed: false), _P(r'M18 4.2v4h-4', dashed: false), _P(r'M6 19.8v-4h4', dashed: false)],
  'loopmod': [_P(r'M8.5 7.5A6 6 0 1 0 12 6', dashed: false), _P(r'm12 2.8 0 6.2-3.4-1.6', dashed: false)],
  'next': [_P(r'm9 5 7 7-7 7', dashed: false)],
  'onion': [_C(9.0, 12.0, 5.6, dashed: true), _C(15.0, 12.0, 5.6, dashed: false)],
  'parts': [_P(r'M8.5 6.5h11M8.5 12h11M8.5 17.5h11', dashed: false), _C(4.8, 6.5, 1.2, dashed: false), _C(4.8, 12.0, 1.2, dashed: false), _C(4.8, 17.5, 1.2, dashed: false)],
  'picture': [_R(3.5, 5.0, 17.0, 14.0, 2.0, dashed: false), _C(9.0, 10.0, 1.6, dashed: false), _P(r'm4 17.5 5-4.5 3.5 3 3-2.5 4 3.5', dashed: false)],
  'plus': [_P(r'M12 5.5v13M5.5 12h13', dashed: false)],
  'pose': [_C(12.0, 4.6, 2.4, dashed: false), _P(r'M12 7v7.2M12 9.4 7.2 6.6M12 9.4l4.4 3M12 14.2l-3.6 6.4M12 14.2l3.4 6.4', dashed: false)],
  'props': [_P(r'M4 8h9M17 8h3M4 16h3M11 16h9', dashed: false), _C(15.0, 8.0, 2.2, dashed: false), _C(9.0, 16.0, 2.2, dashed: false)],
  'redo': [_P(r'm15.5 5.5 4 4-4 4', dashed: false), _P(r'M19.5 9.5H10a5.5 5.5 0 0 0 0 11h3', dashed: false)],
  'search': [_C(10.5, 10.5, 5.5, dashed: false), _P(r'm15 15 4.5 4.5', dashed: false)],
  'sound': [_P(r'M4 9.5h3.5L12 5.5v13l-4.5-4H4z', dashed: false), _P(r'M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11', dashed: false)],
  'swing': [_P(r'M12 3.5v0', dashed: false), _C(12.0, 3.8, 1.0, dashed: false), _P(r'M12 4.5 17 15', dashed: false), _C(17.6, 16.4, 2.4, dashed: false), _P(r'M5.5 14.5a9.5 9.5 0 0 0 14 3.5', dashed: true)],
  'text': [_P(r'M5.5 6h13M12 6v13M9.5 19h5', dashed: false)],
  // D34: the timeline as curves (a curve through a key)
  'curves': [_P(r'M3 18c4 0 5-12 9-12s5 12 9 12', dashed: false), _C(12.0, 6.0, 1.6, dashed: false)],
  // a script layer (K2c): braces
  'script': [_P(r'M8.5 5.5C6.5 5.5 6.5 7 6.5 9s-1.5 3-2.5 3c1 0 2.5 1 2.5 3s0 3.5 2 3.5', dashed: false), _P(r'M15.5 5.5c2 0 2 1.5 2 3.5s1.5 3 2.5 3c-1 0-2.5 1-2.5 3s0 3.5-2 3.5', dashed: false)],
  'undo': [_P(r'M8.5 5.5 4.5 9.5l4 4', dashed: false), _P(r'M4.5 9.5H14a5.5 5.5 0 0 1 0 11h-3', dashed: false)],
  'wiggle': [_P(r'M3 12l2.2-4 2.6 8 2.6-9 2.6 10 2.6-9 2.6 8L20.5 12', dashed: false)],
  'x': [_P(r'M7 7l10 10M17 7 7 17', dashed: false)],
};

const _fillIcons = <String, List<_Shape>>{
  'pause': [_R(6.0, 5.0, 4.2, 14.0, 1.2, dashed: false), _R(13.8, 5.0, 4.2, 14.0, 1.2, dashed: false)],
  'play': [_P(r'M7 5.4v13.2a.8.8 0 0 0 1.23.67l10.3-6.6a.8.8 0 0 0 0-1.34L8.23 4.73A.8.8 0 0 0 7 5.4z', dashed: false)],
  'stepBack': [_R(5.0, 5.5, 2.6, 13.0, 1.0, dashed: false), _P(r'M19 6.6v10.8a.8.8 0 0 1-1.25.66l-7.9-5.4a.8.8 0 0 1 0-1.32l7.9-5.4A.8.8 0 0 1 19 6.6z', dashed: false)],
  'stepFwd': [_R(16.4, 5.5, 2.6, 13.0, 1.0, dashed: false), _P(r'M5 6.6v10.8a.8.8 0 0 0 1.25.66l7.9-5.4a.8.8 0 0 0 0-1.32l-7.9-5.4A.8.8 0 0 0 5 6.6z', dashed: false)],
  'stop': [_R(5.5, 5.5, 13.0, 13.0, 2.0, dashed: false)],
  'toEnd': [_P(r'M3.5 7.2v9.6a.7.7 0 0 0 1.1.57l6.5-4.77a.7.7 0 0 0 0-1.2L4.6 6.63a.7.7 0 0 0-1.1.57z', dashed: false), _P(r'M12 7.2v9.6a.7.7 0 0 0 1.1.57l6.5-4.77a.7.7 0 0 0 0-1.2l-6.5-4.77a.7.7 0 0 0-1.1.57z', dashed: false)],
  'toStart': [_P(r'M12 7.2v9.6a.7.7 0 0 1-1.1.57L4.4 12.6a.7.7 0 0 1 0-1.2l6.5-4.77a.7.7 0 0 1 1.1.57z', dashed: false), _P(r'M20.5 7.2v9.6a.7.7 0 0 1-1.1.57l-6.5-4.77a.7.7 0 0 1 0-1.2l6.5-4.77a.7.7 0 0 1 1.1.57z', dashed: false)],
};
