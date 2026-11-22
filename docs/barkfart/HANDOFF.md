# Handoff brief (2026-11-22, while Claude's usage limit resets)

You are continuing work on Barkfart: a Rust engine (`engine/`) with a
Flutter app (`app/`), remaking the Stick Nodes editor. Read
`docs/barkfart/PORT.md`, `docs/barkfart/UI-SPEC.md` (sections v3 and v3.1)
and `docs/barkfart/research/local/STICK_NODES_UI_REFERENCE.md` (the
original's UI, control by control) before changing anything.

## Rules (hard)
- Commit as `351002590+balsamiqvine@users.noreply.github.com`, name
  `balsamiqvine` (use `git -c user.email=... -c user.name=... commit`).
  End every commit message with a Co-Authored-By line naming the model you
  are: `Co-Authored-By: Gemini 4 Hydrogen <noreply@google.com>` or
  `Co-Authored-By: DeepSeek V4.1 Flash <noreply@together.ai>`.
- Never commit anything from `main/` or `docs/barkfart/research/local/`.
  Never rewrite history or force-push.
- Never weaken or delete tests. Every edit must be undoable (go through
  `Session::step` / `Document::edit`).
- Frames are snapshots: every figure is keyed on every frame
  (`anchor_poses`). Do not reintroduce timeline-style behaviour.
- Write all UI text in your own words (CLEANROOM.md).
- After each feature: `cd engine && cargo test -q --workspace` (all must
  pass), `cd app && ~/.local/flutter/bin/flutter analyze` (no issues),
  `flutter build linux --release`, check it with a scripted run
  (`BARKFART_DOCS=<tmp> BARKFART_SCRIPT="wait 1; ...; shot <png>; quit"
  build/linux/x64/release/bundle/barkfart`, see `_runScript` in
  `app/lib/editor.dart`) and look at the screenshot. Then commit, push and
  run `gh workflow run app-ios.yml --ref main`. Do not download or send IPAs.
- Keep changes small; one commit per feature.

## Status
Done and reviewed so far: everything ticked in docs/barkfart/GAPS.md (the
2026-11-22 handoff tasks, Modify branch and Outline selection, were
reviewed; the outline now follows the shape, drawn by the engine).

## Tasks, in order (stop after these; do not start other GAPS items)
1. **Background image in the figure editor** (reference 12, "Load BG
   image... / Clear BG image"): a picture from the Barkfart Images folder
   (`imageFiles()` in app/lib/files.dart) shown faintly behind the figure
   being edited, view only, never saved. The stage render is
   `bf_render_view` in engine/ffi/src/lib.rs; the figure editor's trace
   ghost (`tess::trace_ghost`, `Session::trace`) shows how a view-only
   underlay is added. Textures for images come from the project's assets,
   so look at how sprites get theirs before choosing an approach.

## Design rules learned since (keep to them)
- `cargo test -q --workspace` must print no `error` and no `FAILED`: a
  compile error in one test target stops cargo before the other suites.
- The Java reference comparisons (tests edit.rs, legacy.rs, reference.rs)
  must stay byte-identical. Barkfart-only behaviour goes in the session
  or behind a switch (e.g. `CanvasTools::layer_handles`), never into
  functions the Java tests call (`figure_ops::modify_branch`,
  `CanvasTools::handles` defaults).
- While a movieclip is edited, the project's root symbol is the
  movieclip's (`Session::main_root` keeps the animation's); saving always
  writes the main one.
- The camera layer holds the per-frame settings: background colour and
  gradient, frame delay, repeat. They are keyed (HOLD) on their own frame
  and read as the exact key there (no key = 0); a project without a camera
  gets one in the same undo step.
- engine/core/src/timing.rs is the one wall-time schedule (delays,
  repeats): playback, sound and exports all follow it.
- frame_ops::hold and slow_motion insert real frames: never use them for a
  frame setting.
- Per-frame sounds are clips on the audio layer named "Frame sounds"; only
  that layer's clips move with their frames (the Java reference keeps other
  audio layers on the timeline).

## Questions
(none open)
