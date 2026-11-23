# .bark format rules

The .bark file is a ZIP holding project.json and one STORED entry per asset
(assets/<id>.<ext>; images and audio are already compressed). Loading from a
file keeps assets on disk until needed; saving copies them across without
re-encoding, so autosave stays cheap. `formatVersion` is 1. These rules keep every saved file loadable:

1. **Locked fixture.** core/src/test/resources/formats/v1-kitchen-sink.bark
   was written by format version 1 (2026-11-17). Every build must load it and
   render it like v1-kitchen-sink.png (FormatLockTest). It is never
   regenerated; a later version adds its own fixture next to it.
2. **New fields are optional with a default** equal to the model's default,
   so files without them load unchanged. Only identity and reference fields
   are required: ids, type names, rootSymbol, link sources.
3. **Changing meaning or removing a field needs a version bump** and a step
   in Migrations (version N to N + 1), with a test that upgrades a fixture.
4. **Unknown fields are ignored** within a version (tested).
5. A file with a newer formatVersion than the app is refused with a message,
   never half-loaded.

## Optional fields added within version 1

- **Figure `base`** (2026-11-23, D7b): a list with one object per segment:
  `id`, plus the segment's look as the figure came into the project
  (`shape`, `length`, `thickness`, `scale`, `color`, `ownColor`, `curve`,
  `static`, `stretchy`, `smartStretch`, `gradient`, `gradientColor`,
  `gradientReverse`, `gradientSideways`, `outline`, `outlineColor`, `hollow`,
  `halfArc`, `sides`, `trapezoidRatio`, `roundStart`, `roundEnd`,
  `rightTriangle`, `upsideDown`). "Reset this segment" restores these. A figure
  without `base` (every file from before) has nothing to reset to, which is
  the default.

- **Layer `joins`** (2026-11-24, D32): a list of switches, each `time`,
  `parent` (null: lets go), `node`, `rotate` and `before` (x, y, rotation,
  scale x, scale y at the switch in the frame before it). Files without it
  keep their one join (`parent`) for the whole animation, the default.
