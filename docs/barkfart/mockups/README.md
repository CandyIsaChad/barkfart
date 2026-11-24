# Mockup generators

These scripts draw the redesign boards (REDESIGN.md) as static HTML.

- `node boards.mjs` writes every editor, sheet and screen board.
- `node figure-editor.mjs` writes the figure editor board.
- `node graph-editor.mjs` writes the Curves (graph) board, built on the
  timeline board (run `boards.mjs` first).

Both write `project/*.dc.html` (for the claude.ai design canvas) and
`preview/*.html` (to open in a browser). They need no packages.

Pages are lists of row descriptors rendered by one function (`rowsHtml`,
`page`, `PAGES`), the same idea the app should follow (REDESIGN.md D15).

To look at a board at 2x, use any Chromium:

```
chromium --headless --window-size=1344,600 --force-device-scale-factor=2 --screenshot=out.png preview/timeline.html
```

`crop.py` splits a screenshot into quarters for close checks.
