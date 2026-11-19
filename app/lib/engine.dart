// Bindings to the Rust engine (engine/ffi, a plain C interface).
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
typedef _RenderC = Int32 Function(Pointer<Void>, Double, Uint32, Uint32, Pointer<Uint8>, Pointer<Uint32>);
typedef _RenderD = int Function(Pointer<Void>, double, int, int, Pointer<Uint8>, Pointer<Uint32>);

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

  void dispose() {
    stop();
    if (_buf != nullptr) calloc.free(_buf);
    _buf = nullptr;
    calloc.free(_micros);
    _free(_handle);
  }
}
