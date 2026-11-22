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
- **Superseded (2026-11-23):** the old rule "frames are snapshots, do not
  reintroduce timeline-style behaviour" is replaced by the agreed redesign.
  Read docs/barkfart/REDESIGN.md first and follow its build order (§7).
  Until section F lands, frames stay snapshots (`anchor_poses`).
- Write all UI text in your own words (CLEANROOM.md).
- After each feature: `cd engine && cargo test -q --workspace` (all must
  pass), `cd app && ~/.local/flutter/bin/flutter analyze` (no issues),
  `flutter build linux --release`, check it with a scripted run
  (`BARKFART_DOCS=<tmp> BARKFART_SCRIPT="wait 1; ...; shot <png>; quit"
  build/linux/x64/release/bundle/barkfart`, see `_runScript` in
  `app/lib/editor.dart`) and look at the screenshot,
  then commit and push. Do not trigger app-ios.yml until the redesign is good
  enough to ship (the user, 2026-11-23). Do not download or send IPAs.
- Keep changes small; one commit per feature.

## Status (2026-11-22, Claude review after the handoff agent)
Done and verified (engine suite green: `cargo test -q --workspace`, 3.5 min;
`flutter analyze` clean; Linux release build checked with scripted runs):
- The handoff agent's tests (commit 3c07b98: undo sweep and on / off test for
  the commands added on 2026-11-22) were reviewed: nothing loosened, every
  original row kept.
- New tests (all checked against planted faults or the old code):
  - `deletes_pastes_and_drags_undo_redo_and_save_exactly`: 24 edits the
    first sweep left out (deletes, every paste, polyfill delete, reorder,
    tween, break apart, join, frame sounds, drags): exact undo / redo of the
    project and the picture, and the saved text reads back as the same project.
  - `copying_and_looking_never_touch_the_project`: 45 copy / select / trace /
    tool / view commands leave the saved text, the undo steps and the
    "unsaved" marker alone.
  - `toggling_a_control_on_and_off_changes_nothing` is stricter (the panel
    must match too, and the reverse order: from "on", off and on again) and
    has about 60 rows (`toggle_rows()`), including rows that start from a
    value the control must give back.
  - The matrix `after_a_switch_*_is_unchanged` (13 tests, one per edit in
    `follow_up_edits()`): "T on, T off, then F" must give the same pictures
    and panel as "F" alone, for every row T. This is the hidden-key hunt.
  - `a_per_frame_control_on_the_middle_frame_leaves_the_other_frames_alone`.
- Bugs found and fixed (each with a test that fails on the old code):
  - Tween stickfigure colours off, then on, flattened eased and held colour
    keys to linear (it rewrote the keys). The figure's flag is now read by the
    evaluator (`Resolver::color_held`) and the keys are never touched.
  - Use segment colour off threw the segment's own colour away (Segment Tools
    and Modify branch). It is kept in the node extra `B.ownColor` and comes
    back when ticked again (`use_own_color` in session.rs).
  - Use gradient on / off, then a blank or tween frame: the new frame's hidden
    gradient colour was 0x404040 instead of the default 0x808080.
  - The undo sweep sent `"on"` to `pin` (it reads `"v"`), so it tested Unlock.
  - Inserting a frame after a frame that a figure is on alone put the figure
    on the last frame too: shifted keys drift a hair off their frames (0.2 +
    0.1 is not 0.3), the gap filler keyed the frame first and `snap_keys` then
    kept the filler. `Session::step` and the insert_frame handler now snap
    before anchoring. `frame_ops` itself must stay as it is (Java parity).
  - Layer up / Layer down stepped over the invisible camera layer first (one
    press did nothing visible): `move_among_drawn` in session.rs.
  - Modify branch's "Use segment color" had the same loss as Segment Tools.
  - Paste frame kept the background of the frame it was inserted after when
    the copied frame had no background key (copy now remembers each copied
    frame's background look and filters; paste restores them when they differ).
  - An edit that changes nothing (Layer down at the back, fps to the same
    value, Reset camera on a camera that has not moved) left an undo step that
    undid nothing and marked the project unsaved: `Document::exact` (on for
    every `Session`) compares contents when the pointers differ. The Java
    parity tests build a `Document` directly and keep the pointer rule.
  - Clear filters / Paste filters removed the filter nodes, so they cleared
    every frame: now one frame (`filters::clear`), and the panel reports a
    silent filter's off value (blur 0, pixelate 1, ...) instead of another
    frame's.
  - Random model tests that found these (keep adding models, they are the
    best bug finders): `random_frame_editing_*` (shown frames, per-frame
    background / pause, pose angle, camera zoom + filters) and
    `random_editing_with_undo_and_redo_walks_the_history_exactly`.
  - Changing the frame rate left the "Frame sounds" clips where they were
    (keys were rescaled, clips not): each frame's sound moved to another frame.
  - Speed: `CanvasTools::handles` made a `Resolver` per figure and a Resolver
    indexes every property of the project when it is made. With 100 figures
    and 30 frames state() fell from 490 ms to 30 ms, an edit from 545 to 45 ms.
    Lead: anything else that calls `Resolver::new` in a loop over layers has
    the same problem (grep it); `edit_time_on_a_big_project` is the probe.
  - One-off deep runs (not committed as defaults, to keep the suite at ~5
    minutes): the history walk with 224 seeds, the five `random_frame_editing_*`
    models with 120 seeds each and the odd-inputs fuzz with 260 seeds were
    clean. To repeat, raise the `for seed in 1..=N` bound in the test.
  - Lesson: `Document::exact` first used the derived PartialEq, and
    0.0 == -0.0, so a flip that made an angle -0.0 changed the saved project
    with no undo step (found at seed 19 of the widened walk). A step is now
    dropped only when the encodings are byte-identical too.
  - The "unsaved changes" marker compared a revision counter that every undo
    bumps, so undoing back to the saved project (or cancelling a drag) still
    said "unsaved": `Document::dirty` now follows where the saved state sits in
    the undo stack (`saved_depth` / `saved_valid`).
  - Copy / paste frame cost: `filters::snapshot` has a fast path for figures
    without filters (100 figures: copy 280 -> 140 ms).
Judgement calls the user may want to reverse (each is a behaviour change, not
just a fix): Clear / Paste filters act on one frame (the reference only says
"resets all filters of the selected figure"); unticking Use segment colour keeps
the own colour for when it is ticked again; Layer up / down step past the next
layer that draws, not the camera; an edit that changes nothing leaves no undo
step; a pasted frame gets the copied frame's background and filters.
Known and left alone:
- A hidden gradient colour of pure black is stored as transparent black, which
  reads as "no colour chosen" (default grey) after Use gradient off, on.
- Projects saved earlier on 2026-11-22 with Tween colours off already have Hold
  keys; ticking it on no longer converts them back (only affects colour keys
  imported from animations).
- `git stash list` has one old stash (2026-11-21, textfields / filters /
  frame backgrounds as a layer): superseded by the camera-layer design, keep
  it only as a reference; do not apply it.
- No `get_usage` or `run_in_terminal` tool existed in the 2026-11-22 Claude
  review session; the terminal is `ptyxis` (Ubuntu), e.g.
  `ptyxis --new-window -- /home/caleb/.claude/barkfart-handoff/after-reset.sh "<time>"`.

## Next (no order; the user: "just keep writing the app. Don't stop.")
- New rows in `toggle_rows()` need a set-up that really changes something (the
  test fails a row whose "on" changes neither a picture nor the panel).
  `@movieclip` and `@connector` are shared set-up steps.
- Movieclip and wobble rows are held to their settings, not their pixels, in
  the frame snapshot tests (they depend on the project's time).
- GitHub's iOS run can fail at the last step with "Failed to CreateArtifact:
  ENOTFOUND" (runner DNS): the IPA was built, it is not a code failure; the
  next run is the check.
- Keep hunting hidden state with tests that compare pictures or saved
  projects before and after. Ideas not done yet: the matrix over drags
  (pose edits) as follow-ups; textfield, movieclip and polyfill rows in
  `toggle_rows()`; a save / reopen between T and F; copy / paste of a frame
  that carries a gradient or filter; Undo of T then F.
- Every new control needs a row in `toggle_rows()` and an op in the undo
  sweep (`every_command_undoes_and_redoes_exactly` or
  `deletes_pastes_and_drags_undo_redo_and_save_exactly`).
- GAPS.md: only "filter quality" is open (optional).

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

- A switch is a switch: turning a control off must keep what turning it on
  needs (kept in a flag the evaluator reads, or in a node extra such as
  `B.ownColor`), never rewrite keys. Tween colours is `Layer::colors_jump`,
  read by `eval::figure_color`.
- A control that keys "every frame" for the first time must key the other
  frames with the value they show, not the property's raw base value (see
  the hidden gradient colour in `frame_bg`).
- Hidden defaults (keys equal to the default on every frame) are by design for
  frame settings, filters and the camera; the tests compare pictures and the
  panel, not the saved text.

## Questions
(none open)
