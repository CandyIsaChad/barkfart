// Where animations live: Documents/Barkfart Projects/*.bark (visible in the
// iOS Files app), exports in Documents/Barkfart Exports.
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
