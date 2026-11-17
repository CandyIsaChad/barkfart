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

## Movieclip (.nodemc) frame layout (decoded locally, version 330)
- Header and library match the public spec. The preview block is
  previewCount frames of W x H RGBA, renders made by the original app,
  apparently stored bottom-up (GL readback). Local-only visual references.
- Frame: soundVolume f32, soundPan f32, soundPitch f32, soundId i32 (-1 =
  none), figure count i32, then per figure: libraryId i32, a 101-byte header
  (instance id i32 @0, scale f32 @4, opacity f32 @8, blur f32 @12, stage x
  f32 @67, stage y f32 @71 on a 1920x1080 stage, a colour @75), then one
  58-byte record per library node in hierarchy order (scale f32 @3, length
  f32 @7, thickness i32 @11, default local angle @15, local angle @19,
  default world angle @23, colour @27, gradient colour @31, outline colour
  @35; the rest unknown and kept raw).
- Verified: 9 of 12 bundled clips (all frames with one figure) parse to EOF
  and re-serialize their body byte for byte.
- Resolved: each figure is libraryId i32 + a 97-byte header + 58-byte node
  records, and every frame ends with a 4-byte trailer (kept raw). Node
  record fields: scale @7, length @11, thickness @15, default local angle
  @19, local angle @23, default world angle @27, colour @31, gradient @35,
  outline @39. All 12 local clips parse to EOF and round-trip byte for byte
  (Rain has 466 figure instances).
- Open: Snow and Action Lines (several figures per frame) render with
  figures in the wrong place (Snow: correct pattern shifted vertically).
  Suspect a per-instance position or rotation field, or tweened timing.

## Shape semantics: current best fit against the original's previews (2026-11-16)
- Ellipse (type 5): centred on the segment midpoint, major axis = segment
  length, minor = thickness. Best of the variants tried, scored like for
  like: extending it by half a thickness at each end dropped Forcefield from
  0.93 to 0.36-0.41 IoU and Sparks to 0.75-0.80; centring on the node with
  semi-major = length also scored worse (older metric).
- Triangle (3, right-triangle direction 0): base of width = thickness at the
  parent, apex at the node. Alternatives not yet scored (apex at the parent
  would be the first to try). Right-triangle and upside-down variants are
  not drawn yet.
- Square segment (1): rectangle of width = thickness, no caps.
- Movieclip previews are frames k x floor(frames / previews), stored
  bottom-up, rendered on an opaque #f0f0f0 stage.

## Golden metric (final form, 2026-11-16)
Ours is rendered at the full 1920x1080 stage and box-downsampled 3x to the
preview size (drawn = at least 5 of 9 subpixels covered, colour = mean), so
both sides have the same pixel grid. A preview pixel is drawn when it
differs from the stage colour by more than half the clip's contrast. Plain
IoU >= 0.85 is asserted; the 1px-tolerant F1 >= 0.9 is asserted too.
Results: Fire 0.98, SSJB Aura 0.99, Ground Dust 0.97, Speed Zoom 0.95-0.96,
Forcefield 0.93, Electricity 0.93 (an earlier "wrong outline" was a test
helper that dropped the concave flag), SSJ Aura 0.79 (gradients, area 1.26),
Sparks 0.82 (known gap: our area 87-98% of the original; open).
Colour difference is 0 on every clip without gradients.

## Project (.stknds) format, version 431 build 117 (2026-11-17)
- Header and libraries follow the public spec up to the stickfigure
  library, with one correction: in the ID conversion maps each level is
  (container UID, count), not (count, UID). Verified on all 3 samples.
- Figures embedded at 431/117 parse with the 423-era node schema and
  re-serialize byte for byte (9 figures across samples 1-2), so
  NodesFormat.MAX_VERSION is now 431.
- Frames (decoded so far, sample 1, 89 frames of 3728 bytes): each figure
  instance is libraryId i32 + a 127-byte header + one 83-byte record per
  library node + 1 byte. Node records keep the v330 offsets for scale @7,
  length @11, thickness @15, angles @19/@23/@27, colour @31 (newer fields
  follow the colour). About 925 bytes per frame of frame-level data are not
  decoded yet (two textfields with strings, camera, sounds, asset-type ints
  for >= 400). Next attempt starts there.
- Import today: the project's figure library comes in as layers, side by
  side, with an honest status message; the animation does not yet.

## Curved segments (fitted 2026-11-17)
`curveRadius` (small ints, -6..8 in the samples) is a bulge height, not an
angle: the segment is a circular arc with sagitta = curve x 0.9 figure units,
bulging to the left of the parent-to-node direction for positive values.
Fit against the original's previews (like-for-like IoU): angle-per-unit
models peaked at Sparks 0.87 / Hit 0.84 with different best angles; the
sagitta model gives Sparks 0.82 -> 0.91 and Hit 0.81 -> 0.85 with one
constant. Feature usage in the bundled content: curves 346 nodes (Hit,
Sparks), upside-down triangles (Forcefield, Hit, SSJB Aura), gradients (SSJ
Aura, Gradient Ball), node scale != 1 (4 files); no trapezoids, polygons,
outlines or hollow circles.

## Triangles and gradients (fitted 2026-11-17)
- triangleUpsideDown: apex at the parent, base at the node (small gains on
  Hit and SSJB Aura, no losses). rightTriangle -1/+1: right angle on one side.
- Gradients: segment colour at the parent end, gradient colour at the node
  end (reverse swaps); sideways = across the width; circles and ellipses are
  radial (centre -> rim). Segments WITHOUT their own colour show the figure
  colour at both ends (the stored gradient colour is ignored): with that rule
  SSJ Aura's colour error is back to its no-gradient level (20) instead of
  31-41. SSJ Aura's remaining gap (area 1.26x, colour 20) is elsewhere,
  likely the large glow ellipse's sizing; open.
