# What the editor still lacks (checked against the UI reference, 2026-11-21)

Work through these top to bottom; tick them off here as they land. Each
item names the reference section (docs/barkfart/research/local, not
committed). Plainest behaviour where the reference is unsure.

## Frames and playback
- [x] Clear frame (red X at the end of Frame Tools; confirm first) (8.4)
- [x] Silence all sounds on a frame (schedule: sounds stop at that slot) (8.4)
- [x] Slow-motion tweening per frame (schedule slot, tweening on only) (8.4)
- [x] Expand gradient (8.4)
- [x] Node counter "Nodes: N" at the top of Frame Tools (8.4)
- [x] "?" help for tweened frames, and the bar that picks 10% / 90% only (8.4)
- [x] Play long-press: "Loop to" start / end frame popup (7.3)
- [x] Animation Tools: looping on/off (7.1)

## Stickfigure Tools (9.2)
- [x] Paste stickfigure with layering (long-press)
- [x] Paste all properties: the window with a checkbox per property
- [x] Keep stickfigure during tween, Tween stickfigure colours, tween easing
      (both on by default, Barkfart's old behaviour; segment colours are not
      keyed per frame in Barkfart, so colour tweening only shows on imported
      animations with colour keys)
- [x] Use segment colour (own colour vs the figure colour)

## View options (8.6)
- [x] Show figure names, neighbour lines, outline selection
- [x] Zoom buttons, guides, stage box
- [x] Show filters (None / No blur / All on the stage; playback switch)
- [ ] Filter quality (the reference saw no difference; left out)

## Camera (10.3, 10.4)
- [x] Copy / paste camera
- [ ] Auto-camera (tween across frames with easing)
- [x] Lock figure to camera
- [x] Widescreen effect, wobble (export / playback only)

## Project and export (6)
- [x] Make vertical, rotate cameras 90 degrees
- [x] Watermark
- [ ] MP4 export (iOS: AVAssetWriter)
- [x] Open: sort, delete, past saves (backups)

## Library Tools (11)
- [x] Stickfigure / movieclip / sprite switcher, rename, delete (Barkfart has no
      separate imported-figure list, so "hide from the list" does not apply)
- [ ] Movieclip editor

## Figure editor (12)
- [x] Special Segment Tools: reorder segment / branch, split N, copy /
      paste segment and branch
- [x] Delete one segment keeping its children
- [x] Modify branch window
- [ ] Add connector, polyfill tool
- [ ] Trace stickfigure / frame, background image, background colour
- [ ] Save stickfigure to a file, add to library, backups

## Textfields (13)
- [ ] Fonts, outline, background box (check the reference table)

## Done today
- [x] Colour picker (copied colours, hue / saturation, brightness, hex)
- [x] Reset thickness, reset length
- [x] Segment gradient, scale, curve radius, shape options, angle lock
