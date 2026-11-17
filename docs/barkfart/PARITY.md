# Feature parity checklist (the original app, every feature)

Built by tools/make_parity.py from research/stick-nodes-features-raw.md (64 update posts, 1.0 to 4.3.4, store listings, FAQ). Descriptions are paraphrased.

Legend: **[x]** usable in Barkfart now, **[~]** partial or in the core without UI, **[ ]** not yet, **(won't)** with the reason.


**Totals (382 features):** 63 usable now, 29 partial or core-only, 259 not yet, 31 won't.


## Order of work (parity first, then our own additions)
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


## Canvas & posing

- [x] **Drag nodes to pose** (since 1.0): Touch-drag a node to rotate its segment around its parent; the main node moves the whole figure *Barkfart: drag any joint; Rotate / Stretch.*
- [x] **Static vs dynamic nodes** (since ≤1.2.5): Static nodes are fixed detail nodes that cannot be dragged while animating *Barkfart: static (detail) nodes have no handles while animating.*
- [~] **Stretchy nodes** (since 1.0): Nodes marked stretchy can change their segment length while animating *Barkfart: Stretch mode stretches any joint; stretchy flag in the model and import, not enforced.*
- [~] **Per-figure scale** (since 1.0): Scale a whole stickfigure up or down *Barkfart: layer scale in the model; no UI.*
- [~] **Per-segment scale** (since 1.0): Scale an individual segment/limb independently *Barkfart: node scale imported (own length only, fitted to the original); no UI.*
- [~] **Per-figure and per-segment color** (since 1.0): Recolor whole figures or single segments while animating *Barkfart: colour tracks in the model and import; no picker UI.*
- [x] **Pinch zoom on stage** (since 1.0): Pinch to zoom the stage view
- [x] **Zoom out below 100% and move stage** (since 1.2.6): Zoom out to 50% and drag the stage to place figures off-frame
- [ ] **Max zoom increases** (since 1.7.0): Max zoom went 500% → 800% (1.7.0) → 2000% (2.3.2) → 5000% (3.2.0)
- [x] **Smoother zoom** (since 2.4.4): Zoom made far more responsive
- [ ] **Panning mode** (since 1.1.3): Drag the canvas to move every figure on the current frame at once *Barkfart: dragging empty stage pans the view; moving all figures at once is not built yet.*
- [~] **Lock stickfigure** (since 1.2.6): Lock button freezes a figure in place to prevent accidental moves *Barkfart: layer lock in the model and undo; no button yet.*
- [x] **Locked figures hide nodes** (since 2.5.1): A locked figure hides all its nodes except the main node (later: regardless of joins) *Barkfart: locked layers show no handles.*
- [ ] **Pinch-rotate figure** (since 1.2.8): Two-finger pinch-rotate rotates the selected stickfigure
- [ ] **Rotation knobs** (since ≤2.1.0 (improved)): Knob controls for angles that preview rotation in real time
- [ ] **Tap knob to snap** (since 3.2.2): Tapping the rotate knob snaps to 0/90/180/270
- [ ] **45° snap on angle fields** (since 4.1.6): Quick button on every angle field to snap to 45° steps
- [ ] **Figure rotation indicator** (since 4.2.0): Main node shows a line for the figure's rotation; rotation editable numerically
- [ ] **Segment angle +/-** (since 2.2.2): Set segment angle with increment buttons
- [ ] **Axis lock while dragging** (since 2.2.0): Dragging a figure, textfield or camera in a straight line locks to that axis (toggle in settings)
- [ ] **Flip figure (X/Y)** (since ≤1.7.8): Flip a stickfigure horizontally/vertically
- [ ] **Flip segments in Animation Mode** (since 2.3.7): Flip individual segments while animating, not just in creation
- [ ] **Join-anchor flip** (since 4.2.3): Long-press flip to mirror joined figures around the join anchor axis
- [ ] **Individual node lock** (since 2.4.8): Pin a single node so it stays in world position while other nodes move (e.g. planted foot)
- [~] **Angle Lock (node)** (since 4.2.0): Keep a node at a fixed angle, absolute or relative to the nearest non-static parent *Barkfart: absolute and relative locks in pose and drag; legacy encoding not mapped; no UI.*
- [~] **Drag Lock (node)** (since 4.2.0): Constrain a node's drag to a specific axis *Barkfart: horizontal / vertical drag lock; legacy encoding not mapped; no UI.*
- [ ] **Floaty stretchy nodes** (since 4.2.0): Alternate tween behaviour for stretchy nodes
- [ ] **Reset length** (since ≤2.4.9): Button that restores a segment's original length (applies smart-stretch)
- [~] **Push back / bring forward** (since ≤4.0.0): Change a figure's layer order in the frame *Barkfart: layer order and node draw order commands exist; no UI.*
- [ ] **Push moves joined figures** (since 4.0.0): Layer push/pull also moves all joined figures
- [ ] **Precision mode** (since 1.4.0): Choose exact (stylus) or approximate (finger) touch precision
- [ ] **Node-digging** (since 4.2.0): Repeated taps in a node cluster cycle through every node there
- [ ] **Selected node sticks** (since 4.2.0): A just-selected node stays selected on the next touch so it can be dragged
- [ ] **Dig to joined main node** (since 4.3.3): Tapping a join-anchor again quickly selects the joined figure's main node
- [x] **Forgiving taps** (since 4.0.2): More tolerant tap hit-testing (esp. sprites, phones) *Barkfart: 44 pt hit radius, nearest joint wins.*
- [x] **Tap off-stage to deselect** (since 4.0.3): Tapping dead area beside the stage counts as tapping nothing *Barkfart: tap empty stage deselects.*
- [x] **Cancel accidental grab on zoom** (since 4.0.3): If a pinch-zoom starts on a node/sprite, the accidental move is undone *Barkfart: second finger cancels the drag.*
- [x] **Keep selection while dragging stage** (since 4.0.3): Dragging the stage no longer clears selection
- [ ] **Drag off stage stays responsive** (since 4.1.6): Dragging continues to track when the finger leaves the stage
- [x] **Selection kept across pan/camera mode** (since 4.3.3): Entering/leaving pan or camera mode keeps the selected node
- [ ] **Max figure scale** (since 4.1.0): Max scale raised from 10x to 20x

## Stickfigure creation (segments, shapes, nodes, polyfills, connectors, smart stretch, locks)

- [ ] **Creation Mode** (since 1.0): Separate editor for building stickfigures from segments
- [ ] **Add segment / limb** (since 1.0): Draw a new segment from a selected node
- [ ] **Duplicate / copy-paste segment** (since ≤1.7.7): Copy a segment (and branch) and paste it elsewhere
- [~] **Draw order control** (since 1.0): Reorder segments' depth within the figure *Barkfart: node draw order in the model and undo; no UI.*
- [x] **Line segment** (since 1.0): Basic straight segment *Barkfart: square-ended segment drawn natively.*
- [x] **Circle segment** (since ≤1.7.6): Circle-shaped segment (outline; fill via gradient from 1.8.0)
- [x] **Rounded segment** (since ≤3.0.2): Segment with rounded caps
- [x] **Triangle segment** (since ≤3.0.0): Triangle shape (exact launch list of segment types unknown) *Barkfart: isosceles, right-angled and upside-down.*
- [x] **Filled Circle segment** (since ≤1.7.x): White-filled circle type; **REMOVED 1.8.0** (replaced by circle + gradient) *Barkfart: imported as a filled circle.*
- [ ] **Draw tools always visible** (since 1.2.5): Drawing tools permanently shown in Creation Mode
- [ ] **Split segment** (since 1.2.5): Split a segment into two
- [ ] **Split into N** (since 4.1.6): Choose how many pieces a segment splits into
- [ ] **Split along curve** (since 3.2.2): Splitting a curved segment follows the curve
- [ ] **Show/hide nodes toggle** (since 1.2.5): Hide nodes in Creation Mode to view the design
- [ ] **Outline selected segment** (since 1.2.5): Selected segment drawn with a highlight outline (red)
- [ ] **Inherit node properties** (since 1.2.5): New nodes copy parent's static/stretchy setting; sensible default draw order
- [ ] **Segment flip X/Y** (since 1.2.6): Flip individual segments in creation
- [ ] **Creation background color/image** (since 1.7.0): Set a background color or image in Creation Mode for tracing
- [ ] **Background in front** (since 2.2.1): Show the background on top of the figure in Creation Mode
- [ ] **Load figure as guide** (since 2.2.2): Load another stickfigure as a background reference in Creation Mode
- [ ] **Trace frame** (since 3.0.0): Use a project frame as a reference background in Creation/MC editing
- [ ] **Trace options** (since 4.2.4): Show/hide nodes when tracing; scrub the traced frame quickly
- [x] **Segment color gradients** (since 1.7.6): Two-color gradient along a segment *Barkfart: along the segment, figure-colour rule fitted.*
- [x] **Radial circle gradients** (since 2.5.6): Circle gradients radiate from center; separate circle outline color
- [x] **Sideways gradients** (since 4.1.0): Gradient direction toggle "sideways" for segments and trapezoids; triangles in 4.3.0
- [ ] **Circle gradient horizontal/vertical** (since 4.2.4): Linear gradients on circles
- [x] **Polygon radial gradient** (since 4.2.6): Radial gradient on polygons *Barkfart: polygons take radial gradients.*
- [x] **Reverse gradient** (since ≤4.2.2): Swap gradient direction
- [x] **Shapes: ellipse, trapezoid, right-triangle, polygon** (since 2.5.6): New shape types with Shape Tools menu *Barkfart: all drawn and imported; trapezoid and polygon geometry unverified (no reference file).*
- [~] **Shape Tools properties** (since 2.5.6): Per-shape properties: half arc, vertex count (polygon), trapezoid top ratio *Barkfart: half arc, polygon sides, trapezoid ratio in the model, drawn, imported; no UI.*
- [~] **Polygon min 4 sides** (since 3.0.5): Polygons from 4 to 16 vertices (was 5) *Barkfart: 4-16 sides in the model; no UI.*
- [x] **Triangle upside-down** (since 3.0.0): Upside-down option for triangles
- [~] **Half-circle reversible** (since 4.2.4): Flip half-arc circles *Barkfart: half arcs drawn either side; no creation UI.*
- [~] **Circle hollow** (since 4.1.0): Circle without interior fill *Barkfart: drawn and imported; no creation UI.*
- [x] **Curved segments** (since 3.2.0): Segments and rounded segments can bend with a curve radius *Barkfart: arcs, bulge fitted to the original's renders.*
- [ ] **Curve "circular" toggle** (since 4.1.0): Alternative circular curve behaviour
- [~] **Trapezoid curves and rounded ends** (since 4.2.0): Trapezoids can curve, have rounded tips, easier thickness *Barkfart: round ends drawn; curved trapezoids not yet.*
- [ ] **0-length segments visible** (since 3.2.1): Zero-length segments render
- [x] **Polyfills** (since 2.3.0): Fill shapes defined by an ordered set of nodes (polynodes) *Barkfart: drawn (concave, ear-clipped) and round-trip; no creation UI.*
- [ ] **Finish polyfill** (since ≤2.4.9): Button to complete a polyfill definition
- [ ] **Polyanchor outline** (since 2.3.2): Selecting a polyanchor highlights its polyfill
- [ ] **Copy/paste polyfills** (since 3.1.0): Polyfills copy with segments when all polynodes are included
- [x] **Polyfills follow curves** (since 4.1.0): Polyfill edges follow curved segments; curve precision setting *Barkfart: fill edges follow the drawn arc.*
- [ ] **Polyfill Precise mode** (since 4.2.4): Easier node picking, view/adjust polynode order
- [x] **Smart-Stretch** (since 2.4.8): Stretching a node proportionally squashes/stretches its static child detail nodes *Barkfart: stretching a smart-stretch joint reshapes its static details (rule fitted to the original's own files).*
- [x] **Smart-stretch changes** (since 2.5.1–2.5.2): Children need not be stretchy; ignores non-stretchy children *Barkfart: details need not be stretchy; moving nodes stop it.*
- [~] **Do not smart-stretch** (since 2.5.2): Exclude a node/branch from smart-stretch *Barkfart: honoured; set only by import (no creation UI).*
- [ ] **Test smart-stretch** (since ≤2.4.9 / keep 3.0.0): Preview smart-stretch in creation; option to keep the test changes
- [ ] **Smart-stretch advanced options** (since 4.2.0): Extra options enabling IK-style rigs with connectors
- [~] **Connectors** (since 4.2.0): Segment that always stays attached between two nodes, with draggable midpoint *Barkfart: imported and exported; not drawn as connectors.*
- [ ] **Connector direction indicators** (since 4.2.4): Visual cue for connector direction
- [ ] **Modify Branch** (since 2.3.7): Mass-edit a node and all children: static, thickness, length, colors (replace), draw order
- [ ] **Modify Branch: limb type** (since 2.4.2): Change segment type for a whole branch
- [ ] **Modify Branch: segment length** (since 3.1.0): Length field in Modify Branch
- [ ] **Segment length field** (since 3.0.5): Length visible/editable in Segment Tools
- [ ] **Paste segment preserves angle** (since 2.5.6 / toggle 2.5.7): Pasted segments keep direction; toggle to preserve visual angle
- [ ] **Copy/paste keeps draw order** (since 2.3.0): Pasted segments remember draw order
- [ ] **Push branch back/forward** (since 4.1.6): Move a whole branch's draw order at once
- [ ] **Copy main node = copy figure** (since 4.2.7): Copy button on main node copies whole stickfigure
- [ ] **Toggle shape type (quick tools)** (since 4.3.0): Quick-switch segment shape in Creation
- [ ] **Node copy/paste/delete quick buttons** (since 3.2.2): In Creation Quick Resize Tools
- [ ] **Show only non-static nodes / show draw order** (since ≤4.0.0): Creation view toggles (moved to View Options in 4.0.0)
- [ ] **Stickfigure auto-save** (since 3.3.0): Creation auto-saves every 5 min, with history browser (disable in settings)
- [ ] **Default IK figure** (since 4.2.0): Ships a sample IK-rigged default stickfigure
- [ ] **Node limit per figure** (since 2.3.6): Limits rose over time: 384 (2.3.6), 400 (4.0.2)
- [ ] **Name/ID on selection** (since 4.0.0): Optional brief display of figure name/ID

## Frames & timeline & tweening

- [~] **Frame list / thumbnails** (since 1.0): Horizontal strip of frame thumbnails with scrollbar *Barkfart: replaced by the dope sheet (by design); thumbnails not yet.*
- [~] **Add/copy/paste/delete frames** (since 1.0 (inferred)): Basic frame management (paste first mentioned 1.2.7) *Barkfart: dope sheet copy/paste/delete keys; insert/delete time in core.*
- [ ] **Add/delete frame buttons by timeline** (since 1.2.9): Quick buttons right of the timeline
- [x] **Play / Stop buttons** (since 1.0 (inferred)): Playback controls next to thumbnails (moved out of Animation Tools in 1.6.3)
- [x] **FPS setting** (since 1.0 / 60 in 1.7.0): Frames per second; limit raised from 20 to 60 *Barkfart: sidebar Frame rate - / +.*
- [x] **Tweening toggle** (since 1.0): Auto-insert interpolated frames between keyframes *Barkfart: per key: Hold vs Linear/eased (easing menu).*
- [x] **Tweened-frame count** (since 1.7.6): Choose 1–8 in-between frames (previously fixed at 2) *Barkfart: keys are placed in time; in-betweens are implicit.*
- [x] **Tween 0 frames** (since 3.3.1): Tweening on with 0 in-betweens, enabling per-frame slow motion only *Barkfart: Hold easing.*
- [x] **Is loop** (since 1.2.7): Tween last frame back to first for seamless loops *Barkfart: playback loops.*
- [ ] **Keep during tween** (since 1.8.0): Per-figure flag to stop a figure flickering/vanishing during tweening
- [x] **Tween colors** (since 2.3.0): Per-figure checkbox to interpolate colors *Barkfart: colour tracks interpolate.*
- [x] **Tween Mode per figure** (since 4.2.4): Linear, ease-in, ease-out or both per figure per frame *Barkfart: per key easing presets with curves.*
- [ ] **Frame delay** (since 2.0.0): Hold a frame for extra time
- [ ] **Move camera during delay** (since 3.1.0): Camera keeps tweening during a frame delay
- [ ] **Slow motion frame** (since 3.1.0): Per-frame checkbox that greatly increases tweening for that frame
- [ ] **Add tweened frame** (since 2.3.0): Generate a new in-between keyframe from neighbours
- [ ] **Add tweened frame percentages** (since 2.3.1–2.3.2): 10/25/50/75/90%, 50, 33/67 combo, 10/90 easing combo
- [ ] **Add many tweened frames** (since 3.1.0): Long-press to insert any number of tweened frames
- [x] **Copy multiple frames** (since 2.2.2): Thumbnail menu to copy several frames (up to 25 in 3.0.0) *Barkfart: box/multi-select keys, copy, paste at playhead.*
- [x] **Clear frame** (since 2.2.0): Delete everything in a frame *Barkfart: select keys at a frame and delete.*
- [~] **Frame repeat** (since 3.3.0): Repeat a series of frames N times *Barkfart: loop modifier in the core; no UI.*
- [ ] **New-frame indicator** (since 2.3.2 / 3.1.0): Blue flash/indicator on newly added frames
- [ ] **Selected-frame direction indicators** (since 3.1.9): Timeline arrows show where the current frame is; delete no longer jumps
- [ ] **Frame number backgrounds** (since 2.4.0): Background behind frame numbers for readability
- [ ] **Custom playback range** (since 4.1.6): Play button options to play only a range
- [x] **Frame skipping under lag** (since 3.3.0): Playback drops frames to keep timing (sound sync) *Barkfart: playback clock is time-based.*
- [x] **Preserve selection across frames** (since 4.1.6): Keep the same figure/node selected while changing frames (setting)
- (won't) **Frame node limit** (since 2.3.6): Per-frame cap (40 figures/6400 nodes in 2.3.6; nodes only 6400 in 3.0.0; 12,800 in 4.0.0) *Barkfart: no artificial limits (budgets only guard runaway recursion).*
- (won't) **Double frame node limit** (since 3.3.0): Setting to double the per-frame node cap *Barkfart: no limits.*
- [x] **Unlimited nodes per frame** (since 4.0.0): Setting to remove the per-frame cap *Barkfart: no limits.*
- [ ] **Render every 3rd thumbnail** (since 3.0.0): Lag option to draw fewer thumbnails

## Onion skin & view options

- [x] **Onion skin** (since ≤1.1.4): Ghosts of neighbouring frames *Barkfart: from the evaluator, past red / future blue.*
- [ ] **Onion skin in front** (since 1.6.2): Show onion skin on top of figures
- [~] **Onion skin frame count** (since 3.0.0): Past/future onion frame counts (0 allowed) *Barkfart: fixed 2 back / 1 forward; no setting yet.*
- [ ] **Textfields in onion skin** (since 2.2.2): Text appears in onion skin
- [ ] **Camera in onion skin** (since 1.7.1): Camera box shown in onion skin
- [~] **Visible in onion skin** (since 4.0.2): Per-figure toggle to leave out of onion skin (e.g. backgrounds) *Barkfart: per-layer flag in the model; no toggle UI yet.*
- [ ] **View Options menu** (since 3.0.0): Popup by play/stop gathering view settings
- [ ] **Segment outline in animation mode** (since 2.2.0): Outline segments while animating
- [ ] **Guidelines** (since 2.2.0): On-stage guide lines for positioning (saved with project from 2.5.1)
- [ ] **Camera gridline patterns** (since 4.2.4): Selectable grid patterns in the camera
- [ ] **Show neighbor nodes** (since 3.1.0): Highlight nodes directly connected to the selection
- [ ] **Neighbor nodes across joins** (since 4.1.3 / toggle 4.1.4): Neighbor lines include joined figures' nodes (toggle)
- [ ] **Show only main nodes** (since ≤4.1.6): Show only main nodes (join anchors included from 4.1.6)
- [ ] **Show all nodes of active figure** (since 4.2.4): Exception to main-nodes-only for the selected figure
- [ ] **Hide locked nodes** (since 4.3.3): Hide locked nodes to reduce clutter
- [ ] **Active-filter notification dot** (since 4.3.3): Dot warns when hide-locked or main-nodes-only is on
- [x] **Remove stage outline** (since 4.0.3): Hide the default stage box outline *Barkfart: no outline by design.*
- [ ] **Culling** (since 2.3.9): Skip drawing nodes outside the view
- [ ] **Node style** (since 4.1.0): Selectable node/line rendering style
- [ ] **Line/node thickness** (since 3.1.0): Setting for UI node/line thickness
- [ ] **Filter quality low/high** (since 2.4.4): Toggle preview filter quality (later in View Options)
- [ ] **Disable blur/glow temporarily** (since 3.1.0): Turn off heavy filters for performance
- [x] **Stop resets view** (since 1.7.1 / 2.2.2): Stop button resets zoom; double-tap jumps to camera view *Barkfart: Stop returns to the frame where play started.*
- [x] **Double-tap to reset zoom** (since 1.2.7): **REPLACED 1.7.0** by double-tap camera mode *Barkfart: double-tap empty stage fits.*
- [ ] **Larger stage** (since 2.3.9): Stage area enlarged
- [ ] **Fullscreen playback** (since 2.1.0): Play animation fullscreen

## Camera

- [x] **V-cam / Camera Mode** (since 1.7.0): Movable, zoomable camera defining output view per frame *Barkfart: camera frame on the stage, applied on play/export.*
- [x] **Mode buttons** (since 2.0.0): Normal/Panning/Camera mode buttons top-left *Barkfart: Figure | All | Camera.*
- [x] **Camera rotation** (since 2.3.0): Rotate the camera *Barkfart: rotate knob on the frame.*
- [~] **Camera wobble/shake** (since 2.3.0): Automatic camera wobble with settings (improved 3.1.0; higher max 3.1.9) *Barkfart: wiggle modifier in the core; no UI.*
- [ ] **Widescreen bars** (since 2.2.0): Letterbox effect
- [ ] **Auto-Camera** (since 2.3.7): Choose start/end frames; camera tweens smoothly between, adapting to inserted frames
- [ ] **Auto-camera easing** (since 2.5.6): Ease in/out plus bounce, swing, elastic
- [ ] **Lock figure to camera** (since 2.3.7): Figures follow the camera (HUD/background)
- [ ] **Parallax** (since 2.3.7 / 2.4.4): Parallax factor for camera-locked figures; applies to camera scale
- [x] **Copy/paste camera** (since 3.1.0 / 4.0.0): Copy/paste camera lock bundles, wobble properties, frame camera (cross-project) *Barkfart: camera keys copy/paste in the dope sheet.*
- [x] **Stay in camera mode on frame tap** (since 4.0.2): Tapping frames stays in Camera Mode

## Movieclips

- [~] **Movieclips** (since 3.0.0): Reusable animated objects with their own frames, placed like figures *Barkfart: symbols with nesting, offset, speed, loop in the core; no UI.*
- [ ] **MC editing mode** (since 3.0.0): Edit a movieclip's frames in place
- [ ] **MC Library Tools** (since 3.0.0): Add/edit movieclips in the library
- [~] **Once / Loop playback** (since 3.0.0): Movieclip playback mode *Barkfart: symbol loop modes in the core.*
- [ ] **Tween protection** (since ≤3.1.5): MC option affecting tween/delay behaviour
- [ ] **Break apart MC** (since ≤3.3.1): Convert a movieclip back to figures
- [ ] **Convert figure to MC / double-tap edit** (since 4.0.0): Settings for double-tap to convert to MC or edit MC
- [ ] **MC background** (since 3.1.0): Load a background in MC creation
- [ ] **MC trace frame increment** (since 3.1.9): Traced project frame advances as MC frames are added
- [ ] **MC playback camera offset** (since 3.1.7): Offset preview camera in MC editing
- [ ] **Cache MC** (since 3.1.9): Rasterize single-frame MCs for faster rendering
- (won't) **Default movieclips** (since 3.3.0): Bundled sample movieclips *Barkfart: cannot ship the original's bundled content; ours later.*
- [ ] **MC limits** (since 4.0.0 / 4.0.2): Up to 256,000 nodes and 200 frames per MC
- [x] **.nodemc files** (since 3.0.0 / send 3.1.0): Movieclip file format, sendable *Barkfart: import from the File panel (all 12 sample clips parse).*
- [~] **Scaled/opacity MCs** (since ≤3.0.2): Movieclips support scale and opacity like figures *Barkfart: layer scale/opacity apply to symbol layers in the core.*
- [ ] **Submission type prompt** (since ≤4.1.5): Asks whether a stickfigure submission should be a movieclip instead

## Sprites & images

- [ ] **Frame background color** (since 1.0): Per-frame solid background color via color chooser
- [ ] **Gradient frame background** (since 2.0.0): Frame backgrounds can be two-color gradients
- [ ] **Background images** (since 1.4.0): Load an image from the device as the frame background
- [ ] **Background image scaling** (since 1.7.0): Scale background image independently in x and y, beyond stage bounds
- [ ] **Higher-res background loading** (since 2.5.3): Background images loaded at higher quality
- [ ] **PNG transparency in backgrounds** (since ≤2.5.4): Transparent PNGs respected (iOS fix)
- [ ] **Sprites** (since 4.0.0): Import images as movable, scalable, rotatable stage objects
- [ ] **Create Sprite dialog** (since 4.0.0 / 4.0.1): Define sprites from an image with selections; remove/cancel selections
- [ ] **Remove background color** (since 4.1.0): Tool in Create Sprite to knock out a background color
- [ ] **Color picker in sprite import** (since ≤4.2.7): Sample colors from the image during sprite import
- [ ] **Minecraft skin import** (since 4.0.0): Sprite importer turns a Minecraft skin into a fully rigged stickfigure
- [ ] **Generated sprites hidden** (since 4.0.1): Auto-generated sprites/figures hidden in library by default
- [ ] **SpriteGroups** (since ≤4.1.6): Named groups of sprites; next name auto-numbered
- [ ] **Mass image import** (since 4.2.7): Import many images as sprites at once
- [ ] **GIF import** (since 4.2.7): Animated GIF imported as a movieclip
- [ ] **Sprite distort / skew** (since 4.3.0): Toggle a distort mode to warp, skew and squash sprites
- [ ] **Separate X/Y sprite scale** (since 4.3.0): Independent axis scale controls for sprites
- [ ] **Negative import scale** (since 4.3.0): Internal scale at import can be negative (baked-in flip)

## Text

- [ ] **Textfields** (since 1.5.0): Add text objects to frames (speech, titles)
- [ ] **Text tweening** (since 1.7.6): Textfields interpolate between keyframes (new projects only)
- [ ] **Textfield limit** (since 1.7.6): Max per frame raised to 5 (1.7.6), then 10 (2.4.9)
- [ ] **Text pans with figures** (since 2.2.2): Panning mode also moves textfields
- [ ] **Lock text to corner** (since 2.2.0): Pin a textfield to a screen corner (name/watermark)
- [ ] **Project-wide watermark** (since 2.4.4): Corner text applies to all frames or none
- [ ] **Text rotation** (since 2.3.7): Rotate textfields
- [ ] **Text transparency** (since 2.3.7): Fade textfields
- [ ] **Text shadow** (since 2.4.2): Drop-shadow property for text
- [ ] **Fonts** (since 2.4.4): Choose among bundled fonts
- [ ] **More fonts** (since 2.5.1): Additional fonts (2.5.1), Impact/Comic Sans/Determination (3.1.0)
- [ ] **Japanese font** (since 3.0.4): Font that renders Japanese characters
- [ ] **Load/unload fonts** (since 4.0.0): Fonts must be loaded to use, unloaded to save memory
- [ ] **Text scale up to 10** (since 2.5.9): Raised textfield scale limit
- [ ] **Lock textfield** (since 3.1.0): Lock a textfield against edits/moves
- [ ] **Text lock to camera** (since 3.1.0): Text follows the camera
- [ ] **Draw above widescreen bars** (since ≤3.1.3): Option to draw text over letterbox bars
- [ ] **Paste external text** (since 3.1.9): Paste button pastes clipboard text from other apps
- [ ] **Copy/paste textfields across projects** (since 4.0.0): Text clipboard persists through opening projects

## Sound

- [ ] **Sounds on frames** (since 1.6.0): Attach sound clips to frames; heard in playback and MP4 export
- [ ] **Bundled sound library** (since 1.6.0): Ships ~40 sounds, regularly expanded (searchable by version tag, e.g. "330")
- [ ] **Custom MP3 import** (since 1.6.0): Use own MP3s (max 256 KB)
- [ ] **iOS share-sheet import** (since 2.2.2): Open an MP3 from another app into Stick Nodes Pro
- [ ] **stknds:// URL download** (since 2.2.2): Prefix a web MP3 URL to download it into the app (iOS)
- [ ] **Sound preview** (since 2.3.7): Preview sounds in the import list
- [ ] **Multi-select sound import** (since 2.3.7): Import several sounds at once
- [ ] **Copy/paste sounds** (since 2.3.0): Copy a frame's sounds and paste onto another
- [ ] **Sound pan** (since 3.1.0): Left/right stereo panning per sound
- [ ] **Sound pitch** (since 3.1.0): Pitch shift per sound
- [ ] **Pause with device music** (since 1.7.6): Clips stay silent while the user plays music
- [ ] **Background music on iOS** (since 2.2.4): Other apps' music can keep playing while animating
- [ ] **Higher export audio quality** (since 3.1.0): Improved audio encoding in exports

## Filters/effects

- [~] **Transparency** (since 2.2.0): Per-figure opacity *Barkfart: layer opacity in the core; no slider.*
- [ ] **Invert colors** (since 2.2.0): Invert a figure's colors
- [ ] **Tint** (since 2.2.0): Wash a figure in a color
- [ ] **Saturation** (since 2.2.0): Desaturate or boost color
- [ ] **Blur** (since 2.2.0): Horizontal, vertical or both
- [ ] **Glow** (since 2.3.0): Glow around a figure
- [ ] **Glow intensity** (since 2.4.2): Strength control for glow
- [ ] **Hue shift** (since 3.0.4): Rotate a figure's hues
- [ ] **Drop shadow** (since 3.3.0): Offset shadow, also usable as edge glow
- [ ] **Pixelate** (since 4.0.3): Pixelation filter
- [ ] **Motion blur** (since 4.0.3): Directional motion-blur toggle (incl. one direction) in blur
- [ ] **Outline** (since 4.1.6): Outline filter around a figure
- [ ] **Blur/glow max 2.0** (since 3.0.5): Raised maximum strength
- [ ] **Low in-app quality** (since 2.2.0): Filters preview at lower quality, full quality on export
- [ ] **Copy/paste/clear filters** (since 2.3.0): Filter clipboard and clear button
- [ ] **Use parent's filters** (since 3.1.0): Joined figures inherit their parent's filters
- [ ] **Collapsible Filter Tools** (since 4.1.6): Filter panel can be collapsed

## Export & sharing

- [ ] **GIF export** (since 1.0): Animated GIF output (Free adds watermark)
- [ ] **GIF >256-color handling** (since 1.4.0): Better color quantization for image-heavy GIFs
- [ ] **MP4 export** (since 1.2.3): Video export
- [ ] **Fast hardware MP4** (since 1.7.0): Faster, higher-quality MP4 encoding
- [ ] **Old MP4 method button** (since 1.7.3): Fallback encoder; **REMOVED 4.0.3**
- [ ] **1080p MP4** (since 3.0.5): Max 1920×1080 output
- [ ] **PNG export** (since 2.5.1): Single-image export
- [ ] **4K PNG** (since 3.3.0): PNG at 4K
- [ ] **Export frame range** (since 2.3.0): Choose start/end frames (single frame allowed)
- [ ] **Anti-aliasing** (since 2.3.6): Smooth-lines export option (GIF unlocked at any size 2.4.1)
- [ ] **Low-quality blur export** (since 3.0.0): Export using preview-quality blur
- [ ] **Better quality blur** (since 3.3.0): Experimental higher-quality blur/glow export
- [ ] **Vertical video** (since 4.0.3): Portrait export toggle (Project Tools)
- [ ] **Export size slider** (since 4.0.3): Size slider on GIF and PNG export windows
- [ ] **Non-blocking export** (since 1.6.0): Exporting no longer freezes the device
- [ ] **Exports to gallery** (since 1.2.3): iOS camera roll; Android Pictures/Movies (3.3.0)
- [ ] **Free watermark** (since 1.0): Free version stamps exports
- [x] **Send files** (since 2.1.0): Email/share projects, figures, sounds, MCs *Barkfart: share sheet for projects and exported figures.*
- [ ] **Auto-ZIP send** (since 4.0.0): Sending >5 files bundles a ZIP

## Files, projects & import

- [x] **Save/open projects** (since 1.0): Project files (.stknds) *Barkfart: File panel: new, open, save, rename, duplicate, delete with undo; autosave.*
- [x] **Save/import stickfigures** (since 1.0): Figure files (.nodes) into a project library *Barkfart: import from the folder or the Files picker; export .nodes byte-exact + share.*
- [ ] **Pivot STK import** (since 1.0): Import Pivot 2.x stickfigures
- (won't) **Default stickfigures** (since 1.0): Bundled starter figures (refreshed 2.0.0) *Barkfart: cannot ship the original's figures; ours later.*
- [x] **Delete files** (since 1.6.0): Permanently delete projects/figures/sounds in-app *Barkfart: soft delete with Undo.*
- [ ] **Alphabetical lists** (since 1.6.0): Open/import lists sorted A–Z
- [ ] **Natural sort** (since 3.1.0): Numbers in names sorted numerically
- [ ] **Sort new/old** (since 2.3.0): Sort file lists by date
- [ ] **List filter** (since 2.0.0): Text filter on open/import lists
- [ ] **Library dropdown filter** (since 3.3.2): Filter in imported figure/MC/sound dropdowns
- [ ] **Figure preview** (since 2.3.7 / 4.1.2): Preview figures before importing; nodes on/off in preview
- [x] **Multi-select import** (since 2.3.7): Import several figures at once *Barkfart: the Files picker allows several files.*
- [ ] **Edit imported library** (since 3.0.0): Rename, hide, delete imported figures (search added 4.1.6)
- [~] **Open with / file association** (since 1.7.9): Open NODES/STK/STKNDS from other apps *Barkfart: the Files picker imports; Open-in from other apps not wired yet.*
- [ ] **Android download auto-scan** (since 1.7.9): Auto-collects downloaded files; **REMOVED 3.2.0**
- [ ] **Open ZIP packs** (since 2.0.1): Extract figure packs into the library
- [ ] **Open mixed ZIPs** (since 4.0.0): ZIP containing any Stick Nodes file types
- [~] **Partial project open** (since 2.2.2 / 2.3.6): Open fewer frames, or a percentage from first/last frame *Barkfart: .stknds: figure library imports; animation not yet.*
- [ ] **Out-of-memory warning** (since 2.5.7): Warns instead of crashing on oversized projects
- [ ] **Save view state** (since 2.5.1): Project saves zoom, position, current frame, guides
- [x] **Local project backup** (since 4.3.0): Overwritten saves kept temporarily, restorable *Barkfart: one rolling .bak per save; deleted projects go to a hidden trash with Undo.*
- [ ] **Library tools** (since 1.2.4): Submit figures to and download figures from the website library in-app (submission form existed earlier)
- [ ] **Pack submission** (since 2.3.7 / 2.5.9): Multi-figure submit with auto-ZIP and file renaming
- [ ] **Submission rules dialog** (since 2.5.9): Guidelines shown before submitting
- [ ] **Sample projects** (since 4.3.2): Onboarding sample projects; refresh defaults in settings *Barkfart: our own samples later.*
- [ ] **Uninstall warning** (since 4.1.4): Warns that uninstalling deletes files

## Joining/layering/figure tools

- [ ] **Joining** (since 1.7.4): Attach a figure's main node to any node of another figure
- [ ] **Join anchor list** (since 3.0.0): Selecting an anchor lists figures joined to it
- [ ] **Joined-figure navigation** (since 4.0.0): Tap names to hop between joined figures; shows parent
- [ ] **Joined scale option** (since 2.4.4): Joined figures scale together
- [ ] **Copy with joins** (since 3.1.0): Long-press copy includes joined figures
- [ ] **Paste with layering** (since 3.1.0): Long-press paste tries to keep layer position
- [ ] **Copy all figure properties** (since 2.5.9): Copy scale, filters, position etc. onto another figure
- [ ] **Properties include layer** (since 2.6.1): Property copy includes layer position
- [ ] **Edit figure shortcut** (since 4.1.6): Edit button in Stickfigure Tools opens that figure
- [ ] **Quick Tools figure lock** (since 4.3.1): Lock toggle in quick tools

## Settings & app tools

- [ ] **Languages** (since 2.0.0): EN, ES, FR, PT, TR
- [ ] **Filipino** (since 2.3.5): Added language
- [ ] **Japanese** (since 2.6.1): Added language
- [ ] **Russian** (since 3.1.0): Added language
- (won't) **Save reminder** (since 2.1.0): Prompt after X minutes unsaved; shows time since save (4.2.4) *Barkfart: replaced by autosave every 30 s and on backgrounding.*
- [ ] **Lag options** (since 2.4.4): Performance options under undo/redo
- (won't) **Memory display** (since 2.3.6): Memory use shown on open, in settings, near add-frame *Barkfart: replaced by performance work (M9).*
- (won't) **Memory Use dialog** (since 4.0.0): Breakdown dialog from the memory indicator *Barkfart: replaced by performance work (M9).*
- (won't) **Lag Problems button** (since 4.3.2): Help button next to memory indicator *Barkfart: replaced by performance work (M9).*
- [ ] **Custom skins** (since 2.4.4): User re-skinning via /skin/ folder
- [x] **Dark Mode skin** (since 3.1.0): Built-in dark theme *Barkfart: dark theme.*
- [ ] **Left-hand mode** (since 2.6.1): Mirrors UI layout
- [ ] **Zoom buttons** (since 3.1.0): Corner zoom buttons
- [ ] **Quick Menu toggle** (since 2.4.8): Enable/disable quick menu
- [ ] **+/- buttons toggle** (since 4.0.0): Show increment buttons (hidden by default)
- [x] **Screen padding** (since 4.2.0): UI inset for notches *Barkfart: safe areas handled automatically.*
- [~] **Haptics** (since 4.2.4): Vibration feedback *Barkfart: long-press vibrates; no setting.*
- [ ] **Nerd Mode** (since 4.1.6): Less descriptive UI text
- [ ] **Keep App Alive** (since 3.2.0 / toggle 4.2.0): Android notification to avoid being killed
- [ ] **Landscape flip** (since 2.0.0): Use either landscape orientation
- [ ] **Non-continuous rendering** (since 1.2.0): Battery-saving redraw
- [ ] **Chromebook support** (since 3.1.6): Saving/exporting on Chromebooks

## UI/quality-of-life (undo, quick tools, magnifier, nudge, etc.)

- [x] **Undo/redo** (since 1.2.7): 20-action history; larger buttons 2.4.1 *Barkfart: every edit; 2-finger / 3-finger tap too.*
- [x] **Grouped undo** (since 4.2.3): Repeated flips/pushes undone as one step *Barkfart: one step per gesture.*
- [x] **Contextual tool menus** (since 2.0.0): Panels appear for the touched object *Barkfart: one context panel titled by the selection.*
- [ ] **Sliders to +/-** (since 2.0.3): **REPLACED** sliders with increment buttons
- [ ] **Hold to repeat** (since 2.2.1): Hold +/- for fast change
- [ ] **Sidebar quick-nav** (since 2.2.1): Buttons to jump in the sidebar
- [ ] **Quick Menu** (since 2.4.8): Top bar to jump between menus
- [ ] **Magnifier** (since 2.1.0): Zoomed inset window for selecting nodes
- [ ] **Magnifier zoom out / hide nodes** (since 3.0.4): Overview mode
- [ ] **Magnifier lock** (since 3.3.0): Lock magnifier in place
- [ ] **Arrows window** (since 2.2.1): Pixel-perfect nudge
- [ ] **Nudge amount** (since 3.0.4): Custom nudge size
- [ ] **Sub-pixel nudge** (since 4.1.0): Nudge less than a pixel
- [ ] **Color picker** (since 2.5.9): Eyedropper samples screen
- [ ] **Recent colors** (since 2.3.7): Recently used colors list
- [ ] **Picker refresh** (since 2.6.0): Manual screen refresh for the picker
- [ ] **Quick Resize Tools** (since 3.2.0): Floating buttons around selection
- [ ] **Quick tools copy/paste** (since 4.1.6): Copy/paste and draggable stretch in quick tools
- [ ] **Hide quick tools** (since 4.1.7): Tap the selected node to hide them
- [ ] **Docked Quick Tools** (since 4.2.4): **REPLACED** floating tools with docked columns
- [ ] **Docked tool gestures** (since 4.2.4): Drag, double-tap reset, tap to round
- [ ] **Menu position memory** (since 4.2.0): Menu holds position between selections
- (won't) **First-run tooltips** (since 2.0.3): Hints for new users *Barkfart: no nags by design; short inline hints instead.*
- (won't) **Onboarding** (since 4.3.2): Welcome screen and guiding tooltips *Barkfart: opens straight into a project by design.*
- [ ] **Lock icons** (since 4.3.0): Lock buttons show a lock icon
- [x] **Splash screen** (since ≤3.0.1): Community character art and links *Barkfart: launch screen with our icon.*
- (won't) **Splash ball minigame** (since 2.4.8): Hidden ball game on the splash screen *Barkfart: not planned.*
- (won't) **Easter egg** (since 2.4.4): Hidden extra *Barkfart: not planned.*

## Website/community features

- (won't) **Stickfigure library** (since 2014): Website catalogue of figures by category *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Animation examples** (since ≤1.4.4): Downloadable sample projects *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **User-Created Animations page** (since 2016): Auto-lists YouTube uploads *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Top Commentators** (since 2016): Comment ranking widget *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Accounts** (since 2017): Website profiles *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Friends and activity** (since 2017): Social feed with friends *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Groups** (since 2017): Public/private community groups *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Comment caption** (since 2017): Custom caption under comments *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Ratings** (since 2020): Star ratings for registered users *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Sort by likes** (since 2020): Library sorted by ratings *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Users' Choice and badges** (since 2020): Tiered awards and roles *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Daily Feature** (since ≤2020): Featured figure each day *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Animation Spotlights** (since ≤2020): Featured animations with category ratings *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Comment notifications** (since 2020): Email alerts for page authors *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Report button** (since 2020): Report bad or stolen figures *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Activity bot** (since 2020): Bot logs site events *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **GIF picker** (since 2023): GIF picker for comments *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Movieclip submissions** (since 4.0.0 (2023)): Upload movieclips from the app to the website library *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*
- (won't) **Contests and collabs** (since 2015): Official events and splash-screen contests *Barkfart: won't: Barkfart makes no network requests (NETWORK.md); files from the site import normally.*

## Removed / replaced features

- (removed in the original) **Filled Circle segment**: Removed 1.8.0; replaced by circle + gradient
- (removed in the original) **Double-tap to reset zoom**: Replaced in 1.7.0 by double-tap camera mode
- (removed in the original) **Play/Stop in Animation Tools**: Removed 1.6.3; buttons kept by thumbnails
- (removed in the original) **Sliders**: Replaced by +/- 2.0.3
- (removed in the original) **Android download auto-scan**: Removed 3.2.0
- (removed in the original) **Old MP4 export method**: Removed 4.0.3
- (removed in the original) **Per-frame stickfigure limit**: Removed 3.0.0 (node limit only)
- (removed in the original) **Floating Quick Resize Tools**: Replaced by Docked Quick Tools in 4.2.4; legacy kept as a toggle
- (removed in the original) **Shared Free/Pro storage (Android)**: Split into separate folders 3.3.0
- (removed in the original) **Per-frame watermark**: Became project-wide 2.4.4
