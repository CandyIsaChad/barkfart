// Barkfart: Flutter interface over the Rust engine (engine/ffi).
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';

import 'editor.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
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
      theme: ThemeData(brightness: Brightness.dark, colorSchemeSeed: cAccent, useMaterial3: true),
      home: const EditorPage(),
    );
  }
}
