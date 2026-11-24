// Where animations live: Documents/Barkfart Projects/*.bark (visible in the
// iOS Files app), exports in Documents/Barkfart Exports.
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/foundation.dart' show ValueNotifier;
import 'package:path_provider/path_provider.dart';

Future<Directory> _docs() async {
  final override = Platform.environment['BARKFART_DOCS'];
  if (override != null && override.isNotEmpty) return Directory(override);
  return getApplicationDocumentsDirectory();
}

Future<Directory> projectsDir() async {
  final d = Directory('${(await _docs()).path}/Barkfart Projects');
  await d.create(recursive: true);
  return d;
}

/// K2: installed plugins (.luau files), loaded when the app starts.
Future<Directory> pluginsDir() async {
  final d = Directory('${(await _docs()).path}/Barkfart Plugins');
  await d.create(recursive: true);
  return d;
}

Future<Directory> pastSavesDir() async {
  final d = Directory('${(await projectsDir()).path}/Past saves');
  await d.create(recursive: true);
  return d;
}

Future<Directory> exportsDir() async {
  final d = Directory('${(await _docs()).path}/Barkfart Exports');
  await d.create(recursive: true);
  return d;
}

class ProjectFile {
  final String path;
  ProjectFile(this.path);

  String get name {
    final f = path.split(Platform.pathSeparator).last;
    return f.endsWith('.bark') ? f.substring(0, f.length - 5) : f;
  }

  String get when {
    final m = File(path).lastModifiedSync();
    String two(int v) => v.toString().padLeft(2, '0');
    return '${m.year}-${two(m.month)}-${two(m.day)} ${two(m.hour)}:${two(m.minute)}';
  }

  Future<Uint8List> read() => File(path).readAsBytes();

  bool get isPastSave => path.contains('${Platform.pathSeparator}Past saves${Platform.pathSeparator}');

  /// Writes through a temporary file so a crash never leaves half a project.
  /// The version it replaces is kept in "Past saves" (the newest 10 per name).
  Future<void> write(Uint8List bytes) async {
    final old = File(path);
    if (old.existsSync() && !isPastSave) {
      try {
        final d = await pastSavesDir();
        final m = old.lastModifiedSync();
        String two(int v) => v.toString().padLeft(2, '0');
        final stamp = '${m.year}-${two(m.month)}-${two(m.day)} ${two(m.hour)}.${two(m.minute)}.${two(m.second)}';
        await old.copy('${d.path}/$name ($stamp).bark');
        final mine = d.listSync().whereType<File>().where((f) => f.path.split(Platform.pathSeparator).last.startsWith('$name (')).toList()
          ..sort((a, b) => b.lastModifiedSync().compareTo(a.lastModifiedSync()));
        for (final f in mine.skip(10)) {
          f.deleteSync();
        }
      } catch (_) {
        // a past save is a bonus; the save itself goes on
      }
    }
    final tmp = File('$path.saving');
    await tmp.writeAsBytes(bytes, flush: true);
    await tmp.rename(path);
  }

  static Future<List<ProjectFile>> all() async {
    final d = await projectsDir();
    final files = d.listSync().whereType<File>().where((f) => f.path.endsWith('.bark')).toList()
      ..sort((a, b) => b.lastModifiedSync().compareTo(a.lastModifiedSync()));
    return files.map((f) => ProjectFile(f.path)).toList();
  }

  /// Earlier versions of saved animations, newest first.
  static Future<List<ProjectFile>> pastSaves() async {
    final d = await pastSavesDir();
    final files = d.listSync().whereType<File>().where((f) => f.path.endsWith('.bark')).toList()
      ..sort((a, b) => b.lastModifiedSync().compareTo(a.lastModifiedSync()));
    return files.map((f) => ProjectFile(f.path)).toList();
  }

  static Future<ProjectFile?> mostRecent() async {
    final a = await all();
    return a.isEmpty ? null : a.first;
  }

  /// A new unused name: "Animation 1", "Animation 2", ...
  static Future<ProjectFile> fresh() async {
    final d = await projectsDir();
    for (var i = 1;; i++) {
      final p = '${d.path}/Animation $i.bark';
      if (!File(p).existsSync()) return ProjectFile(p);
    }
  }
}

/// The background copy of unsaved work (never shown as an animation):
/// recovery.bark plus recovery.json naming the file it belongs to.
class Recovery {
  static Future<Directory> _dir() async {
    final override = Platform.environment['BARKFART_DOCS'];
    final base = override != null && override.isNotEmpty ? Directory('$override/.support') : await getApplicationSupportDirectory();
    await base.create(recursive: true);
    return base;
  }

  static Future<void> write(Uint8List bytes, String? forFile) async {
    final d = await _dir();
    await File('${d.path}/recovery.bark.saving').writeAsBytes(bytes, flush: true);
    await File('${d.path}/recovery.bark.saving').rename('${d.path}/recovery.bark');
    await File('${d.path}/recovery.json').writeAsString(jsonEncode({'file': forFile}));
  }

  /// (bytes, the file it belongs to or null) when there is unsaved work.
  static Future<(Uint8List, String?)?> read() async {
    final d = await _dir();
    final f = File('${d.path}/recovery.bark');
    if (!f.existsSync()) return null;
    String? owner;
    try {
      owner = (jsonDecode(File('${d.path}/recovery.json').readAsStringSync()) as Map)['file'] as String?;
    } catch (_) {}
    return (await f.readAsBytes(), owner);
  }

  static Future<void> clear() async {
    final d = await _dir();
    for (final n in ['recovery.bark', 'recovery.json']) {
      final f = File('${d.path}/$n');
      if (f.existsSync()) await f.delete();
    }
  }
}

/// A project file named by the user (unsafe characters removed).
Future<ProjectFile> named(String name) async {
  final d = await projectsDir();
  final clean = name.replaceAll(RegExp(r'[/\\:*?"<>|]'), '').trim();
  return ProjectFile('${d.path}/${clean.isEmpty ? 'Animation' : clean}.bark');
}

/// Stick Nodes figure files the user keeps for the figure picker.
Future<Directory> figuresDir() async {
  final d = Directory('${(await _docs()).path}/Barkfart Figures');
  await d.create(recursive: true);
  return d;
}

Future<List<File>> figureFiles() async {
  final d = await figuresDir();
  final out = d.listSync().whereType<File>().where((f) {
    final n = f.path.toLowerCase();
    return n.endsWith('.nodes') || n.endsWith('.nodemc') || n.endsWith('.stknds');
  }).toList()
    ..sort((a, b) => a.path.toLowerCase().compareTo(b.path.toLowerCase()));
  return out;
}

/// Sound files the user keeps for Frame Tools > Import a sound.
Future<Directory> soundsDir() async {
  final d = Directory('${(await _docs()).path}/Barkfart Sounds');
  await d.create(recursive: true);
  return d;
}

const soundExtensions = ['.wav', '.mp3', '.m4a', '.aac', '.caf', '.aif', '.aiff', '.ogg', '.flac'];

Future<Directory> imagesDir() async {
  final d = Directory('${(await _docs()).path}/Barkfart Images');
  await d.create(recursive: true);
  return d;
}

const imageExtensions = ['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp'];

/// Pictures for sprites, from the Barkfart Images folder.
Future<List<File>> imageFiles() async {
  final d = await imagesDir();
  return d.listSync().whereType<File>().where((f) => imageExtensions.any(f.path.toLowerCase().endsWith)).toList()
    ..sort((a, b) => a.path.toLowerCase().compareTo(b.path.toLowerCase()));
}

Future<List<File>> soundFiles() async {
  final d = await soundsDir();
  final out = d.listSync().whereType<File>().where((f) {
    final n = f.path.toLowerCase();
    return soundExtensions.any(n.endsWith);
  }).toList()
    ..sort((a, b) => a.path.toLowerCase().compareTo(b.path.toLowerCase()));
  return out;
}

/// App settings (App Tools > Settings), kept in one small JSON file.
class AppSettings {
  static final Map<String, Object?> values = {
    'axisLock': false,
    'leftHand': false,
    'keepSelection': false,
    'stylus': false,
    'padLeft': 0,
    'padRight': 0,
    'nerd': false,
    'saveReminder': 5,
    // View options
    'showIds': false,
    'neighbourLines': true,
    'outlineSelection': false,
    'pasteLayering': false,
    // Show filters on the stage: 0 none, 1 no blur, 2 all; playback has its own switch
    'showFilters': 2,
    'playFilters': true,
    // figure editor View options
    'drawOrder': false,
    'nodeHighlight': 0,
    // Camera Tools guide overlay (0 default ... 6)
    'cameraGuide': 0,
    // Settings: joints on the stage
    'nodeStyle': 1,
    'lineWidth': 2,
    'haptics': true,
    'doubleTapMovieclip': false,
    'doubleTapNone': false,
    'zoomButtons': false,
    'guides': false,
    'stageBox': true,
    // the timeline toolbar's Loop, the Preview row, the motion path (round 10)
    'loopPlay': true,
    'previewRow': true,
    'motionPath': true,
    // Settings › Screen › UI size (1 = 100%)
    'uiScale': 1.0,
  };

  /// The UI size, which the app's root listens to (Settings board).
  static final ValueNotifier<double> uiScale = ValueNotifier(1.0);

  static Future<File> _file() async {
    final override = Platform.environment['BARKFART_DOCS'];
    final dir = (override != null && override.isNotEmpty) ? Directory(override) : await getApplicationSupportDirectory();
    await dir.create(recursive: true);
    return File('${dir.path}/barkfart-settings.json');
  }

  static Future<void> load() async {
    try {
      final f = await _file();
      if (!f.existsSync()) return;
      final m = jsonDecode(await f.readAsString());
      if (m is Map<String, dynamic>) {
        for (final e in m.entries) {
          if (values.containsKey(e.key) && e.value.runtimeType == values[e.key].runtimeType) values[e.key] = e.value;
        }
      }
    } catch (_) {
      // a damaged settings file: the defaults
    }
    uiScale.value = ((values['uiScale'] as num?) ?? 1).toDouble().clamp(0.75, 1.5);
  }

  static Future<void> save() async {
    try {
      await (await _file()).writeAsString(jsonEncode(values));
    } catch (_) {}
  }

  static bool flag(String k) => values[k] == true;
  static int number(String k) => (values[k] as num?)?.toInt() ?? 0;
}
