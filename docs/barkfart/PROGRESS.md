# Progress

## Decisions
- 2026-11-16: The repo had only a compiled App Store IPA, no source. User chose a
  clean-room original app instead of a fork. Rules in CLEANROOM.md.
- 2026-11-16: Stack: LibGDX + MobiVM (Java 17, Gradle), modules core / desktop /
  ios (Android later). Why: matches the original's feel and toolkit, runs the
  core and a desktop window on Linux for tests and screenshots, and builds an
  iOS IPA on a GitHub macOS runner.
- 2026-11-16: Milestones adjusted for clean room. M0 = scaffold, CI, format
  notes. M1 = branding + zero-network check (no trackers to remove). Golden
  tests compare imports against user screen captures + our own renders.
- 2026-11-16: Branding: display name "Barkfart", bundle id `app.barkfart.animator`, Java package `app.barkfart`. The "Stick Nodes" name is not used (clean room).
- 2026-11-16: Toolchain lives in the user's home: Temurin JDK 17.0.21 in ~/.local/jdks. Gradle 8.10.2 (CI pins the same version).
- 2026-11-16: UI direction (from the user): as close to the original's layout and
  workflow as possible so it feels familiar, while fixing what is bad. Inventory
  and critique in UI.md. Layout claims stay unverified until the user sends
  screenshots.
- 2026-11-16: The brief's rule "reuse existing UI widgets" cannot apply, since
  there is no original source. We build our own widgets on LibGDX scene2d.
- 2026-11-16: IPA is built unsigned on GitHub Actions for LiveContainer.

## Done
- Inspected the reference bundle, wrote NOTES.md and CLEANROOM.md.

- JDK 17 installed. `LegacyFormat` (unwraps .nodes/.nodemc/.stknds) compiled
  with javac and unwrapped all 45 reference files.

- Gradle 8.10.2 in ~/.local/gradle, wrapper committed. `core:test` passes
  (5 tests, the reference-bundle test included). `desktop:run` launched and ran
  60 s without crashing (visual output not captured).
- UI.md: inventory of the original's tools with keep / fix / replace verdicts.

## In progress
- M0 exit: the iOS path has never run. Needs a private GitHub repo, a green CI
  build and a LiveContainer launch of the placeholder before any UI work.

## Next
- Get the user's screenshots and pain points for UI.md.
- Push to a private GitHub repo and confirm the CI IPA builds and launches in
  LiveContainer. Then M1, M2 (DESIGN.md).
