--!strict
-- Particle Burst: ballistic launch with spread, gravity, drag, and spin.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.particle_burst",
	name = "Particle Burst",
	version = "1.0.0",
	author = "Barkfart",
	description = "Launches a figure or prop along a ballistic arc with seeded spread, gravity, and spin.",
	targets = {
		figure = bf.figure { label = "Particle" },
	},
	props = {
		speed = bf.number { default = 260.0, min = 0.0, max = 1500.0, step = 10.0, label = "Launch speed" },
		angle = bf.angle { default = 65.0, label = "Angle" },
		spread = bf.number { default = 35.0, min = 0.0, max = 180.0, step = 5.0, label = "Spread" },
		gravity = bf.number { default = 520.0, min = -1000.0, max = 2000.0, step = 20.0, label = "Gravity" },
		spin = bf.number { default = 180.0, min = -1440.0, max = 1440.0, step = 15.0, label = "Spin deg/s" },
		drag = bf.number { default = 0.04, min = 0.0, max = 0.5, step = 0.01, label = "Air drag" },
	},

	start = function(s, p, t)
		local a = math.rad(p.angle + t.random(-p.spread, p.spread))
		local spd = p.speed * t.random(0.75, 1.25)
		s.x, s.y = 0, 0
		s.vx = math.cos(a) * spd
		s.vy = math.sin(a) * spd
		s.rot = 0
		s.vrot = p.spin * t.random(0.6, 1.4)
	end,

	step = function(s, p, t)
		local keep = math.max(0.0, 1.0 - p.drag)
		s.vy -= p.gravity * t.dt
		s.vx *= keep
		s.vy *= keep
		s.x += s.vx * t.dt
		s.y += s.vy * t.dt
		s.rot += s.vrot * t.dt
		return {
			figure = {
				move = { x = s.x, y = s.y },
				rotate = { root = s.rot },
			},
		}
	end,
}
