--!strict
-- Spring Bounce: juicy damped spring follow-through on figure movement.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.spring_bounce",
	name = "Spring Bounce",
	version = "1.0.0",
	author = "Barkfart",
	description = "Adds snappy spring-mass overshoot and squashy settle whenever a figure moves.",
	targets = {
		figure = bf.figure { label = "Figure" },
	},
	props = {
		stiffness = bf.number { default = 0.32, min = 0.05, max = 1.0, step = 0.05, label = "Stiffness" },
		damping = bf.number { default = 0.80, min = 0.4, max = 0.96, step = 0.02, label = "Damping" },
		bounce = bf.number { default = 1.0, min = 0.0, max = 3.0, step = 0.1, label = "Bounce" },
		kick = bf.number { default = 14.0, min = 0.0, max = 80.0, step = 2.0, label = "Start hop" },
	},

	start = function(s, p)
		s.ox, s.oy = 0, 0
		s.vx, s.vy = 0, p.kick * 0.45
		s.rot, s.vrot = 0, 0
	end,

	step = function(s, p, t, scene)
		local fx = -scene.figure.velocity.x * t.dt * 0.35 * p.bounce
		local fy = -scene.figure.velocity.y * t.dt * 0.35 * p.bounce
		s.vx = (s.vx + (fx - s.ox) * p.stiffness) * p.damping
		s.vy = (s.vy + (fy - s.oy) * p.stiffness) * p.damping
		s.ox += s.vx
		s.oy += s.vy
		local tilt = -s.ox * 0.25
		s.vrot = (s.vrot + (tilt - s.rot) * p.stiffness) * p.damping
		s.rot += s.vrot
		return {
			figure = {
				move = { x = s.ox, y = s.oy },
				rotate = { root = s.rot },
			},
		}
	end,
}
