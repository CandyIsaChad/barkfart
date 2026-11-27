--!strict
-- Organic Flicker: deterministic random jitter for handheld energy or flame intensity.
local bf = require("@barkfart")

return bf.modifier {
	id = "barkfart.flicker_noise",
	name = "Organic Flicker",
	version = "1.0.0",
	author = "Barkfart",
	description = "Adds repeatable jitter to any curve: great for fire glow, neon signs, or handheld shake.",
	props = {
		amount = bf.number { default = 0.3, min = 0.0, max = 100.0, step = 0.05, label = "Amount" },
		bias = bf.number { default = 0.0, min = -1.0, max = 1.0, step = 0.1, label = "Bias" },
	},

	value = function(v, p, t)
		local n = (t.random(-1, 1) + t.random(-1, 1)) * 0.5 + p.bias
		return v.value + n * p.amount
	end,
}
