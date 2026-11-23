// Bindings to the Rust engine (engine/ffi, a plain C interface).
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

typedef _NewC = Pointer<Void> Function();
typedef _FreeC = Void Function(Pointer<Void>);
typedef _FreeD = void Function(Pointer<Void>);
typedef _StrC = Pointer<Utf8> Function(Pointer<Void>);
typedef _LoadC = Int32 Function(Pointer<Void>, Pointer<Uint8>, IntPtr);
typedef _LoadD = int Function(Pointer<Void>, Pointer<Uint8>, int);
typedef _InfoC = Int32 Function(Pointer<Void>, Pointer<Double>);
typedef _InfoD = int Function(Pointer<Void>, Pointer<Double>);
typedef _PlayC = Void Function(Pointer<Void>, Double, Double, Double);
typedef _PlayD = void Function(Pointer<Void>, double, double, double);
typedef _TimeC = Double Function(Pointer<Void>);
typedef _TimeD = double Function(Pointer<Void>);
typedef _WallC = Double Function(Pointer<Void>, Double, Double);
typedef _WallD = double Function(Pointer<Void>, double, double);
typedef _FrameWallC = Double Function(Pointer<Void>, Int32);
typedef _FrameWallD = double Function(Pointer<Void>, int);
typedef _ExBeginC = Pointer<Void> Function(Pointer<Void>, Int32, Pointer<Utf8>, Uint32, Uint32, Uint32, Uint32, Int32);
typedef _ExBeginD = Pointer<Void> Function(Pointer<Void>, int, Pointer<Utf8>, int, int, int, int, int);
typedef _ExStepC = Int32 Function(Pointer<Void>, Pointer<Void>);
typedef _ExStepD = int Function(Pointer<Void>, Pointer<Void>);
typedef _ExCountC = Int32 Function(Pointer<Void>);
typedef _ExCountD = int Function(Pointer<Void>);
typedef _SoundC = Int32 Function(Pointer<Void>, Pointer<Utf8>, Uint32, Uint32);
typedef _SoundD = int Function(Pointer<Void>, Pointer<Utf8>, int, int);
typedef _RenderC = Int32 Function(Pointer<Void>, Double, Uint32, Uint32, Pointer<Uint8>, Pointer<Uint32>);
typedef _RenderD = int Function(Pointer<Void>, double, int, int, Pointer<Uint8>, Pointer<Uint32>);

typedef _DocNewC = Int32 Function(Pointer<Void>);
typedef _DocNewD = int Function(Pointer<Void>);
typedef _CmdC = Pointer<Utf8> Function(Pointer<Void>, Pointer<Utf8>);
typedef _ViewC = Int32 Function(Pointer<Void>, Double, Uint32, Uint32, Float, Float, Float, Float, Uint32, Pointer<Uint8>, Pointer<Uint32>);
typedef _ViewD = int Function(Pointer<Void>, double, int, int, double, double, double, double, int, Pointer<Uint8>, Pointer<Uint32>);
typedef _FigC = Int32 Function(Pointer<Void>, Pointer<Utf8>, Uint32, Uint32, Pointer<Uint8>);
typedef _LayerC = Int32 Function(Pointer<Void>, Pointer<Utf8>, Double, Uint32, Uint32, Pointer<Uint8>);
typedef _LayerD = int Function(Pointer<Void>, Pointer<Utf8>, double, int, int, Pointer<Uint8>);
typedef _FigD = int Function(Pointer<Void>, Pointer<Utf8>, int, int, Pointer<Uint8>);
typedef _FileC = Int32 Function(Pointer<Void>, Pointer<Utf8>, Pointer<Uint8>, IntPtr, Uint32, Uint32, Pointer<Uint8>);
typedef _FileD = int Function(Pointer<Void>, Pointer<Utf8>, Pointer<Uint8>, int, int, int, Pointer<Uint8>);
typedef _ImportC = Pointer<Utf8> Function(Pointer<Void>, Pointer<Utf8>, Pointer<Uint8>, IntPtr);
typedef _ImportD = Pointer<Utf8> Function(Pointer<Void>, Pointer<Utf8>, Pointer<Uint8>, int);
typedef _BytesC = Int32 Function(Pointer<Void>, Pointer<Uint8>, IntPtr);
typedef _BytesD = int Function(Pointer<Void>, Pointer<Uint8>, int);
typedef _ImportC2 = Int32 Function(Pointer<Void>, Pointer<Utf8>, Pointer<Uint8>, IntPtr);
typedef _ImportD2 = int Function(Pointer<Void>, Pointer<Utf8>, Pointer<Uint8>, int);
typedef _SaveC = Pointer<Uint8> Function(Pointer<Void>, Pointer<IntPtr>);

DynamicLibrary _open() {
  final override = Platform.environment['BARKFART_ENGINE'];
  if (override != null && override.isNotEmpty) return DynamicLibrary.open(override);
  if (Platform.isIOS || Platform.isMacOS) return DynamicLibrary.process();
  if (Platform.isAndroid) return DynamicLibrary.open('libbarkfart_ffi.so');
  if (Platform.isLinux) {
    final dir = File(Platform.resolvedExecutable).parent.path;
    return DynamicLibrary.open('$dir/lib/libbarkfart_ffi.so');
  }
  if (Platform.isWindows) return DynamicLibrary.open('barkfart_ffi.dll');
  throw UnsupportedError('no engine library for ${Platform.operatingSystem}');
}

/// Facts about the loaded project.
class ProjectInfo {
  final double duration, fps;
  final int width, height;
  const ProjectInfo(this.duration, this.fps, this.width, this.height);
}

class Engine {
  final DynamicLibrary _lib;
  final Pointer<Void> _handle;
  late final _RenderD _render = _lib.lookupFunction<_RenderC, _RenderD>('bf_render_frame');
  late final _FreeD _free = _lib.lookupFunction<_FreeC, _FreeD>('bf_engine_free');
  late final _StrC _error = _lib.lookupFunction<_StrC, _StrC>('bf_engine_error');
  Pointer<Uint8> _buf = nullptr;
  int _bufLen = 0;
  final Pointer<Uint32> _micros = calloc<Uint32>();

  Engine._(this._lib, this._handle);

  /// The engine, or null when the GPU could not be opened.
  static Engine? open() {
    final lib = _open();
    final h = lib.lookupFunction<_NewC, _NewC>('bf_engine_new')();
    if (h == nullptr) return null;
    return Engine._(lib, h);
  }

  /// "GPU name via backend".
  String get adapter {
    final p = _lib.lookupFunction<_StrC, _StrC>('bf_engine_adapter')(_handle);
    return p == nullptr ? '?' : p.toDartString();
  }

  String get lastError => _error(_handle).toDartString();

  /// Sound output device ("name at rate Hz"), or why there is none.
  String get audioDevice {
    final p = _lib.lookupFunction<_StrC, _StrC>('bf_audio_device')(_handle);
    return p == nullptr ? '?' : p.toDartString();
  }

  late final _PlayD _play = _lib.lookupFunction<_PlayC, _PlayD>('bf_play');
  late final _FreeD _stop = _lib.lookupFunction<_FreeC, _FreeD>('bf_stop');
  late final _TimeD _time = _lib.lookupFunction<_TimeC, _TimeD>('bf_playback_time');

  /// Plays the project's sound from t, looping [loopStart, loopEnd).
  void play(double t, {double loopStart = 0, double loopEnd = 0}) => _play(_handle, t, loopStart, loopEnd);

  void stop() => _stop(_handle);

  late final _TimeD _wallTotal = _lib.lookupFunction<_TimeC, _TimeD>('bf_wall_total');
  late final _WallD _wallTime = _lib.lookupFunction<_WallC, _WallD>('bf_wall_time');
  late final _TimeD _wallRedirect = _lib.lookupFunction<_TimeC, _TimeD>('bf_wall_redirect');

  /// Where the last wallTime picture blends to (a repeat's jump), or -1.
  double get wallRedirect => _wallRedirect(_handle);
  late final _FrameWallD _frameWall = _lib.lookupFunction<_FrameWallC, _FrameWallD>('bf_frame_wall');

  /// Seconds one pass plays for (frame delays and repeats included).
  double get wallTotal => _wallTotal(_handle);

  /// The project time shown at wall time w (one picture per 1/rate s).
  double wallTime(double w, double rate) => _wallTime(_handle, w, rate);

  /// The wall time a frame first plays at.
  double frameWall(int frame) => _frameWall(_handle, frame);

  /// Time of the sound being played, or null (stopped, or no sound output).
  double? get playbackTime {
    final t = _time(_handle);
    return t < 0 ? null : t;
  }

  /// Loads a .bark file (or project.json); null on success, else the reason.
  String? load(Uint8List bytes) {
    final p = calloc<Uint8>(bytes.length);
    p.asTypedList(bytes.length).setAll(0, bytes);
    final r = _lib.lookupFunction<_LoadC, _LoadD>('bf_project_load')(_handle, p, bytes.length);
    calloc.free(p);
    return r == 0 ? null : lastError;
  }

  ProjectInfo? get info {
    final out = calloc<Double>(4);
    final r = _lib.lookupFunction<_InfoC, _InfoD>('bf_project_info')(_handle, out);
    final i = r == 0 ? ProjectInfo(out[0], out[1], out[2].round(), out[3].round()) : null;
    calloc.free(out);
    return i;
  }

  /// Microseconds the last frame took in the engine.
  int get lastMicros => _micros.value;

  /// The shot at time t, as straight RGBA rows (w * h * 4 bytes).
  Uint8List? render(double t, int w, int h) {
    final need = w * h * 4;
    if (need > _bufLen) {
      if (_buf != nullptr) calloc.free(_buf);
      _buf = calloc<Uint8>(need);
      _bufLen = need;
    }
    if (_render(_handle, t, w, h, _buf, _micros) != 0) return null;
    return Uint8List.fromList(_buf.asTypedList(need));
  }

  late final _CmdC _cmd = _lib.lookupFunction<_CmdC, _CmdC>('bf_cmd');
  late final _ViewD _view = _lib.lookupFunction<_ViewC, _ViewD>('bf_render_view');

  /// Opens a new project (one frame, one figure).
  void newProject() => _lib.lookupFunction<_DocNewC, _DocNewD>('bf_doc_new')(_handle);

  /// Opens a new project whose figure comes from a Stick Nodes figure file.
  void newProjectWith(String name, Uint8List figure) {
    final data = calloc<Uint8>(figure.length);
    data.asTypedList(figure.length).setAll(0, figure);
    final n = name.toNativeUtf8();
    _lib.lookupFunction<_ImportC2, _ImportD2>('bf_doc_new_with')(_handle, n, data, figure.length);
    calloc.free(n);
    calloc.free(data);
  }

  /// Runs an editing command; returns the editor state (null without a project).
  Map<String, dynamic>? command(Map<String, Object?> c) {
    final p = jsonEncode(c).toNativeUtf8();
    final r = _cmd(_handle, p);
    calloc.free(p);
    if (r == nullptr) return null;
    return jsonDecode(r.toDartString()) as Map<String, dynamic>;
  }

  /// The stage rectangle [x0, x1] x [y0, y1] (y up) as straight RGBA rows.
  /// camera: as played (through the camera); onion: ghosts of nearby frames.
  Uint8List? renderView(double t, int w, int h, double x0, double y0, double x1, double y1, {bool camera = false, bool onion = false, bool outline = false, int filters = 2}) {
    final need = w * h * 4;
    if (need > _bufLen) {
      if (_buf != nullptr) calloc.free(_buf);
      _buf = calloc<Uint8>(need);
      _bufLen = need;
    }
    final flags = (camera ? 1 : 0) | (onion ? 2 : 0) | (outline ? 4 : 0) | (filters <= 0 ? 8 : filters == 1 ? 16 : 0);
    if (_view(_handle, t, w, h, x0, y0, x1, y1, flags, _buf, _micros) != 0) return null;
    return Uint8List.fromList(_buf.asTypedList(need));
  }

  /// The project as .bark bytes (marks it saved); null with the reason in [lastError].
  Uint8List? save() {
    final len = calloc<IntPtr>();
    final p = _lib.lookupFunction<_SaveC, _SaveC>('bf_save')(_handle, len);
    final n = len.value;
    calloc.free(len);
    if (p == nullptr) return null;
    return Uint8List.fromList(p.asTypedList(n));
  }

  /// One built-in figure drawn alone into w x h (transparent around it).
  Uint8List? renderFigure(String kind, int w, int h) {
    final out = calloc<Uint8>(w * h * 4);
    final k = kind.toNativeUtf8();
    final r = _lib.lookupFunction<_FigC, _FigD>('bf_render_figure')(_handle, k, w, h, out);
    calloc.free(k);
    final px = r == 0 ? Uint8List.fromList(out.asTypedList(w * h * 4)) : null;
    calloc.free(out);
    return px;
  }

  /// M4: one layer alone at time t (the pictures inside timeline blocks).
  Uint8List? renderLayer(String id, double t, int w, int h) {
    final out = calloc<Uint8>(w * h * 4);
    final k = id.toNativeUtf8();
    final r = _lib.lookupFunction<_LayerC, _LayerD>('bf_render_layer')(_handle, k, t, w, h, out);
    calloc.free(k);
    final px = r == 0 ? Uint8List.fromList(out.asTypedList(w * h * 4)) : null;
    calloc.free(out);
    return px;
  }

  /// H2: the selected figure alone with a filter command applied to a copy
  /// (`json`, or "{}"), fitted into w x h; null without a selected figure.
  Uint8List? renderEffectPreview(String json, int w, int h) {
    final out = calloc<Uint8>(w * h * 4);
    final k = json.toNativeUtf8();
    final r = _lib.lookupFunction<_FigC, _FigD>('bf_render_effect_preview')(_handle, k, w, h, out);
    calloc.free(k);
    final px = r == 0 ? Uint8List.fromList(out.asTypedList(w * h * 4)) : null;
    calloc.free(out);
    return px;
  }

  /// What a Stick Nodes file holds, drawn into w x h (null when unreadable).
  Uint8List? renderFile(String name, Uint8List bytes, int w, int h) {
    final out = calloc<Uint8>(w * h * 4);
    final data = calloc<Uint8>(bytes.length);
    data.asTypedList(bytes.length).setAll(0, bytes);
    final n = name.toNativeUtf8();
    final r = _lib.lookupFunction<_FileC, _FileD>('bf_render_file')(_handle, n, data, bytes.length, w, h, out);
    calloc.free(n);
    calloc.free(data);
    final px = r == 0 ? Uint8List.fromList(out.asTypedList(w * h * 4)) : null;
    calloc.free(out);
    return px;
  }

  /// Imports a file into the open project (one undo step); the new editor state.
  /// Figure editor "Load BG image" (null clears it); false when the picture
  /// could not be read.
  bool editorImage(Uint8List? bytes) {
    final n = bytes?.length ?? 0;
    final data = calloc<Uint8>(n == 0 ? 1 : n);
    if (bytes != null) data.asTypedList(n).setAll(0, bytes);
    final r = _lib.lookupFunction<_BytesC, _BytesD>('bf_editor_image')(_handle, data, n);
    calloc.free(data);
    return r == 0;
  }

  Map<String, dynamic>? importFile(String name, Uint8List bytes) {
    final data = calloc<Uint8>(bytes.length);
    data.asTypedList(bytes.length).setAll(0, bytes);
    final n = name.toNativeUtf8();
    final r = _lib.lookupFunction<_ImportC, _ImportD>('bf_import')(_handle, n, data, bytes.length);
    calloc.free(n);
    calloc.free(data);
    if (r == nullptr) return null;
    return jsonDecode(r.toDartString()) as Map<String, dynamic>;
  }

  /// The project as .bark bytes without marking it saved (recovery copies).
  /// The edited or selected stickfigure as a Stick Nodes .nodes file.
  Uint8List? figureFile() {
    final len = calloc<IntPtr>();
    final p = _lib.lookupFunction<_SaveC, _SaveC>('bf_figure_file')(_handle, len);
    final n = len.value;
    calloc.free(len);
    if (p == nullptr) return null;
    return Uint8List.fromList(p.asTypedList(n));
  }

  Uint8List? saveCopy() {
    final len = calloc<IntPtr>();
    final p = _lib.lookupFunction<_SaveC, _SaveC>('bf_save_copy')(_handle, len);
    final n = len.value;
    calloc.free(len);
    if (p == nullptr) return null;
    return Uint8List.fromList(p.asTypedList(n));
  }

  /// Starts an export (0 = GIF, 1 = PNG frames in a ZIP, 2 = video with sound); null with the reason in [lastError].
  ExportJob? export(int format, String path, int w, int h, {int start = 0, int count = 0, bool transparent = false}) {
    final p = path.toNativeUtf8();
    final j = _lib.lookupFunction<_ExBeginC, _ExBeginD>('bf_export_begin')(_handle, format, p, w, h, start, count, transparent ? 1 : 0);
    calloc.free(p);
    return j == nullptr ? null : ExportJob._(this, j);
  }

  /// Writes the soundtrack as WAV; false when the project has no sound.
  bool exportSoundtrack(String path, {int start = 0, int count = 0}) {
    final p = path.toNativeUtf8();
    final r = _lib.lookupFunction<_SoundC, _SoundD>('bf_export_soundtrack')(_handle, p, start, count);
    calloc.free(p);
    return r == 0;
  }

  void dispose() {
    stop();
    if (_buf != nullptr) calloc.free(_buf);
    _buf = nullptr;
    calloc.free(_micros);
    _free(_handle);
  }
}

/// An export in progress: call [step] until it returns null (done) or throws.
class ExportJob {
  final Engine _e;
  Pointer<Void> _job;
  ExportJob._(this._e, this._job);

  int get count => _e._lib.lookupFunction<_ExCountC, _ExCountD>('bf_export_count')(_job);

  /// Renders one frame; frames done so far, or null when the file is complete.
  int? step() {
    final r = _e._lib.lookupFunction<_ExStepC, _ExStepD>('bf_export_step')(_e._handle, _job);
    if (r == -1) {
      free();
      return null;
    }
    if (r == -2) {
      final why = _e._lib.lookupFunction<_StrC, _StrC>('bf_export_error')(_job).toDartString();
      free();
      throw StateError(why);
    }
    return r;
  }

  /// Cancels (an unfinished file is removed).
  void free() {
    if (_job == nullptr) return;
    _e._lib.lookupFunction<_FreeC, _FreeD>('bf_export_free')(_job);
    _job = nullptr;
  }
}
