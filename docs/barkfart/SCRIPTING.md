# Scripting (proposal)

Status: proposal, nothing built yet. Barkfart gets plugins written in Luau,
the sandboxed Lua dialect, so people can add their own effects, property
drivers, tools and panels. LuaJIT is out, because iOS forbids executable
memory, and Luau's interpreter is fast enough without it.

## Ground rules

- **Sandboxed.** A plugin sees only its own folder and the `@barkfart` API: no
  `io`, `os`, `debug` or bytecode loading. Each call has a time budget and each
  plugin has a memory cap; going over stops that call and switches the plugin
  off with a message.
- **No network.** NETWORK.md still holds. Plugins and shader code come in the
  way pictures and sounds do: the user imports a file, a folder or pasted text.
  A plugin cannot fetch anything, so a thumbnail is a file in its folder, not a
  URL.
- **Same picture everywhere.** A plugin's output depends only on its property
  values and the frame being drawn, so playing, scrubbing backwards and
  exporting all show the same thing. No state is carried from one frame to the
  next. That is why there is no `on_play(delta)`: scrubbing and export jump
  around in time. Script layers are the one exception, and they keep the
  promise another way (see below).
- **Undoable.** Every change a plugin makes to the project goes through the
  same commands the editor uses, so it is one undo step.
- **Missing plugins don't break projects.** If a project uses a plugin that
  isn't installed, its settings and keys stay in the file. The effect draws
  nothing (the layer shows as it would without it) and the Effects list shows
  "Missing: Warp 1.0".

## Plugin kinds, in build order

1. **Effects:** a shader plus properties. This covers the custom warp, glows,
   halftones and anything else from shader sites.
2. **Modifiers:** a function that drives any keyable value, such as a custom
   wiggle. They sit next to the built-in Wiggle, Swing, Loop, Follow and Link.
3. **Script layers:** code that moves figures over time, such as physics. See
   "Script layers" below.
4. **Tools:** run on demand and change the project through editor commands,
   for example "mirror this walk cycle".
5. **Panels:** a plugin describes controls (sliders, tick boxes, buttons,
   lists, colours, text) and the app draws them in its own style. Plugins don't
   draw raw pixels in panels at first, which keeps the look consistent and the
   sandbox simple.

## Writing an effect

A plugin is a folder (or a `.bfplugin` zip of one) with `plugin.luau`, its
shader and a thumbnail. `plugin.luau` returns one definition:

```lua
--!strict
local bf = require("@barkfart")

return bf.effect {
	id = "yeah.warp",          -- stable: saved projects find the plugin by this
	name = "Warp",
	version = "1.0.0",
	author = "yeah",
	description = "Bends the layer like heat haze.",
	thumbnail = "thumbnail.png",

	-- every property is keyable and shows up in Properties automatically
	props = {
		amount = bf.number { default = 0.3, min = 0, max = 1 },
		centre = bf.point { default = bf.vec2(0.5, 0.5) },  -- gets a handle on the stage
		depth = bf.number { default = 0 },                   -- no limits
		wobbly = bf.toggle { default = true },
		tint = bf.colour { default = bf.rgb(1, 0.8, 0.2) },
		shape = bf.choice { "Ripple", "Twist", "Bulge", default = "Ripple" },
	},

	shader = bf.glsl("warp.frag"),  -- or bf.wgsl(...), or inline source text

	-- optional, runs on the CPU for each drawn frame, before the shader:
	-- p holds this frame's property values (keys and modifiers applied),
	-- t holds frame, seconds and fps. Returns extra shader inputs.
	frame = function(p, t)
		return { phase = t.seconds * 6 * p.amount }
	end,

	-- optional: a property was changed by hand (handy for a debug readout)
	changed = function(name, new, old)
		print(name, old, "->", new)
	end,
}
```

Property types map onto the engine's own values: `number` and `angle` are
floats, `point` is a 2D vector, `colour` is a colour, `toggle` is a
true/false value, `choice` is a list of names, `text` is a string. Properties
can also have a `label`, a `group` (a section in Properties) and a `step`.

### The shader

Properties arrive in the shader under the same names. The layer's picture is
`layer`, and `resolution`, `time` and `frame` are always there. Code written
for Shadertoy-style sites also works with their names (`mainImage`,
`fragCoord`, `iResolution`, `iTime`, `iChannel0` for the layer), so most
single-pass shaders paste in with small edits:

```glsl
// warp.frag
void mainImage(out vec4 colour, in vec2 uv) {
	vec2 d = uv - centre;
	float wave = sin(length(d) * 40.0 - phase);
	colour = texture(layer, uv + d * wave * amount * 0.05);
}
```

GLSL goes through naga, which wgpu already uses, so it runs on Metal, Vulkan
and the rest. Shaders with several passes, or that read other buffers, are a
later step.

Licences: shaders from public sites keep their authors' licences, and many
forbid commercial use. Importing one for your own animation is the user's
call. Anything we bundle with the app is written by us.

## Other kinds, briefly

```lua
-- a modifier: driven value = function of the keyed value and time
return bf.modifier {
	id = "yeah.stutter", name = "Stutter", version = "1.0.0",
	props = { every = bf.number { default = 3, min = 1, step = 1 } },
	value = function(v, p, t)
		return v:at(math.floor(t.frame / p.every) * p.every)  -- holds every Nth frame
	end,
}

-- a tool: shows up in a Plugins menu; edits are one undo step
return bf.tool {
	id = "yeah.mirror-walk", name = "Mirror walk cycle", version = "1.0.0",
	run = function(project, selection)
		for _, layer in selection.layers do
			project:flip(layer, "sideways")
		end
	end,
}
```

## Script layers

A Script is a kind of layer, a row in the timeline like a figure or a sound.
It runs code that moves other layers. Typical uses:
- simple deterministic physics: a swinging cape, a bouncing ball, a ragdoll fall;
- procedural motion: breathing, a crowd idling;
- anything that is easier to describe than to key.

- **Targets, not names.** A script declares what it moves, and the user picks
  the figures in Properties, so a script never hard-codes layer names.
- **Additive by default.** A script's output (rotation and position offsets) is
  added on top of the keyed poses. Two scripts on the same figure add up
  instead of fighting from frame to frame, and the hand-made animation
  underneath stays intact. A "Replace" toggle on the script switches to
  overriding, for whole simulations such as a ragdoll.
- **A block is one run.** A script lives as long as its block on the row: it
  starts on the block's first frame and stops after its last. Drag the block's
  ends to change that, as with poses. Several blocks of the same script on a
  row are separate runs, each with its own random seed, so placing the same
  "dust puff" at every footstep gives a different puff each time.
- **Ending.** When a block ends, its offsets go away and the figure is back on
  its keys. A "Keep the last frame" toggle holds the final result instead,
  for physics that should settle and stay.
- **Mix.** A keyable Mix (0 to 100%) fades a script in and out.
- **Bare-bones inputs.** The same `props` and sandbox as effects, but no
  shader. `t.dt` is the time since the last step and `t.elapsed` the time
  since the block started. `t.frame` is the frame number. `t.key(target)` is
  the target's current pose key: its frame and how far along it is to the
  next one.
- **State, kept deterministic.** Physics needs the previous frame. So a script
  runs forward from its first frame, and the app caches every frame's result:
  - scrubbing reads the cache;
  - editing anything the script reads clears the cache from that frame on;
  - export uses the same cache;
  - randomness comes from a seeded `t.random`, so a re-run gives the same
    result.
- **Bake to keys.** One tap turns a script's output into ordinary pose keys,
  to hand-tweak it, or to save to Stick Nodes formats, which can't run
  scripts.

```lua
return bf.script {
	id = "yeah.cape", name = "Cape sway", version = "1.0.0",
	targets = { cape = bf.figure() },
	props = { stiffness = bf.number { default = 0.4, min = 0, max = 1 } },

	start = function(s)                 -- the block's first frame
		s.swing, s.speed = 0, 0
	end,

	step = function(s, p, t, scene)     -- every frame of the block, in order
		local push = -scene.cape.velocity.x * t.dt * 0.05
		s.speed = (s.speed + push - s.swing * p.stiffness) * 0.9
		s.swing += s.speed
		return { cape = { rotate = { root = s.swing } } }  -- added on top of the keys
	end,
}
```

### Emitters

A script can also spawn things instead of moving them: sparks, dust, confetti,
a flock of birds. `t.spawn(source, settings)` puts out a copy of a library
figure, picture or movieclip:

```lua
return bf.script {
	id = "yeah.dust", name = "Dust puff", version = "1.0.0",
	props = {
		amount = bf.number { default = 12, min = 0, max = 200, step = 1 },
		puff = bf.source { default = "Dust" },   -- picked from the library
	},
	step = function(s, p, t)
		if t.elapsed == 0 then               -- a burst on the block's first frame
			for _ = 1, p.amount do
				t.spawn(p.puff, {
					angle = t.random(200, 340), speed = t.random(40, 120),
					spin = t.random(-90, 90), gravity = 30, fade = true, life = 0.6,
				})
			end
		end
	end,
}
```

Spawned copies are not layers. Here is how they behave:
- **No new rows.** 200 sparks don't add 200 rows; the block shows a count.
- **Shared.** They share their source, the way movieclip instances already do, so they're cheap.
- **Movement:** the engine moves them, with speed, spin, gravity and fading, for `life` seconds. The script can still nudge them each step. A movieclip source plays its own animation from the moment each copy appears.
- **Capped:** each emitter has a limit (500 to start), so a phone stays smooth.
- **Repeatable:** they use the block's seed, so a re-run gives the same burst, and a copied block gets a new seed.
- **Export:** emitters can't be baked into Stick Nodes formats without turning every copy into a figure. Video and GIF export are fine; Stick Nodes project export warns and leaves them out.

In the engine, the built-in modifiers (wiggle, sine, loop, follow and link)
are the stateless version of this. A script layer becomes a new `LayerKind`.
Its offsets apply after keys and modifiers, before joins are resolved.

## How it fits the engine

- **Effects:** `EffectNode` is already generic: a type name plus named
  `Property` values with their own keys and modifiers. A plugin effect is
  `ty = "plugin:yeah.warp"` with one `Property` per prop, so keyframing,
  modifiers, copy and paste, undo and saving work with no extra code.
- **Rendering:** the GPU renderer (`render/src/lib.rs`) gets one more branch.
  It compiles the plugin's shader once per version, caches the pipeline, and
  fills a uniform buffer from the props. The CPU effect path (`core/src/fx.rs`)
  can't run shaders, so it passes plugin effects through. Anything with
  plugin effects is rendered and exported on the GPU.
- **Luau:** through the `mlua` crate with its `luau` feature, in sandbox mode,
  with an interrupt callback for the time budget.
- **UI:**
  - Plugin effects appear in the effects browser under "Yours", with their own
    thumbnails.
  - "Import…" in the browser's header brings a plugin in.
  - Properties builds each plugin's controls from `props`:
    - `number` becomes a slider;
    - `point` becomes two fields plus a handle on the stage;
    - `colour` becomes a swatch;
    - `toggle` becomes a tick box;
    - `choice` becomes a list.

## Sharing, and the App Store

There is no plugin store, now or later. Plugins travel as files, through
AirDrop, Files, messages or a website the author runs themselves, and the user
imports them.

Apps like Codea already run user-written Lua on iPad. The rule to watch is
Apple's ban on apps downloading code that changes what they do. Scripts the
user writes or imports themselves are the usual safe ground. Read Apple's
current wording before shipping.

## Open questions

- Whether a plugin may declare its own keyboard shortcut or stage gesture.
- How much of the editor's command set tools get in the first version.
