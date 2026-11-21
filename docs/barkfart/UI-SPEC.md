# Editor screen spec (v2, after design review 2026-11-17)

First interactive screen: pose figures across frames and play back.
Arrangement follows the original (UI.md "Verified layout"): sidebar left,
Undo/Redo top-left, frame strip top, Play top-right, stage centre, floating
tool windows. Units are points. Reference device: iPhone landscape 844 x 390
pt; safe insets apply on BOTH sides (47 pt, 59 on Pro) plus 21 pt at the
bottom. Panel colours run under the insets; controls never sit inside them.
**Scaling (2026-11-17):** every size here is multiplied by an automatic
scale from the screen height in points (phones 1.0; tablets height/656,
capped 1.0-1.5: iPad mini ~1.13, Air/11-inch ~1.25, 12.9-inch 1.5) and by
the user's "UI size" (80-180%, saved on the device, in the sidebar when
nothing is selected). Fonts are re-rasterized at the resulting pixel size.

## Regions
| Region | Phone | Large (iPad) |
|---|---|---|
| Sidebar | 168 pt wide | 200 pt |
| Frame strip | 72 pt high: 54 pt cells + 6 pt padding top/bottom + 3 pt scroll bar + 3 pt | 96 pt, 128 x 72 cells |
| Stage | the rest (about 582 x 297 on a notched phone; 16:9 stage fits at 528 x 297) | |
Below 700 pt window width on iPad, the sidebar collapses to a 56 pt rail that
opens as an overlay (later).

## Sidebar (12 pt padding, 8 pt gaps; paired buttons 68 x 40 pt)
1. **Undo | Redo**; disabled = 40% opacity.
2. Readout (11 pt, tabular digits): "Figure · 120%" / "Camera · 120%".
3. Segmented control **Figure | All | Camera** (three 46 x 40 cells).
   Figure: handles of the selected figure only. All: every figure's handles.
   Camera: drags pan the camera layer, pinch zooms it (keyed, undoable);
   a 2 pt accent border is inset around the stage while in Camera.
4. Context panel (scrolls, fades at the edges), titled by the selection,
   names truncated in the middle; joints without names read "Joint 7".
   - Nothing selected: **Add figure** (accent outline; the only accent use
     outside the three below), Onion skin on/off, FPS - / +.
   - Figure / joint selected: **Previous | Next** figure, **Copy pose |
     Paste pose**, **Drag: Rotate | Stretch**, joint angle as a scrub value
     (drag sideways, 1 degree per 4 pt) with - / + (40 x 40), Delete figure
     last (immediate; undo via the status line).
5. Status line (11 pt): "Frame 4 / 12 · 0.10 s" (frame 1 = 0.00 s); tap to
   jump to a frame number. Transient messages replace it for 4 s, e.g.
   "Deleted man · Undo" with Undo tappable.

Auto-key is always on: every edit keys the current frame. There is no toggle.

## Dope sheet (replaces the frame strip; user decision 2026-11-17)
The timeline is a dope sheet, not a strip of per-frame snapshots. It sits
where the original's strip sits (top, right of the sidebar), Play stays at
the top-right.
- **Collapsed (default, 72 pt):** time ruler (18 pt: frame numbers every
  5 / 10 frames by zoom, ticks per frame) and a summary row (all keys of the
  shown figures as diamonds). Playhead: accent 2 pt line with a grab tab on
  the ruler; drag to scrub (snaps to frames), tap the ruler to jump.
- **Expanded:** drag the sheet's bottom edge (a 4 pt grip, 44 pt hit) down,
  up to 60% of the screen height. Rows: per figure (header row, tap to
  collapse), then one row per joint (rotation and length keys share it),
  plus Move (layer position) and Camera rows. Filtered to the selected
  figure by default; a Selected | All filter sits at the left of the ruler.
  Row labels in a 72 pt column on the left of the sheet.
- **Keys:** 9 pt diamonds; key-on-frame filled, selected = accent. Tap a key
  = select it (tap empty = clear). Drag a selected key = retime all selected
  keys, snapped to frames (one undo step). Long-press empty space then drag
  = box select. Drag empty space = scroll time; pinch = zoom time.
- **Sidebar with keys selected:** title "N keys"; Copy | Paste (at the
  playhead); Delete; easing Hold | Linear | Ease (in-out) | Ease in |
  Ease out; Offset along chain - / + frames (stagger), Scale time 50% | 200%.
- **Length:** the symbol's duration end is a handle at the end of the ruler;
  drag to change it. Play loops over it.
- Frame-by-frame survives as behaviour, not layout: posing at the playhead
  keys the dragged joint (auto-key); "Paste pose" keys the whole figure.

## Stage
- Surround #17191c; stage rectangle white, no border.
- Handles: joint = 7 pt white ring with 1.5 pt black 60% outline; root =
  9 pt square #ff9f0a; selected joint = accent fill + 2 pt halo. Hit radius
  22 pt; overlaps go to the nearest centre, preferring the selected figure.
- Drag a handle (after 8 pt of movement) = pose; no separate select step.
  Root square = move the figure. Tap empty stage = deselect. Drag empty
  stage = pan the view; pinch = zoom the view; double-tap = fit.
  Two-finger tap = undo; three-finger tap = redo.
- iOS: defer system edge gestures on top and bottom so strip and stage drags
  are not stolen; hide the home indicator while editing.
- Onion skin: 2 frames back (red), 1 forward (blue), from the evaluator.

## Visual language
- Colours: panel #1e2024, control #2b2e33, control pressed #363a40,
  selected segment #40454c with 1 pt #5a6068 inner border, surround #17191c,
  text #e6e6e6, secondary #9aa0a6, accent #0a84ff (only: current frame
  outline, selected joint, Camera border), destructive #ff453a, root handle
  #ff9f0a.
- Inter (OFL) via gdx-freetype at 11 / 13 / 15 pt; titles semibold;
  tabular digits for numbers. Words, not icons, except play/stop and "+".
- 4 pt grid; controls 40 pt tall, 44 pt touch targets; corner radius 4 pt on
  controls, 0 on frame cells; no gradients; shadow only on floating windows.

## Floating windows (phone)
Never open on their own; snap to stage corners; collapse to a 44 pt tab;
nudge pad at most 128 x 128 pt.

## Missing states
- New project: one frame, one default figure centred, Figure mode; the first
  touch poses something. No welcome screens.
- 100+ frames: 4-digit numbers fit; the strip auto-centres the current frame
  after jumps and during play.

## What this fixes from the original (UI.md)
One context panel instead of separate tool menus; words instead of ~150
icons; no "lag problems" or memory readouts; no nags; 44 pt targets; deletes
are undoable instead of confirmed; always-on auto-key behaves like the
original's per-frame snapshots but stores sparse keys.

## Out of scope for this screen
Curve editor, layers panel, creation mode, effects, sound, export, files.

## v3 (2026-11-21): layout after the user's reference of the original
The user documented the original editor control by control (kept locally in
docs/barkfart/research/local, not committed because it quotes the app).
Barkfart now follows its layout and flow, in its own words and style:
- Left panel: fixed header (Undo, Redo, "Mode: Normal (zoom %)", three mode
  buttons Normal / Panning / Camera), then one scrolling list of sections
  that follows the selection, and a status line.
  Nothing selected: App Tools, Project Tools, Animation Tools, Library Tools.
  A frame picture tapped: Frame Tools. A figure: Stickfigure Tools, Segment
  Tools. Camera mode: Camera Tools.
- Section tabs above the stage jump to each section (can be hidden).
- Filmstrip of frame pictures (drawn through the camera); top-right grid:
  Add frame, Copy frame, Play / Delete frame, Paste frame, Stop; View
  options opens a panel on the right edge.
- Frames are whole poses. Animation Tools: FPS (new projects 10), Tweening
  on, Tweened frames 0-8: playback and export add in-between pictures
  (at most 60 a second). Frame Tools: Add tweened frame at 50%, 33/67% or
  10/90% (hold for N evenly spaced frames), only between two frames; Do not
  tween this frame.
- Stickfigure Tools: delete (this frame; hold for every frame), edit (pencil),
  previous / next, copy / paste figure, properties and pose, flip both ways,
  layer down / up with position, centre, colour, scale, angle (+45), locked,
  visible in onion skin, drag rotate / stretch, tween easing cycle (Linear,
  Ease, Ease in, Ease out, None), cut here.
- Segment Tools (the segment ending at the selected joint): flip, colour,
  thickness, length, stretchy, lock node, drag lock. Adding, deleting,
  splitting and shapes are in the figure editor only (as in the original's
  creation mode).
- Camera Tools: zoom, rotation, centre, reset; drag on the stage moves the
  camera.
Deliberate differences: the pencil edits the figure itself on every frame
(the original makes a new library figure); Open and the figure picker are
windows with a filter (the picker shows pictures); playback quality and the
frame cache are ours.
Not yet: textfields, movieclip editor, filters, sounds per frame, frame
delay and repeat, settings, magnifier and arrows windows, join, loop tween.
