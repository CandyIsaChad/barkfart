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
| checker | #6C6F74 / #606368, 16 px | offstage |
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
A white stage on the checkerboard. On the stage:
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
- [ ] B3. The Animation popover: FPS, smoothness (tweened frames),
  looping, fullscreen. **Built right after C7**, because its anchor is the
  timeline toolbar's "fps" button, which C7 adds.
- [x] B4. The View card (today's View options).
- [x] B5. The Export sheet. Watermark and "Make vertical" join it when C5
  removes Project Tools.
- [x] B6. The Settings screen.

### C. The editor shell (frames are still snapshots)
- [ ] C1. Checkerboard offstage.
- [ ] C2. Top bar: Projects, View and Export words, the project name, the
  unsaved dot.
- [ ] C3. Left rail: the mode pictograms.
- [ ] C4. Right rail: undo, redo, Properties, delete, hide panels.
- [ ] C5. The Properties card holds today's sections, tabbed by selection.
  It replaces the sidebar and the tab strip.
- [ ] C6. The "+" menu (absorbs Library Tools) and the stick figure chooser.
- [ ] C7. The folded timeline: control toolbar, fps ruler (D16), tiled
  Preview row. It replaces the frame strip and the top-right grid.
- [ ] C8. Big screens dock Properties (D20).

### D. Properties pages as descriptors (D15)
- [ ] D1. Sound.
- [ ] D2. Movieclip.
- [ ] D3. Text.
- [ ] D4. Camera.
- [ ] D5. Frame.
- [ ] D6. Stickfigure.
- [ ] D7. Segment, with changed-dots and swipe-to-reset. The engine
  reports file defaults.

### E. Home
- [ ] E1. First run.
- [ ] E2. The Projects screen: saved projects with thumbnails.
- [ ] E3. The backups list.

### F. Engine: layers keep their own keys (D2, D3)
- [ ] F1. The break-off-and-join command.
- [ ] F2. Hold or Tween per key, read by the evaluator. Tweened frames
  become "smoothness".
- [ ] F3. The whole-figure key rule replaces anchor_poses. Legacy imports
  must play exactly as they do today.
- [ ] F4. Commands: Add frame, New pose, Hold longer, moving and extending
  blocks.

### G. The expanded timeline
- [ ] G1. Layer rows, the names column, eyes (read-only).
- [ ] G2. Quiet blocks, with selection opening the Pose page.
- [ ] G3. Grips to extend, long-press to move, pinch to zoom.
- [ ] G4. Swipe actions, and right-click on Linux.
- [ ] G5. Selection box, lasso, the multi-select page.

### H. Keyed properties and effects
- [ ] H1. The FX toggle.
- [ ] H2. The effects browser: built-ins with previews.
- [ ] H3. The keying toggle, property rows, diamonds.

### I. Joins over time (D8)
- [ ] I1. Engine: the parent becomes a value over time, with offsets stored.
- [ ] I2. Chain markers, the join popover, toasts with Undo.

### J. The figure editor
- [ ] J1. Header and the save-as-new sheet (D12).
- [ ] J2. The shape shelf.
- [ ] J3. The Parts list.
- [ ] J4. Shape-coded handles, the size reference.
- [ ] J5. Ghost and readout while drawing; the Mirror toggle.
- [ ] J6. Polyfills in either winding direction.

### K. Hardest, later
Groups; the Luau plugin runtime (it needs the `mlua` crate, so it waits
until a dependency fetch is approved); Script layers; emitters.

## 8. Designed in round 10 (2026-11-23)

The boards are on the canvas under "Round 10". All of them use the same
components.

- **Projects** (the home screen):
  - top bar: the title "Your animations"; on the right, "Open a file…",
    "Backups" and "Settings";
  - a grid of cards, four across on phones, each with a thumbnail, name,
    unsaved dot, and "N frames, S seconds. Edited …";
  - the first card is "New animation".
- **First run:** a waving stock figure, "No animations yet", the blue "Make
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
