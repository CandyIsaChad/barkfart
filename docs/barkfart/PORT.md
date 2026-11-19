# Port to Rust + Flutter

Decision and reasons: PROGRESS.md, 2026-11-19.

## Layout
- `engine/` Rust workspace
  - `core`: model, file format, evaluator, tessellator (pure Rust, plain
    `cargo test`).
  - `render`: wgpu renderer (Metal on Apple, Vulkan or GLES on Android and
    Linux, DX12 on Windows, WebGPU in browsers). 4x MSAA.
  - `ffi`: C interface the Flutter app calls through `dart:ffi`. Never
    unwinds across the boundary; null handles are no-ops.
- `app/` Flutter interface (Linux, Android, iOS; more later).
  - Linux: CMake runs cargo and ships `libbarkfart_ffi.so` in the bundle.
  - iOS: the static library is force-loaded into Runner (xcconfig);
    CI workflow `app-ios.yml` builds an unsigned IPA.
  - Android: the .so per ABI in jniLibs (not set up yet).
- The Java/LibGDX tree stays in place (tag `java-reference`) to generate
  reference data until the port matches it.

## Rendering link
1. Now: the engine renders offscreen and Flutter shows the pixels. Release
   build: 7 ms for 1280x685 on an Intel HD 5500, including readback.
2. Later, per platform: share the GPU texture with Flutter directly
   (Android SurfaceProducer, iOS IOSurface / CVPixelBuffer, Linux GL).

## Porting rules
- Reference data first: a Gradle task in the Java tree writes, for each
  fixture project, the evaluated scene and draw list as JSON and the CPU
  reference render as PNG. The Rust tests compare against those files.
- First gate: Rust loads the v1 `.bark` fixture and matches its reference
  render.
- Then, in order: model + format, evaluator, tessellator, effects, audio,
  export, legacy import (.nodes, .nodemc, .stknds). Each step keeps the
  Java tests' cases.
- The interface is built in Flutter after the engine parts it needs exist.

## Machine notes
- Dev box: 4 cores, 3 GB RAM: cargo jobs = 2, no debug info in dev builds.
- Toolchains: Rust (rustup), Flutter in `~/.local/flutter`.

## Status (2026-11-19)
- Done and checked against the Java reference (engine/testdata/ref, 107
  projects at 3 times; planted errors are caught by every comparison):
  model + project.json + .bark reading; evaluator (scenes number for
  number); tessellator (all vertices); layered isolate trees (bounds and
  membership); CPU effects reference (within 1/255 of the Java compositor);
  GPU renderer with isolates, blend modes, adjustment layers and all nine
  effects (Vulkan and GL, tight per-pixel tolerance after snapping the
  reference to the GPU's 1/256 px vertex grid).
- App: plays the bundled v1 project through the layered GPU path; draws
  only when the project frame changes.
- Laptop numbers (Intel HD 5500, 3 GB RAM, swapping): engine 21.5 ms per
  1280x720 layered frame alone, ~50 ms inside the running app (Flutter
  shares the GPU). The fix is texture sharing instead of readback.
- Next: textures (sprite images from .bark assets, glyph atlas for text),
  then onion skin, audio, export, legacy import, then the interface.

## Notes from device tests
- 2026-11-19, iPad: first Barkfart Next IPA works; 2.9 ms per render.
  Later (options menu): a frame-rate cap (60 / 120 fps / display rate).
- Phone for Android (last): ZTE Blade A75 5G on USB; adb needs USB
  debugging allowed on the phone. Android SDK not installed yet (needs the
  user's OK for Google's license).
- IPAs are named Barkfart-Next-<UTC date-time>-<commit>.ipa.
