--!strict
-- Camera Shake: impact trauma shake with smooth decay for camera X/Y or any figure.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.camera_shake",
	name = "Camera Shake",
	version = "1.0.0",
	author = "Barkfart",
	description = "Impact shake with smooth decay. Point it at a figure, or at Camera X and Camera Y values.",
	targets = {
		figure = bf.figure { label = "Figure (optional)" },
		x = bf.value { label = "Camera X (optional)" },
		y = bf.value { label = "Camera Y (optional)" },
	},
	props = {
		amount = bf.number { default = 18.0, min = 0.0, max = 120.0, step = 1.0, label = "Strength" },
		frequency = bf.number { default = 18.0, min = 1.0, max = 60.0, step = 1.0, label = "Speed" },
		decay = bf.number { default = 1.4, min = 0.0, max = 10.0, step = 0.1, label = "Decay" },
		roll = bf.number { default = 2.0, min = 0.0, max = 20.0, step = 0.5, label = "Tilt" },
	},

	start = function(s, _p, t)
		s.p1 = t.random() * math.pi * 2
		s.p2 = t.random() * math.pi * 2
		s.p3 = t.random() * math.pi * 2
	end,

	step = function(s, p, t)
		local env = math.exp(-t.elapsed * p.decay)
		local w = t.elapsed * p.frequency
		local jx = (math.sin(w * 1.13 + s.p1) * 0.6 + t.random(-1, 1) * 0.4) * p.amount * env
		local jy = (math.cos(w * 1.47 + s.p2) * 0.6 + t.random(-1, 1) * 0.4) * p.amount * env
		local jr = math.sin(w * 0.91 + s.p3) * p.roll * env
		return {
			figure = { move = { x = jx, y = jy }, rotate = { root = jr } },
			x = jx,
			y = jy,
		}
	end,
}
