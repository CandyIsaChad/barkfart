# Progress

## Decisions
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
- 2026-11-16: Started the headless half of M4 while M3 is still open (M3c
  .stknds blocked on a newer, undocumented format version and on user files;
  M4 UI blocked on screenshots). Recorded here so the order change is visible.
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
- Figures saved by Stick Nodes 4.3.4 are probably rejected
  (NodesFormat.MAX_VERSION = 423) until a real 4.3.4 file confirms the layout.
- .stknds import not started (M3c).
- Movieclips: Snow and Action Lines render figures in the wrong place;
  Sparks is a known gap (IoU 0.82). Gradients, trapezoids, polygons, curves,
  right/upside-down triangles, node scale and smart-stretch/angle/drag locks
  are not applied in rendering.
- The evaluator fuzz test does not yet generate polyfills or the new shapes
  (EarClip has its own hostile-input test).
- Effects resolve but do not render (M6). Text layers draw nothing (M5).
  Group opacity is per-shape alpha until M6.
- No touch UI exists yet, so the only manual checklist is the device check
  below.

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
