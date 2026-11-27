--!strict
-- Orbit & Bob: smooth hovering bob or circular/elliptical orbit motion.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.orbit_bob",
	name = "Orbit & Bob",
	version = "1.0.0",
	author = "Barkfart",
	description = "Smooth floating hover or elliptical orbit with gentle banking tilt.",
	targets = {
		figure = bf.figure { label = "Figure" },
	},
	props = {
		radius_x = bf.number { default = 32.0, min = 0.0, max = 300.0, step = 2.0, label = "Width" },
		radius_y = bf.number { default = 16.0, min = 0.0, max = 300.0, step = 2.0, label = "Height" },
		speed = bf.number { default = 0.75, min = -5.0, max = 5.0, step = 0.05, label = "Turns/sec" },
		tilt = bf.number { default = 8.0, min = -45.0, max = 45.0, step = 1.0, label = "Bank tilt" },
	},

	step = function(_s, p, t)
		local a = t.elapsed * p.speed * math.pi * 2
		local x = math.cos(a) * p.radius_x
		local y = math.sin(a) * p.radius_y
		local bank = math.cos(a) * p.tilt
		return {
			figure = {
				move = { x = x, y = y },
				rotate = { root = bank },
			},
		}
	end,
}
