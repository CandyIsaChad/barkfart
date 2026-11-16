# Barkfart design (M2)

Status: accepted 2026-11-16. Changes go through PROGRESS.md decisions.

## 1. Goals and shape

- One data model: symbols with layers, and sparse tracks on every animatable
  value. The frame strip, dope sheet and layer panel are views of it.
- `evaluate(project, symbol, t)` is a pure function of the model and time
  in seconds. Frame rate belongs to playback and export.
- Core is plain Java 8 API (MobiVM-safe, see IosApiPolicyTest), with no
  LibGDX graphics calls in model or evaluator code, so it is unit-testable on a
  JVM and fuzzable.

Packages in `core`:

| Package | Contents |
|---|---|
| `app.barkfart.model` | Project, Symbol, Layer (+ types), Figure, Node, Property, Track, Key, Modifier, Effect, NodeSet |
| `app.barkfart.eval` | Evaluator, PropertyResolver, Scene (render tree), Diagnostics |
| `app.barkfart.geom` | Tessellation of the scene into a DrawList (triangles with colour) |
| `app.barkfart.io` | Native format (.bark), JSON schema, migrations |
| `app.barkfart.legacy` | Importers/exporter for .nodes, .stknds, .nodemc |
| `app.barkfart.edit` | Document, commands, undo/redo |
| `app.barkfart.render` | LibGDX renderer: fills the DrawList, runs effects (GL only) |

## 2. Schema

All ids are stable strings (short random base-36), unique per project.

```
Project
  formatVersion: int            (1 at first release; see §6)
  settings: { fps: float (default 30), width, height, background: Color }
  library: Map<id, Symbol>
  rootSymbol: id
  assets: Map<id, Asset>        (image | audio | font), stored in the container
  nodeSets: Map<id, NodeSet>
  presets: list of embedded preset bundles (optional)

Symbol
  id, name
  duration: double seconds      (playback end; keys may lie beyond it)
  layers: List<Layer>           (index 0 = bottom)
  markers: List<{time, name, color}>

Layer (common)
  id, name, type
  visible, locked, solo: bool   (not animatable; visibility over time is the
                                 `opacity` track or a hold-keyed `shown` bool)
  shown: Property<bool>         (hold-only; legacy "figure absent in frame")
  opacity: Property<float>      0..1
  blend: enum { NORMAL, ADD, MULTIPLY, SCREEN }
  parent: layer id | null       (transform inheritance only)
  transform: { x, y, rotation, scaleX, scaleY, anchorX, anchorY } as Properties
  effects: EffectStack

Layer types
  FIGURE      figure: Figure
  SPRITE      asset: id, width, height, tint: Property<Color>
  TEXT        text, font: asset id, size, color: Property<Color>
  SYMBOL      symbol: id, timeOffset: Property<double>, speed: Property<double>,
              loop: { ONCE, LOOP, PING_PONG }, recursionLimit: int (default 8)
  ADJUSTMENT  (no content; its effects apply to the composite of layers below)
  AUDIO       clips: List<Clip>
  CAMERA      x, y, zoom, rotation as Properties (transform unused)

Figure
  nodes: List<Node> in draw order; node 0 is the root (no shape)
Node
  id, parent: id | null, shape: { NONE, LINE, CIRCLE, ... }   (see §7 for legacy)
  rotation: Property<float>     degrees, relative to parent, unwrapped
  length: Property<float>
  thickness: Property<float>
  scale: Property<float>
  color: Property<Color>
  shown: Property<bool>
  static flags (not animated): lockLength, drawBehindParent, etc. as imported

Property<T>
  value: T                      (used when the track is empty)
  track: List<Key<T>> sorted by time, unique times
  modifiers: List<Modifier>
Key<T>
  time: double seconds, value: T
  interp: { HOLD, LINEAR, BEZIER } governs the segment to the NEXT key
  ease: (x1, y1, x2, y2) normalised cubic timing curve, used when BEZIER
        (CSS cubic-bezier semantics; presets are named values of this)

Clip (audio)
  asset, start (s), in, out (s within source), loop: bool,
  volume: Property<float>, pan: Property<float>, fadeIn, fadeOut (s)

NodeSet
  id, name, members: List<{ layer id, node id }>   (may span figures)

EffectStack
  graph: DAG { nodes: Map<id, EffectNode>, output: id }
  The UI only builds linear chains: input -> e1 -> e2 -> ... -> output.
EffectNode
  type (string id, e.g. "blur.gaussian"), enabled: bool, mix: Property<float>,
  params: Map<name, Property>, inputs: List<id>, mask: Scope | null
Scope
  { NODE(layer, node), NODE_SET(set), FIGURE(layer), LAYER(layer),
    SYMBOL_INSTANCE(layer), COMPOSITION(symbol) }
  Adjustment layers hold their stack directly. Node and node-set stacks live
  in `Symbol.scopedEffects: List<{scope, stack}>`.
```

Value types and interpolation:

| Type | LINEAR / BEZIER | Notes |
|---|---|---|
| float | numeric lerp | |
| angle (degrees) | numeric lerp of the stored unwrapped value | no shortest-arc; matches legacy files, which store unwrapped angles such as −363.24 (open question Q1) |
| double (time) | numeric lerp | |
| Color | per channel, straight sRGB, alpha included | Q3 |
| bool / enum | always HOLD | |

## 3. Modifiers

Applied in list order after key interpolation: `v = m_n(...m_1(keys(t)))`.

| Modifier | Parameters | Result |
|---|---|---|
| Wiggle | frequency (Hz), amplitude, seed, octaves | v + amplitude × smooth value noise(seed, t × frequency) |
| Sine | frequency, amplitude, phase | v + amplitude × sin(2π(f t + phase)) |
| Loop | mode CYCLE / PING_PONG, start, end (s) | re-maps t into [start, end] before key sampling; it must be first in the list (enforced) |
| DelayFollow | source property ref, delay (s), scale, offset | source(t − delay) × scale + offset, replacing v |
| Link | source property ref, scale, offset | source(t) × scale + offset, replacing v |

Delays are stored in seconds. The UI shows and edits them in frames at the
project fps.

## 4. Evaluation

`Scene evaluate(Project p, String symbolId, double t)`:

1. **Property resolution** goes through a `PropertyResolver` that memoises
   `(propertyRef, time)` for this call. Link and DelayFollow read other
   properties through it. A `visiting` set detects cycles. A property reached
   again while it is being resolved returns its keys-only value (modifiers
   skipped) and records a `Diagnostic(CYCLE, ref)`. So cycles can never
   recurse unboundedly.
2. **Layers** run bottom to top. If any layer in the symbol is soloed, only
   soloed layers (plus cameras and adjustment layers) contribute. Hidden layers
   and layers with `shown = false` are skipped. Parent transforms compose
   child-first up the chain. A parent cycle is cut at the first repeated layer,
   with a diagnostic.
3. **Figures** use forward kinematics from the root, iteratively in draw-list
   order: parents come before children by construction, and the importer and
   editor enforce it. World angle = parent world angle + rotation. Position =
   parent position + length × scale × (cos, sin) of world angle. Each shaped
   node emits a primitive (line capsule, circle, ...).
4. **Symbol instances**: local time = timeOffset(t) + (t − 0) × speed(t),
   then ONCE (clamp to [0, duration]), LOOP (mod) or PING_PONG. Nesting is
   processed with an **explicit work stack** of `(symbol, time, transform,
   depth, path)` frames, never Java recursion. An instance stops expanding
   when its target symbol already occurs on the nesting path more than
   `recursionLimit` times (default 8, so a self-containing symbol draws itself
   9 times). Two global guards stop runaway nesting: a hard depth cap of 64,
   and a budget of 250,000 scene objects (shapes plus groups). Hitting any
   guard truncates and records a diagnostic.
5. **Camera**: the topmost visible CAMERA layer of the evaluated symbol gives
   the view transform. Cameras inside nested symbols are ignored.
6. **Effects** are not executed by the evaluator. The scene is a tree of
   `Group { transform, opacity, blend, effects: resolved stack (param values
   at t), children }` and `Shape` leaves. Adjustment layers become groups whose
   input is "everything below in this symbol".
7. **Audio**: `evaluateAudio(p, symbol, t0, t1)` returns active clips with
   resolved volume and pan. Mixing happens in the platform layer.

The evaluator is pure. It allocates its result, never mutates the model, and
has no static state. Fuzz tests generate random projects, including
self-referencing symbols and link/delay cycles, and assert that it terminates
under the budgets without throwing.

## 5. Render pipeline

1. `geom` tessellates the Scene into a **DrawList**: per group, triangle
   meshes with per-vertex RGBA (capsules, circles with a segment count from
   radius × zoom, polygons by ear clipping). Core owns this geometry, so tests
   check exactly what ships.
2. The LibGDX renderer uploads the DrawList and draws it with one
   ShaderProgram. Groups without effects draw straight to the target.
3. Groups with an enabled effect stack render their children into a pooled
   FrameBuffer sized to the group bounds plus effect padding. They run the
   chain (ping-pong between two pooled buffers), mix with the input by `mix`,
   then composite with the group's blend and opacity.
4. Preview quality: effect buffers at 50% (configurable) in the editor, 100% on
   export. On weak GPUs (no float textures, or a slow benchmark at startup) the
   fallback disables blur radii above a cap and uses half-resolution buffers.
5. Caching: a group's output buffer is reused when its content hash (resolved
   values of everything inside, plus effect params) is unchanged since last
   frame. Static backgrounds therefore cost one blit.
6. Golden tests (M3+): a test-only rasterizer (Java2D, in test scope only)
   fills the same DrawList into a PNG. Until captures from the original app
   arrive these are **self-regression goldens**. Only synthetic fixtures (made
   by our own serializer) are committed. Renders of bundled legacy files are
   compared locally only.

## 6. Native file format

- Extension `.bark`. A ZIP container with:
  - `project.json`: the schema above, UTF-8, written with LibGDX `Json`
    (no new dependency)
  - `assets/<id>.<ext>`
  - `thumb.png`
- `formatVersion` is an int. Loading runs migrations `v → v+1` in sequence.
  Saving always writes the current version. A newer file than the app opens
  read-only with a warning.
- Saves are atomic: write `name.bark.tmp`, fsync, rename. One rolling backup,
  `name.bark.bak`.
- Data folder: the app's own Documents/Barkfart (iOS) or local storage
  (desktop/Android). The original app's folders are never written. Imports
  copy from them.
- Floats are written with full round-trip precision. Save, then load, then save
  must be byte-identical (tested).

## 7. Legacy import mapping

Formats are learned from data files only (NOTES.md). Every legacy parser must
consume each local reference file to exact EOF. The legacy `.nodes` serializer
must reproduce the input byte for byte (this is also the stickfigure export
path).

| Legacy | Barkfart |
|---|---|
| `.nodes` stickfigure | Figure. Node tree flattened depth-first into draw order. Fields: type → shape, relative angle (deg) → rotation, length, thickness, scale, colour. Unknown fields are kept in `legacyExtras` for round-trip export |
| `.stknds` project | Root Symbol. Project fps → settings.fps. Each figure that ever appears → one FIGURE layer. Frame *i* at time i / fps becomes a key on **every** node property of every present figure (a full pose snapshot), plus `shown` hold keys where figures appear or vanish |
| frame tweening | Keys get LINEAR when tweening is on for that span, otherwise HOLD (Q2) |
| camera per frame | CAMERA layer keys |
| frame sounds | AUDIO layer clips starting at the frame's time |
| figure filters | Effect stacks on FIGURE scope, where an equivalent effect exists; unmapped filters are listed in an import report |
| `.nodemc` movieclip | Symbol (same mapping as a project); instances become SYMBOL layers with timeOffset/speed keys |

Optional **key reduction** after import: for each track, drop a key when
interpolating its neighbours reproduces it within ε (default 0.01 deg /
0.01 units / 1/255 colour), and never drop the first or last key.

## 8. Editing, undo and caching

- All model mutation goes through `Document.apply(Command)`. Commands hold
  do/undo data. Gestures (a drag) open a `Transaction` that merges every
  change until finger-up into one undo step. Selection, viewport pan/zoom and
  playhead are view state, not undoable. Camera layer edits are undoable.
- History is capped by size (default 500 steps, oldest dropped).
- Each Property has a revision counter. `Document` keeps a project revision.
  Evaluator memoisation is per call. The renderer caches per-group content
  hashes (§5).
- Auto-key: when on, changing a property at the playhead writes (or
  replaces) one key on that property only. When off, a change edits `value`
  if the track is empty. If the track has keys, the change is refused with a
  hint to turn auto-key on. This matches mainstream animation tools.
- Frame-strip view: frames are `round(t × fps)` slots. "Posing a figure" in
  that view keys all of the figure's node properties at the frame time, in
  one undo step.

## 9. Open questions (default chosen, verify later)

| # | Question | Default |
|---|---|---|
| Q1 | Legacy angle tweening: numeric or shortest arc? | Numeric on stored unwrapped relative angles |
| Q2 | Legacy tween curve shape and per-frame easing options | Linear |
| Q3 | Colour interpolation space | Straight sRGB |
| Q4 | Legacy coordinate units and y direction | **Settled for movieclips (2026-11-16):** `length` is 1:1 in stage pixels, y up, stage 1920x1080 with centre (960,540). Evidence: our render of the Fire clip matches the original app's own preview frames at 0.94-0.95 silhouette IoU |
| Q5 | Legacy fields not yet understood (ints like 21/21, header block) | Preserved in `legacyExtras`; ignored by the evaluator |
| Q6 | Recursion and primitive budgets | depth 8 per instance, 64 global, 250k primitives |
| Q7 | Default fps for new projects | 30 |
| Q8 | Polyfills and textured segments in the legacy format | Import as POLYGON / SPRITE shapes once the format is decoded; until then, report them as skipped |

| Q9 | Symbol instance time with an animated speed: local = offset + t x speed(t) plays backwards when speed ramps down | Before M5: integrate speed over time (or offer a keyed time-remap track instead of speed) |

Q1 and Q2 are settled by the user's requested capture projects
(PROGRESS.md, "Waiting on the user").
