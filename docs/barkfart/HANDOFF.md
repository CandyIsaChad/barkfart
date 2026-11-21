# Handoff brief (2026-11-21, while Claude's usage limit resets)

You are continuing work on Barkfart: a Rust engine (`engine/`) with a
Flutter app (`app/`), remaking the Stick Nodes editor. Read
`docs/barkfart/PORT.md`, `docs/barkfart/UI-SPEC.md` (sections v3 and v3.1)
and `docs/barkfart/research/local/STICK_NODES_UI_REFERENCE.md` (the
original's UI, control by control) before changing anything.

## Rules (hard)
- Commit as `351002590+balsamiqvine@users.noreply.github.com`, name
  `balsamiqvine` (use `git -c user.email=... -c user.name=... commit`).
  End every commit message with
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

## Tasks, in order
1. **Textfield Tools** (reference section 13). The engine already has text
   layers (`Layer::text`, glyph rendering). Add session commands: add a
   textfield on the current frame only (like `add_figure`: `only_on_frame`),
   set text, colour, scale, alignment, locked, delete (this frame), copy and
   paste. Dragging a textfield moves it (keyed). Add a "Textfield Tools"
   section to the nothing-selected panel ("Add textfield") and a panel when a
   textfield is selected. Tests in `engine/core/tests/session.rs`.
2. **Stickfigure Filters** (reference 9.4): opacity, blur, glow, tint,
   outline, drop shadow, hue shift, saturation, invert, pixelate, clear all,
   copy/paste filters. The effect stack and all effects exist in the engine
   (`edit/structure.rs` add_effect / remove_effect, `fx.rs`, renderer
   `fx.wgsl`); map each control to an effect node on the selected figure's
   layer, keyed on this frame. Panel section after Stickfigure Tools.
3. **Frame background colour** (Frame Tools, reference 8.4): background
   colour per frame (and optional gradient). If the model has no per-frame
   background, use the camera layer's background properties or add a keyed
   project background; keep it undoable and tested.

When unsure how the original behaves, follow the reference document and
choose the plainest behaviour; note open questions at the end of this file
under "## Questions".
