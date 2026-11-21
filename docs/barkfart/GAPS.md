# What the editor still lacks (checked against the UI reference, 2026-11-21)

Work through these top to bottom; tick them off here as they land. Each
item names the reference section (docs/barkfart/research/local, not
committed). Plainest behaviour where the reference is unsure.

## Frames and playback
- [x] Clear frame (red X at the end of Frame Tools; confirm first) (8.4)
- [x] Silence all sounds on a frame (schedule: sounds stop at that slot) (8.4)
- [ ] Slow-motion tweening per frame (schedule slot, tweening on only) (8.4)
- [ ] Expand gradient (8.4)
- [x] Node counter "Nodes: N" at the top of Frame Tools (8.4)
- [ ] "?" help for tweened frames, and the bar that picks 10% / 90% only (8.4)
- [x] Play long-press: "Loop to" start / end frame popup (7.3)
- [ ] Animation Tools: looping on/off (7.1)

## Stickfigure Tools (9.2)
- [ ] Paste stickfigure with layering (long-press)
- [x] Paste all properties: the window with a checkbox per property
- [ ] Keep stickfigure during tween, Tween stickfigure colours, tween easing
- [ ] Use segment colour (own colour vs the figure colour): needs a model flag

## View options (8.6)
- [x] Show figure names, neighbour lines (outline selection: still to do)
- [x] Zoom buttons, guides, stage box
- [ ] Show filters, filter quality

## Camera (10.3, 10.4)
- [x] Copy / paste camera
- [ ] Auto-camera (tween across frames with easing)
- [ ] Lock figure to camera
- [ ] Widescreen effect, wobble (export / playback only)

## Project and export (6)
- [x] Make vertical, rotate cameras 90 degrees
- [ ] Watermark
- [ ] MP4 export (iOS: AVAssetWriter)
- [ ] Open: sort, delete, past saves (backups)

## Library Tools (11)
- [ ] Stickfigure / movieclip / sprite switcher, rename, hide, delete
- [ ] Movieclip editor

## Figure editor (12)
- [x] Special Segment Tools: reorder segment / branch, split N, copy /
      paste segment and branch
- [ ] Modify branch window; delete one segment keeping its children
- [ ] Add connector, polyfill tool
- [ ] Trace stickfigure / frame, background image, background colour
- [ ] Save stickfigure to a file, add to library, backups

## Textfields (13)
- [ ] Fonts, outline, background box (check the reference table)

## Done today
- [x] Colour picker (copied colours, hue / saturation, brightness, hex)
- [x] Reset thickness, reset length
- [x] Segment gradient, scale, curve radius, shape options, angle lock
