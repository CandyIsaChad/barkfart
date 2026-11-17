# Progress

## Decisions
- 2026-11-17: Order changed to core-first (user asked; advisor's plan):
  1 lock the save format, 2 renderer foundation (per-vertex colour,
  textures, offscreen buffers), 3 shapes/gradients/curves/node options
  checked against the original's previews, 4 effects pipeline, 5 audio
  engine, 6 export, 7 UI per feature. Each step ends with an IPA.
- 2026-11-17: .stknds frames: stop blind probing; ask the user for tiny
  projects that differ by one thing each, then diff.
- 2026-11-16: The repo had only a compiled App Store IPA, no source. User chose a
  clean-room original app instead of a fork. Rules in CLEANROOM.md.
- 2026-11-16: Stack: LibGDX + MobiVM (Java 17, Gradle), modules core / desktop /
  ios (Android later). Why: matches the original's feel and toolkit, runs the
  core and a desktop window on Linux for tests and screenshots, and builds an
  iOS IPA on a GitHub macOS runner.
- 2026-11-16: Milestones adjusted for clean room. M0 = scaffold, CI, format
  notes. M1 = branding + zero-network check (no trackers to remove). Golden
  tests compare imports against user screen captures + our own renders.
- 2026-11-16: Branding: display name "Barkfart", bundle id `app.barkfart.animator`, Java package `app.barkfart`. The "Stick Nodes" name is not used (clean room).
- 2026-11-16: Toolchain lives in the user's home: Temurin JDK 17.0.21 in ~/.local/jdks. Gradle 8.10.2 (CI pins the same version).
- 2026-11-16: UI direction (from the user): as close to the original's layout and
  workflow as possible so it feels familiar, while fixing what is bad. Inventory
  and critique in UI.md. Layout claims stay unverified until the user sends
  screenshots.
- 2026-11-16: The brief's rule "reuse existing UI widgets" cannot apply, since
  there is no original source. We build our own widgets on LibGDX scene2d.
- 2026-11-16: IPA is built unsigned on GitHub Actions for LiveContainer.
- 2026-11-16: Repo made public (user's choice) so macOS builds are free.
  Commits use the GitHub noreply address; never commit a personal email.
- 2026-11-16: Started the headless half of M4 while M3 is still open (M3c
  .stknds blocked on a newer, undocumented format version and on user files;
  M4 UI blocked on screenshots). Recorded here so the order change is visible.
- 2026-11-17: The timeline UI is a dope sheet (user decision), replacing
  the frame strip in the same top position. Frame-by-frame remains as
  behaviour (posing at the playhead), not as a strip of snapshots.
- 2026-11-16: Golden comparison against the original uses the preview frames
  inside its movieclips (local only); metric and tolerances in NOTES.md.

## Done

### M0 Explore and baseline (closed 2026-11-16)
What works
- Reference bundle inspected (Info.plist, layout, data files, UI atlases and
  strings; no disassembly). NOTES.md, CLEANROOM.md, UI.md written.
- Gradle project: core, desktop (LWJGL3), ios (MobiVM 2.3.21). Wrapper 8.10.2.
- `LegacyFormat` unwraps all three legacy wrappers (zlib .nodes, raw
  big-endian .nodes, magic+gzip .nodemc/.stknds).
- Private repo github.com/balsamiqvine/barkfart; CI (macos-latest) runs core
  tests and builds an unsigned IPA (about 16.7 MB, about 1 minute).

How it was tested
- `./gradlew core:test`: 5 tests pass locally and on CI, including one that
  unwraps all 45 local reference files.
- `desktop:run` launched and ran 60 s without crashing.
- IPA inspected: arm64 Mach-O, bundle id app.barkfart.animator, landscape,
  no ad/analytics strings.
- User launched the CI IPA in LiveContainer: it starts (2026-11-16).

Not tested
- Desktop and iOS visual output were not captured.

Deviations from the brief (clean room)
- No original source, so there was nothing to "build" and no original renderer
  to capture golden frames from. Golden references move to M3 and will come
  from user screen captures of the original plus our own renders.

CI notes
- The `macos-14` label was cancelled before a runner started, so CI uses
  `macos-latest`. `gh run watch` exits early because the token cannot read
  check annotations (HTTP 403); poll `gh run view` instead.

### M1 Rebrand and clean (done here; waiting on the user's device check)
What works
- Name "Barkfart", bundle id app.barkfart.animator (set in M0).
- App icon is the user's own sketch (art/icon-sketch.jpg: white fox stick
  figure, :3 mouth, waving, black offset shadow, orange background).
  tools/make_icon.py crops it, snaps pixels to its three colours, pads it for
  iOS's rounded corners and downscales. Code-drawn versions were rejected by
  the user. "Black backdrop" is read as the black shadow in the sketch; the
  orange background is kept as drawn.
- LaunchScreen.storyboard (orange, "Barkfart"). CI compiles it to
  LaunchScreen.storyboardc. It also makes iOS run the app full screen instead
  of letterboxed.
- No trackers to remove (none were ever added). NETWORK.md lists the policy,
  dependencies and checks.
How it was tested
- NetworkPolicyTest: passes; proven to fail on a planted java.net.Socket.
- tools/netcheck.sh: desktop under strace, 20 s, zero AF_INET/AF_INET6.
- CI IPA contains the icons and LaunchScreen.storyboardc.
Not tested
- Icon and full-screen launch on a device. A create/edit/export network check
  is impossible until those features exist (re-run in M4 and M8).

Build hygiene added with M1
- All modules compile with `options.release = 8` (MobiVM runtime is old).
- IosApiPolicyTest bans java.util.stream, java.time, java.nio.file, java.awt
  and similar in core main code.
- CI split: ci.yml runs core tests on ubuntu for every push; ios.yml (macOS,
  billed at a multiple) runs on manual dispatch or `m*` tags only.

### M2 Design doc (done 2026-11-16)
- DESIGN.md: schema, modifiers, evaluation order with cycle, recursion and
  budget guards, render pipeline, .bark format, legacy import mapping,
  undo/caching, and open questions Q1-Q8 with the defaults chosen.

### M3a Core (done 2026-11-16)
What works
- Model (DESIGN §2), pure evaluator with FK, layer parenting, solo, camera,
  symbol instances, modifiers (wiggle, sine, loop, delay-follow, link),
  explicit work stack, recursion limit, depth cap 64, 250k object budget,
  cycle detection for links/delays/parents.
- Tessellation into a DrawList; GL renderer fills it; test rasterizer fills
  it for goldens (4 self-regression goldens, synthetic).
- .bark save/load: deterministic JSON, ZIP with fixed times, atomic save with
  .bak, migrations hook, newer formats refused.
- Key reduction; evaluator fuzz (400 random hostile projects).
- In-app self-test (9 checks) shown at launch; demo scene; desktop
  `--screenshot` flag.
How it was tested
- core:test locally and on CI (ubuntu): all pass. Desktop screenshot checked
  by eye. IPA built on CI (run for commit with the self-test) and sent to the
  user.
Not tested
- The self-test on a real device (needs the user's LiveContainer run).
- Effects are resolved in the scene but not rendered (M6). Text layers render
  nothing yet (M5). Group opacity is per-shape alpha (overlaps show) until M6.

### M3b Legacy .nodes import and export (done 2026-11-16)
What works
- Schema-driven reader/writer for every field the public spec lists, versions
  105 to 423, both endiannesses, zlib wrapper; iterative, bounded parsing.
- FigureMapper: legacy figure to figure layer and back; unedited figures
  export byte for byte, edits (angle, length, thickness, colour) are written.
How it was tested
- Local-only: all 30 reference figures round-trip byte for byte through
  read/write, and through import, .bark save/load and export.
- Committed: synthetic figures at 6 version/build combinations; truncated and
  garbage files fail with IOException; edit export test.
Known issues
- Triangles, ellipses, trapezoids, polygons, square-ended segments and
  polyfills render as approximations (listed per import). Node "scale" and
  the smart-stretch/angle-lock/drag-lock options are preserved, not applied.

### M3d Legacy .nodemc import (partial, 2026-11-16)
What works
- Reader/writer for version 330 clips: all 12 local clips parse to EOF and
  re-serialize their body byte for byte (multi-figure frames included).
- MovieclipMapper: clip to Symbol (layer per figure instance, full pose keys
  per frame, position/scale/opacity keys, hold keys on shown).
- **Golden test against the original renderer** (local only): clip frames
  rendered our way vs the original's own preview frames (preview k = frame
  k x floor(frames / previews)). Metric: F1 with 1-pixel tolerance, drawn =
  more than half the clip's contrast from the stage colour; asserted >= 0.9
  for clips using only shapes we draw, on frames with >= 500 drawn pixels.
  Passing: Fire, Sparks, Hit, Speed Zoom, Ground Dust (F1 0.97-1.00).
  Reported only (use gradients, not drawn yet): Forcefield 1.00, SSJB Aura
  0.98, SSJ Aura 0.90, Electricity 0.44-0.48 (its polyfill outline lands in
  the wrong place; suspect a node option such as stretchy positioning; open).
- Shapes now drawn natively: round segment, square segment, circle, ellipse,
  isosceles triangle, polyfills (concave, ear-clipped, drawn just before the
  anchor). Trapezoid, polygon, gradients and curves still approximate.
Not done (recorded in NOTES.md)
- Snow and Action Lines render figures in the wrong place (parse is exact).
- Movieclip versions other than 330.

### M4 Timeline: headless half (done 2026-11-16; UI waits for screenshots)
What works
- Document with command history: every edit is a Command; gestures group
  into one undo step (begin/commit), cancel reverts; redo cleared by new
  edits; history capped (500).
- Editor: auto-key (keys only the changed property; off = edit static value
  or refuse), frame-strip posing (keys rotation and length of every node at
  the snapped frame; modifiers never baked in), retime with collision
  replacement, scale time around a pivot, offset/stagger along a node chain
  by N frames, delete, copy/paste (with remapping onto another figure; type
  mismatches skipped), easing presets / per-key bezier on a selection.
How it was tested
- EditorTest (12 tests) including 50 random edit sequences that undo back
  to the exact starting state (compared as saved-file text).
Not done
- All M4 UI (dope sheet, curve editor, modifier UI, onion skin, markers
  panel): waits for the user's screenshots so the layout matches the
  original. Markers exist in the model but have no edit commands yet.

### Renderer foundation (2026-11-17, core-first step 2)
- DrawList batches carry per-vertex colour, texture coordinates and a
  texture (asset id); sprite quads are textured.
- MeshRenderer (one dynamic mesh + one shader: vertex colour x texture, four
  blend modes) replaces the per-triangle ShapeRenderer path; AssetTextures
  decodes image assets once per project; BufferPool provides pooled
  offscreen buffers for the effects step.
- Test rasterizer: Java2D for flat triangles (older goldens byte-stable),
  barycentric fill for gradients and textures; new goldens: gradient
  triangle, textured rotated sprite (orientation asserted).
- Checked on desktop: the editor renders as before; a generated sprite
  shows the right way up. Not tested on device yet.

### Format lock (2026-11-17)
- FORMAT.md rules; v1 fixture + reference render that every build must load
  and match; loader treats every non-identity field as optional with the
  model default; unknown fields ignored; Migrations has a real, tested step
  mechanism; fixed: a non-root node with draw order 0 was renumbered on load.
- Assets live in the .bark ZIP as STORED entries, lazily read from the file,
  copied across on save (tested: repeated saves, stream loads, determinism).
- Last project remembered by file name (iOS container paths change).
- Self-test (now 13 checks) saves and reloads a project with an asset in
  the real documents folder.

### .stknds import, partial (2026-11-17)
- Header + stickfigure library reader (spec-based, corrected ID maps);
  import brings a project's figures in as layers; animation not yet.
- Figures from Stick Nodes 4.3.x (version 431) now load (cap lifted after
  byte-exact round trips of 9 embedded figures).
- Frame layout partly decoded (NOTES.md); time box reached.

### Files (parity phase 1, 2026-11-17)
- Barkfart folder = the app's Documents (iOS; visible in the Files app via
  UIFileSharingEnabled), ~/Barkfart on desktop (-Dbarkfart.docs overrides).
- File panel (sidebar "File"): projects list (newest first, current marked;
  tap to open, saving the current one first), New, Save, Rename (system text
  prompt), Duplicate, Delete (soft: .trash, Undo in the status line);
  import list of every .nodes/.nodemc in the folder (tap = import into the
  open project, one undo step); From Files... (iOS document picker, copies
  into the folder then imports/opens); Export figure (.nodes, then the iOS
  share sheet); Share project.
- Autosave every 30 s when changed and whenever the app is backgrounded or
  closed; the last project reopens at launch; a new project gets a file
  ("Untitled.bark") on its first save.
- Tested: FilesTest (store lifecycle, sanitizing, import as one undo step,
  byte-exact figure export, unsupported messages, local movieclip import,
  session dirty tracking). Desktop: relaunch reopened the saved project;
  File panel and in-app imports checked by screenshot. Not tested: the iOS
  picker and share sheet on a device (they compile against the iOS SDK).

### Fixes from the user's first iPad test (2026-11-17)
- Auto-key scope (figure panel): Joint = the dragged joint's angle AND
  length are keyed (stretch is never left unkeyed); Figure = every joint's
  angle and length plus position (a full pose, like the original). Saved.
- Camera is a stand-in frame on the stage (move inside, corners zoom, knob
  rotates), not the whole screen; it applies only when playing / exporting
  or with "Preview shot". Editing happens in stage space.
- Easing is a dropdown with a curve preview per preset: Hold, Linear, Ease
  in-out, in, out, Sine, Strong, Anticipate, Overshoot, Anticipate +
  overshoot.
- Dope sheet: double-tap empty space toggles multi-select (taps add/remove
  keys; also a Multi-select button in the key panel); calmer grid.
- Tests: canvas key-scope tests (3 new). Screens checked in the iPad preview.

### UI scaling for iPad (2026-11-17; the user tests on an iPad)
Automatic UI scale from the screen height (iPad Air ~1.25x, 12.9-inch
1.5x) times a user "UI size" setting (Smaller / Larger, 80-180%, saved per
device). Changing it rebuilds the UI layer with re-rasterized fonts and keeps
the project, selection and undo history. Desktop `--ipad` preview (1180 x
820 pt). Tested: autoScale unit test; iPad preview screenshot. Not tested:
on the user's iPad.

### M4 Timeline: dope sheet (2026-11-17)
The frame strip was replaced by a dope sheet (user decision): ruler with
tabular frame numbers and a draggable playhead, summary row, per-figure
header (tap to collapse), Move row, one row per joint, Camera row; filter
Selected | All; keys as diamonds; tap/box-select (long-press then drag);
drag selected keys to retime (snapped, one undo step); pinch to zoom time,
drag to scroll; drag the length handle on the ruler to change duration;
drag the bottom grip to expand up to 60% of the screen. Sidebar key panel:
copy/paste at the playhead, delete (undo from the status line), easing
Hold/Linear/Ease/In/Out, offset along the chain -1/+1 frame, scale time
50%/200%. Tested: DopeRows unit tests (3); desktop phone-preview screenshots
collapsed and expanded with a selection. Touch on a device not yet tested.

### M4 Timeline: first interactive editor (2026-11-17, prototype)
What works (UI-SPEC v2, after an independent design review)
- Layout like the original: sidebar (Undo/Redo, Figure | All | Camera,
  context panel, status line), frame strip on top (16:9 cells, key vs
  in-between with tween badge, current outline, "+" cell, momentum scroll,
  scroll bar), Play/Stop top-right, white stage on a dark surround.
- Posing: drag any joint (after 8 pt) to rotate, or stretch mode; drag the
  main node to move the figure; always-on auto-key keys the current frame;
  one undo step per drag. Pan/pinch/double-tap the view; two-finger tap
  undo, three-finger tap redo. Camera mode pans/zooms a camera layer
  (created on first use, undoable).
- Frame menu (tap current frame or long-press): insert, duplicate, delete
  (undo from the status line), hold/tween to next.
- Figure panel: previous/next figure, copy/paste pose, rotate/stretch,
  angle scrub with -/+, delete figure. Nothing selected: add figure, onion
  skin, frame rate.
- Onion skin from the evaluator (2 back red, 1 forward blue).
- Inter font via gdx-freetype at device resolution, tabular digits.
- iOS: system edge gestures deferred top/bottom, home indicator hidden.
- Structural undo for layers, nodes, polyfills, modifiers, markers, fps.
How it was tested
- 92 core tests (canvas picking/dragging through a rotated, zoomed camera,
  frame ops, structural undo, speed integral, onion skin).
- Desktop phone preview (844 x 390 pt at 2x) screenshots by eye: new
  project, posed frames with onion skin and selection, camera mode.
Not tested
- Touch on a real device (see the checklist below). Thumbnails in the
  strip, the status-line "jump to frame", the nudge pad and magnifier, iPad
  metrics and the collapsed rail are not built yet. The panel rebuilds on
  every change (fine now; revisit in M9).

## In progress
M3 is split (recorded per the brief's rule; nothing cut):
- M3a done, M3b done (above).
- M3c Legacy .stknds import: not started. The samples are version 431,
  newer than any public spec (<= 423). Depends on the multi-figure frame
  layout (same frame structure as movieclips, presumably).
- M3d partial (above).
- M3 stays open until: shapes beyond segment/circle are drawn so more clips
  pass the original-renderer golden; .stknds imports; figures from 4.3.4 load.

## Known issues and not done (nothing here is cut; each is tracked)
- No undo commands yet for modifiers, markers, layer flags (visible, lock,
  solo), layer/node structure (add, remove, reorder) or polyfills. Needed
  before the M4/M5 UI exposes those edits.
- .stknds import brings figures only; frames not decoded yet (NOTES.md).
- Movieclips: Snow and Action Lines render figures in the wrong place;
  Sparks is a known gap (IoU 0.82). Gradients, trapezoids, polygons, curves,
  right/upside-down triangles, node scale and smart-stretch/angle/drag locks
  are not applied in rendering.
- Effects resolve but do not render (M6). Text layers draw nothing (M5).
  Group opacity is per-shape alpha until M6.
- No touch UI exists yet, so the only manual checklist is the device check
  below.

- Resolved 2026-11-16: the user confirmed the M3a IPA self-test passes 9/9
  on the phone, and chose to make the repo public (free macOS runners).
  Before publishing, commit authorship was rewritten to the GitHub noreply
  address and old Actions runs (which linked the old commits) were deleted.
- **Resolved (2026-11-16 22:05 UTC):** a label probe (mac-probe.yml) showed
  only macos-15-intel gets a hosted runner on this account; every Apple
  Silicon label stays queued. ios.yml now builds on macos-15-intel (MobiVM
  cross-compiles arm64). IPA at the current commit built in ~7 min and was
  sent to the user.
- **GitHub-side (16:00 UTC):** after the user lifted the $0 Actions budget,
  the run page says "Internal server error ... The job was not acquired by
  Runner of type hosted even after multiple attempts". macos-15 and
  macos-26 both fail the same way. Not fixable from our side; retry later.
- **Still blocked after going public (15:00-15:30 UTC):** macos-latest and
  macos-15 jobs stay queued with no runner (one cancelled after 15 min);
  Linux jobs start instantly; githubstatus shows no Actions incident. Points
  to an account-level hold on macOS runners. Needs the user to look at
  github.com/settings/billing and Actions settings.
- (Was) **IPA builds blocked (2026-11-16 ~14:20 UTC):** two dispatches of
  ios.yml (commits 4e12277, d31e87b) waited 15 min without a macOS runner
  and were cancelled; Linux CI keeps passing. Most likely the private repo's
  Actions macOS minutes/spending limit. The token cannot read billing. The
  newest IPA the user has is from commit 1139106 (M3a self-test, 9 checks);
  everything after it (shapes, polyfills, legacy import, edit package) has
  not run under MobiVM yet.

## Touch checklist for the editor (manual)
1. Drag a joint: it rotates around its parent and follows the finger; a
   short tap selects without moving. Toggle Stretch and drag: length changes.
2. Drag the orange square: the whole figure moves.
3. One-finger drag on empty stage pans; pinch zooms; double-tap fits.
4. Two-finger tap undoes; three-finger tap redoes; Undo/Redo buttons too.
5. Drag the playhead on the dope sheet's ruler; pose at a few frames; drag
   the length handle (end of the ruler) to make the animation longer; Play
   loops; Stop returns to where you were.
6. Tap a key diamond to select it; drag it sideways to retime; long-press
   empty sheet space then drag to box-select; pinch the sheet to zoom time;
   drag the grip under the sheet to expand it and see joint rows; tap a
   figure row's "-" to collapse it. Key panel: easing, offset, scale,
   delete (then "Undo" in the status line).
7. Camera: drag pans and pinch zooms the shot; the blue border shows.
8. Swiping down from the very top edge or up from the bottom edge does not
   open Notification Centre / go home on the first swipe.

## Device checklist (manual; I cannot automate these)
1. Install the latest Barkfart.ipa in LiveContainer; launch it.
2. Home-screen icon is the fox sketch; launch screen is orange "Barkfart";
   the app fills the screen (no letterbox bars).
3. Top line turns green "core self-test PASS 12/12" within ~10 s. If it is
   red, screenshot the lines below it.
4. The demo animates smoothly (fps shown under the self-test line).

## Next
- Waiting on the user (not blocking):
  1. Screenshots of the original's main screens and their worst pain points
     (UI.md).
  2. Tiny projects made in the real app (one figure, two frames; tween on and
     off) with screenshots of each frame and a mid-tween frame. These become
     the true golden references and settle the tween curve and angle wrapping.
  3. Check on the phone: new icon, full-screen launch, self-test PASS line.
  4. A stickfigure and a small project saved from your Stick Nodes 4.3.4
     (the bundled figures are version <= 420; the reader rejects > 423 until
     a real 4.3.4 file confirms the layout).
