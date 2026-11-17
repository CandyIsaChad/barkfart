"""Builds docs/barkfart/PARITY.md from the raw research inventory plus a status map.

Usage: python3 -I tools/make_parity.py <repo root>
Status codes: x = usable in the app now, ~ = partial / core-only (no UI yet),
' ' = not yet, - = won't (reason given).
"""
import os
import sys

STATUS = {
    # Canvas & posing
    "Drag nodes to pose": ("x", "drag any joint; Rotate / Stretch"),
    "Stretchy nodes": ("~", "Stretch mode stretches any joint; stretchy flag in the model and import, not enforced"),
    "Static vs dynamic nodes": ("x", "static (detail) nodes have no handles while animating"),
    "Per-figure scale": ("~", "layer scale in the model; no UI"),
    "Per-segment scale": ("~", "node scale imported (own length only, fitted to the original); no UI"),
    "Per-figure and per-segment color": ("~", "colour tracks in the model and import; no picker UI"),
    "Pinch zoom on stage": ("x", ""),
    "Zoom out below 100% and move stage": ("x", ""),
    "Smoother zoom": ("x", ""),
    "Panning mode": (" ", "dragging empty stage pans the view; moving all figures at once is not built yet"),
    "Lock stickfigure": ("~", "layer lock in the model and undo; no button yet"),
    "Locked figures hide nodes": ("x", "locked layers show no handles"),
    "Push back / bring forward": ("~", "layer order and node draw order commands exist; no UI"),
    "Tap off-stage to deselect": ("x", "tap empty stage deselects"),
    "Cancel accidental grab on zoom": ("x", "second finger cancels the drag"),
    "Keep selection while dragging stage": ("x", ""),
    "Forgiving taps": ("x", "44 pt hit radius, nearest joint wins"),
    "Selection kept across pan/camera mode": ("x", ""),
    # Creation
    "Line segment": ("x", "square-ended segment drawn natively"),
    "Rounded segment": ("x", ""),
    "Circle segment": ("x", ""),
    "Filled Circle segment": ("x", "imported as a filled circle"),
    "Triangle segment": ("x", "isosceles, right-angled and upside-down"),
    "Shapes: ellipse, trapezoid, right-triangle, polygon": ("x", "all drawn and imported; trapezoid and polygon geometry unverified (no reference file)"),
    "Polyfills": ("x", "drawn (concave, ear-clipped) and round-trip; no creation UI"),
    "Curved segments": ("x", "arcs, bulge fitted to the original's renders"),
    "Segment color gradients": ("x", "along the segment, figure-colour rule fitted"),
    "Sideways gradients": ("x", ""),
    "Radial circle gradients": ("x", ""),
    "Reverse gradient": ("x", ""),
    "Triangle upside-down": ("x", ""),
    "Polyfills follow curves": ("x", "fill edges follow the drawn arc"),
    "Circle hollow": ("~", "drawn and imported; no creation UI"),
    "Half-circle reversible": ("~", "half arcs drawn either side; no creation UI"),
    "Shape Tools properties": ("~", "half arc, polygon sides, trapezoid ratio in the model, drawn, imported; no UI"),
    "Polygon min 4 sides": ("~", "4-16 sides in the model; no UI"),
    "Polygon radial gradient": ("x", "polygons take radial gradients"),
    "Trapezoid curves and rounded ends": ("~", "round ends drawn; curved trapezoids not yet"),
    "Smart-Stretch": ("x", "stretching a smart-stretch joint reshapes its static details (rule fitted to the original's own files)"),
    "Smart-stretch changes": ("x", "details need not be stretchy; moving nodes stop it"),
    "Do not smart-stretch": ("~", "honoured; set only by import (no creation UI)"),
    "Angle Lock (node)": ("~", "absolute and relative locks in pose and drag; legacy encoding not mapped; no UI"),
    "Drag Lock (node)": ("~", "horizontal / vertical drag lock; legacy encoding not mapped; no UI"),
    "Connectors": ("~", "imported and exported; not drawn as connectors"),
    "Draw order control": ("~", "node draw order in the model and undo; no UI"),
    # Frames
    "Frame list / thumbnails": ("~", "replaced by the dope sheet (by design); thumbnails not yet"),
    "Add/copy/paste/delete frames": ("~", "dope sheet copy/paste/delete keys; insert/delete time in core"),
    "Play / Stop buttons": ("x", ""),
    "FPS setting": ("x", "sidebar Frame rate - / +"),
    "Tweening toggle": ("x", "per key: Hold vs Linear/eased (easing menu)"),
    "Tweened-frame count": ("x", "keys are placed in time; in-betweens are implicit"),
    "Tween 0 frames": ("x", "Hold easing"),
    "Is loop": ("x", "playback loops"),
    "Tween colors": ("x", "colour tracks interpolate"),
    "Tween Mode per figure": ("x", "per key easing presets with curves"),
    "Frame repeat": ("~", "loop modifier in the core; no UI"),
    "Copy multiple frames": ("x", "box/multi-select keys, copy, paste at playhead"),
    "Clear frame": ("x", "select keys at a frame and delete"),
    "Custom playback range": (" ", ""),
    "Frame skipping under lag": ("x", "playback clock is time-based"),
    "Preserve selection across frames": ("x", ""),
    "Frame node limit": ("-", "no artificial limits (budgets only guard runaway recursion)"),
    "Double frame node limit": ("-", "no limits"),
    "Unlimited nodes per frame": ("x", "no limits"),
    # Onion
    "Onion skin": ("x", "from the evaluator, past red / future blue"),
    "Onion skin frame count": ("~", "fixed 2 back / 1 forward; no setting yet"),
    "Visible in onion skin": ("~", "per-layer flag in the model; no toggle UI yet"),
    "Double-tap to reset zoom": ("x", "double-tap empty stage fits"),
    "Stop resets view": ("x", "Stop returns to the frame where play started"),
    "Remove stage outline": ("x", "no outline by design"),
    # Camera
    "V-cam / Camera Mode": ("x", "camera frame on the stage, applied on play/export"),
    "Mode buttons": ("x", "Figure | All | Camera"),
    "Camera rotation": ("x", "rotate knob on the frame"),
    "Camera wobble/shake": ("~", "wiggle modifier in the core; no UI"),
    "Copy/paste camera": ("x", "camera keys copy/paste in the dope sheet"),
    "Stay in camera mode on frame tap": ("x", ""),
    # Movieclips
    "Movieclips": ("~", "symbols with nesting, offset, speed, loop in the core; no UI"),
    ".nodemc files": ("x", "import from the File panel (all 12 sample clips parse)"),
    "Once / Loop playback": ("~", "symbol loop modes in the core"),
    "Default movieclips": ("-", "cannot ship the original's bundled content; ours later"),
    "Scaled/opacity MCs": ("~", "layer scale/opacity apply to symbol layers in the core"),
    # Filters
    "Transparency": ("x", "sidebar Opacity - / +, drawn as one composite (no overlap seams)"),
    "Invert colors": ("~", "sidebar: Add Invert, Remove"),
    "Tint": ("~", "sidebar Tint with amount; colour fixed to blue for now"),
    "Saturation": ("~", "sidebar Grey: saturation - / +; hue and brightness in the core only"),
    "Hue shift": ("~", "colour adjust hue in the core; no control yet"),
    "Blur": ("~", "sidebar Blur with radius; horizontal-only / vertical-only not yet"),
    "Glow": ("~", "sidebar Glow with radius; colour fixed to yellow for now"),
    "Glow intensity": ("~", "strength 0-4 in the core; no control yet"),
    "Drop shadow": ("~", "sidebar Shadow with softness; offset and colour fixed for now"),
    "Blur/glow max 2.0": ("x", "radius up to 400 px, strength up to 4"),
    "Low in-app quality": (" ", "effects render at full resolution for now; half-resolution preview not yet"),
    # Sound
    "Sounds on frames": ("x", "import a sound at the current frame; heard on Play; move, volume, remove (export with sound later)"),
    "Custom MP3 import": ("x", "mp3, m4a, wav, aac, caf, aiff, no size limit (ogg: iOS cannot decode it)"),
    "Multi-select sound import": ("x", "the Files picker imports several at once"),
    "Sound pan": ("~", "per-clip pan track in the mixer; no control yet"),
    "Bundled sound library": ("-", "cannot ship the original's sounds; our own later"),
    # Export
    "GIF export": ("x", "Export: GIF (looping, exact timing, transparency), no watermark"),
    "GIF >256-color handling": ("~", "fixed 252-colour palette with ordered dither; an adaptive palette is not done yet"),
    "MP4 export": ("x", "Export: Video, H.264 with the timeline's sound (iPad; checked by Diagnostics on the device)"),
    "Fast hardware MP4": ("x", "AVAssetWriter hardware encoder"),
    "PNG export": ("x", "Export: PNG frames (zip), optional transparent background"),
    "Better quality blur": ("x", "exports always use full-quality effects"),
    "Low-quality blur export": ("-", "not needed: exports always use full quality"),
    "Export frame range": (" ", "whole timeline only for now"),
    "Non-blocking export": ("x", "exports run a frame per tick with progress and Cancel"),
    "Exports to gallery": ("x", "share sheet after export: Save Video / Save Image to Photos"),
    "Export size slider": ("~", "Full / Half / Small presets, no slider"),
    "Free watermark": ("-", "no watermark (no free/pro split)"),
    "1080p MP4": ("~", "Full = project size (1280x720 by default); a canvas-size setting is not in the UI yet"),
    "Pixelate": ("~", "rendered (GPU + reference), grid anchored to the stage; no panel button yet"),
    "Motion blur": ("~", "directional smear with angle and distance; no panel button yet"),
    "Outline": ("~", "coloured outline of a set width; no panel button yet"),
    "Joining": ("~", "core: join to any joint (follows position and angle), unjoin, no jump; no button yet"),
    "Add segment / limb": ("~", "core: add segment inheriting the parent's look; no creation screen yet"),
    "Split segment": ("~", "core: split, undoable; no button yet"),
    "Split into N": ("~", "core: 2-64 pieces"),
    "Split along curve": ("~", "core: pieces follow the arc exactly (tested)"),
    "Duplicate / copy-paste segment": ("~", "core: duplicate a branch (with its polyfills) on top"),
    "Segment flip X/Y": ("~", "core: flip a branch about its joint, keys included"),
    "Flip segments in Animation Mode": ("~", "core: same branch flip, works on animated figures"),
    "Flip figure (X/Y)": ("~", "core: mirror a figure (scale sign, keys included)"),
    "Modify Branch": ("~", "core: static, thickness, colour, shape, length scale for a whole branch"),
    "Modify Branch: limb type": ("~", "core: shape for a branch"),
    "Modify Branch: segment length": ("~", "core: length scale for a branch"),
    "Push branch back/forward": ("~", "core: whole-branch layering keeps its own order"),
    "Reset length": ("~", "core: back to rest length, keyed, with smart-stretch"),
    "Inherit node properties": ("~", "core: new segments copy static/stretchy/thickness/colour"),
    "Copy/paste keeps draw order": ("~", "core: duplicated branches keep their relative layering"),
    "Break apart MC": ("~", "core: instance to layers at the current frame, placement baked in"),
    "Convert figure to MC / double-tap edit": ("~", "core: layers to a movieclip, animation unchanged; editing in place not yet"),
    "MC Library Tools": ("~", "core: symbols are created by convert; no library screen yet"),
    "Add tweened frame": ("~", "core: key the in-between pose (nothing moves)"),
    "Add tweened frame percentages": ("~", "core: any fraction between two keys"),
    "Frame delay": ("~", "core: hold a frame for N frames"),
    "Lock figure to camera": ("~", "core: locked layers stay put in the shot, no jump; no button yet"),
    "Text lock to camera": ("~", "same camera lock works for text layers"),
    "Widescreen bars": ("~", "core: letterbox setting drawn over the shot and export; no setting UI yet"),
    "Auto-Camera": ("~", "core: camera follows a layer over a frame range, keys thinned"),
    "Sprites": ("x", "import png/jpg as sprites (File panel), move/scale/rotate as layers"),
    "Mass image import": ("x", "the Files picker imports several images at once"),
    "Separate X/Y sprite scale": ("~", "separate x/y scale in the model; no control yet"),
    "Negative import scale": ("~", "flip figure works on sprites (negative scale)"),
    "Remove background color": ("~", "core + device decode: tolerance, soft edge, edges-only; no button yet"),
    "Background images": ("~", "an image layer at the bottom works; no dedicated background option yet"),
    "Textfields": ("~", "text layers draw (Inter font, multi-line, accents); no add-text button yet"),
    "Text rotation": ("x", "layer rotation applies to text"),
    "Text transparency": ("x", "layer opacity applies to text"),
    "Text shadow": ("x", "the drop-shadow effect works on text layers"),
    "Text tweening": ("x", "text colour and transform keys interpolate"),
    # Files
    "Save/open projects": ("x", "File panel: new, open, save, rename, duplicate, delete with undo; autosave"),
    "Save/import stickfigures": ("x", "import from the folder or the Files picker; export .nodes byte-exact + share"),
    "Default stickfigures": ("-", "cannot ship the original's figures; ours later"),
    "Sample projects": (" ", "our own samples later"),
    "Local project backup": ("x", "one rolling .bak per save; deleted projects go to a hidden trash with Undo"),
    "Delete files": ("x", "soft delete with Undo"),
    "Open with / file association": ("~", "the Files picker imports; Open-in from other apps not wired yet"),
    "Multi-select import": ("x", "the Files picker allows several files"),
    "Partial project open": ("~", ".stknds: figure library imports; animation not yet"),
    "Send files": ("x", "share sheet for projects and exported figures"),
    "Save reminder": ("-", "replaced by autosave every 30 s and on backgrounding"),
    # Settings
    "Screen padding": ("x", "safe areas handled automatically"),
    "Haptics": ("~", "long-press vibrates; no setting"),
    "Dark Mode skin": ("x", "dark theme"),
    "Memory display": ("-", "replaced by performance work (M9)"),
    "Lag Problems button": ("-", "replaced by performance work (M9)"),
    "Memory Use dialog": ("-", "replaced by performance work (M9)"),
    "Nerd Mode": (" ", ""),
    # QoL
    "Undo/redo": ("x", "every edit; 2-finger / 3-finger tap too"),
    "Grouped undo": ("x", "one step per gesture"),
    "Contextual tool menus": ("x", "one context panel titled by the selection"),
    "Hold to repeat": (" ", ""),
    "First-run tooltips": ("-", "no nags by design; short inline hints instead"),
    "Onboarding": ("-", "opens straight into a project by design"),
    "Splash screen": ("x", "launch screen with our icon"),
    "Splash ball minigame": ("-", "not planned"),
    "Easter egg": ("-", "not planned"),
}

ROADMAP = """
## Order of work (parity first, then our own additions)
Our own additions wait in ADDITIONS.md until this list is done.
1. **Files and content:** project save/open/new with a file list; import
   the user's .nodes / .nodemc / .stknds and Pivot .stk; stickfigure export;
   our own default figures and samples.
2. **Figure tools in animation mode:** add/delete/duplicate/lock figures,
   push back/bring forward, flip X/Y, figure scale/rotation/colour/opacity,
   per-segment colour and scale, copy/paste all properties, joining
   (attach to a node), panning all figures, pinch-rotate, nudge pad,
   magnifier, colour picker with recent colours.
3. **Creation mode:** build figures from segments (all shape types, curves,
   gradients, outlines), polyfill creation, split segments, connectors,
   smart-stretch, angle/drag locks, static/stretchy nodes, trace frame.
4. **Timeline extras:** onion settings (count, in front, per-layer
   toggle), playback range, frame repeat / loop, thumbnails, slow motion.
5. **Movieclips UI:** library, create from selection, edit in place,
   once/loop, break apart.
6. **Text, sprites, background images, GIF/image import.**
7. **Sound** (M7), **filters/effects** (M6), **export** GIF/MP4/PNG (M8).
8. **Settings:** languages, left-hand mode, haptics, node style, zoom
   buttons, save reminder, auto-save.
"""

WONT_CATEGORIES = {"Website/community features": "won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally"}


def main(root):
    src = os.path.join(root, "docs/barkfart/research/stick-nodes-features-raw.md")
    out = ["# Feature parity checklist (the original app, every feature)", "",
           "Built by tools/make_parity.py from research/stick-nodes-features-raw.md "
           "(64 update posts, 1.0 to 4.3.4, store listings, FAQ). Descriptions are paraphrased.", "",
           "Legend: **[x]** usable in Barkfart now, **[~]** partial or in the core without UI, "
           "**[ ]** not yet, **(won't)** with the reason.", ""]
    counts = {}
    cat = None
    skip = True
    for line in open(src, encoding="utf-8"):
        if line.startswith("## "):
            cat = line[3:].strip()
            skip = cat.startswith("Sources") or cat.startswith("Not real")
            if not skip:
                out += ["", "## " + cat, ""]
            continue
        if skip or not line.startswith("|") or line.startswith("|---") or line.startswith("| Feature"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 2:
            continue
        name, what = cells[0], cells[1]
        since = cells[2] if len(cells) > 2 else ""
        pro = cells[3] if len(cells) > 3 else ""
        if cat.startswith("Removed"):
            out.append("- (removed in the original) **%s**: %s" % (name, what))
            continue
        code, note = STATUS.get(name, (" ", ""))
        if cat in WONT_CATEGORIES:
            code, note = "-", WONT_CATEGORIES[cat]
        box = {"x": "[x]", "~": "[~]", " ": "[ ]", "-": "(won't)"}[code]
        meta = ", ".join(x for x in [("since " + since) if since else "", "Pro" if pro.lower().startswith("y") else ""] if x)
        line_out = "- %s **%s**%s: %s" % (box, name, (" (" + meta + ")") if meta else "", what)
        if note:
            line_out += " *Barkfart: " + note + ".*"
        out.append(line_out)
        counts[code] = counts.get(code, 0) + 1
    total = sum(counts.values())
    summary = ["", "**Totals (%d features):** %d usable now, %d partial or core-only, %d not yet, %d won't." % (
        total, counts.get("x", 0), counts.get("~", 0), counts.get(" ", 0), counts.get("-", 0)), ""]
    out[6:6] = summary + ROADMAP.split("\n")
    open(os.path.join(root, "docs/barkfart/PARITY.md"), "w", encoding="utf-8").write("\n".join(out) + "\n")
    print(summary[1])
    missing = [n for n in STATUS if n not in open(src, encoding="utf-8").read()]
    if missing:
        print("status names not found in research:", missing)


if __name__ == "__main__":
    main(sys.argv[1])
