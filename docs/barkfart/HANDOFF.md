# Handoff brief (2026-11-22, second handoff, while Claude's usage limit resets)

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
1. **Extend the undo / redo sweep** (`every_command_undoes_and_redoes_exactly`
   in engine/core/tests/session.rs) with the commands added on 2026-11-22
   that it does not cover yet: `textfield` with `font`, `branch_edit`,
   `polyfill_new` (inside `build`), `polyfill_color`, `polyfill_undo`,
   `connector_set` (after making a connector), `camera_lock_set`,
   `movieclip` (loop / speed / offset on a selected movieclip),
   `edit_movieclip` / `movieclip_done`, `new_movieclip`, `frame_bg` with
   `expand`, `segment` with `trapRatio` / `trapRoundStart`, `figure` with
   `tweenColors` / `keepInTween`. Each must undo to exactly the project
   before and redo to exactly the project after. Commands that need a set-up
   (a textfield, the figure editor, a movieclip) get it the way the existing
   `early` set-up does. Run it; if one fails, find the real cause and fix it
   with the smallest change (never by loosening the test), one commit each.
2. **Extend the on / off test** (`toggling_a_control_on_and_off_changes_nothing`)
   the same way with: expand gradient (with Use gradient on first), use
   segment colour (on then off), trapezoid rounded start / end, half arc,
   textfield font (Bangers then back to the default). Same rule for failures.
Write anything you could not settle under "## Questions" and commit it.

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
