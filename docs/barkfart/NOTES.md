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
