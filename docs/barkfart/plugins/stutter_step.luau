--!strict
-- Anime Stepped: holds curves on 2s, 3s, or Nth frames for hand-drawn timing.
local bf = require("@barkfart")

return bf.modifier {
	id = "barkfart.stutter_step",
	name = "Anime Stepped",
	version = "1.0.0",
	author = "Barkfart",
	description = "Holds a curve on every 2nd or 3rd frame for snappy hand-drawn anime timing.",
	props = {
		every = bf.number { default = 2, min = 1, max = 12, step = 1, label = "Hold frames" },
	},

	value = function(v, p, t)
		local step = math.max(1, math.floor(p.every + 0.5))
		return v:at(math.floor(t.frame / step) * step)
	end,
}
