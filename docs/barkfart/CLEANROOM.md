# Clean-room rules

Barkfart Edition is an original app in the same genre as Stick Nodes. It is not
a fork. The original app bundle in `main/` (decrypted App Store IPA, v4.3.4) is
reference material kept outside version control.

Allowed
- Using the original app on a device and recording how it behaves (screens,
  gestures, workflow). Re-implementing that behaviour in our own code.
- Reading the original's *data files* (.nodes, .nodemc, .stknds) to learn the
  file formats, so users can import their own work (interoperability).
- Looking at the bundle's UI atlases and UI string files to inventory tools,
  menus and pain points (documented in UI.md). We redraw every icon and write
  every string ourselves; nothing is traced, copied or paraphrased line by line.
- Using the bundled sample files as local-only test fixtures, read from
  `main/` at test time and skipped when absent. Never copied into the repo.

- Comparing our renders with the preview images stored inside the bundled
  movieclips (renders made by the original app). Local only: neither the
  previews nor renders of bundled content are committed.
- Public, openly licensed documentation of the formats, such as the MIT
  sticknodes-rs spec, credited in NOTES.md.

Not allowed
- Disassembling or decompiling the `IOSLauncher` binary, or reading its code,
  strings or symbol names to copy implementation.
- Committing or shipping any original asset: sprites, skins (`darkskin2`),
  node styles, fonts, shaders, i18n text, icons, splash, bundled stickfigures,
  movieclips or sample projects.
- The Stick Nodes name, logo or look-alike branding.

Golden references for import tests come from the original app's own preview
images inside its movieclip files (local only), screen captures made by the
user, and our own renders. Never from running the original's code.

## Decision 2026-11-19: stock figures
The user decided that the original's bundled stock stickfigures and effect
movieclips count as stock material and may ship with Barkfart, so the figure
picker has the figures people expect. They live in app/assets/stock
(figures: generic stickfigures, props, backgrounds; effects: fire, rain,
sparks ...). Figures of third-party characters (Naruto, Goku, Vegeta,
Sasuke, Pikachu, Spider-Man, Iron Man, Batman, Creeper) and the "SSJ"
auras stay out for trademark reasons; users can import them from their own
device. Everything else above still holds (no binary, skins, icons, fonts,
strings or branding).
