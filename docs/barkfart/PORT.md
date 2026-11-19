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
- Also done (same checks): textures (PNG/JPEG sprite assets; missing
  images draw plain like Java) and text (Inter glyph atlas via fontdue);
  onion skin (ghost isolates match Java); audio (plan, mixer, bounce match
  the Java soundtrack within one sample step); export writers (GIF identical
  byte for byte, PNG frames in a ZIP) and supersampled export frames.
- Sound: assets decode on every platform (WAV exactly; MP3, AAC/M4A,
  OGG Vorbis, FLAC via symphonia 0.5); engine/audio plays the plan on the
  device (cpal) and the picture follows the sound's clock.
- Export through the C interface as a job stepped one frame at a time
  (GIF, PNG frames in a ZIP, soundtrack WAV); in the app a long-press on
  the stage exports into Documents/Barkfart Exports (visible in the iOS
  Files app). Checked: 480x270, 60 frames, 2.0 s soundtrack.
- iOS still to do: AVAudioSession "mix with others" (the Java version had
  it), video export (AVAssetWriter), then Android (MediaCodec).
- Saving: project.json identical to the Java writer's text on all 113
  reference projects (Java number spelling, ties to even); .bark writer.
- Import: .nodes (read and write byte for byte), .nodemc, .stknds figure
  libraries, sounds, images and animated GIFs (as movieclips); identical
  project text to the Java importer on 12 synthetic samples (committed,
  engine/testdata/legacy/synthetic) and 45 originals (local only,
  engine/testdata/legacy/local, made by LegacyDumpTest; never committed).
  The GIF decoder agrees with the gif crate.
- Editing (engine/core/src/edit): snapshot undo document (nested steps,
  cancel, saved marker, 200 steps), editor (keying, posing, retime, scale,
  stagger, copy/paste keys, easing), figure, structure, join, layer,
  symbol, camera and frame operations, and the canvas tools (handles,
  picking with digging, joint and figure drags with stretch, snap, axis
  lock, smart-stretch and pinned joints, turning a figure, panning).
  Checked by EditDumpTest: 85 scenarios give the Java project text after
  the edit and after undo, plus handle positions and pick order.
- Speed (laptop, 100 figures, 17,100 triangles): evaluate + tessellate
  about 10 ms, GPU + readback about 5 ms per picture; onion skin triples
  the CPU part. Playback caches drawn frames, so later loops are free.
- Next: dope sheet, frame thumbnails, importing files, faster evaluation.

## Notes from device tests
- 2026-11-19, iPad: first Barkfart Next IPA works; 2.9 ms per render.
  Later (options menu): a frame-rate cap (60 / 120 fps / display rate).
- Phone for Android (last): ZTE Blade A75 5G on USB; adb needs USB
  debugging allowed on the phone. Android SDK not installed yet (needs the
  user's OK for Google's license).
- IPAs are named Barkfart-Next-<UTC date-time>-<commit>.ipa.

## Interface checklist (from the Java editor, UI-SPEC.md and UI.md)
Status: [x] in the Flutter app, [ ] not yet.
- Layout: sidebar left, Undo/Redo top-left, frame strip top, Play top-right, stage centre. [x]
- Modes Figure | All | Camera (segmented). [x]
- Stage: joint rings, square main node, selected joint highlighted; drag
  after a few points of movement poses (no separate select step); tap on a
  cluster digs through joints; tap empty = deselect; drag empty = pan view;
  pinch = zoom; double-tap = fit. [x]
- Export frame (camera rectangle) drawn on the stage; grey surround. [x]
- Onion skin (2 back red, 1 forward blue), toggle. [x]
- Frames: strip of numbered cells with a key mark, tap to go, insert
  (copy pose), delete, tween on/off, FPS. [x]
- Play / stop with sound, loop. [x]
- Nothing selected: Add figure, Onion skin, FPS, Add camera. [x]
- Figure/joint selected: Previous/Next figure, Previous/Next joint, Copy
  pose / Paste pose, Drag: Rotate | Stretch, Pose: Joint | Whole figure,
  angle snap, flip figure, duplicate, front/back, delete figure. [x]
- Segment tools: add segment, delete, split, duplicate branch, flip branch,
  lock joint (pin), static, shape, thickness, colour. [x]
- Files: New, Open (projects in Documents), Save, Import from Files
  (.bark/.nodes/.nodemc/.stknds/images/sounds). [ ] (Save/New/Open only)
- Export panel: GIF, PNG frames, soundtrack, video. [ ] (long-press export kept)
- Dope sheet (keys, easing, retime, stagger, scale time, multi-select). [ ]
- Status line with frame / time and transient messages. [x]
- Frames are poses: new frames key the whole pose; in-betweens (+1/2/3/5)
  tween to the next key frame; easing presets per frame. [x]
- Delete a figure on one frame only; cut a figure in two at a frame. [x]
- Figure editor (segment tools apart from posing). [x]
- Built-in figure picker (our own figures, figures.rs). [x]
- Save asks for a name / confirms; unsaved work kept in a hidden recovery
  copy. [x]
- Playback quality (Full / Half / Fast) and frames cached while playing. [x]
- Frame thumbnails in the strip; tool tabs above the stage. [ ]
- Two-finger tap undo, three-finger tap redo. [ ]
- Nudge pad, magnifier windows. [ ]
- Effects, sounds panels, diagnostics, UI size setting. [ ]
- Creation mode (draw tools: segment, circle, polyfill ...). [ ]
