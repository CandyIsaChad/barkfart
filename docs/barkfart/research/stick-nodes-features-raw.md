# Stick Nodes / Stick Nodes Pro: raw feature inventory (parity checklist)

Compiled 2026-11-17 from the developer's public posts and store listings. Each description is paraphrased. Nothing is quoted beyond a few words.

## Sources read

- sticknodes.com blog index, every page from https://sticknodes.com/page/1/ to https://sticknodes.com/page/41/ (121 unique posts; every post was downloaded).
- **67 update-titled posts** were read in full: **64 genuine update/changelog posts**, 2 April Fools jokes (see "Not real features") and 1 pre-announcement with no changelog ("New Update is Coming"). Also read: the "Coming soon" posts (1.7.0, 2.0.0), the news posts (July, August and September 2015, "What's Up", "Random Site News", "Latest News"), the birthday and anniversary posts (6th, 7th, 8th, 11th, 12th, and the 2023 roundup), "Announcing Stick Nodes Desktop", "The StickNodes.com Redesign", "Stick Nodes Community accounts", and the beta and release posts.
- Update posts, oldest to newest: https://sticknodes.com/new-update-1-0-8/ through https://sticknodes.com/new-version-4-3-2-official-collab-the-next-stick-idol/. The full slug list is in the blog index above.
- https://sticknodes.com/faqs/
- https://sticknodes.com/tutorials/ and https://sticknodes.com/stickfigures/ (the "how to import" dropdown)
- The developer's personal blog, which has the pre-release posts that describe the 1.0 feature set:
  - https://fortheloss.org/new-app-under-development/
  - https://fortheloss.org/animation-app-update-2/
  - https://fortheloss.org/stick-nodes-update-3/ through https://fortheloss.org/stick-nodes-update-6/
  - https://fortheloss.org/stick-nodes-beta/
  - https://fortheloss.org/stick-nodes-released/
  - https://fortheloss.org/stick-nodes-one-month-later/
  - https://fortheloss.org/stick-nodes-released-on-ios-devices/
  - https://fortheloss.org/stick-nodes-on-ios-one-month-later/
- Google Play listings: https://play.google.com/store/apps/details?id=org.fortheloss.sticknodes and https://play.google.com/store/apps/details?id=org.fortheloss.sticknodespro
- App Store listings: https://apps.apple.com/us/app/id932127902 (Free) and https://apps.apple.com/us/app/id932128168 (Pro). The listing shows 4.3.4 as the current version.

**Version range:** The app launched 19 Aug 2014 (Android). The posts never state a version number for the launch build. The earliest numbered changelog is **1.0.8**. The newest version found is **4.3.4** (4 Sep 2026), confirmed on the App Store listing.

**Not accessible or not found:**
- No Stick Nodes wiki was found. A guessed Fandom URL returned 403, its API returned 404, and a web search turned up only unrelated Stick War wikis.
- The Wayback Machine lookup timed out.
- No changelog was found for 1.0.0–1.0.7, 1.1.x feature details (only crash fixes), 1.4.1–1.4.4 (a lag fix and the iOS catch-up are mentioned, not itemised), 2.2.4 (one line only), 2.6.3 (joke changelog), 3.0.3 (skipped), 3.1.6, 3.1.14, 4.0.4, and 4.1.4/4.1.5 (partial). Post bodies have embedded videos that were not watched, so features shown only in video are not captured.

**Notation:**
- **Since** is the earliest version on any platform (Android usually came first). "1.0" means the feature was in the launch build, according to the pre-release posts. "≤X" means the feature is first mentioned in that version but already existed. "?" means unknown. The launch (1.0) set is partly inferred from the pre-release dev-blog posts; "1.0 (inferred)" marks rows not stated explicitly there. Years in the Website section are approximate post dates, not app versions.
- **Pro?** "Pro" means Pro-only as stated by the developer. Blank means Free and Pro alike.
- Free vs Pro at launch was the same feature set. Free had a startup ad and a watermark on exports.

---

## Canvas & posing

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Drag nodes to pose | Touch-drag a node to rotate its segment around its parent; the main node moves the whole figure | 1.0 | |
| Static vs dynamic nodes | Static nodes are fixed detail nodes that cannot be dragged while animating | ≤1.2.5 | |
| Stretchy nodes | Nodes marked stretchy can change their segment length while animating | 1.0 | |
| Per-figure scale | Scale a whole stickfigure up or down | 1.0 | |
| Per-segment scale | Scale an individual segment/limb independently | 1.0 | |
| Per-figure and per-segment color | Recolor whole figures or single segments while animating | 1.0 | |
| Pinch zoom on stage | Pinch to zoom the stage view | 1.0 | |
| Zoom out below 100% and move stage | Zoom out to 50% and drag the stage to place figures off-frame | 1.2.6 | |
| Max zoom increases | Max zoom went 500% → 800% (1.7.0) → 2000% (2.3.2) → 5000% (3.2.0) | 1.7.0 | |
| Smoother zoom | Zoom made far more responsive | 2.4.4 | |
| Panning mode | Drag the canvas to move every figure on the current frame at once | 1.1.3 | |
| Lock stickfigure | Lock button freezes a figure in place to prevent accidental moves | 1.2.6 | |
| Locked figures hide nodes | A locked figure hides all its nodes except the main node (later: regardless of joins) | 2.5.1 | |
| Pinch-rotate figure | Two-finger pinch-rotate rotates the selected stickfigure | 1.2.8 | |
| Rotation knobs | Knob controls for angles that preview rotation in real time | ≤2.1.0 (improved) | |
| Tap knob to snap | Tapping the rotate knob snaps to 0/90/180/270 | 3.2.2 | |
| 45° snap on angle fields | Quick button on every angle field to snap to 45° steps | 4.1.6 | |
| Figure rotation indicator | Main node shows a line for the figure's rotation; rotation editable numerically | 4.2.0 | |
| Segment angle +/- | Set segment angle with increment buttons | 2.2.2 | |
| Axis lock while dragging | Dragging a figure, textfield or camera in a straight line locks to that axis (toggle in settings) | 2.2.0 | |
| Flip figure (X/Y) | Flip a stickfigure horizontally/vertically | ≤1.7.8 | |
| Flip segments in Animation Mode | Flip individual segments while animating, not just in creation | 2.3.7 | |
| Join-anchor flip | Long-press flip to mirror joined figures around the join anchor axis | 4.2.3 | |
| Individual node lock | Pin a single node so it stays in world position while other nodes move (e.g. planted foot) | 2.4.8 | |
| Angle Lock (node) | Keep a node at a fixed angle, absolute or relative to the nearest non-static parent | 4.2.0 | |
| Drag Lock (node) | Constrain a node's drag to a specific axis | 4.2.0 | |
| Floaty stretchy nodes | Alternate tween behaviour for stretchy nodes | 4.2.0 | |
| Reset length | Button that restores a segment's original length (applies smart-stretch) | ≤2.4.9 | |
| Push back / bring forward | Change a figure's layer order in the frame | ≤4.0.0 | |
| Push moves joined figures | Layer push/pull also moves all joined figures | 4.0.0 | |
| Precision mode | Choose exact (stylus) or approximate (finger) touch precision | 1.4.0 | |
| Node-digging | Repeated taps in a node cluster cycle through every node there | 4.2.0 | |
| Selected node sticks | A just-selected node stays selected on the next touch so it can be dragged | 4.2.0 | |
| Dig to joined main node | Tapping a join-anchor again quickly selects the joined figure's main node | 4.3.3 | |
| Forgiving taps | More tolerant tap hit-testing (esp. sprites, phones) | 4.0.2 | |
| Tap off-stage to deselect | Tapping dead area beside the stage counts as tapping nothing | 4.0.3 | |
| Cancel accidental grab on zoom | If a pinch-zoom starts on a node/sprite, the accidental move is undone | 4.0.3 | |
| Keep selection while dragging stage | Dragging the stage no longer clears selection | 4.0.3 | |
| Drag off stage stays responsive | Dragging continues to track when the finger leaves the stage | 4.1.6 | |
| Selection kept across pan/camera mode | Entering/leaving pan or camera mode keeps the selected node | 4.3.3 | |
| Max figure scale | Max scale raised from 10x to 20x | 4.1.0 | |

## Stickfigure creation (segments, shapes, nodes, polyfills, connectors, smart stretch, locks)

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Creation Mode | Separate editor for building stickfigures from segments | 1.0 | |
| Add segment / limb | Draw a new segment from a selected node | 1.0 | |
| Duplicate / copy-paste segment | Copy a segment (and branch) and paste it elsewhere | ≤1.7.7 | |
| Draw order control | Reorder segments' depth within the figure | 1.0 | |
| Line segment | Basic straight segment | 1.0 | |
| Circle segment | Circle-shaped segment (outline; fill via gradient from 1.8.0) | ≤1.7.6 | |
| Rounded segment | Segment with rounded caps | ≤3.0.2 | |
| Triangle segment | Triangle shape (exact launch list of segment types unknown) | ≤3.0.0 | |
| Filled Circle segment | White-filled circle type; **REMOVED 1.8.0** (replaced by circle + gradient) | ≤1.7.x | |
| Draw tools always visible | Drawing tools permanently shown in Creation Mode | 1.2.5 | |
| Split segment | Split a segment into two | 1.2.5 | |
| Split into N | Choose how many pieces a segment splits into | 4.1.6 | |
| Split along curve | Splitting a curved segment follows the curve | 3.2.2 | |
| Show/hide nodes toggle | Hide nodes in Creation Mode to view the design | 1.2.5 | |
| Outline selected segment | Selected segment drawn with a highlight outline (red) | 1.2.5 | |
| Inherit node properties | New nodes copy parent's static/stretchy setting; sensible default draw order | 1.2.5 | |
| Segment flip X/Y | Flip individual segments in creation | 1.2.6 | |
| Creation background color/image | Set a background color or image in Creation Mode for tracing | 1.7.0 | |
| Background in front | Show the background on top of the figure in Creation Mode | 2.2.1 | |
| Load figure as guide | Load another stickfigure as a background reference in Creation Mode | 2.2.2 | |
| Trace frame | Use a project frame as a reference background in Creation/MC editing | 3.0.0 | |
| Trace options | Show/hide nodes when tracing; scrub the traced frame quickly | 4.2.4 | |
| Segment color gradients | Two-color gradient along a segment | 1.7.6 | |
| Radial circle gradients | Circle gradients radiate from center; separate circle outline color | 2.5.6 | |
| Sideways gradients | Gradient direction toggle "sideways" for segments and trapezoids; triangles in 4.3.0 | 4.1.0 | |
| Circle gradient horizontal/vertical | Linear gradients on circles | 4.2.4 | |
| Polygon radial gradient | Radial gradient on polygons | 4.2.6 | |
| Reverse gradient | Swap gradient direction | ≤4.2.2 | |
| Shapes: ellipse, trapezoid, right-triangle, polygon | New shape types with Shape Tools menu | 2.5.6 | |
| Shape Tools properties | Per-shape properties: half arc, vertex count (polygon), trapezoid top ratio | 2.5.6 | |
| Polygon min 4 sides | Polygons from 4 to 16 vertices (was 5) | 3.0.5 | |
| Triangle upside-down | Upside-down option for triangles | 3.0.0 | |
| Half-circle reversible | Flip half-arc circles | 4.2.4 | |
| Circle hollow | Circle without interior fill | 4.1.0 | |
| Curved segments | Segments and rounded segments can bend with a curve radius | 3.2.0 | |
| Curve "circular" toggle | Alternative circular curve behaviour | 4.1.0 | |
| Trapezoid curves and rounded ends | Trapezoids can curve, have rounded tips, easier thickness | 4.2.0 | |
| 0-length segments visible | Zero-length segments render | 3.2.1 | |
| Polyfills | Fill shapes defined by an ordered set of nodes (polynodes) | 2.3.0 | |
| Finish polyfill | Button to complete a polyfill definition | ≤2.4.9 | |
| Polyanchor outline | Selecting a polyanchor highlights its polyfill | 2.3.2 | |
| Copy/paste polyfills | Polyfills copy with segments when all polynodes are included | 3.1.0 | |
| Polyfills follow curves | Polyfill edges follow curved segments; curve precision setting | 4.1.0 | |
| Polyfill Precise mode | Easier node picking, view/adjust polynode order | 4.2.4 | |
| Smart-Stretch | Stretching a node proportionally squashes/stretches its static child detail nodes | 2.4.8 | |
| Smart-stretch changes | Children need not be stretchy; ignores non-stretchy children | 2.5.1–2.5.2 | |
| Do not smart-stretch | Exclude a node/branch from smart-stretch | 2.5.2 | |
| Test smart-stretch | Preview smart-stretch in creation; option to keep the test changes | ≤2.4.9 / keep 3.0.0 | |
| Smart-stretch advanced options | Extra options enabling IK-style rigs with connectors | 4.2.0 | |
| Connectors | Segment that always stays attached between two nodes, with draggable midpoint | 4.2.0 | |
| Connector direction indicators | Visual cue for connector direction | 4.2.4 | |
| Modify Branch | Mass-edit a node and all children: static, thickness, length, colors (replace), draw order | 2.3.7 | |
| Modify Branch: limb type | Change segment type for a whole branch | 2.4.2 | |
| Modify Branch: segment length | Length field in Modify Branch | 3.1.0 | |
| Segment length field | Length visible/editable in Segment Tools | 3.0.5 | |
| Paste segment preserves angle | Pasted segments keep direction; toggle to preserve visual angle | 2.5.6 / toggle 2.5.7 | |
| Copy/paste keeps draw order | Pasted segments remember draw order | 2.3.0 | |
| Push branch back/forward | Move a whole branch's draw order at once | 4.1.6 | |
| Copy main node = copy figure | Copy button on main node copies whole stickfigure | 4.2.7 | |
| Toggle shape type (quick tools) | Quick-switch segment shape in Creation | 4.3.0 | |
| Node copy/paste/delete quick buttons | In Creation Quick Resize Tools | 3.2.2 | |
| Show only non-static nodes / show draw order | Creation view toggles (moved to View Options in 4.0.0) | ≤4.0.0 | |
| Stickfigure auto-save | Creation auto-saves every 5 min, with history browser (disable in settings) | 3.3.0 | |
| Default IK figure | Ships a sample IK-rigged default stickfigure | 4.2.0 | |
| Node limit per figure | Limits rose over time: 384 (2.3.6), 400 (4.0.2) | 2.3.6 | |
| Name/ID on selection | Optional brief display of figure name/ID | 4.0.0 | |

## Frames & timeline & tweening

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Frame list / thumbnails | Horizontal strip of frame thumbnails with scrollbar | 1.0 | |
| Add/copy/paste/delete frames | Basic frame management (paste first mentioned 1.2.7) | 1.0 (inferred) | |
| Add/delete frame buttons by timeline | Quick buttons right of the timeline | 1.2.9 | |
| Play / Stop buttons | Playback controls next to thumbnails (moved out of Animation Tools in 1.6.3) | 1.0 (inferred) | |
| FPS setting | Frames per second; limit raised from 20 to 60 | 1.0 / 60 in 1.7.0 | |
| Tweening toggle | Auto-insert interpolated frames between keyframes | 1.0 | |
| Tweened-frame count | Choose 1–8 in-between frames (previously fixed at 2) | 1.7.6 | |
| Tween 0 frames | Tweening on with 0 in-betweens, enabling per-frame slow motion only | 3.3.1 | |
| Is loop | Tween last frame back to first for seamless loops | 1.2.7 | |
| Keep during tween | Per-figure flag to stop a figure flickering/vanishing during tweening | 1.8.0 | |
| Tween colors | Per-figure checkbox to interpolate colors | 2.3.0 | |
| Tween Mode per figure | Linear, ease-in, ease-out or both per figure per frame | 4.2.4 | |
| Frame delay | Hold a frame for extra time | 2.0.0 | |
| Move camera during delay | Camera keeps tweening during a frame delay | 3.1.0 | |
| Slow motion frame | Per-frame checkbox that greatly increases tweening for that frame | 3.1.0 | |
| Add tweened frame | Generate a new in-between keyframe from neighbours | 2.3.0 | |
| Add tweened frame percentages | 10/25/50/75/90%, 50, 33/67 combo, 10/90 easing combo | 2.3.1–2.3.2 | |
| Add many tweened frames | Long-press to insert any number of tweened frames | 3.1.0 | |
| Copy multiple frames | Thumbnail menu to copy several frames (up to 25 in 3.0.0) | 2.2.2 | |
| Clear frame | Delete everything in a frame | 2.2.0 | |
| Frame repeat | Repeat a series of frames N times | 3.3.0 | |
| New-frame indicator | Blue flash/indicator on newly added frames | 2.3.2 / 3.1.0 | |
| Selected-frame direction indicators | Timeline arrows show where the current frame is; delete no longer jumps | 3.1.9 | |
| Frame number backgrounds | Background behind frame numbers for readability | 2.4.0 | |
| Custom playback range | Play button options to play only a range | 4.1.6 | |
| Frame skipping under lag | Playback drops frames to keep timing (sound sync) | 3.3.0 | |
| Preserve selection across frames | Keep the same figure/node selected while changing frames (setting) | 4.1.6 | |
| Frame node limit | Per-frame cap (40 figures/6400 nodes in 2.3.6; nodes only 6400 in 3.0.0; 12,800 in 4.0.0) | 2.3.6 | |
| Double frame node limit | Setting to double the per-frame node cap | 3.3.0 | |
| Unlimited nodes per frame | Setting to remove the per-frame cap | 4.0.0 | |
| Render every 3rd thumbnail | Lag option to draw fewer thumbnails | 3.0.0 | |

## Onion skin & view options

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Onion skin | Ghosts of neighbouring frames | ≤1.1.4 | |
| Onion skin in front | Show onion skin on top of figures | 1.6.2 | |
| Onion skin frame count | Past/future onion frame counts (0 allowed) | 3.0.0 | |
| Textfields in onion skin | Text appears in onion skin | 2.2.2 | |
| Camera in onion skin | Camera box shown in onion skin | 1.7.1 | |
| Visible in onion skin | Per-figure toggle to leave out of onion skin (e.g. backgrounds) | 4.0.2 | |
| View Options menu | Popup by play/stop gathering view settings | 3.0.0 | |
| Segment outline in animation mode | Outline segments while animating | 2.2.0 | |
| Guidelines | On-stage guide lines for positioning (saved with project from 2.5.1) | 2.2.0 | |
| Camera gridline patterns | Selectable grid patterns in the camera | 4.2.4 | |
| Show neighbor nodes | Highlight nodes directly connected to the selection | 3.1.0 | |
| Neighbor nodes across joins | Neighbor lines include joined figures' nodes (toggle) | 4.1.3 / toggle 4.1.4 | |
| Show only main nodes | Show only main nodes (join anchors included from 4.1.6) | ≤4.1.6 | |
| Show all nodes of active figure | Exception to main-nodes-only for the selected figure | 4.2.4 | |
| Hide locked nodes | Hide locked nodes to reduce clutter | 4.3.3 | |
| Active-filter notification dot | Dot warns when hide-locked or main-nodes-only is on | 4.3.3 | |
| Remove stage outline | Hide the default stage box outline | 4.0.3 | |
| Culling | Skip drawing nodes outside the view | 2.3.9 | |
| Node style | Selectable node/line rendering style | 4.1.0 | |
| Line/node thickness | Setting for UI node/line thickness | 3.1.0 | |
| Filter quality low/high | Toggle preview filter quality (later in View Options) | 2.4.4 | |
| Disable blur/glow temporarily | Turn off heavy filters for performance | 3.1.0 | |
| Stop resets view | Stop button resets zoom; double-tap jumps to camera view | 1.7.1 / 2.2.2 | |
| Double-tap to reset zoom | **REPLACED 1.7.0** by double-tap camera mode | 1.2.7 | |
| Larger stage | Stage area enlarged | 2.3.9 | |
| Fullscreen playback | Play animation fullscreen | 2.1.0 | Pro |

## Camera

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| V-cam / Camera Mode | Movable, zoomable camera defining output view per frame | 1.7.0 | |
| Mode buttons | Normal/Panning/Camera mode buttons top-left | 2.0.0 | |
| Camera rotation | Rotate the camera | 2.3.0 | |
| Camera wobble/shake | Automatic camera wobble with settings (improved 3.1.0; higher max 3.1.9) | 2.3.0 | |
| Widescreen bars | Letterbox effect | 2.2.0 | |
| Auto-Camera | Choose start/end frames; camera tweens smoothly between, adapting to inserted frames | 2.3.7 | |
| Auto-camera easing | Ease in/out plus bounce, swing, elastic | 2.5.6 | |
| Lock figure to camera | Figures follow the camera (HUD/background) | 2.3.7 | |
| Parallax | Parallax factor for camera-locked figures; applies to camera scale | 2.3.7 / 2.4.4 | |
| Copy/paste camera | Copy/paste camera lock bundles, wobble properties, frame camera (cross-project) | 3.1.0 / 4.0.0 | |
| Stay in camera mode on frame tap | Tapping frames stays in Camera Mode | 4.0.2 | |

## Movieclips

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Movieclips | Reusable animated objects with their own frames, placed like figures | 3.0.0 | |
| MC editing mode | Edit a movieclip's frames in place | 3.0.0 | |
| MC Library Tools | Add/edit movieclips in the library | 3.0.0 | |
| Once / Loop playback | Movieclip playback mode | 3.0.0 | |
| Tween protection | MC option affecting tween/delay behaviour | ≤3.1.5 | |
| Break apart MC | Convert a movieclip back to figures | ≤3.3.1 | |
| Convert figure to MC / double-tap edit | Settings for double-tap to convert to MC or edit MC | 4.0.0 | |
| MC background | Load a background in MC creation | 3.1.0 | |
| MC trace frame increment | Traced project frame advances as MC frames are added | 3.1.9 | |
| MC playback camera offset | Offset preview camera in MC editing | 3.1.7 | |
| Cache MC | Rasterize single-frame MCs for faster rendering | 3.1.9 | |
| Default movieclips | Bundled sample movieclips | 3.3.0 | |
| MC limits | Up to 256,000 nodes and 200 frames per MC | 4.0.0 / 4.0.2 | |
| .nodemc files | Movieclip file format, sendable | 3.0.0 / send 3.1.0 | |
| Scaled/opacity MCs | Movieclips support scale and opacity like figures | ≤3.0.2 | |
| Submission type prompt | Asks whether a stickfigure submission should be a movieclip instead | ≤4.1.5 | |

## Sprites & images

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Frame background color | Per-frame solid background color via color chooser | 1.0 | |
| Gradient frame background | Frame backgrounds can be two-color gradients | 2.0.0 | |
| Background images | Load an image from the device as the frame background | 1.4.0 | |
| Background image scaling | Scale background image independently in x and y, beyond stage bounds | 1.7.0 | |
| Higher-res background loading | Background images loaded at higher quality | 2.5.3 | |
| PNG transparency in backgrounds | Transparent PNGs respected (iOS fix) | ≤2.5.4 | |
| Sprites | Import images as movable, scalable, rotatable stage objects | 4.0.0 | |
| Create Sprite dialog | Define sprites from an image with selections; remove/cancel selections | 4.0.0 / 4.0.1 | |
| Remove background color | Tool in Create Sprite to knock out a background color | 4.1.0 | |
| Color picker in sprite import | Sample colors from the image during sprite import | ≤4.2.7 | |
| Minecraft skin import | Sprite importer turns a Minecraft skin into a fully rigged stickfigure | 4.0.0 | |
| Generated sprites hidden | Auto-generated sprites/figures hidden in library by default | 4.0.1 | |
| SpriteGroups | Named groups of sprites; next name auto-numbered | ≤4.1.6 | |
| Mass image import | Import many images as sprites at once | 4.2.7 | |
| GIF import | Animated GIF imported as a movieclip | 4.2.7 | |
| Sprite distort / skew | Toggle a distort mode to warp, skew and squash sprites | 4.3.0 | |
| Separate X/Y sprite scale | Independent axis scale controls for sprites | 4.3.0 | |
| Negative import scale | Internal scale at import can be negative (baked-in flip) | 4.3.0 | |

## Text

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Textfields | Add text objects to frames (speech, titles) | 1.5.0 | |
| Text tweening | Textfields interpolate between keyframes (new projects only) | 1.7.6 | |
| Textfield limit | Max per frame raised to 5 (1.7.6), then 10 (2.4.9) | 1.7.6 | |
| Text pans with figures | Panning mode also moves textfields | 2.2.2 | |
| Lock text to corner | Pin a textfield to a screen corner (name/watermark) | 2.2.0 | |
| Project-wide watermark | Corner text applies to all frames or none | 2.4.4 | |
| Text rotation | Rotate textfields | 2.3.7 | |
| Text transparency | Fade textfields | 2.3.7 | |
| Text shadow | Drop-shadow property for text | 2.4.2 | |
| Fonts | Choose among bundled fonts | 2.4.4 | |
| More fonts | Additional fonts (2.5.1), Impact/Comic Sans/Determination (3.1.0) | 2.5.1 | |
| Japanese font | Font that renders Japanese characters | 3.0.4 | |
| Load/unload fonts | Fonts must be loaded to use, unloaded to save memory | 4.0.0 | |
| Text scale up to 10 | Raised textfield scale limit | 2.5.9 | |
| Lock textfield | Lock a textfield against edits/moves | 3.1.0 | |
| Text lock to camera | Text follows the camera | 3.1.0 | |
| Draw above widescreen bars | Option to draw text over letterbox bars | ≤3.1.3 | |
| Paste external text | Paste button pastes clipboard text from other apps | 3.1.9 | |
| Copy/paste textfields across projects | Text clipboard persists through opening projects | 4.0.0 | |

## Sound

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Sounds on frames | Attach sound clips to frames; heard in playback and MP4 export | 1.6.0 | Pro |
| Bundled sound library | Ships ~40 sounds, regularly expanded (searchable by version tag, e.g. "330") | 1.6.0 | Pro |
| Custom MP3 import | Use own MP3s (max 256 KB) | 1.6.0 | Pro |
| iOS share-sheet import | Open an MP3 from another app into Stick Nodes Pro | 2.2.2 | Pro |
| stknds:// URL download | Prefix a web MP3 URL to download it into the app (iOS) | 2.2.2 | Pro |
| Sound preview | Preview sounds in the import list | 2.3.7 | Pro |
| Multi-select sound import | Import several sounds at once | 2.3.7 | Pro |
| Copy/paste sounds | Copy a frame's sounds and paste onto another | 2.3.0 | Pro |
| Sound pan | Left/right stereo panning per sound | 3.1.0 | Pro |
| Sound pitch | Pitch shift per sound | 3.1.0 | Pro |
| Pause with device music | Clips stay silent while the user plays music | 1.7.6 | Pro |
| Background music on iOS | Other apps' music can keep playing while animating | 2.2.4 | |
| Higher export audio quality | Improved audio encoding in exports | 3.1.0 | Pro |

## Filters/effects

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Transparency | Per-figure opacity | 2.2.0 | |
| Invert colors | Invert a figure's colors | 2.2.0 | Pro |
| Tint | Wash a figure in a color | 2.2.0 | Pro |
| Saturation | Desaturate or boost color | 2.2.0 | Pro |
| Blur | Horizontal, vertical or both | 2.2.0 | Pro |
| Glow | Glow around a figure | 2.3.0 | Pro |
| Glow intensity | Strength control for glow | 2.4.2 | Pro |
| Hue shift | Rotate a figure's hues | 3.0.4 | Pro |
| Drop shadow | Offset shadow, also usable as edge glow | 3.3.0 | Pro |
| Pixelate | Pixelation filter | 4.0.3 | Pro |
| Motion blur | Directional motion-blur toggle (incl. one direction) in blur | 4.0.3 | Pro |
| Outline | Outline filter around a figure | 4.1.6 | Pro |
| Blur/glow max 2.0 | Raised maximum strength | 3.0.5 | Pro |
| Low in-app quality | Filters preview at lower quality, full quality on export | 2.2.0 | |
| Copy/paste/clear filters | Filter clipboard and clear button | 2.3.0 | |
| Use parent's filters | Joined figures inherit their parent's filters | 3.1.0 | |
| Collapsible Filter Tools | Filter panel can be collapsed | 4.1.6 | |

## Export & sharing

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| GIF export | Animated GIF output (Free adds watermark) | 1.0 | |
| GIF >256-color handling | Better color quantization for image-heavy GIFs | 1.4.0 | |
| MP4 export | Video export | 1.2.3 | Pro |
| Fast hardware MP4 | Faster, higher-quality MP4 encoding | 1.7.0 | Pro |
| Old MP4 method button | Fallback encoder; **REMOVED 4.0.3** | 1.7.3 | Pro |
| 1080p MP4 | Max 1920×1080 output | 3.0.5 | Pro |
| PNG export | Single-image export | 2.5.1 | |
| 4K PNG | PNG at 4K | 3.3.0 | |
| Export frame range | Choose start/end frames (single frame allowed) | 2.3.0 | |
| Anti-aliasing | Smooth-lines export option (GIF unlocked at any size 2.4.1) | 2.3.6 | |
| Low-quality blur export | Export using preview-quality blur | 3.0.0 | |
| Better quality blur | Experimental higher-quality blur/glow export | 3.3.0 | |
| Vertical video | Portrait export toggle (Project Tools) | 4.0.3 | |
| Export size slider | Size slider on GIF and PNG export windows | 4.0.3 | |
| Non-blocking export | Exporting no longer freezes the device | 1.6.0 | |
| Exports to gallery | iOS camera roll; Android Pictures/Movies (3.3.0) | 1.2.3 | |
| Free watermark | Free version stamps exports | 1.0 | |
| Send files | Email/share projects, figures, sounds, MCs | 2.1.0 | |
| Auto-ZIP send | Sending >5 files bundles a ZIP | 4.0.0 | |

## Files, projects & import

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Save/open projects | Project files (.stknds) | 1.0 | |
| Save/import stickfigures | Figure files (.nodes) into a project library | 1.0 | |
| Pivot STK import | Import Pivot 2.x stickfigures | 1.0 | |
| Default stickfigures | Bundled starter figures (refreshed 2.0.0) | 1.0 | |
| Delete files | Permanently delete projects/figures/sounds in-app | 1.6.0 | |
| Alphabetical lists | Open/import lists sorted A–Z | 1.6.0 | |
| Natural sort | Numbers in names sorted numerically | 3.1.0 | |
| Sort new/old | Sort file lists by date | 2.3.0 | |
| List filter | Text filter on open/import lists | 2.0.0 | |
| Library dropdown filter | Filter in imported figure/MC/sound dropdowns | 3.3.2 | |
| Figure preview | Preview figures before importing; nodes on/off in preview | 2.3.7 / 4.1.2 | |
| Multi-select import | Import several figures at once | 2.3.7 | |
| Edit imported library | Rename, hide, delete imported figures (search added 4.1.6) | 3.0.0 | |
| Open with / file association | Open NODES/STK/STKNDS from other apps | 1.7.9 | |
| Android download auto-scan | Auto-collects downloaded files; **REMOVED 3.2.0** | 1.7.9 | |
| Open ZIP packs | Extract figure packs into the library | 2.0.1 | |
| Open mixed ZIPs | ZIP containing any Stick Nodes file types | 4.0.0 | |
| Partial project open | Open fewer frames, or a percentage from first/last frame | 2.2.2 / 2.3.6 | |
| Out-of-memory warning | Warns instead of crashing on oversized projects | 2.5.7 | |
| Save view state | Project saves zoom, position, current frame, guides | 2.5.1 | |
| Local project backup | Overwritten saves kept temporarily, restorable | 4.3.0 | |
| Library tools | Submit figures to and download figures from the website library in-app (submission form existed earlier) | 1.2.4 | |
| Pack submission | Multi-figure submit with auto-ZIP and file renaming | 2.3.7 / 2.5.9 | |
| Submission rules dialog | Guidelines shown before submitting | 2.5.9 | |
| Sample projects | Onboarding sample projects; refresh defaults in settings | 4.3.2 | |
| Uninstall warning | Warns that uninstalling deletes files | 4.1.4 | |

## Joining/layering/figure tools

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Joining | Attach a figure's main node to any node of another figure | 1.7.4 | |
| Join anchor list | Selecting an anchor lists figures joined to it | 3.0.0 | |
| Joined-figure navigation | Tap names to hop between joined figures; shows parent | 4.0.0 | |
| Joined scale option | Joined figures scale together | 2.4.4 | |
| Copy with joins | Long-press copy includes joined figures | 3.1.0 | |
| Paste with layering | Long-press paste tries to keep layer position | 3.1.0 | |
| Copy all figure properties | Copy scale, filters, position etc. onto another figure | 2.5.9 | |
| Properties include layer | Property copy includes layer position | 2.6.1 | |
| Edit figure shortcut | Edit button in Stickfigure Tools opens that figure | 4.1.6 | |
| Quick Tools figure lock | Lock toggle in quick tools | 4.3.1 | |

## Settings & app tools

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Languages | EN, ES, FR, PT, TR | 2.0.0 | |
| Filipino | Added language | 2.3.5 | |
| Japanese | Added language | 2.6.1 | |
| Russian | Added language | 3.1.0 | |
| Save reminder | Prompt after X minutes unsaved; shows time since save (4.2.4) | 2.1.0 | |
| Lag options | Performance options under undo/redo | 2.4.4 | |
| Memory display | Memory use shown on open, in settings, near add-frame | 2.3.6 | |
| Memory Use dialog | Breakdown dialog from the memory indicator | 4.0.0 | |
| Lag Problems button | Help button next to memory indicator | 4.3.2 | |
| Custom skins | User re-skinning via /skin/ folder | 2.4.4 | |
| Dark Mode skin | Built-in dark theme | 3.1.0 | |
| Left-hand mode | Mirrors UI layout | 2.6.1 | |
| Zoom buttons | Corner zoom buttons | 3.1.0 | |
| Quick Menu toggle | Enable/disable quick menu | 2.4.8 | |
| +/- buttons toggle | Show increment buttons (hidden by default) | 4.0.0 | |
| Screen padding | UI inset for notches | 4.2.0 | |
| Haptics | Vibration feedback | 4.2.4 | |
| Nerd Mode | Less descriptive UI text | 4.1.6 | |
| Keep App Alive | Android notification to avoid being killed | 3.2.0 / toggle 4.2.0 | |
| Landscape flip | Use either landscape orientation | 2.0.0 | |
| Non-continuous rendering | Battery-saving redraw | 1.2.0 | |
| Chromebook support | Saving/exporting on Chromebooks | 3.1.6 | |

## UI/quality-of-life (undo, quick tools, magnifier, nudge, etc.)

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Undo/redo | 20-action history; larger buttons 2.4.1 | 1.2.7 | |
| Grouped undo | Repeated flips/pushes undone as one step | 4.2.3 | |
| Contextual tool menus | Panels appear for the touched object | 2.0.0 | |
| Sliders to +/- | **REPLACED** sliders with increment buttons | 2.0.3 | |
| Hold to repeat | Hold +/- for fast change | 2.2.1 | |
| Sidebar quick-nav | Buttons to jump in the sidebar | 2.2.1 | |
| Quick Menu | Top bar to jump between menus | 2.4.8 | |
| Magnifier | Zoomed inset window for selecting nodes | 2.1.0 | |
| Magnifier zoom out / hide nodes | Overview mode | 3.0.4 | |
| Magnifier lock | Lock magnifier in place | 3.3.0 | |
| Arrows window | Pixel-perfect nudge | 2.2.1 | |
| Nudge amount | Custom nudge size | 3.0.4 | |
| Sub-pixel nudge | Nudge less than a pixel | 4.1.0 | |
| Color picker | Eyedropper samples screen | 2.5.9 | |
| Recent colors | Recently used colors list | 2.3.7 | |
| Picker refresh | Manual screen refresh for the picker | 2.6.0 | |
| Quick Resize Tools | Floating buttons around selection | 3.2.0 | |
| Quick tools copy/paste | Copy/paste and draggable stretch in quick tools | 4.1.6 | |
| Hide quick tools | Tap the selected node to hide them | 4.1.7 | |
| Docked Quick Tools | **REPLACED** floating tools with docked columns | 4.2.4 | |
| Docked tool gestures | Drag, double-tap reset, tap to round | 4.2.4 | |
| Menu position memory | Menu holds position between selections | 4.2.0 | |
| First-run tooltips | Hints for new users | 2.0.3 | |
| Onboarding | Welcome screen and guiding tooltips | 4.3.2 | |
| Lock icons | Lock buttons show a lock icon | 4.3.0 | |
| Splash screen | Community character art and links | ≤3.0.1 | |
| Splash ball minigame | Hidden ball game on the splash screen | 2.4.8 | Pro |
| Easter egg | Hidden extra | 2.4.4 | |

## Website/community features

| Feature | What it does | Since | Pro? |
|---|---|---|---|
| Stickfigure library | Website catalogue of figures by category | 2014 | |
| Animation examples | Downloadable sample projects | ≤1.4.4 | |
| User-Created Animations page | Auto-lists YouTube uploads | 2016 | |
| Top Commentators | Comment ranking widget | 2016 | |
| Accounts | Website profiles | 2017 | |
| Friends and activity | Social feed with friends | 2017 | |
| Groups | Public/private community groups | 2017 | |
| Comment caption | Custom caption under comments | 2017 | |
| Ratings | Star ratings for registered users | 2020 | |
| Sort by likes | Library sorted by ratings | 2020 | |
| Users' Choice and badges | Tiered awards and roles | 2020 | |
| Daily Feature | Featured figure each day | ≤2020 | |
| Animation Spotlights | Featured animations with category ratings | ≤2020 | |
| Comment notifications | Email alerts for page authors | 2020 | |
| Report button | Report bad or stolen figures | 2020 | |
| Activity bot | Bot logs site events | 2020 | |
| GIF picker | GIF picker for comments | 2023 | |
| Movieclip submissions | Upload movieclips from the app to the website library | 4.0.0 (2023) | |
| Contests and collabs | Official events and splash-screen contests | 2015 | |

---

## Removed / replaced features

| Feature | Fate |
|---|---|
| Filled Circle segment | Removed 1.8.0; replaced by circle + gradient |
| Double-tap to reset zoom | Replaced in 1.7.0 by double-tap camera mode |
| Play/Stop in Animation Tools | Removed 1.6.3; buttons kept by thumbnails |
| Sliders | Replaced by +/- 2.0.3 |
| Android download auto-scan | Removed 3.2.0 |
| Old MP4 export method | Removed 4.0.3 |
| Per-frame stickfigure limit | Removed 3.0.0 (node limit only) |
| Floating Quick Resize Tools | Replaced by Docked Quick Tools in 4.2.4; legacy kept as a toggle |
| Shared Free/Pro storage (Android) | Split into separate folders 3.3.0 |
| Per-frame watermark | Became project-wide 2.4.4 |

## Not real features

- April Fools posts, not features: the paid node-limit "DLC" (2.6.1) and the "ArcNodes" takeover (3.6.9).
- Stick Nodes Desktop (announced 2015, timeline layers, dynamic onion skin) never shipped.
