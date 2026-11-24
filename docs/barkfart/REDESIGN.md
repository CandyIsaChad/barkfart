# Redesign (v4): plan and decision log

Status: agreed with the user on 2026-11-22/23 over nine rounds of mockups.
This replaces UI-SPEC.md v2/v3 wherever they disagree. The mockups are on
the claude.ai canvas "Barkfart Redesign", and docs/barkfart/mockups can
rebuild them. Still, **this document is the source of truth, not the
boards**.

Read the "Do not" list and the decision log before changing any UI. Most
entries record something the user corrected. Losing one has already cost a
redesign round.

## 1. Direction in one paragraph

Barkfart keeps **Stick Nodes' look** and lays it out with **ToonSquid's
manners**. "Different and familiar":
- **The look:** a light UI, blue words, red centred section titles,
  stick-figure pictogram icons, big square tick boxes, a checkerboard
  offstage, chatty hints in yellow bubbles.
- **The layout:** canvas first, slim icon rails, a timeline at the bottom,
  panels that open on demand, flat surfaces.

## 2. Do not (each one was rejected by the user)

- **No generic pro-editor look.** That means no dark three-pane "Adobe
  Animate except it isn't" layout, no near-black with one accent, no Inter,
  no uppercase labels. The user called round 1 "100% vibecoded".
- **No invented visual world.** Round 2 ("exposure sheet", Archivo, pencil
  palette) "tries too hard" and "insists upon itself".
- **No 1:1 copy of the original either.** Round 3, with its bevelled tiles
  and the 236 px left panel, looked "scuffed".
- **No mixed grammar in a row.** A row of controls is all words or all icons.
  The bottom bar is the exception: "+" alone on the left, words on the right.
- **No loud markers on timeline blocks.** No arrows, badges, pointed ends or
  dots. They are "childish" and "too loud".
- **No features that yell.** Quality-of-life features stay subtle: "don't try
  too hard", "let it blend in".
- **No middle-dot meta strings** ("a · b").
- **Nothing copied from ToonSquid** (icons, strings) or from Stick Nodes
  (CLEANROOM.md). Take layout logic only, and write every word ourselves.
- **No plugin store, ever.**
- **Never drop an existing control** because it seems minor. "Reset length"
  and "Reset thickness" matter a lot to users.

## 3. Visual tokens

| Token | Value | Use |
|---|---|---|
| ink | #222428 | text |
| ink2 | #6B7078 | secondary text |
| icon grey | #4A4F57 | transport and toolbar glyphs |
| blue | #1577D6 | word buttons, active states, ticks on |
| red | #CC1736 | section titles, the active mode's pictogram |
| handle red / purple / blue | #D91A3A / #A534D0 / #2D63E0 | selected / connected / other joints |
| main node | #2A2B30 square | root handle |
| rail | #F3F4F6 | rails, names column |
| line | #DADDE2 | dividers |
| field | #EEF0F3 | field and word-button fill |
| hint | #FFD98C | hint bubbles |
| offstage | #E2E5E9 at 72%, the shot edged #B9BEC6 | offstage (no checkerboard since 2026-11-24) |
| tween/hold blocks | #EEF0F3 fill, #DCDFE4 edge | quiet, never coloured by type |

- **Type:** Open Sans 400, 600 and 700.
- **Shape:** panels are floating cards with a 16 radius and a soft shadow.
  Buttons have a 9–11 radius. No gradients or bevels.
- **Handle colours are the user's:** red is selected, purple is connected,
  blue is the rest.

**Sizes:**
- The mockups are 1344×600 px for a phone in landscape, about 1.55× an
  844×390 pt iPhone. Read the proportions from the mockups, but every
  **hit area is at least 44 pt**, even where the drawn control is smaller.
- The automatic scale and the "UI size" setting from UI-SPEC.md v2 still
  apply.
- Safe insets apply on both sides.

## 4. Screens and regions

### Top bar
- Words on the left: **Projects, View, Export**.
- The project name in the centre, with the unsaved marker.
- Nothing on the right.
- "Library" was removed, because adding things lives in "+".
- The round-4 red context tabs moved into the Properties panel.

### Left rail (icons only)
The modes as stick-figure pictograms: pose one figure (red when active),
move everything, camera.

### Right rail (icons only)
Undo, Redo, **Properties** (sliders), Delete, and Hide panels at the bottom.
The colour swatch is **not** here; it moved into Properties.

### Stage
A white stage on a quiet grey pasteboard (the checkerboard went, the user
2026-11-24: moving the camera showed it over the stage). On the stage:
- the motion path of the selected joint: red, with one dot per in-between
  frame and key rings labelled with frame numbers;
- onion skin in blue;
- yellow hint bubbles.

### Properties panel
- A floating card on the right, opened from the rail.
- It has tabs for whatever is selected: **Stickfigure | Segment | Pose**.
  Every kind of thing (frame, figure, node) gets the same kind of page with
  different contents.
- Sections have red centred titles.

**Stickfigure page:**
- the figure's name, with "Edit as a new figure…";
- Colour (swatches);
- Effects, with "+";
- Blending (opacity, blend);
- Stickfigure Tools, including "Draw order: free" (see decision D9).

**Segment page:** Segment Tools: Flip sideways and Flip over, then:
- Size: thickness, length, segment scale, curve radius.
- Colour: own colour and swatches, gradient (with reverse and sideways when
  on).
- The extra options for the segment's shape: trapezoid, circle or ellipse,
  triangle, polygon.
- Movement: angle lock (None, Absolute or Relative), show motion path,
  is stretchy, lock node, drag lock.
- Root node: drag lock only.

**Resets:**
- A small blue dot marks a value that differs from the figure file.
- Swiping that row reveals "Reset to N", the value in the figure file.
- A red "Reset this segment" button stays pinned at the bottom.

**Pose page:** Hold or Tween, ease pictures (even, slow end, slow both ends,
bounce), Copy pose and Paste pose.

### Effects browser
- Inside Properties: "‹ Properties" back on the left, "Import…" on the
  right, then search.
- Tiles in **3 columns** (4 was too many). Each tile previews the selected
  figure with the effect applied.
- Groups:
  - **Yours** (plugins);
  - **Look:** blur, motion blur, glow, colour, tint, invert, pixelate;
  - **Movement:** wiggle, swing (the sine modifier), loop, follow, link;
  - **Edges:** outline, shadow.

### Timeline (bottom)
**Control toolbar.** It is always visible, even when the timeline is folded:
- step back and step forward on the far left;
- Onion skin, Loop and **FX** (FX only appears when something has effects);
- to first frame, Play and to last frame in the middle;
- "24 fps" and Fold on the right.
- There are no scenes. The transport glyphs are quiet grey; Play is not a
  blue button.

**Ruler.** It counts **frames**, not seconds. The playhead is a pale pill
(#DCEAF9 with blue text) and a thin line. There is no "Frame x of y" text.

**Names column:**
- a blank cell beside the ruler, with a divider;
- a "Preview" row, thinner, with an eye to hide it;
- layer rows of 32 px, each with an icon, a name and an eye.

**Preview row.** All layers merged, **tiled like a film strip**, not one
boxed snapshot per frame. Tapping it or the ruler selects the whole moment.

**Layer rows.** The list is **flat**: every figure sits on the same level,
and only groups nest. Row order is draw order.

**Blocks are poses:**
- A tween block shows the figure's changing pose in every frame.
- A hold block shows its pose once.
- Grips appear only on the selected block.

**Joins on a row:**
- a faint grey chain at the frame it joins;
- a faint red broken chain where it lets go;
- tapping a chain says what it is joined to, with "Let go" and "Join
  elsewhere".

**Keyed properties.** These are thin rows under their layer, with grey
diamonds. They are folded behind the layer's disclosure arrow, and an × stops
animating that property.

**Bottom bar:**
- "+" (the add menu) and the keying diamond toggle on the left;
- the words **Add frame, New pose, Hold longer** on the right.

**Folded.** The toolbar, ruler and Preview row only.

### "+" menu
Stick figure, Picture, Text, Sound, Movieclip, then Group (planned).

### Swipe actions
Swiping a row (a layer, an effect, a key, a property) reveals Duplicate and
Delete, as Procreate's layers do. On the Segment page, the swipe reveals
"Reset to N".

### Figure editor
- **Entry.** "Edit as a new figure…" opens a sheet titled "Editing makes a
  new figure", with Cancel and Edit a copy. **Editing never updates the
  animated figure** (decision D12).
- **Header:** Cancel; "Hero (copy)" with "Hero in your animation stays as it
  is" under it; **Save as new figure**.
- **Shape shelf** where the timeline usually is: Select, Segment, Rounded,
  Circle, Ellipse, Triangle, Trapezoid, Polygon, Polyfill, a divider, then a
  quiet Mirror toggle.
- **Drawing.** Drag from a joint. A light ghost of the shape follows, with a
  "Length N, A°" readout.
- **Left rail:** Parts and which handles show. **Right rail:** Undo, Redo,
  Properties, Delete.
- **Parts list.** Flat, front at the top, drag to reorder draw order, with a
  quiet node count. No tree.
- **Handles are shape-coded:** circle for a joint, square for the main node,
  diamond for a connector, triangle for a polyfill anchor.
- **Size reference.** A faint stock figure labelled "Stock size".
- **Properties tabs:** Shape (live previews), Segment (the same page as
  above), Figure.
- **Tracing** is a quiet option in View, not a button.

## 5. Gestures

- **Timeline:**
  - pinch to zoom;
  - drag to scroll;
  - long-press a block to move it;
  - drag a block's end to hold it longer;
  - double-tap and drag for a selection box over several layers;
  - triple-tap for a freehand lasso.
- **Swipe** a row for its actions.
- **Figure editor:** double-tap selects a branch (the selection box);
  triple-tap gives a lasso.
- **Linux (mouse and keys):**
  - right-click a row for its swipe actions;
  - Ctrl+wheel zooms the timeline;
  - every swipe action is also reachable without a gesture.

## 6. Decision log (why, so it survives)

- **D1. Look: familiar, not new.**
  - Rounds 1–3 failed on "generic", "trying too hard" and "1:1 scuffed".
  - Round 4 (Stick Nodes look, ToonSquid manners) was approved: "works
    really, REALLY well".
- **D2. Layer timeline instead of snapshot frames.**
  - The engine already stores sparse keys per property with Hold, Linear or
    Bezier interpolation and an ease, and joins through parent/parent_node.
    Snapshot frames are only `anchor_poses` (session.rs) keying everything on
    every frame.
  - **One rule replaces `anchor_poses`:** a key on a figure keys every node of
    that figure, plus its transform, at the same time. There are no rows for
    single limbs.
  - **"Add frame" is the Stick Nodes bridge.** It inserts a frame after the
    playhead on every row, keys every figure there with its current pose, and
    pushes later keys right.
  - **Open:** what the global tweening and tweened-frames settings mean once
    each block chooses Hold or Tween. The plan is a smoothness setting
    (extra pictures per frame), so legacy projects play exactly as before.
- **D3. Splitting limbs needs a command.** Replacing limb rows with joined
  figures only works with "break this branch off into its own figure, joined
  where it was". It doesn't exist yet (figure_ops has split, duplicate_branch
  and flip_branch). Build it before removing anything.
- **D4. Selection.**
  - A block is one figure's pose.
  - A Preview or ruler cell is the whole moment, with Frame tools: copy and
    paste the frame, insert or delete time.
  - Tapping a figure on the stage selects its row and the block under the
    playhead.
- **D5. Keyed properties.**
  - Poses are always keyed, as blocks.
  - Everything else is static unless the keying toggle is on. Then a change
    adds a key and the property gets a row.
  - This exists so a held pose can carry an animated effect.
- **D6. FX toggle.** Effects always render during playback. FX decides
  whether they render while scrubbing and editing.
- **D7. Preview row.** It is optional, merged and tiled. The old "Everything"
  header looked like a "fat forehead".
- **D8. Flat list and dynamic parenting.**
  - Joins are moments, not nesting, which leaves room for joining and
    letting go over time.
  - **Engine work:** parent and parent_node become a value over time (hold
    keys). An offset is stored at each switch so nothing jumps.
  - Joining on the stage never changes draw order.
  - Old Stick Nodes files import as a chain at frame 1 that never breaks.
- **D9. "Ignore layer hierarchy" → "Draw order: free".**
  - The user's toggle exists because a nested head drew under its body.
  - With the flat list, row order is draw order and the toggle mostly
    disappears.
  - Keep it only if groups need it: a figure inside a group drawn in front
    of things outside the group.
- **D10. Effects live in Properties.** They are per layer, matching the
  engine's EffectStack. The old "Stickfigure Filters" tab became the Effects
  section.
- **D11. Resets.**
  - The base is the figure file. In the figure editor it is the last saved
    version.
  - Swipe resets one value; the pinned button resets the segment.
  - A dot shows what changed.
- **D12. Editing an animated figure makes a new, independent figure**, as in
  the original. The user insisted this be explicit: the sheet, the header
  line and the "Save as new figure" button.
- **D13. Figure editor scope.**
  - Mirror is only a drawing toggle, with no stored link: .nodes can't hold
    one, and copy, paste and flip covers finished limbs.
  - "Try it" posing was dropped: there is no clear pose to return to.
  - Polyfills must work in either winding direction (check tess.rs) so the
    original's "add clockwise" warning goes.
- **D14. Plugins and scripts.** See SCRIPTING.md.
  - Luau, sandboxed, no network, files only, no store.
  - Script layers: a block is one run; additive by default; cached forward
    runs; bake to keys.
  - Emitters spawn shared copies, not layers.
- **D15. UI technology: no ECS, but nothing hand-built twice.**
  - Flutter is already a retained, diffed tree, and the Rust engine owns all
    document state (Session JSON commands). An ECS would be a third model to
    keep in sync.
  - The user's real worry is a hardcoded UI instead of element types that
    are initialised properly. The answer is a **data-driven UI**:
    - **Pages are lists of row descriptors:** title, tick, step, seg, pair,
      one, swatch, slider, link, text, hint, info, name, search, ease, fx,
      buttons. One renderer draws every page: Stickfigure, Segment, Pose,
      Frame, Camera, Text, Picture, Movieclip, Sound, Group, the multi-select
      page, View, Animation, Export, Settings.
    - **The engine supplies them.** It reports "the properties of the
      selection" as descriptors (type, label, value, range, keyable,
      file-default for reset, changed-from-file). Flutter has no per-tool code.
    - **Plugins use the same descriptors** (SCRIPTING.md `props`), so a plugin
      effect's controls are built exactly like built-in ones.
    - **Layer rows come from a kind registry:** icon, name, block painter
      (pose blocks, clip, waveform, group summary, script block) and Properties
      page. Adding a layer kind means adding one entry.
    - **One of each component:** panel card, sheet, popover, rail button,
      swipe row, toast. Pages, sheets and popovers differ only in their
      descriptors.
    - The mockup generator (docs/barkfart/mockups) already works this way:
      `rowsHtml`, `page`, `ROWDEF`, `PAGES`.
  - Performance:
    - CustomPainter for the stage, timeline cells and Preview row;
    - thumbnails rendered by the engine and cached by (layer, frame, revision);
    - only visible rows and frames built;
    - one immutable snapshot of the engine state per change, with widgets
      reading only their slice of it.
- **D16. The ruler follows the frame rate.**
  - The mockups first counted in fives while saying 24 fps. That must not
    reach the app.
  - Labels sit at frame 1 and every fps/4 frames (12 fps: 3, 6, 9, 12…).
  - Every whole second (each multiple of fps) gets a taller tick and a bold
    number.
  - Zooming out steps the labels to fps/2, fps, then 2×fps.
  - Numbers are always frames, never seconds.
- **D17. Leave nothing behind.** Every control in today's editor has a home.
  The mapping is in section 9. A few are deliberately replaced:
  - ◀ ▶ "previous/next figure" by tapping rows;
  - Library Tools by the "+" menu and chooser;
  - "Show section tabs" by nothing, since the tabs are gone;
  - per-frame sound by Sound rows (old files import each frame's sound as a
    clip).
- **D18. Mixed values.** With several things selected, a tick box shows a
  dash, a swatch row says "Mixed", and actions apply to all of them.
- **D19. Undo is offered where a surprise is likely.** For example, joining or
  letting go on the stage shows a light toast with Undo. Errors use dark
  toasts and always say what happened and what to do.
- **D20. Tablet and big windows.**
  - From iPad size up (or a Linux window at least 900 pt tall and 1200 pt
    wide), Properties docks as a full-height column beside the right rail,
    with no float or shadow.
  - The stage grows and more layer rows show.
  - Below that, panels float as on phones.
- **D21. The old Open dialog splits into Home (decided while building E3).**
  - Opening is a card tap on the Projects screen. Deleting is a long-press
    (right-click on Linux) on a card, with a confirm. The open animation can't
    be deleted from there.
  - The filter and the order (Newest, A–Z, Oldest) stay, but they only appear
    once there are more than 6 animations.
  - "Past saves" became the Backups sheet: grouped by animation, the open one
    first, each row a time ("Today, 07:04:56") with Restore. Restore opens
    the backup as an unsaved copy.
  - "Open a file…" (first run and Projects) imports from disk, including
    Stick Nodes files.

- **D22. Real figures, never likenesses (the user, 2026-11-23).** Wherever
  the app shows a stick figure (the first-run greeting, empty states,
  tiles), it is an actual figure drawn by the engine in its own small render,
  not a hand-drawn lookalike. Pictograms stay for small icons only.
- **D23. What "Reset this segment" goes back to (D7b).** A figure remembers
  each segment's look when it enters the project: added from the "+" menu,
  imported from a file, or the starter figure of a new animation (pasting
  carries the copy's memory along). That memory is saved in the file
  (FORMAT.md, "base"). It covers shape, made-with length, thickness, scale,
  colours, curve, gradient, outline and the shape settings; angles and locks
  are not part of it. Figures from older projects have none, and the page
  then compares only thickness (32) and length, as in D7a. The pinned red
  button shows only on the Segment tab while something differs.

- **D24. Hold or Tween belongs to each figure's key (F2).** The engine
  already kept Hold, Linear or Bezier on every key, and the evaluator reads
  it: one figure can hold on a frame while another tweens (tested). The
  `easing` command sets it for the selected figure on the playhead; the Pose
  page (G2) uses it. The global "Tweening" switch stays as the master switch
  for in-between pictures, so old projects play as before, and "smoothness"
  is the old tweened-frames count (Animation popover).
- **D25. The whole-figure key rule (F3).** Around every edit, a figure's
  pose (every joint's angle and length, and its transform) is keyed together
  wherever any part of it is keyed, and on the first frame; no other figure
  is touched. Between its own keys a figure holds or tweens.
  - The camera and textfields stay frame by frame (keyed on every frame), as
    in Stick Nodes, where a textfield sits on one frame. (The on/off matrix
    found a textfield's scale, set and set back, leaving a block boundary.)
  - Being shown stays keyed on every frame: it is a held value, so it changes
    nothing on screen, and "delete on this frame only" relies on it.
  - Frame tools that act on a whole frame ("Do not tween") first key every
    figure on that frame. Add frame already keys every figure on the new
    frame (F4's bridge).
  - Old projects and imports are keyed on every frame already, so they play
    exactly as before.

- **D26. Blocks before the timeline (F4).** A block is one figure's pose key,
  lasting until its next one. The commands work on the selected figure's
  row: `new_pose`, `hold_longer`, `block_length` and `move_block` (a pose
  moves only between its neighbours; the first stays on the first frame).
  "Add frame" stays the bridge that keys every figure on the new frame.
  - Until the expanded timeline (G) can select blocks, the Pose page is a tab
    beside Stickfigure (four tabs fit). It shows the pose under the playhead:
    where it started and its length, Hold or "Tween into it", the ease, New
    pose here, Hold longer, Copy and Paste pose (moved from Stickfigure, §9).
  - The eases are the four the engine has: Even, Slow start, Slow end, Slow
    both. Bounce (§8) waits for a bounce ease in the engine.

- **D27. The first expanded timeline (G1, G2).** A chevron at the right of
  the toolbar unfolds the layer rows (script `layers`). The names column has
  a blank cell by the ruler, "Preview", then each layer front first with its
  icon, name and eye (read-only). Rows scroll sideways with the strip, since
  each frame column draws its slice of every row. Blocks are plain pale bars,
  no markers; the selected figure's pose under the playhead is pale blue
  with a blue edge. Tapping a block selects the figure on that frame and
  opens the Pose tab. Pictures of the pose inside blocks (§4) wait for a
  per-layer render in the engine.

- **D28. Selecting several figures (G5).** In the unfolded timeline,
  double-tap and drag draws a box, triple-tap and drag a lasso; the taps are
  counted from raw touches so a plain tap on a block stays instant. The
  figures whose poses the shape covers are selected (a plain double tap: the
  row under it). Several open the multi-select page: Hold or Tween into it,
  New pose here, Hold longer, colour, flip, locked, onion skin, Duplicate,
  Delete from this frame, Pick one. Each change is one step for all of them
  (`each`); a figure it can't apply to is skipped. Mixed values show a dash,
  no chosen segment, or "Mixed" (D18). The group ends when one figure is
  picked.

- **D29. The timeline matched to the round 10 board (after G).** The first
  build had drifted from the mockup; corrected:
  - The Preview row is one strip, every layer merged, tiled at the
    picture's shape (16:9 tiles that don't line up with frames), each tile
    the moment under its middle; one rounded outline, no box per frame.
  - The playhead is a pale line through the ruler, strip and rows, with the
    pill on the ruler; the current frame gets no box.
  - The names column shows even folded: a blank cell by the ruler and
    "Preview" with an eye that hides the strip (View › While editing ›
    Preview row brings it back).
  - A block is one rounded bar per pose (not a bar per frame); the selected
    pose is pale blue with a 2 pt blue edge and grips at both ends: the
    right sets its length, the left where it starts.
  - FX is the word "FX" in the toolbar, as on the board, and shows only when
    an effect really changes the picture (a filter turned on and off again
    leaves a silent node, which doesn't count).
  - The bottom bar's words are Add frame, New pose, Hold longer (§4). Copy,
    Paste, Add and Delete frame are on the Frame page (tap the ruler); the
    right rail's Delete also deletes a selected frame.
  - Not yet: pictures of the pose inside blocks (needs a per-layer render)
    and the toolbar's Loop toggle.

- **D30. Effects (H2).** The Filters tab is gone: the tabs are
  Stickfigure, Segment, Pose, in the board's order. The Stickfigure page has
  an Effects section after Colour: "+" opens the browser; each effect the
  figure has is a row with an eye (hidden keeps its settings and keys), a tap
  opens its settings under it, a swipe removes it; Copy and Paste effects;
  then Blending with the opacity slider.
  - The browser fills the Properties card: "‹ Properties", search, then
    Look, Movement and Edges in tiles of three. Look and Edges tiles are the
    selected figure on this frame with the effect, drawn by the engine on a
    copy (`bf_render_effect_preview`), with stronger settings than a new
    effect gets, since a tile shows the figure about a quarter size.
  - Movement: Wiggle (noise on the figure's position) and Swing (a sine on
    its angle) are modifiers on top of the poses (`move_fx`). Loop, Follow
    and Link are shown dimmed ("come later"): they need a source to pick.
  - "Yours" and "Import…" wait for plugins (K); without any there is nothing
    to show, so neither appears.

- **D31. Keying (H3), and why keys reach across frames.** The user asked
  (2026-11-23) whether keyframes shouldn't be unique to each frame. They are:
  a key sits on one frame. What spans frames is the value between keys: it
  holds, or tweens into the next key. That is what lets a held pose (D2)
  carry an effect without setting it on every frame, and a ramp be two keys.
  Stick Nodes felt per-frame because every frame was a full copy; files from
  it are keyed on every frame and still behave per frame.
  - The keying toggle (◇ by "+") decides whether a still value starts
    animating: off, a change is the value on every frame; on, it is a key on
    this frame. A value that is already animated always keys, so keying off
    never flattens an animation.
  - A first key holds the old value from the start, so an effect switched on
    at frame 9 appears at 9 instead of fading in from frame 1.
  - New effects no longer get a silent key on every frame (a snapshot-model
    leftover that made keyed effects drop back on the next frame).
  - Animated values other than the pose show as thin rows under their layer
    (the disclosure arrow): grey diamonds where the value changes, × to stop
    animating (its value here, on every frame).
  - Tests that asserted frame snapshots for effects now use a project keyed
    on every frame (as Stick Nodes files are), or check that a key never
    reaches back; frame tools still belong to their frame alone.

- **D32. Joins over time (I1, I2).** A layer keeps its static join (the
  value before any switch: old files still join at frame 1 for good) and a
  list of switches: from each switch's frame it is joined to a figure's
  joint, the figure, or nothing. A switch keeps the transform it had in the
  frame before it, used as the end of the interval before, so a tween into
  the switch never mixes two parents' frames and nothing jumps. The keys
  from a switch to the next are written in the new frame, with one on the
  switch (FORMAT.md, "joins").
  - Joining on the stage and "Let go" act from the current frame on; on the
    first frame they act for the whole animation, as before (`join_here`,
    `let_go_here`, `join_to` for scripts).
  - A switch stores where things were at that moment: moving the parent on
    the switch's own frame afterwards moves only the joined side, so a
    figure can then hop at its let-go. Make the move before letting go.
  - The timeline shows a grey chain where a figure joins and a red broken
    chain where it lets go; tapping one says what it is, with Let go / Join
    elsewhere (or Join again), and changes show a light toast with Undo.
  - The canvas's "main joints only" and the joints linked across joins
    read the join at the current frame too (2026-11-24).
- **D34. Automatic easing and handle types** (the user, 2026-11-24:
  "automatic easing… replacing the linear easing as the default, similar to
  Blender. We'll also add handle types").
  - A new interpolation, Smooth: a curve through the poses (per channel for
    colours). Each key has a handle type: Settle (auto-clamped, the default:
    smooth, flat at the ends and on a turning point, never past the keys
    either side), Swing (auto: may swing past a pose), Sharp (vector:
    straight to the neighbours), and Free / Aligned (handles kept on the key;
    switching to them freezes the current shape).
  - Automatic handles reach a third of the way to each neighbour, so those
    segments are plain Hermite curves; stored handles are solved for time.
  - The default lives in the project (`tween_interp`): the app's new
    animations and their starter figure tween Smooth; Stick Nodes files and
    the reference starter stay Linear, so imports play as they were made.
    Hold, Linear and the eased curves (slow end, slow both, bounce) are
    unchanged. It covers everything the Segment and Stickfigure pages can
    animate: angle, length, thickness, scale, colours, position, turn,
    opacity and every effect value. The camera, text and the frame
    background are keyed on every frame, so there is nothing between poses
    for an ease to shape; choices (shape, blend) and switches don't tween.
  - The Pose page gets Auto as a fifth ease, first (the board has four), and
    "Tween into it" means Auto. With Auto, a "Through this pose" row picks
    Settle, Swing or Sharp. Free and Aligned need a graph to drag handles in.
  - Under Smooth, a key added where there was none reshapes the curve a
    little (Linear never did): poses are keyed for the whole figure, so this
    only happens where a pose is made.
  - Graph (decided 2026-11-24: yes, after a board). Free and Aligned
    handles are only useful with somewhere to drag them, so the timeline
    gets a Curves toggle next to FX: the rows become the selected figure's
    animated values (an eye each) drawn as curves on the same frames, keys
    as dots, the selected key with its two handles and a "Frame N, V"
    readout, a held stretch shaded, and the bottom bar holding the key's
    handle type and Fit. Board: mockups/graph-editor.mjs (approved
    2026-11-24, with these from the user: lengths as curves too; an "Add a
    value…" list of the figure's numbers and colours not animated yet,
    keyed on this frame; the main joint's x and y). Values in other units
    than the selected curve's are drawn dashed, scaled to fit; colours are
    a band of the colour over time. Choices and switches stay on their
    pages (a curve for a choice could only ever jump).
  - Built 2026-11-24: the Curves toggle in the timeline's toolbar; the
    engine sends each stretch's exact shape (`curves` in the state while
    `set curves` is on: keys with their handles, and per stretch a cubic, a
    line, a step or samples). Dragging a key changes its value only (whole-
    figure keys keep their frames, D25); dragging a handle makes an
    automatic one Aligned, and the stretch it shapes Smooth. Ops:
    `curve_down`, `curve_move`, `up`, `curve_add`, `curve_show`, `handles`
    with `property`. Fit lets go of the scale kept still during a drag.
- **D35. Parts have names** (the user: so they don't show up as "Node 32"
  in the graph). A part may have a name of its own (`rename_part`); the
  Parts list, the Segment page ("Rename") and the graph use it, else its
  shape and place ("Rounded 9"). Double-tap a part in the Parts list to
  name it (holding still drags it). Saved in Barkfart projects; a Stick
  Nodes figure file has no place for it, so a figure saved as .nodes loses
  its names.

- **D36. Modifiers on a value** (the graph-mods boards, approved
  2026-11-24). A value's modifiers apply after its keys, top to bottom:
  the built-in Wiggle, Swing and Loop (Follow and Link need a value to
  follow, so they come later) and plugin modifiers. Plugins are installed
  for the whole app (one list, every project and export sees the same);
  a project names a plugin by id and keeps its settings, so a missing
  plugin does nothing (the value passes through), shows orange, and comes
  back when imported again. A plugin that runs too long, uses too much
  memory or fails is switched off with its message until it is installed
  again. Plugin settings are static for now (one value for the whole
  animation), as group effects are.
  - Built 2026-11-24: in Curves, the selected value's page in Properties
    (its modifiers as cards with their settings, an eye, swipe to remove;
    missing or switched-off ones orange), "+" opens the Modifiers browser
    (Yours, Built in: Wiggle, Swing, Loop; Import… takes a .luau file and
    keeps it in Barkfart Plugins, installed again at every start). The
    graph draws the result solid and the keys' curve faint and dashed, and
    marks the value's row. Not yet: Follow and Link; checking that Luau
    builds for iOS (it compiles C++; untested until the next iOS build).
- **D33. Groups (K).** A group is a record in its symbol and a `group` id on
  each member (not a member list, so deleting or copying layers can't leave
  stale ids). One level only. Members sit next to each other in row order,
  gathered at the frontmost one when grouped (the layers between go behind,
  and the toast says so); `tidy` keeps this after every edit, drops empty
  groups and keeps cameras, sounds and adjustment layers out. The members
  draw inside one scene group with the group's opacity and effects.
  - Group opacity is a plain value and group effects hold static settings
    for now: hooking group properties into keys (frame insert and delete,
    lookups by id) is its own job.
  - "Draw order: free" (D9) stays off the group page: with the group drawn
    as one, a member can't go in front of layers outside it. Adding it means
    breaking the wrapper for that member.
  - Delete group takes its layers; figures joined to them are let go.
  - A grouped textfield with "above bars" leaves the group's drawing to
    go over the widescreen bars, as it does outside a group.
  - Grouping selects the new group; the group ops act on the selected group
    when no `group` is given.

## 7. Build order (easiest to hardest, nested)

The user's instruction (2026-11-23): "From easiest to hardest. List out
what's the easiest, then list out what's the easiest to the hardest in that
easiest section, then repeat until you get to the hard part. Commit when you
finish each one."

Sections run easiest first, and so do the items inside each section. Tick an
item (`[x]`) in the same commit that finishes it.

**Every item:**
- engine tests if the engine changed (`cd engine && cargo test -q
  --workspace`);
- `flutter analyze` clean;
- `flutter build linux --release`;
- a scripted run with a screenshot compared against its board;
- commit, push.

Don't trigger the iOS workflow per item. The user: build iOS only when it's
good enough to ship. Behaviour must not regress: every edit stays undoable
and tests are never weakened.

### A. The look (no behaviour change)
- [x] A1. Light colour tokens. The colour constants in editor.dart become
  REDESIGN §3's values.
- [x] A2. The font. Noto Sans is bundled: it's Open Sans's sibling, OFL, and
  copied from the system fonts so nothing is downloaded. Swap to Open Sans if
  its files are ever added.
- [x] A3. Pictograms. A small SVG-path reader plus the mockups' icon set (one
  stroke weight).
- [x] A4. Kit components: word button, rail button, tick box, segmented,
  stepper, swatches, slider, red section title, panel card, sheet, popover,
  toast, hint bubble.
- [x] A5. Row descriptors and one renderer (D15).
- [x] A6. Today's `_Btn`, `_Toggle`, `_Segmented` and `_stepper` draw with
  the kit, so every existing page takes the new look at once.

### B. Small surfaces that already have behaviour
- [x] B1. The "Save your work?" sheet.
- [x] B2. Toasts replace the status line.
- [x] B3. The Animation popover: FPS, smoothness (tweened frames),
  looping, fullscreen. **Built right after C7**, because its anchor is the
  timeline toolbar's "fps" button, which C7 adds.
- [x] B4. The View card (today's View options).
- [x] B5. The Export sheet. Watermark and "Make vertical" joined it after
  E3 (script `exportfull`). "Turn the cameras 90°" stays a press of its own
  under the tick, because it changes the animation (one undo step) while
  the rest of the sheet only sets up the export.
- [x] B6. The Settings screen.

### C. The editor shell (frames are still snapshots)
- [x] C1. Offstage: a see-through grey veil (was a checkerboard until
  2026-11-24).
- [x] C2. Top bar: Projects, View and Export words, the project name, the
  unsaved dot.
- [x] C3. Left rail: the mode pictograms.
- [x] C4. Right rail: undo, redo, Properties, delete, hide panels.
- [x] C5. The Properties card holds today's sections, tabbed by selection.
  It replaces the sidebar and the tab strip.
- [x] C6. The "+" menu (absorbs Library Tools) and the stick figure chooser.
  **Built right after C7**, because "+" lives in the timeline's bottom bar.
  - [x] C6a. The "+" menu in the bottom bar. Library Tools and App Tools
    leave the Properties card.
  - [x] C6b. The figure chooser as the board's sheet: Stock, Yours and
    Effects tabs, search, tiles. The "New figure" tile waits for the figure
    editor (J).
- [x] C7. The folded timeline: control toolbar, fps ruler (D16), tiled
  Preview row. It replaces the frame strip and the top-right grid.
  - [x] C7a. The control toolbar (steps, onion skin, transport, fps) and the
    bottom bar of frame words (Add, Copy, Paste, Delete frame) replace the
    grid.
  - [x] C7b. The ruler and the tiled Preview row move into the timeline,
    replacing the top strip. Keep every per-frame mark and the copy popup
    (D17).
- [x] C8. Big screens dock Properties (D20). `BARKFART_DOCK=1` forces it for
  checks on small screens.

### D. Properties pages as descriptors (D15)
- [x] D1. Sound.
- [x] D2. Movieclip. (Not yet seen on screen: a scripted run could not select a movieclip; check it with one placed from the + menu.)
- [x] D3. Text.
- [x] D4. Camera.
- [x] D5. Frame.
- [x] D6. Stickfigure.
- [x] D7. Segment, with changed-dots and swipe-to-reset.
  - [x] D7a. Segment Tools as rows, grouped Size, Colour and Movement.
    Thickness (default 32) and length (the rest length) get changed-dots and
    swipe-to-reset.
  - [x] D7b. Engine: the selection reports each segment value's file default
    and whether it changed, plus a "reset segment" op. Then dots and swipes
    for every value, and the pinned "Reset this segment" button.

### E. Home
- [x] E1. First run.
- [x] E2. The Projects screen: saved projects with thumbnails.
- [x] E3. The backups list.

### F. Engine: layers keep their own keys (D2, D3)
- [x] F1. The break-off-and-join command.
- [x] F2. Hold or Tween per key, read by the evaluator. Tweened frames
  become "smoothness".
- [x] F3. The whole-figure key rule replaces anchor_poses. Legacy imports
  must play exactly as they do today.
- [x] F4. Commands: Add frame, New pose, Hold longer, moving and extending
  blocks.

### G. The expanded timeline
- [x] G1. Layer rows, the names column, eyes (read-only).
- [x] G2. Quiet blocks, with selection opening the Pose page.
- [x] G3. Grips to extend, long-press to move, pinch to zoom.
- [x] G4. Swipe actions, and right-click on Linux.
- [x] G5. Selection box, lasso, the multi-select page.

### H. Keyed properties and effects
- [x] H1. The FX toggle.
- [x] H2. The effects browser: built-ins with previews.
- [x] H3. The keying toggle, property rows, diamonds.

### I. Joins over time (D8)
- [x] I1. Engine: the parent becomes a value over time, with offsets stored.
- [x] I2. Chain markers, the join popover, toasts with Undo.

### J. The figure editor
- [x] J1. Header and the save-as-new sheet (D12). `edit_copy` keeps the
  document as it was and edits a copy (the original hidden meanwhile);
  `edit_copy_end` puts the document back exactly, history included. Save as
  new figure writes the copy to your figures first (chooser › Yours).
- [x] J2. The shape shelf.
- [x] J3. The Parts list. The editor's Properties are Shape / Segment / Figure tabs (the main joint shows only Figure).
- [x] J4. Shape-coded handles, the size reference. The main node is a dark square and the selected joint a red dot everywhere; in the figure editor connectors are diamonds and polyfill anchors triangles (handle flag 128). The "Stock size" figure is the app's default stickfigure (`bf_set_stock`) drawn faintly by the engine to the right, feet level; View options › Stock size figure hides it.
- [x] J5. Ghost and readout while drawing; the Mirror toggle. `drawLine` is now {line, length, angle, width, shape, mirror}: a light ghost of the shape at its real thickness, a ring at the finger and a "Length N, A°" pill in the figure's own units (left of the finger where a panel may cover it). Mirror (`set mirror`) draws the same segment from the joint across the centre line in the same undo step; nothing is stored (D13).
- [x] J6. Polyfills in either winding direction. `ear_clip` already turned clockwise rings round and curved edges take their bulge from the segment, not the ring; a test now proves a concave fill covers exactly its outline either way round, and the hint no longer asks for clockwise.

### M. Matching the round 10 boards (after the user, 2026-11-23: "the mockup is there for a reason")
Render the boards (docs/barkfart/mockups: `mkdir -p project preview && node boards.mjs`) and compare each built screen with its board.
- [x] M1. Layout: the timeline spans the full width under both rails; folded is toolbar, ruler and Preview only (no bottom bar); unfolded by default.
- [x] M2. Timeline sizes and toolbar: frames 46 wide, names column about 210 (narrower on phones), the Loop toggle, "12 fps" in grey.
- [x] M3. Stickfigure page as on the board: the name in bold with its link first, no repeated title; swatch rows (presets, the chosen one ringed, a rainbow "more") everywhere colours are picked; effects as pale cards with an "animated" tag.
  Notes (M1–M3, M6, M7): tabs are pages again (one page per tab, as the
  boards draw them; without tabs the pages stack). The figure editor link
  stays "Edit segments ›" until editing makes a new figure (J1, D12); the
  board's "Edit as a new figure…" arrives with it. Blend cycles Normal,
  Multiply, Screen, Add (`blend`). The Pose page's eases are the board's
  four, bounce included (D26's note is superseded); "Delete this pose"
  (`delete_pose`) and "Add time here" (`add_time`) are new engine commands.
  The Loop toggle plays once when off. Fixed on the way: an effect setting
  changed for the first time was created with the new value, so the key
  reached back to frame 1.
- [x] M4. Pose pictures inside blocks (one per frame for a tween, once for a hold), from a per-layer render.
- [x] M5. The Camera row, Sound rows, join chain markers on rows.
- [x] M6. Pose page: "Pose Tools", name and "frames 5 to 8", ease pictures, "Length in frames", pinned "Delete this pose".
- [x] M7. Frame page: "Frame N" title and hint, Add time here / Remove this frame, background swatches, pinned "Clear frame"; the selected frame outlined in the strip.
- [x] M8. The "+" menu: Stick figure, Picture, Text, Sound, Movieclip, then Group (planned); paste and import live in the choosers' footers.
- [x] M9. The rest, board by board (done: Animation popover, Export, View,
  Camera, Text, Sound, Movieclip, multi-select, Projects, Settings' About,
  States' empty animation and nothing-selected, Tablet docking). Still apart
  from the boards, each waiting on a feature: "Edit as a new figure…" (J1), the
  Missing-plugin card (K). (UI size, the motion path and "Edit as a new
  figure…" are done. A selected picture now has its dashed box on the
  stage: drag inside it to move, a corner grip to scale about the opposite
  corner; joints in front still win the press. Several figures now have
  Copy poses / Paste poses (`copy_poses`, `paste_poses`: each pose back on
  its own figure, one step) and Make movieclip (`to_movieclip` takes the
  whole selection).) Earlier list: Animation popover, Export, View, Camera (the frame on the stage), Text, Picture, Movieclip, Sound, Group, Projects, First run, Settings, States, Tablet.

### K. Hardest, later
Groups; the Luau plugin runtime (it needs the `mlua` crate, so it waits
until a dependency fetch is approved); Script layers; emitters.
- [ ] K3. The Workshop (the user, 2026-11-24: "make it its own dedicated
  page? in the style of a workshop … You decide"). Decided: a run's or an
  effect's settings stay in Properties beside the stage; the Workshop is the
  page for the plugins themselves (the planned Plugins page): a shelf of
  kinds (Everything, Scripts, Effects, Modifiers, Switched off), cards
  saying where each is used or why it's off, and the picked one's page:
  what it moves, its settings, where it's used, a peek at its code, Share
  the file…, Add to this animation, Remove from Barkfart. Board:
  mockups/workshop.mjs (sent 2026-11-24, waiting for the user's okay).
- [x] D37. Motion (the user, 2026-11-24: "UI element animations"). Motion
  answers something done, is short (80 to 200 ms), eases out
  (`kit.kEase`), and is off when the system asks for less motion
  (`kit.motion`). Popovers grow out of what opened them; Properties and
  View options slide in from their edge and back; the timeline's fold
  eases (the stage's picture stays in place meanwhile); segmented controls
  and the Properties tabs slide their card; a tick draws itself in;
  button-sized taps dip to 95%; toasts rise in; a block's highlight fades;
  the fold chevron turns. Nothing moves by itself, and playback and
  scrubbing stay instant (the timeline snaps when play starts or stops).
- [x] K2c. Script layers (SCRIPTING.md "Script layers"). The app (2026-11-24,
  the board approved): + › Script (installed scripts, Import a script…);
  the Script row (runs with their trace; tap one for the Script page, drag
  its ends, hold and slide it); the Script page as on the board, plus New
  run here / Delete this run; target pickers (a figure; a value, grouped
  by layer). Samples to import: docs/barkfart/plugins (Sway, Breathe,
  Pulse; tested in core/tests/scripts.rs). Engine done
  2026-11-24: `LayerKind::Script` (plugin, settings, targets, runs with
  seed, Replace, Keep the last frame, keyable Mix), `bf.script` with
  `start` / `step`, targets `bf.figure()` and `bf.value()`. The user asked
  (2026-11-24) that scripts reach effect and plugin-effect settings too: a
  value target is any keyable number (segment angle or length, position,
  opacity, an effect's or plugin effect's setting), picked from one list;
  colours and choices are left out. The output is applied in the Resolver
  after keys and modifiers, so the stage, joins and the canvas agree;
  scripts read the keyed motion only (never each other). Runs step once
  per frame at the project rate, are cached per thread until the project
  or the installed plugins change, and are deterministic. Ops
  `script_add`, `script_pick`, `script_set`, `script_run`,
  `script_run_add`, `script_run_remove`, `script_bake`, `script_delete`;
  state `rows[].runs` (with a trace) and `script` (the page). Tests:
  core/tests/scripts.rs, script/tests/runtime.rs. Board:
  mockups/script-layer.mjs (`script`); its Moves section needs a value
  target row ("Glow › Strength") before the app's page is built. Not yet:
  `t.key`, emitters (`t.spawn`), colour targets.
- [x] K2b. Plugin effects: WGSL shaders run over the layer as one more
  effect pass (the renderer compiles each plugin once per install; a
  shader that doesn't compile switches the plugin off with the compiler's
  line), settings keyable like any effect value; the effects browser's
  Yours with real previews and Import…; cards with settings on the
  Stickfigure page; a missing plugin orange, the layer drawn without it.
  Fixed on the way: the filters' chain dropped any effect that wasn't a
  built-in filter.
- [x] K2a. The Luau runtime (SCRIPTING.md; the user approved fetching mlua
  2026-11-24): `engine/script` with the sandbox (safe libraries only,
  read-only globals), a time budget per call (loading 200 ms, a modifier
  5 ms) and a 32 MB cap, `require("@barkfart")`, definitions read into typed
  props, and modifier calls. Next: modifiers in projects (a ModifierKind
  that names a plugin; Missing-plugin rows), then the app's Plugins page.
  Board (mockups/graph-editor.mjs, `graph-mods`, `graph-mods-add`; sent
  for approval 2026-11-24): in Curves, a value with modifiers draws its
  result solid and its keys' curve faint and dashed (keys stay draggable
  on it) and gets a small wave on its row; Properties shows the value's
  page with "Modifiers +" (cards in the order applied, each with its
  settings and an eye; a missing plugin orange, its settings kept); "+"
  opens a browser like the effects one: Yours (plugins), Built in (Wiggle,
  Swing, Loop, Follow, Link), Import….
- [x] K1. Groups (D33): engine (`group`, `ungroup`, `group_set`,
  `group_order`, `delete_group`; `select` with `group`), the folder row
  with its chevron and the members 20 in, the summary bar with a tick at
  every member pose while open, and folded, a preview strip of its members
  merged (the user, 2026-11-24: only when folded, since open its members'
  rows already show the same; `bf_render_group`), the Group page (name and Rename, opacity, Send back /
  Bring forward, Ungroup, Make movieclip, "Delete group" pinned) and Group
  on the several-selected page. Group effects: the effects browser aims at
  the selected group (`filter` with a group selected, static values; no
  Movement, which moves joints), the FX toggle counts them. Membership
  changes through Group and Ungroup (the timeline's rows don't drag; only
  the figure editor's Parts list does).

## 8. Designed in round 10 (2026-11-23)

The boards are on the canvas under "Round 10". All of them use the same
components.

- **Projects** (the home screen):
  - top bar: the title "Your animations"; on the right, "Open a file…",
    "Backups" and "Settings";
  - a grid of cards, four across on phones, each with a thumbnail, name,
    unsaved dot, and "N frames, S seconds. Edited …";
  - the first card is "New animation".
- **First run:** the stock figure a new animation starts with, drawn by the
  engine (D22), "No animations yet", the blue "Make
  an animation" button, and a yellow hint saying Stick Nodes files open
  through "Open a file…".
- **Settings** (two cards side by side):
  - Screen: UI size 80–180%, screen padding;
  - Joints: joint style Legacy, Modern or Solid; line width;
  - Saving: save reminder, keep backups every 5 minutes;
  - This device: graphics, sound;
  - About: version, the no-network line, licences, backups.
- **"+ Stick figure" chooser** (a sheet):
  - Stock or Yours, then search;
  - tiles, with "New figure" first (it opens the figure editor);
  - footer: "Import from Files…" and "Paste figure" (dimmed when the
    clipboard is empty).
- **Export** (a sheet):
  - Format: Video, GIF or PNG frames. Size: 640, 1280 or 1920, with the
    picture size shown.
  - From and To frame.
  - Include sound; transparent background (dimmed for video); show
    watermark; make vertical (phone video), which also turns the cameras a
    quarter turn.
  - A one-line summary, then Cancel and Export.
- **Save your work?** (a sheet): "Later", "Save as…", "Save".
- **Pose page:**
  - "Hold until the next pose" or "Tween into it";
  - Ease (even, slow end, slow both ends, bounce);
  - Length in frames;
  - Copy pose and Paste pose;
  - "Delete this pose" pinned at the bottom.
- **Frame page.** It opens when you tap the ruler or the Preview row, and
  acts on every layer at that moment. It holds all of today's Frame Tools:
  - Copy frame, Paste frame, Add time here, Remove this frame;
  - add an in-between (halfway, a third in, a tenth in);
  - background colour and gradient;
  - do not tween this frame;
  - slow motion, with in-between pictures;
  - pause here, with seconds;
  - repeat the frames before this, with back, repeat and "use tweening
    during repeat";
  - silence all sounds;
  - "Clear frame" pinned at the bottom.
- **Stickfigure page** (complete):
  - the name, with "Edit as a new figure…";
  - Colour;
  - Effects;
  - Blending (opacity, blend);
  - Stickfigure Tools: flip sideways and over, send back and bring forward,
    scale, turn 45°, center on stage, "Dragging a joint: Rotates or
    Stretches", joined to…, locked, visible in onion skin, draw order: free;
  - Tweening: stay on stage between poses, blend colours between poses;
  - Copy and paste: figure, properties;
  - "Convert to movieclip…";
  - "Delete stickfigure" pinned.
- **Camera mode and page.**
  - The camera pictogram turns red. The stage shows the camera frame with
    corner handles and the chosen guide. Figure handles hide.
  - Page:
    - copy and paste camera;
    - zoom, rotation;
    - center, reset;
    - auto-camera ›;
    - lock a figure to the camera ›;
    - guide: None, Thirds, Phi or Spiral (spiral cycles through its four
      orientations);
    - effects on this frame: widescreen, wobble position, wobble rotation,
      wobble speed, scale wobble with zoom, copy and paste wobble.
- **Text page:**
  - a text field;
  - alignment;
  - colour;
  - scale;
  - locked, locked to camera (with a hint);
  - copy and paste text;
  - "Delete text".
  - The row's block shows the text itself.
- **Picture page:**
  - picture › (replace);
  - scale;
  - flip;
  - blending;
  - locked, locked to camera;
  - "Delete picture".
  - On the stage it gets a dashed box with corner handles.
- **Movieclip page:**
  - Plays: once, loop or ping-pong;
  - speed;
  - start offset;
  - Edit movieclip, Break apart;
  - blending;
  - "Delete movieclip".
- **Editing a movieclip.** The header shows "‹ Rooftop fight", then the
  clip's name with "Editing a movieclip" under it, then "Finish". The
  timeline shows the clip's own rows. This matches the figure editor's
  header.
- **Sound page:**
  - sound › (replace);
  - volume;
  - pan;
  - pitch;
  - fade in, fade out;
  - loop;
  - copy and paste sound;
  - "Delete sound".
  - The selected clip gets grips.
- **Several selected** (selection box). The page title says "3 figures",
  with the hint "Changes here apply to all three". Mixed values follow D18.
  Then: Group, Make movieclip, Copy poses, Paste poses.
- **Group:**
  - a folder row with a disclosure arrow; children indented 20 px;
  - its timeline row is a thin summary bar with a tick at every child pose;
  - page: name and Rename, Effects for the whole group, opacity,
    "Draw order: free" (D9), Ungroup, Make movieclip, "Delete group".
- **View options** (a card under "View"):
  - Onion skin: on, frames before and after, behind or in front;
  - Handles: only main joints, static joints, draw order numbers;
    highlight polyfills, connectors or locks;
  - While editing: effects None, No blur or All (the FX toggle switches
    between None and this choice); Preview row; Motion path;
  - Playback: quality Full, Half or Fast;
  - Helpers: fit stage, nudge arrows, magnifier;
  - Running slow?: ms per picture, memory and advice.
  - The figure editor's View adds tracing.
- **Animation settings** (a popover from "12 fps"):
  - frames per second;
  - extra smoothness (the old tweened frames, D2);
  - loop back to the start;
  - fullscreen when playing.
- **Edge states** (one board):
  - a missing plugin row, orange, with settings and keys kept;
  - the backups list with Restore;
  - an empty animation: "Nothing here yet. Tap + to add a stick figure",
    with a yellow "Start here" by "+";
  - toasts (dark for errors, light with Undo for joins and let-gos).
- **Tablet:** Properties docked (D20), the camera row, the Hero row expanded
  with its keyed property.

## 9. Where today's controls went

| Today | Redesign |
|---|---|
| App Tools: New project, Open, Settings | Projects screen; Settings screen |
| Project Tools: Save, Save as, Export, watermark, make vertical, rotate cameras | Projects (auto-save and the unsaved dot), the "Save your work?" sheet, the Export sheet |
| Animation Tools: FPS, tweening on, looping, tweened frames, show filters while playing, fullscreen | the Animation popover ("12 fps"); View › While editing; the FX toggle |
| Library Tools: types, paste, import, create sprite, create movieclip | the "+" menu and its choosers |
| Frame Tools (all) | the Frame page |
| Stickfigure Tools (all) | the Stickfigure page; pose copy and paste on the Pose page; ◀ ▶ replaced by rows |
| Stickfigure Filters | Stickfigure page › Effects (the effects browser) |
| Segment Tools (all, including the extras for each shape) | the Segment page (section 4) |
| Camera Tools and camera effects | Camera mode and the Camera page |
| Textfield Tools | the Text page |
| Movieclip Tools | the Movieclip page; editing a movieclip |
| Sounds (per frame) | Sound rows and the Sound page |
| View options | the View card |
| Settings dialog | the Settings screen |
| Export dialog | the Export sheet |
