# Barkfart additions (after parity)

Rule (user decision, 2026-11-17): rebuild every Stick Nodes feature first
(PARITY.md), then add Barkfart's own ideas. Adding our own features along the
way makes each build harder to test, so they wait here until parity is done.

## Ideas, in the order they came up

1. **Timeline next to the dope sheet** (user, 2026-11-17). Two views of the
   same project:
   - *Dope sheet* (exists): keys per figure and joint, for refining
     animation.
   - *Timeline*: layers as bars over time, like a video editor. It shows
     sound clips with their waveforms for previewing sound effects, and
     layer-level effects (blur, glow and the rest as clips or tracks you
     can trim and fade). Symbols / movieclips would show as bars too.
   Notes for later: both views share the playhead and selection; the model
   already has what the timeline needs (layers, clips with in/out/fades,
   effect stacks with animatable parameters).
