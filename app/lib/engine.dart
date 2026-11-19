// Bindings to the Rust engine (engine/ffi, a plain C interface).
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

typedef _NewC = Pointer<Void> Function();
typedef _FreeC = Void Function(Pointer<Void>);
typedef _FreeD = void Function(Pointer<Void>);
typedef _AdapterC = Pointer<Utf8> Function(Pointer<Void>);
typedef _RenderC = Int32 Function(Pointer<Void>, Float, Uint32, Uint32, Pointer<Uint8>);
typedef _RenderD = int Function(Pointer<Void>, double, int, int, Pointer<Uint8>);

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

class Engine {
  final DynamicLibrary _lib;
  final Pointer<Void> _handle;
  late final _RenderD _render = _lib.lookupFunction<_RenderC, _RenderD>('bf_render_demo');
  late final _FreeD _free = _lib.lookupFunction<_FreeC, _FreeD>('bf_engine_free');
  Pointer<Uint8> _buf = nullptr;
  int _bufLen = 0;

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
    final p = _lib.lookupFunction<_AdapterC, _AdapterC>('bf_engine_adapter')(_handle);
    return p == nullptr ? '?' : p.toDartString();
  }

  /// The demo figure at time t, as straight RGBA rows (w * h * 4 bytes).
  Uint8List? renderDemo(double t, int w, int h) {
    final need = w * h * 4;
    if (need > _bufLen) {
      if (_buf != nullptr) calloc.free(_buf);
      _buf = calloc<Uint8>(need);
      _bufLen = need;
    }
    if (_render(_handle, t, w, h, _buf) != 0) return null;
    return Uint8List.fromList(_buf.asTypedList(need));
  }

  void dispose() {
    if (_buf != nullptr) calloc.free(_buf);
    _buf = nullptr;
    _free(_handle);
  }
}
