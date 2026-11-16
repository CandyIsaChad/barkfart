# Editor screen spec (v2, after design review 2026-11-17)

First interactive screen: pose figures across frames and play back.
Arrangement follows the original (UI.md "Verified layout"): sidebar left,
Undo/Redo top-left, frame strip top, Play top-right, stage centre, floating
tool windows. Units are points. Reference device: iPhone landscape 844 x 390
pt; safe insets apply on BOTH sides (47 pt, 59 on Pro) plus 21 pt at the
bottom. Panel colours run under the insets; controls never sit inside them.
iPad and windows >= 700 pt wide use the "large" metrics.

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

## Frame strip
- Cells 96 x 54 pt (16:9, ready for thumbnails), 4 pt gap, 0 pt corner
  radius. Number top-left, 11 pt tabular. A frame with its own keys is a
  normal cell; an in-between frame is dimmed with a tween-arrow badge along
  the bottom. Current frame: 2 pt accent outline.
- Tap = go to frame. Tap the current frame again, or long-press (400 ms,
  8 pt slop, haptic) = frame menu: Insert after, Duplicate, Delete, Hold /
  Tween to next. Horizontal drag scrolls with momentum; no reordering in v1.
- Last cell "+": adds a frame after the last, repeating its pose.
- 3 pt scroll indicator under the cells.
- Play 56 x 56 pt at the top-right, inside the right safe inset; turns into
  Stop. Play loops; handles hidden and panel disabled while playing; the strip
  keeps the playing frame in view. Stop or a tap on the stage returns to the
  frame where playback started.

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
Dope sheet, layers panel, creation mode, effects, sound, export, files.
