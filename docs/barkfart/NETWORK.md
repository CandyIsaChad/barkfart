# Network policy

Barkfart makes **no network requests**. There are no ads, analytics, crash
reporters, update checks or remote config.

## Remaining network calls
None. Any future one must be user-initiated (for example "share"), listed here
with its reason, and added to the allow list in `NetworkPolicyTest`.

## Enforcement
1. `NetworkPolicyTest` (runs with `core:test`, locally and on CI) fails if app
   source in core/desktop/ios uses a network API (java.net, Gdx.net, sockets,
   NSURLSession, ad/analytics SDK names). Verified on 2026-11-16 by planting a
   `java.net.Socket` field: the test failed, then passed once it was removed.
2. `tools/netcheck.sh [seconds]` runs the desktop build under `strace` and
   fails on any AF_INET/AF_INET6 connect/send. 2026-11-16 result: PASS. Only
   local Unix sockets were seen: D-Bus, PipeWire (audio), nscd (user lookup),
   systemd journal.
3. Dependencies (runtime classpath), none of which phone home unless called:
   LibGDX 1.12.1 (gdx, backend-lwjgl3 / backend-robovm, platform natives,
   jnigen-loader, jlayer, jorbis), LWJGL 3.3.3, MobiVM 2.3.21 (rt,
   cocoatouch, objc). LibGDX's `Gdx.net` exists but is unused and banned by
   rule 1.

## Original app, for contrast (from its Info.plist and bundled jars)
Google Mobile Ads, Google UMP consent, Firebase Core/Analytics/Crashlytics,
about 50 SKAdNetwork ids, App Tracking Transparency prompt, and
NSAllowsArbitraryLoads. Barkfart's Info.plist has none of these.

## Re-run points
The check has only covered the placeholder app so far. It must be re-run once
editing exists (M4) and once export exists (M8), on a create/edit/export
session as the brief asks.
