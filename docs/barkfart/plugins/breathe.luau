--!strict
-- Breathe: a standing figure rises and falls a little, as if breathing.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.breathe", name = "Breathe", version = "1.0.0", author = "Barkfart",
	description = "A gentle rise and fall, so a figure standing still looks alive.",
	targets = { figure = bf.figure { label = "Figure" } },
	props = {
		depth = bf.number { default = 2, min = 0, max = 10, label = "Depth" },
		rate = bf.number { default = 0.3, min = 0.05, max = 2, label = "Breaths a second" },
	},

	step = function(s, p, t)
		-- each run starts at its own point of the breath
		s.phase = s.phase or t.random() * math.pi * 2
		local y = math.sin(s.phase + t.elapsed * p.rate * math.pi * 2) * p.depth
		return { figure = { move = { y = y } } }
	end,
}
