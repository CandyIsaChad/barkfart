// Barkfart: Flutter interface over the Rust engine (engine/ffi).
import 'package:material_ui/material_ui.dart';
import 'package:flutter/foundation.dart' show LicenseRegistry, LicenseEntryWithLineBreaks;
import 'package:flutter/services.dart';

import 'dart:io';

import 'editor.dart';
import 'files.dart';
import 'ui/gallery.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  // Settings › Licences lists the bundled font with the packages
  LicenseRegistry.addLicense(() async* {
    yield LicenseEntryWithLineBreaks(const ['Noto Sans'], await rootBundle.loadString('assets/fonts/NotoSans-LICENSE.txt'));
  });
  SystemChrome.setEnabledSystemUIMode(SystemUiMode.immersiveSticky);
  runApp(const BarkfartApp());
}

class BarkfartApp extends StatelessWidget {
  const BarkfartApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Barkfart',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(fontFamily: 'NotoSans', brightness: Brightness.light, colorSchemeSeed: cAccent, useMaterial3: true, scaffoldBackgroundColor: cPanel, dialogTheme: const DialogThemeData(backgroundColor: cCard)),
      home: Platform.environment['BARKFART_GALLERY'] != null
          ? Gallery(shotKey: appShot, sections: Platform.environment['BARKFART_GALLERY'] == 'page' ? [('A page from descriptors', pageSheet())] : [('Pictograms', pictogramSheet()), ('Kit', kitSheet())], onReady: Platform.environment['BARKFART_GALLERY'] == 'sheet' ? openSheetAndToast : null)
          : const EditorPage(),
      // Settings › UI size: everything drawn larger or smaller, sharp (the
      // pixel ratio follows), touches still landing where they look
      builder: (c, child) => ValueListenableBuilder<double>(
        valueListenable: AppSettings.uiScale,
        builder: (c, k, _) {
          final mq = MediaQuery.of(c);
          final inner = RepaintBoundary(key: appShot, child: child);
          if ((k - 1).abs() < 1e-3) return inner;
          final size = mq.size / k;
          return MediaQuery(
            data: mq.copyWith(size: size, devicePixelRatio: mq.devicePixelRatio * k, padding: mq.padding / k, viewPadding: mq.viewPadding / k, viewInsets: mq.viewInsets / k),
            child: ClipRect(child: OverflowBox(alignment: Alignment.topLeft, minWidth: 0, minHeight: 0, maxWidth: double.infinity, maxHeight: double.infinity, child: Transform.scale(scale: k, alignment: Alignment.topLeft, child: SizedBox(width: size.width, height: size.height, child: inner)))),
          );
        },
      ),
    );
  }
}
