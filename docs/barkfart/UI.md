# UI plan: familiar, then better

Goal: someone who knows Stick Nodes picks up Barkfart without a tutorial. Then
it fixes what is painful. Same layout, workflow, gestures and generic words
(node, segment, stickfigure, frame, tween, onion skin). Icons, wording and
skin are our own (see CLEANROOM.md).

Evidence used: the reference bundle's UI atlases (icon inventory) and English
UI strings (tool and menu inventory), plus the four public App Store
screenshots of the current app (viewed 2026-11-17 for layout only; nothing
copied or committed). Gestures are still unverified.

## Verified layout (landscape, from the App Store screenshots)
- **Left sidebar**, about 14% of the width, full height:
  - top: Undo and Redo side by side; a mode readout with zoom ("Mode:
    Normal (260%)"); three large mode buttons (single figure, all figures,
    camera); a "lag problems?" help button;
  - middle: a scrolling context panel titled by what is selected
    ("Stickfigure Tools", "Segment Tools", "Textfield Tools", sound
    controls), stacked two-column buttons and value fields;
  - bottom: memory readout with a help button, and up/down arrows that
    scroll the panel.
- **Top strip**, about 15% of the height, right of the sidebar: frame
  thumbnails left to right, each numbered in its top-left corner, with small
  badges along the bottom (tween arrow, sound, frame tools); the current
  frame has a blue outline; a blue scrollbar runs under the strip. Top-right
  corner: three buttons (frame/camera tools, copy, play).
- **Stage**: the rest of the screen. A darker stage rectangle (the export
  frame) sits on a grey surround; figures show small circular handles at
  every joint and a square handle at the main node.
- **Floating windows** over the stage, each with a red X: a magnifier
  (live zoomed view, lock button, -/+), a nudge pad (arrow buttons, nudge
  amount -/+), an onion/preview toggle ("Not showing nodes").
- **Creation mode** (figure editor): the top strip becomes a row of draw
  tools (None, Segment, Rounded segment, Circle, Ellipse, Triangle,
  Trapezoid, Polygon, Polyfill), and the left panel shows segment tools
  (segment colour, gradient colour, reverse gradient, segment scale).
- Style: flat light-grey panels on dark grey, blue accent, red titles.

Barkfart keeps this arrangement (sidebar left, frame strip top, stage
centre, floating tool windows) so it feels the same, and changes what is
inside it (see the table below).

## Inventory, with a verdict for each

| Area | What exists (evidence) | Verdict |
|---|---|---|
| Canvas | Modes "Normal / Panning / Camera" shown as a text label with zoom %; three mode buttons in the sidebar (verified) | **Fix**: keep the modes, show them as a visible segmented toggle, two-finger pan/zoom always works |
| Node handles | Tap a node to select, drag to rotate/stretch; lock node; smart-stretch with its own "what is this" window | **Keep** the gestures. **Fix** smart-stretch: a toggle with a live preview instead of an explainer window |
| Quick Menu | Radial/quick buttons near the selection; a tooltip tells users how to turn it off | **Fix**: keep it, make it smaller and positioned away from the finger, configurable rather than all-or-nothing |
| Tool menus | Separate Frame / Stickfigure / Segment / Camera / Polyfill tool menus, each with its own copy/paste per property (color, filters, sound, camera, textfield, segment, segment+children) | **Replace** with one context panel for whatever is selected, one clipboard, and Paste Special (choose properties) |
| Frame strip | Top strip of frames (verified); tweened frames only between two frames; "copy this frame plus N after" | **Keep** as the default view (brief decision 8). Backed by tracks, so tweens are per-property and need no "between two frames" rule |
| Limits and warnings | Max stickfigures per frame; frame-count memory warning | **Remove**: these come from the full-snapshot model; the track core avoids them |
| Filters | Per-stickfigure filters, "may cause lag", visible only on stage, quality toggle | **Replace** with effect stacks (M6) with keyframes, cached preview, honest quality setting |
| Sounds | Attached to frames; dialogs for missing or removed sound files | **Replace** with audio layers (M7); missing files get a relink panel, not a modal dialog |
| Polyfill | Modal multi-step creation with three separate error dialogs | **Fix**: inline guidance on the canvas, no blocking errors |
| Stickfigure editor | Segment tools, thickness (0 = invisible), color, scale, join figure to node, lock to camera | **Keep** the scope. **Fix** naming and grouping |
| Icons | About 150 icons in at least four styles (flat blue, black glyph, orange, red, glossy 3D, skeuomorphic film strips); many near-duplicates (copy, copy-1, copy-children) | **Replace** with one consistent original icon set plus text labels where meaning is ambiguous |
| Onboarding / nags | Welcome screens, rate-us art, Pro upsell buttons on the splash | **Remove** |
| Copy tone | Jokey error text | **Fix**: short, plain, says what to do next |

## Principles
1. The default screen looks and works like the original. Depth (dope sheet,
   layers, effects, graph editors) is opt-in, never in the way.
2. One selection model, one context panel, one clipboard.
3. No modal dialog for something the canvas can show inline.
4. Touch targets at least 44 pt. Nothing important sits under the finger.
5. Every action is undoable, including selection-free ones like camera moves.

## Needed from the user
- Screenshots or a short screen recording of: the main editor (nothing
  selected, a node selected, a figure selected), the frame strip, the
  stickfigure editor, the tool menus, the export screen.
- The worst three or four pain points from daily use.
