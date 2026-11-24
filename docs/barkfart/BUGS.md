# Barkfart bug hunt (Linux release build, 2026-11-24)

**Summary.** Barkfart was driven through scripted real touches on the Linux release build, mainly at iPad sizes (1180x820 landscape, 820x1180 portrait) and at 844x390 (iPhone landscape) as a secondary check. Nothing crashed and no script hung. Core editing (scrubbing, grips, joint drags inside a pose, group, save and reopen, GIF/PNG/video export, figure editor drawing) works, and all six recent fixes hold at both iPad sizes (see "Recent fixes at iPad size"). The problems found are mostly layout: floating panels that cover what is being edited or play back, controls clipped at phone size, one broken Redo, and popovers that drift when UI size is not 100%.

**Count by severity:** crash 0, broken 0, wrong 7, rough 8. Plus 12 open questions (all six recent fixes pass). Not reproduced: the flaky "figure drawn bent" item in HANDOFF (0 of 4 runs).

**Which build.** All findings are against the prebuilt binary app/build/linux/x64/release/bundle/barkfart (presumably built from HEAD dc12901; not verified). While I worked, the working tree gained uncommitted edits that are not mine (app/lib/editor.dart, app/lib/ui/kit.dart, docs HANDOFF.md and REDESIGN.md). Their diff touches the Magnifier mount, `kit.Anchored`, `kit.Tap` and several popover call sites, so the popover and tap-size findings below may already be addressed in the tree and need re-checking after the next build. Code cited here was read from the live tree.

**How the sizes were made.** This machine's screen is 1366x768, so the window manager caps any window above about 1299x699. iPad sizes are therefore emulated: 1180x820 = BARKFART_WINDOW=1006x699 plus script command `uiscale 0.852`; 820x1180 = BARKFART_WINDOW=480x690 plus `uiscale 0.585`. Shots come out at the true logical size (1181x821, 821x1180). The helper /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/r.sh does this for you: `/tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/r.sh "<script>" ipadL|ipadP|844x390 [docsdir] [open file]`. Swipe coordinates in this file are logical (shot pixels); r.sh multiplies them by the scale before sending. Because of the emulation, a few results only hold for "UI size not 100%" (marked). Screenshots are in /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ (the named PNGs are single shots; mm.png is only a scratch montage and is overwritten).

## Status (2026-11-24, after the hunt)

Fixed and checked on the Linux build at iPad sizes (tests in brackets):
- Redo after undoing "Edit a copy": Undo at the editor's start now leaves it as Cancel does, history and all (session test `undo_in_the_figure_editor_stops_at_its_start_and_redo_brings_nothing_stray`).
- Properties card covering the figure (iPad portrait): selecting a figure the card would hide eases the view so it sits in the open part of the stage (after the finger lifts; also when the card is opened from the rail).
- "Playing" card over the stage: the card steps aside while playing.
- No refit on rotation: a new screen width fits the stage again unless the view was zoomed or panned by hand (the timeline folding or a docked panel keeps the zoom).
- Popovers off their button at UI size below 100%: the anchor is measured in the overlay's own (scaled) space.
- Right rail clipped at phone size: it scrolls (checked at 844x390: a swipe up on the rail brings Delete and Hide the panels into view).
- "Save changes?" alert: every `_ask` question is the kit sheet (last button blue, Don't save red).
- Emoji arrows: "Previous joint" / "Next joint".
- Imported figures stacking: they step aside from a figure already there (test `placing.rs`).
- Card buttons under 44 pt: they keep their look and take touches a few points beyond their edges (`kit.HitPad` on the button and on its row; a row alone turns away touches outside itself). Checked at 1180x820: a tap 2 pt below Flip sideways flips, 2 pt above Send back sends back.
- Settings labels: shorter labels, details in the hint; a long value gives way, never the label.
- Names column: 180 pt on a tablet held upright.
- Question 1 (grow then shrink): a figure whose last pose ends the animation moves the end with its poses both ways, so it's a round trip (test `lengthening_then_shortening_a_pose_is_a_round_trip`).

Still open: the phone-size Properties card and Effects browser (little room), the phone-size Animation popover's rows, the hint bubble over the canvas on phones, and Questions 2 to 12.

## Findings, most severe first

### Redo after undoing "Edit a copy" leaves a stray "(copy)" layer and a hidden original
- Severity: wrong
- Where: figure editor entry, iPad landscape 1180x820
- Steps (r.sh ... ipadL): `wait 1.2; tap 0; wait 0.8; editcopy; wait 1.2; swipe 1149 72 1149 72; wait 0.8; swipe 1149 120 1149 120; wait 0.8; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/fe3.png; quit`
- Expected: Undo from a fresh figure editor either does nothing or leaves the editor; Redo puts things back exactly (back in the editor) or is not offered.
- Actual: Undo (rail, active as soon as the editor opens) leaves the editor and the animation is as before. Redo then does not re-enter the editor: the animation now has a visible "Stickfigure (copy)" layer and the original "Stickfigure" row is greyed with an empty cell. Cancel in the editor restores cleanly (checked), so only Undo then Redo breaks it.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/fe3.png (crop /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/fe3c.png), the undo step before it: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/fe2.png

### Properties card covers the figure being edited (iPad portrait)
- Severity: wrong
- Where: any selection, iPad portrait 820x1180
- Steps: `wait 1.2; swipe 560 1159 560 1159 (x4, Add frame); swipe 263 1123 263 1123; wait 0.5; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pq1.png; quit` (r.sh ... ipadP)
- Expected: the selected figure stays visible and reachable.
- Actual: the card is 360 pt wide and opens by default; with the 64 pt rails it leaves about 330 pt of stage, and the figure sits in the middle (x about 410), so the card (x 389 to 749) hides the figure's right half. A drag starting on the hidden half lands on the card and does nothing (a joint drag at (435,527) did nothing until the card was closed with the Properties rail button). On Pose and Frame pages the card is also mostly empty space.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pq1.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pq2.png (Frame page)

### During playback a "Playing" card covers the right half of the stage
- Severity: wrong
- Where: playback, iPad landscape (the same card opens at every size)
- Steps: `wait 1.2; swipe 922 800 922 800 (x4); swipe 241 705 241 705; wait 0.4; play; wait 0.35; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pl1.png; stop; quit` (r.sh ... ipadL)
- Expected: playback shows the whole picture.
- Actual: the stage zooms to the camera, and a card titled "Playing" (three lines of help text, otherwise empty) covers from x 750 to 1110, cutting the figure off at the card edge.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pl1.png

### Stage does not refit when the window size changes (rotation)
- Severity: wrong
- Where: stage, iPad portrait (emulated resize)
- Steps: `wait 1; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/i_port.png; quit` (r.sh ... ipadP). The view is fitted once at the first layout (editor.dart `_fit`, only reset on load/new), then the window size changes when `uiscale` is applied.
- Expected: after rotating the iPad the canvas is refitted to the stage.
- Actual: the canvas stays at 51% (View > Helpers > Zoom says 51%) as a 330x185 box in a 690-wide stage. "Fit stage" in View fixes it (View panel scrolled to the bottom). A real rotation takes the same code path.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/i_port.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/v1.png (Zoom 51%), /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pf.png (after Fit stage)

### Popovers anchored to a button land away from it when UI size is not 100% (verified on a size-scaled run)
- Severity: wrong
- Where: the "10 fps" Animation popover; seen at 844x390 with UI size 75%, and in every iPad run (emulated by UI size)
- Steps: `wait 1; animation; wait 0.6; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/u1.png; back; uiscale 0.75; wait 0.6; animation; wait 0.6; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/u2.png; quit` (r.sh ... 844x390)
- Expected: the popover sits next to the fps button at any UI size.
- Actual: at 100% it is clamped near the top right; at 75% (Settings allows 75 to 150) it appears about 275 pt left of the button, the window-space anchor not scaled with the UI. At iPad landscape the Animation popover sits in the middle of the stage, 130 pt left of "10 fps". Projects and + menus (anchored near the left or top edge) look right.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/u1.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/u2.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/q1.png (iPad landscape)

### Right rail clipped at phone size: Delete half hidden, "Hide the panels" unreachable
- Severity: wrong
- Where: editor, 844x390, timeline open
- Steps: `wait 1; swipe 810 120 810 60; wait 0.4; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/rail1.png; quit`
- Expected: Undo, Redo, Properties, Delete and Hide the panels all reachable.
- Actual: the rail is a plain non-scrolling Column (editor.dart `_rightRail`); the Delete button is cut off at the toolbar and Hide the panels is below it, off screen. Swiping the rail does not scroll it. Folding the timeline brings them back. Fine at both iPad sizes.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/rail1.png

### Properties card shows about one row of content at phone size; the Effects browser a sliver of tiles
- Severity: wrong
- Where: Stickfigure, Segment, Effects pages; 844x390, timeline open
- Steps: `wait 1; tap 0; wait 0.6; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/p_fig.png; effects; wait 0.8; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/s1.png; quit`
- Expected: enough of the page visible to work with.
- Actual: the card is about 170 pt tall; the tabs, the name row and the pinned red "Delete stickfigure" button fill it, leaving about 30 pt for the scrolling body (the colour swatches are cut in half). In the Effects browser only the top edge of the first tile row shows. The View card shows 3 rows. Fine at iPad sizes.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/p_fig.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/s1.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/m_view.png

### Settings labels truncated
- Severity: rough
- Where: Settings, 844x390, 1180x820 and 820x1180
- Steps: `wait 1.2; settings; wait 0.8; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/x3.png; quit`
- Expected: labels read in full (or the long value wraps).
- Actual: "Graphics" shows as "Graph..." (phone, iPad portrait) or "Gr..." (iPad landscape) before the long GPU string; "Save reminder (minutes, 0: ..." and "Double-tap the stage for Select (figure e..." are cut.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/x3.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/m_settings.png

### Layer names truncated in the names column
- Severity: rough
- Where: timeline, 844x390 and 820x1180 ("Stickfi..."); 1180x820 for longer names ("Classic Stickfi...")
- Steps: `wait 1; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/a.png; quit`
- Expected: a readable name, or the column wide enough.
- Actual: the column is about 140 pt wide on phone and iPad portrait, and about 210 pt at 1180 wide; "Stickfigure" is already cut at 140.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/a.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pp0c.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/mf1.png

### "Save changes?" is an old Material alert, not the redesigned sheet
- Severity: rough
- Where: Projects > New animation (and Open) with unsaved changes; 1180x820
- Steps: `wait 1.2; swipe 922 800 922 800; wait 0.5; swipe 51 20 51 20; wait 0.7; swipe 80 71 80 71; wait 1; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/na1.png; quit`
- Expected: the kit sheet "Save your work?" (REDESIGN B1).
- Actual: `_leaveUnsaved` calls `_ask`, a stock AlertDialog (grey card, three small plain text buttons Cancel / Don't save / Save with no emphasis on Save). The Export-finished dialog uses the same `_ask`.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/na1.png

### Emoji arrows on the main node's "Joint" buttons
- Severity: rough
- Where: figure editor, Figure Tools with the main node selected; 1180x820 (and 844x390)
- Steps: `wait 1.2; tap 0; wait 0.8; editcopy; wait 1.2; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/fe1.png; quit`
- Expected: blue words or a pictogram, like every other row.
- Actual: "◀ Joint" and "Joint ▶" draw orange emoji squares (on iPhone and iPad these will be Apple emoji). Zoom: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/jc.png.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/fe1.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/jc.png

### Imported or added figures land exactly on top of the existing one
- Severity: rough
- Where: + > Stick figure, 1180x820
- Steps: `wait 1.5; swipe 24 800 24 800; wait 0.8; swipe 96 425 96 425; wait 1.5; swipe 882 300 882 300; wait 1.5; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ad5.png; quit` (then add Classic Stickfigure and Block Head the same way)
- Expected: a new figure is easy to tell apart, or offset.
- Actual: every added figure appears at the same centre point, so joint handles of up to three figures overlap and tapping picks one without telling you which.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/mf1.png

### Animation popover shows 4 of 7 rows at phone size and sits over the top bar
- Severity: rough
- Where: "10 fps" popover, 844x390
- Steps: `wait 1; animation; wait 0.6; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/u1.png; quit`
- Expected: all the rows, or a clear scroll cue.
- Actual: the popover fills the stage height (about y 8 to 195) over the top bar, showing 4 rows and a sliver of the fifth; "Effects while playing" and "Fullscreen when playing" are below the fold with no hint they scroll.
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/u1.png

### Hint bubble overlaps the canvas on phone
- Severity: rough
- Where: nothing selected, 844x390
- Steps: `wait 1; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/a.png; quit`
- Expected: the hint stays off the canvas.
- Actual: the white hint bubble (x 452 to 772, y 48 to 112) covers the right third of the canvas. It stays while nothing is selected and goes when a figure is selected (the card replaces it).
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/a.png

### Card buttons and the Magnifier/Nudge controls are under 44 pt tall
- Severity: rough
- Where: every grey card button (Flip sideways, Copy frame, ...), the Magnifier window, Nudge arrows; 1180x820 (same code on every size)
- Steps: `wait 1.2; tap 9; wait 0.5; magnifier; wait 0.8; arrows; wait 0.6; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/mg2.png; quit`
- Expected: 44 pt hit areas (kit.dart says "every tappable thing has a 44 pt hit area").
- Actual: from the code, `_Btn` (editor.dart) wraps its label in `kit.Tap(minSize: 0)` with height 38 (30 for the `small` variant), so the hit area is 38 or 30 pt, not 44. The Magnifier's "Joints shown" is the 30 pt variant and its minus and plus are 36 wide. On the shot they measure the same (about 30 to 38 pt).
- Screenshot: /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/mg2.png

## Recent fixes at iPad size

All runs: `/tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/r.sh "<script>" ipadL` (1180x820) or `ipadP` (820x1180); coordinates are logical (shot pixels). Two pieces repeat, so they are named here and pasted in full in each script below:

- `ADD` (landscape, five frames) = `swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.4`
- `SETUP_P` (portrait: five frames, then View > scroll > Fit stage > tap an empty stage spot, so the canvas is fitted) = `wait 1; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 124 20 124 20; wait 0.7; swipe 237 900 237 300; wait 0.4; swipe 159 754 159 754; wait 0.4; swipe 700 200 700 200; wait 0.5`

Landscape layout (five frames): cells at x 241, 287, 333, 379, 425; ruler y 705, Preview row y 731, figure row y 764; chevron (1157, 674) unfolded and (1157, 740) folded. Portrait: cells x 171, 217, 263, 309, 355; ruler y 1064, Preview y 1090, figure row y 1123; chevron (796, 1033) unfolded and (796, 1099) folded. Frame numbers printed by `state` are zero based (2/5 means frame 3).

**1. Scrubbing along the ruler and the Preview row: works at both sizes.**
- Landscape: `wait 1; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.4; swipe 241 705 333 705; wait 0.3; state; swipe 241 705 425 705; wait 0.3; state; swipe 425 705 287 731; wait 0.3; state; swipe 287 731 425 731; wait 0.3; state; swipe 333 705 333 705; wait 0.3; state; swipe 287 731 287 731; wait 0.3; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/f1.png; quit`. Printed 2/5, 4/5, 1/5, 4/5, 2/5, 1/5: every slide ends on the cell under the finger, including a slide that starts on the ruler and ends on the Preview row, and taps on both rows land on the cell tapped. No undo step was added (undo stayed "Insert frame").
- Portrait: `wait 1; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 124 20 124 20; wait 0.7; swipe 237 900 237 300; wait 0.4; swipe 159 754 159 754; wait 0.4; swipe 700 200 700 200; wait 0.5; swipe 171 1064 263 1064; wait 0.3; state; swipe 171 1064 355 1064; wait 0.3; state; swipe 355 1090 217 1090; wait 0.3; state; swipe 217 1090 355 1090; wait 0.3; state; quit`. Printed 2/5, 4/5, 1/5, 4/5.

**2. Block tap and right grip: works (lengthen and shorten); the Preview strip stays intact.**
- Landscape: `wait 1; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.4; swipe 333 764 333 764; wait 0.5; swipe 347 764 393 764; wait 0.5; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/g2.png; swipe 393 764 347 764; wait 0.5; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/g3.png; quit`. After the first drag: 6 frames, undo "Pose length", block 3 two cells wide, Preview strip has 6 thumbnails (/tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/g2.png, montage of g2 to g5 was in mm.png). After the second drag the block is one cell again, Preview still has 6 thumbnails.
- Portrait: `wait 1; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 124 20 124 20; wait 0.7; swipe 237 900 237 300; wait 0.4; swipe 159 754 159 754; wait 0.4; swipe 700 200 700 200; wait 0.5; swipe 263 1123 263 1123; wait 0.5; swipe 277 1123 341 1123; wait 0.5; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ps1.png; swipe 323 1123 259 1123; wait 0.5; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ps2.png; quit`. After the lengthen: 2/6, "Pose length", Preview 6 thumbnails. After the shorten: block 3 one cell, Preview 6 thumbnails (crops in /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ps1.png, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ps2.png, rows y 1040 to 1150). What the later blocks do on shortening is Question 1.
- Drag distance note: a 92 pt drag (two cells) lengthened by one cell in the 0.852 scale run and by two cells at 844x390 (no scale). A drag loses about 18 pt to the touch slop, so I used one cell plus 18 (64 pt) for one cell.

**3. Joint drag on a frame inside a pose changes that pose and adds no new pose: works at both sizes.**
- Landscape: `wait 1; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.3; swipe 922 800 922 800; wait 0.4; swipe 333 764 333 764; wait 0.5; swipe 347 764 411 764; wait 0.5; state; swipe 379 705 379 705; wait 0.4; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/h1.png; swipe 620 348 690 300; wait 0.5; state h; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/h2.png; quit`. After the drag: undo "Pose node", selected n10, the Pose page reads "frames 3 to 4", Length in frames 2, the timeline still shows one block over frames 3 and 4. The playhead stays on frame 4.
- Portrait: `wait 1; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 124 20 124 20; wait 0.7; swipe 237 900 237 300; wait 0.4; swipe 159 754 159 754; wait 0.4; swipe 700 200 700 200; wait 0.5; swipe 263 1123 263 1123; wait 0.5; swipe 277 1123 341 1123; wait 0.5; swipe 309 1064 309 1064; wait 0.4; swipe 788 168 788 168; wait 0.5; state; swipe 435 527 500 470; wait 0.5; state; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/pq3.png; quit`. The Properties rail tap closes the card first; then the drag gives undo "Pose node" and "frames 3 to 4". With the card left open the same drag did nothing (it landed on the card; see the portrait finding).

**4. The 10 fps popover, the Projects menu and the + menu stay fully on screen: works at both sizes.**
- Landscape: `wait 1; swipe 1111 674 1111 674; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/q1.png; back; wait 0.5; swipe 51 20 51 20; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/q2.png; back; wait 0.5; swipe 24 800 24 800; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/q3.png; back; wait 0.5; swipe 192 20 192 20; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/q4.png; quit` (the last one is the Export dialog, also fully on screen).
- Portrait: `wait 1; swipe 751 1033 751 1033; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/r1.png; back; wait 0.5; swipe 51 20 51 20; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/r2.png; back; wait 0.5; swipe 24 1159 24 1159; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/r3.png; back; wait 0.5; swipe 192 20 192 20; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/r4.png; quit`.
- The fps popover is on screen but not next to its button (see the UI size finding). At 844x390 the popover is on screen but shows only 4 rows (rough, above), and the Export dialog is taller than the window and scrolls (Export GIF reachable after one or two scrolls).

**5. Folding and unfolding the timeline does not stretch the figures: works at both sizes (fold and unfold both run).** The figure was measured as the bounding box of its blue handle rings in the stage area (script `/tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/meas.py`):
- Landscape: `wait 1; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/uf_a.png; swipe 1157 674 1157 674; wait 0.9; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/uf_b.png; swipe 1157 740 1157 740; wait 0.9; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/uf_c.png; quit`. Unfolded 88 x 241 at (546, 222); folded 88 x 241 (moved down 33 pt, re-centred in the taller stage); unfolded again 88 x 241 at (546, 222), identical to the start. The unfold tap at (1157, 740) hit the chevron (the folded toolbar sits 66 pt lower).
- Portrait: `wait 1; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 124 20 124 20; wait 0.7; swipe 237 900 237 300; wait 0.4; swipe 159 754 159 754; wait 0.4; swipe 700 200 700 200; wait 0.5; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ufp_a.png; swipe 796 1033 796 1033; wait 0.9; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ufp_b.png; swipe 796 1099 796 1099; wait 0.9; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ufp_c.png; quit`. 77 x 205 at (371, 420), then 77 x 205 at (371, 453), then 77 x 205 at (371, 420).
- Caution: after folding, the "Hide the panels" rail button moves to the old chevron height. A second tap at the old chevron position hides every panel (it did in my first landscape try, /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/z2.png). That is expected geometry, not a defect I would list, but easy to hit.

**6. No checkerboard offstage, including after moving the camera: works at both sizes.**
- Landscape (panels shown): `wait 1; swipe 31 163 31 163; wait 0.7; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/cm0.png; swipe 300 300 560 420; wait 0.5; swipe 300 300 560 420; wait 0.5; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/cm1.png; swipe 700 300 300 150; wait 0.5; swipe 700 300 300 150; wait 0.5; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/cm2.png; quit`. The strip left of the canvas (x 70 to 200, y 60 to 600) is one colour, (234, 236, 239), in all three shots (4 colours only where the blue camera outline crosses it). The camera box moved by (+520, +240) and then by (-800, -300) and goes off screen.
- Portrait: `wait 1; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 560 1159 560 1159; wait 0.3; swipe 124 20 124 20; wait 0.7; swipe 237 900 237 300; wait 0.4; swipe 159 754 159 754; wait 0.4; swipe 700 200 700 200; wait 0.5; swipe 31 163 31 163; wait 0.7; swipe 300 400 480 560; wait 0.5; swipe 300 400 480 560; wait 0.5; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/w2.png; swipe 600 800 100 300; wait 0.5; swipe 600 800 100 300; wait 0.5; shot /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/w3.png; quit` (this run also had the timeline folded first). The Camera Tools card covers the right half; the visible offstage is flat grey.

## Also checked and fine (iPad sizes unless noted)

Add frame, New pose and Hold longer buttons; box select with double-tap drag; hold-and-slide of a block (correctly refused with the toast "A pose can move only as far as the poses on either side" when neighbours are 1 frame); row tap select; Curves view (add value Main joint x, Value page, modifier browser: Follow and Link are greyed with "come later"); group (select all, Group, Ungroup/Make movieclip buttons, row nests); Stickfigure, Segment (thickness edit shows the blue dot and "Reset this segment"), Pose, Frame, Camera, Text pages; Effects browser (phone sliver aside); figure editor: Parts panel, drawing a segment and a circle from a joint, Cancel restores the project; save as Test1 and reopen with BARKFART_OPEN (frames, figure move and title kept, projects screen lists it with a thumbnail); first-run screen; Backups sheet; GIF, PNG zip and video (.mov) export all produced files with no error; undo and redo in the normal editor; magnifier; Nudge arrows; New animation sheet; hint bubble. Not tested: Sound (needs a file picker), Picture (same), pinch zoom on the timeline (single-finger harness; only the `zoomtimeline` helper exists), freehand lasso (a straight swipe has no area), Movieclip editing, Effects on a figure, Camera row after Add camera.

## Questions (unsure whether these are bugs)

1. **Grow then shrink with the right grip does not get back to the original animation.** Landscape: five frames, select block 3 (`swipe 333 764 333 764`), grow (`swipe 347 764 393 764`, 6 frames, block 3 two cells), shrink back (`swipe 393 764 347 764`). Frames stay 6, and the blocks become 1, 2, 3, 4 and one block over frames 5 and 6, instead of the original five one-frame blocks (growing inserts a frame; shrinking does not remove one, it slides the later poses up and holds the last pose for two frames). Same in portrait (`swipe 323 1123 259 1123`, crops in /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/ps1.png and ps2.png). Evidence: state 2/6 with undo "Pose length" after each drag. All poses were identical, so I could not tell which pose moved. May be intended ripple behaviour, but the round trip changes the animation.
2. **Figure picker sometimes ignores the tile tap.** `wait 1; swipe 24 800 24 800; wait 0.6; swipe 96 425 96 425; wait 1; swipe 882 300 882 300; wait 1; state` (ipadL) imported Cat in 4 of 6 runs; in 2 runs the tile tap did nothing (state kept handles=12, no toast, no dialog), and once a 3-add run lost its first two picks and kept the third. With 1.5 s waits it worked in the one run tried. May be a harness timing issue on this loaded machine (3 GB, swapping) rather than the app.
3. **Duplicated figure has the same name.** `tap 0; cmd duplicate_figure; selectall` gives two rows both called "Stickfigure" (and two stacked figures). Other flows add "(copy)".
4. **"Add a movieclip" dialog is the figure grid.** + > Movieclip opens a dialog titled "Add a movieclip" that lists the same stock figures and has "Paste figure" at the bottom right. Is pasting a figure into a movieclip chooser meant? (/tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/mc1.png)
5. **Export dialog's first control is a one-item "Animation" bar.** It looks like a segmented control with a single option and does nothing visible. (/tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/e1.png)
6. **Undo is enabled right after entering the figure editor, and undoes the whole entry** (see the Redo finding). Is leaving the editor on Undo intended?
7. **Three ways out of the figure editor.** Cancel, Done (blue button at the bottom of Figure Tools) and "Save as new figure" at the top right. It is not clear from the screen what Done does that the other two do not.
8. **"Do not tween this frame" is ticked on frame 4 inside a pose whose Pose page says "Tween into it".** Landscape: grow block 3 to frames 3 to 4, tap the ruler at frame 4 (/tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/h1.png). Probably the held second frame of the pose.
9. **Group page has no Blend row** while the Stickfigure page does (opacity and effects are there; REDESIGN D33 says group effects are static for now).
10. **The scripted `tap 9; drag 9 60 -40` moves the whole figure** ("Move figure") where a real swipe on the same hand gives "Pose node". Probably the scripted double press being read as a double-tap-and-drag; real swipes behave (checked). Listed so nobody takes it for the iPad behaviour.
11. **The "Triangle" hover tooltip sticks to the bottom shelf** in /tmp/claude-1000/-home-caleb-stick-nodes-barkfartEdition/bughunt/fe1.png (no real hover on a touch screen; Linux mouse pointer left there). Likely a harness artifact.
12. **The flaky HANDOFF item (figure drawn bent after `tap 6; wait 1; editcopy; wait 1; tap 4`)** was run 4 times at 844x390 with `state h` after each; all four handle lists were identical and none was bent. Not reproduced (0 of 4).
