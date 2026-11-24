# Mockup generators

These scripts draw the redesign boards (REDESIGN.md) as static HTML.

- `node boards.mjs` writes every editor, sheet and screen board.
- `node figure-editor.mjs` writes the figure editor board.
- `node graph-editor.mjs` writes the Curves (graph) boards, built on the
  timeline and effects boards (run `boards.mjs` first): `graph`,
  `graph-mods` (a value with a modifier) and `graph-mods-add`.
- `node script-layer.mjs` writes the Script layer board (`script`), built on
  the timeline board.
- `node workshop.mjs` writes the Workshop board (`workshop`, iPad landscape,
  1180 x 820), built on the Settings board's page.

Both write `project/*.dc.html` (for the claude.ai design canvas) and
`preview/*.html` (to open in a browser). They need no packages.

Pages are lists of row descriptors rendered by one function (`rowsHtml`,
`page`, `PAGES`), the same idea the app should follow (REDESIGN.md D15).

To look at a board at 2x, use any Chromium:

```
chromium --headless --window-size=1344,600 --force-device-scale-factor=2 --screenshot=out.png preview/timeline.html
```

`crop.py` splits a screenshot into quarters for close checks.
