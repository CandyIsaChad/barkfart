// Where animations live: Documents/Barkfart Projects/*.bark (visible in the
// iOS Files app), exports in Documents/Barkfart Exports.
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

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

  /// Writes through a temporary file so a crash never leaves half a project.
  Future<void> write(Uint8List bytes) async {
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
