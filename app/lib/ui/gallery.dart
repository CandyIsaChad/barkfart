// A debug gallery of the redesign's pieces (REDESIGN.md §7 A). Shown when
// BARKFART_GALLERY is set; with BARKFART_GALLERY_SHOT=<png> it saves a
// screenshot and quits, for scripted checks.
import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import 'pictograms.dart';

class Gallery extends StatefulWidget {
  const Gallery({super.key, required this.sections});
  final List<(String, Widget)> sections;
  @override
  State<Gallery> createState() => _GalleryState();
}

class _GalleryState extends State<Gallery> {
  final _key = GlobalKey();

  @override
  void initState() {
    super.initState();
    final shot = Platform.environment['BARKFART_GALLERY_SHOT'];
    if (shot != null) {
      Timer(const Duration(milliseconds: 1200), () async {
        final b = _key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
        final img = await b.toImage(pixelRatio: 2);
        final data = await img.toByteData(format: ui.ImageByteFormat.png);
        await File(shot).writeAsBytes(data!.buffer.asUint8List());
        exit(0);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return RepaintBoundary(
      key: _key,
      child: DefaultTextStyle(
        style: const TextStyle(fontFamily: 'NotoSans', fontSize: 14, color: Color(0xFF222428), decoration: TextDecoration.none),
        child: Container(
        color: const Color(0xFFF3F4F6),
        padding: const EdgeInsets.all(16),
        child: SingleChildScrollView(
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            for (final (title, w) in widget.sections) ...[
              Padding(padding: const EdgeInsets.only(top: 8, bottom: 6), child: Text(title, style: const TextStyle(color: Color(0xFFCC1736), fontSize: 15, fontWeight: FontWeight.w700))),
              w,
            ],
          ]),
        ),
      ),
      ),
    );
  }
}

/// Every pictogram with its name.
Widget pictogramSheet() {
  const names = ['pose', 'all', 'camera', 'undo', 'redo', 'props', 'bin', 'full', 'figure', 'headonly', 'hat', 'sound', 'picture', 'text', 'clip', 'folder', 'eye', 'chevron', 'back', 'next', 'plus', 'x', 'key', 'onion', 'loop', 'search', 'wiggle', 'swing', 'loopmod', 'follow', 'link', 'chain', 'broken', 'parts', 'handles', 'stepBack', 'stepFwd', 'toStart', 'play', 'pause', 'stop', 'toEnd'];
  return Wrap(spacing: 10, runSpacing: 10, children: [
    for (final n in names)
      SizedBox(
        width: 62,
        child: Column(children: [
          Pictogram(n, size: 28, color: n == 'pose' ? const Color(0xFFCC1736) : const Color(0xFF1577D6)),
          const SizedBox(height: 2),
          Text(n, style: const TextStyle(fontSize: 10, color: Color(0xFF6B7078))),
        ]),
      ),
  ]);
}
