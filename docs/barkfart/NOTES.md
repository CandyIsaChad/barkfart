# M0 notes: what the reference bundle shows

Source: `main/Payload/IOSLauncher.app`, Stick Nodes 4.3.4 (build 119),
bundle id `org.fortheloss.sticknodes`, iOS 14+, landscape only, GLES2.
Observed from Info.plist, folder layout, data files, UI atlas images and UI
string files only. The binary was not disassembled (see CLEANROOM.md).

## Stack (from bundled library jars)
- LibGDX 1.12.1 on RoboVM/MobiVM (all Java AOT-compiled into one arm64
  binary; no bytecode ships). ObjectAL for audio. commons-lang3, isoparser
  (MP4 muxing, so export likely encodes frames then muxes).
- Ads/tracking present in the original: Google Mobile Ads, Google UMP consent,
  Firebase Core/Analytics/Crashlytics, ~50 SKAdNetwork ids,
  NSUserTrackingUsageDescription, NSAllowsArbitraryLoads. Barkfart ships none
  of these, so M1 "remove trackers" reduces to "never add them" plus a
  zero-network check.

## Bundled content (names only)
26 stickfigures, 12 movieclips, 3 sample projects, 4 "SNMC rig" figures,
HD/SD texture sets, a dark UI skin, node style textures, 7 GLSL shaders
(premultiply, argb, removeColor, distance-field text box).

## File formats (first pass; full spec is an M3 task)
| Ext | Wrapper | Body |
|---|---|---|
| `.nodes` stickfigure | raw zlib | little-endian. int32 version (105 seen), then node records: RGBA color bytes, type byte, ids, float32 x/y, scale 1.0, widths (e.g. 90), ints, rotation (degrees) |
| `.nodes` (newer) | none | big-endian, int32 version (163, 334 seen), then float 1.0, colors. Seen in `_Sasuke`, `_Creeper`, the SNMC rigs |
| `.nodemc` movieclip | 9-byte magic `01..09` + gzip | big-endian (Java DataOutputStream). int32 version (330 seen), length-prefixed UTF-8 title, small ints, then a 640x360 ARGB thumbnail, then frames |
| `.stknds` project | same as .nodemc | big-endian, int32 version (431 seen), title, settings, then frames |

Decompressed `_Hit.nodemc` is 2.8 MB: consistent with the brief's "list of
full-snapshot frames" model.

## Still unknown (needs device observation or more format work)
Exact node record layout per version, tween curve shapes, how sounds and
filters attach to frames, export pipeline details, undo depth.

## Public format spec (found 2026-11-16)
github.com/vinceTheProgrammer/sticknodes-rs (MIT, commit 3ea03d6) documents
`.nodes` fully (versions up to 423) and `.stknds`/`.nodemc` partly (frame
layout blank). Barkfart's importers follow that spec; it is public
documentation, not the original's code. Key facts:
- Big-endian (Java DataOutputStream). Colour ints are ABGR byte order.
- `.nodes`: version, build (>=403), scale, colour, then the node tree
  depth-first, each node followed by its child count; then polyfills (>=230)
  and connector connections (>=403 build 38). Node fields are versioned.
- Node types: -1 main, 0 rounded segment, 1 segment, 2 circle, 3 triangle,
  4 filled circle, 5 ellipse, 6 trapezoid, 7 polygon.
- The node's own id is its draw-order index.

Our own check: the old zlib-compressed `.nodes` files (version 105) are the
same structure in **little-endian**, and parse to exact EOF with the v105
field set (type, draw index, static, stretchy, use-segment-colour,
use-segment-scale, local x/y, scale, default length, length, default
thickness, thickness, local angle, colour, child count). In Classic
Stickfigure the local offset's magnitude is half the length field (45 vs 90),
which matters for import scale (DESIGN Q4).

## Import geometry findings (M3b, 2026-11-16)
- All 30 local reference figures (versions 105 to 420 build 62) parse to exact
  EOF and re-serialize byte for byte, including the 5 zlib files (Java's
  default Deflater reproduces the original compression).
- Sum of local angles down the tree matches atan2(localY, localX) within
  0.033 degrees over 3615 segments: localAngle is relative to the parent,
  counter-clockwise, y up.
- |localX, localY| / length is 0.5 in some files and 1.0 in others, per file,
  independent of version and figure scale. Treated as a cached, device-scaled
  value; `length` is authoritative. Which on-screen size the original uses is
  still open (DESIGN Q4); the user's capture projects will settle it.
- Shapes approximated as plain segments until drawn natively: square-ended
  segment (type 1), triangle (3), ellipse (5), trapezoid (6), polygon (7);
  polyfills are preserved for export but not drawn yet.
