--!strict
-- Sway: a figure leans into its movement and settles when it stops, like a
-- body with a little weight. A Barkfart script (SCRIPTING.md "Script layers").
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.sway", name = "Sway", version = "1.0.0", author = "Barkfart",
	description = "The figure leans into its movement and settles when it stops.",
	targets = { figure = bf.figure { label = "Figure" } },
	props = {
		lean = bf.number { default = 0.6, min = 0, max = 2, label = "Lean" },
		stiffness = bf.number { default = 0.35, min = 0.05, max = 1, label = "Stiffness" },
	},

	start = function(s)
		s.angle, s.speed = 0, 0
	end,

	step = function(s, p, t, scene)
		-- moving right leans it back to the left, and the other way round
		local pull = -scene.figure.velocity.x * 0.02 * p.lean
		s.speed = (s.speed + (pull - s.angle) * p.stiffness) * 0.85
		s.angle += s.speed
		return { figure = { rotate = { root = s.angle } } }
	end,
}
