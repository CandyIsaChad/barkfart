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
- 2026-11-16: IPA is built unsigned on GitHub Actions for LiveContainer.

## Done
- Inspected the reference bundle, wrote NOTES.md and CLEANROOM.md.

## In progress
- M0: blocked on a local JDK (no java/gradle on this machine).

## Next
- Gradle scaffold (core, desktop, ios), a trivial core test, CI workflow
  that builds an unsigned IPA. Then M1, M2 (DESIGN.md).
