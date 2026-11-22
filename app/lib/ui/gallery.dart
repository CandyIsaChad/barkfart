// A debug gallery of the redesign's pieces (REDESIGN.md §7 A). Shown when
// BARKFART_GALLERY is set; with BARKFART_GALLERY_SHOT=<png> it saves a
// screenshot and quits, for scripted checks.
import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import 'kit.dart';
import 'pictograms.dart';
import 'rows.dart';
import 'tokens.dart';

class Gallery extends StatefulWidget {
  const Gallery({super.key, required this.sections, this.shotKey, this.onReady});
  final List<(String, Widget)> sections;
  final GlobalKey? shotKey;
  final void Function(BuildContext)? onReady;
  @override
  State<Gallery> createState() => _GalleryState();
}

class _GalleryState extends State<Gallery> {
  final _key = GlobalKey();

  @override
  void initState() {
    super.initState();
    final shot = Platform.environment['BARKFART_GALLERY_SHOT'];
    if (widget.onReady != null) WidgetsBinding.instance.addPostFrameCallback((_) => widget.onReady!(context));
    if (shot != null) {
      Timer(const Duration(milliseconds: 1200), () async {
        final b = (widget.shotKey ?? _key).currentContext!.findRenderObject()! as RenderRepaintBoundary;
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

/// The kit, as it looks inside a panel.
Widget kitSheet() {
  final panel = PanelCard(
    width: 340,
    child: Column(mainAxisSize: MainAxisSize.min, crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Segmented(const ['Stickfigure', 'Segment', 'Pose'], 1, onSelect: (_) {}),
      const SectionTitle('Segment Tools'),
      ButtonPair('Flip sideways', () {}, 'Flip over', () {}),
      StepRow('Thickness', '28', onMinus: () {}, onPlus: () {}, changed: true),
      StepRow('Length', '95', onMinus: () {}, onPlus: () {}),
      TickRow('Show motion path', true, onChanged: (_) {}),
      TickRow('Is stretchy', false, onChanged: (_) {}),
      TickRow('Locked', null, onChanged: (_) {}),
      const SectionTitle('Colour', onAdd: _noop),
      PropRow('Own colour', trailing: SwatchRow(const [Color(0xFF9B1C2E), Color(0xFF2160A8), Color(0xFF2A2B30), Color(0xFFEDB98A), Color(0xFF1E9E55)], const Color(0xFFEDB98A), onPick: (_) {}, onMore: () {})),
      SliderRow('Opacity', '80%', 0.8, onChanged: (_) {}),
      LinkRow('Blend', 'Normal', onTap: () {}),
      const Hint('Quiet explanatory text, in our own words.'),
      WordButton('Reset this segment', onTap: () {}, filled: true, danger: true, expand: true, height: 38),
    ]),
  );
  final rail = Container(
    width: 64,
    color: cPanel,
    padding: const EdgeInsets.symmetric(vertical: 6),
    child: Column(children: [
      RailButton('pose', label: 'Pose', onTap: () {}, active: true),
      RailButton('all', label: 'Move everything', onTap: () {}),
      RailButton('camera', label: 'Camera', onTap: () {}),
      RailButton('props', label: 'Properties', onTap: () {}, pressed: true),
      const RailButton('bin', label: 'Delete'),
    ]),
  );
  final words = Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
    Row(children: [WordButton('Projects', onTap: () {}), WordButton('View', onTap: () {}), WordButton('Export', onTap: () {})]),
    Row(children: [
      IconTool('stepBack', label: 'Back', onTap: () {}),
      IconTool('onion', label: 'Onion', onTap: () {}, on: true),
      IconTool('loop', label: 'Loop', onTap: () {}),
      IconTool('play', label: 'Play', onTap: () {}, size: 22),
    ]),
    const SizedBox(height: 8),
    const HintBubble('One red dot per frame.\nA held pose has none.', width: 190),
    const SizedBox(height: 8),
    SizedBox(width: 200, height: 60, child: CustomPaint(painter: const CheckerPainter())),
  ]);
  return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [rail, const SizedBox(width: 16), panel, const SizedBox(width: 16), words]);
}

void _noop() {}

/// Opens a sheet and a toast over the gallery (for the screenshot).
void openSheetAndToast(BuildContext context) {
  showSheet<void>(context, title: 'Save your work?', width: 420, children: (c) => [
        const Hint('Rooftop fight has changes you haven’t saved.'),
        Row(children: [
          Expanded(child: WordButton('Later', onTap: () {}, filled: true, expand: true, height: 38)),
          const SizedBox(width: 8),
          Expanded(child: WordButton('Save as…', onTap: () {}, filled: true, expand: true, height: 38)),
          const SizedBox(width: 8),
          Expanded(child: WordButton('Save', onTap: () {}, primary: true, expand: true, height: 38)),
        ]),
      ]);
  showToast(context, 'Hat lets go of Rival at frame 12', action: 'Undo', onAction: () {}, light: true, icon: 'broken', duration: const Duration(seconds: 30));
}

/// A page drawn from descriptors (D15), with a row swiped open to "Reset to 32".
Widget pageSheet() => SizedBox(
      width: 360,
      height: 420,
      child: PropertyPage(
        tabs: const ['Stickfigure', 'Segment', 'Pose'],
        tab: 1,
        onTab: (_) {},
        footer: DDanger('Reset this segment', () {}),
        [
          const DTitle('Segment Tools'),
          DButtons('Flip sideways', () {}, b: 'Flip over', onB: () {}),
          const DTitle('Size'),
          DWidget(SwipeActions(initiallyOpen: true, actions: [SwipeAction('Reset to 32', () {})], child: StepRow('Thickness', '28', onMinus: () {}, onPlus: () {}, changed: true))),
          DStep('Length', '95', onMinus: () {}, onPlus: () {}, changed: true, reset: DReset('Reset to 90', () {})),
          DTick('Use segment scale', false, onChanged: (_) {}),
          DSeg(const ['None', 'Absolute', 'Relative'], 0, label: 'Angle lock', onSelect: (_) {}),
          DSwatch('Colour', const [Color(0xFF9B1C2E), Color(0xFF2160A8), Color(0xFFEDB98A)], null, mixed: true, onPick: (_) {}, onMore: () {}),
          DSlider('Opacity', '100%', 1, onChanged: (_) {}),
          DLink('Blend', 'Normal', onTap: () {}),
          const DInfo('Graphics', 'Vulkan'),
        ],
      ),
    );
