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
- 2026-11-16: IPA is built unsigned on GitHub Actions for LiveContainer.

## Done
- Inspected the reference bundle, wrote NOTES.md and CLEANROOM.md.

- JDK 17 installed. `LegacyFormat` (unwraps .nodes/.nodemc/.stknds) compiled
  with javac and unwrapped all 45 reference files.

## In progress
- M0: Gradle scaffold written (core, desktop, ios, CI workflow) but NOT yet
  built and NOT committed. Blocked: downloading Gradle and Maven dependencies
  was denied by the permission check.

## Next
- Run `gradle core:test desktop:compileJava`, generate the wrapper, commit.
- Push to a private GitHub repo and confirm the CI IPA builds and launches in
  LiveContainer. Then M1, M2 (DESIGN.md).
