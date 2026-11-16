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

### M0 Explore and baseline (closed 2026-11-16)
What works
- Reference bundle inspected (Info.plist, layout, data files, UI atlases and
  strings; no disassembly). NOTES.md, CLEANROOM.md, UI.md written.
- Gradle project: core, desktop (LWJGL3), ios (MobiVM 2.3.21). Wrapper 8.10.2.
- `LegacyFormat` unwraps all three legacy wrappers (zlib .nodes, raw
  big-endian .nodes, magic+gzip .nodemc/.stknds).
- Private repo github.com/balsamiqvine/barkfart; CI (macos-latest) runs core
  tests and builds an unsigned IPA (about 16.7 MB, about 1 minute).

How it was tested
- `./gradlew core:test`: 5 tests pass locally and on CI, including one that
  unwraps all 45 local reference files.
- `desktop:run` launched and ran 60 s without crashing.
- IPA inspected: arm64 Mach-O, bundle id app.barkfart.animator, landscape,
  no ad/analytics strings.
- User launched the CI IPA in LiveContainer: it starts (2026-11-16).

Not tested
- Desktop and iOS visual output were not captured.

Deviations from the brief (clean room)
- No original source, so there was nothing to "build" and no original renderer
  to capture golden frames from. Golden references move to M3 and will come
  from user screen captures of the original plus our own renders.

CI notes
- The `macos-14` label was cancelled before a runner started, so CI uses
  `macos-latest`. `gh run watch` exits early because the token cannot read
  check annotations (HTTP 403); poll `gh run view` instead.

## In progress
- M1 Rebrand and clean: placeholder icon and launch screen, dependency and
  network audit, zero-network runtime check.

## Known issues
- `ios/data/.gitkeep` is copied into the app bundle. Harmless; remove when
  real assets land.

## Next
- M2 DESIGN.md.
- Waiting on the user: screenshots of the original's main screens and their
  worst pain points (for UI.md).
