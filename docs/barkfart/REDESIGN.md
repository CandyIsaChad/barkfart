# Redesign (v4): plan and decision log

Status: agreed with the user on 2026-11-22/23 over nine rounds of mockups.
This replaces UI-SPEC.md v2/v3 wherever they disagree. The mockups are on
the claude.ai canvas "Barkfart Redesign"; their generators were session
scratch files, so **this document is the source of truth, not the boards**.

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
- **D15. UI technology: no ECS.**
  - Flutter is already a retained, diffed tree, and the Rust engine owns all
    document state (Session JSON commands).
  - An ECS would be a third model to keep in sync.
  - Performance comes from these instead:
    - CustomPainter for the stage, timeline cells and Preview row;
    - thumbnails rendered by the engine and cached by (layer, frame, revision);
    - only visible rows and frames built;
    - one immutable snapshot of the engine state per change, with widgets
      reading only their slice of it.

## 7. Build plan

Each pass ends with:
- a Linux build;
- screenshots compared side by side with the matching board;
- fixes;
- commits per feature.

Behaviour must not regress: every edit is undoable and tests are never
weakened. Laptop first; iOS builds only near "good enough to ship".

0. **Components.** Tokens, Open Sans, rails, word buttons, tick boxes,
   segmented controls, red section titles, panel cards, swipe rows, the
   swatch row, the stepper with a changed-dot, hint bubbles. Check each on a
   gallery screen behind a debug flag.
1. **Shell, on today's model.**
   - Top bar, rails, checkerboard and stage.
   - The Properties panel holding today's tool groups, mapped onto the new
     pages.
   - The "+" menu, absorbing Library Tools.
   - The folded timeline: toolbar, ruler, and Preview as today's frame
     strip.
   - Same behaviour, new look. Visually 1:1 with the boards.
2. **Engine: layer keys** (D2, D3).
   - Whole-figure key rule, Hold or Tween per key, Add frame, New pose and
     Hold longer as commands.
   - The break-off command.
   - Legacy import and play-through must match today's output (tests).
3. **Expanded timeline.** Rows, quiet blocks, grips, the Pose page, the tiled
   Preview, the pill playhead, gestures, swipe actions, the selection box.
4. **Keyed properties and effects.** The keying toggle, property rows and
   diamonds, the effects browser (built-ins), the FX toggle.
5. **Joins over time** (D8). Engine, chain markers, the join popover.
6. **Figure editor.**
7. **Polish passes.** Repeat side-by-side comparisons until nothing stands
   out. Then the screens still to design (section 8).

Later: groups, the plugin runtime and Script layers, emitters.

## 8. Not designed yet

Each gets a mockup round before its pass:
- **Projects screen:** recent projects, new, open, an empty first run.
- **Export sheet:** video, GIF or frames; range; watermark; vertical.
- **View options:** onion settings, guides, which handles show, UI size,
  the "Running slow?" tips, memory.
- **Settings.**
- **The "+ Stick figure" chooser:** stock figures, yours, effects.
- **Pose page** as a panel page (it was drawn only as a card).
- **Properties pages** for the camera, text, picture, movieclip and sound.
- **Editing a movieclip:** breadcrumbs back to the scene.
- **Group rows,** and Properties when several things are selected.
- **States:** missing plugin, unsaved prompt, backups, errors, empty
  timeline.
- **iPad and resizable Linux windows:** what grows, and when the panels dock.
