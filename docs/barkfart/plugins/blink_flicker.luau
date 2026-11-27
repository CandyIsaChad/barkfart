--!strict
-- Blink & Flicker: crisp on/off blink or noisy neon flicker for any value.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.blink_flicker",
	name = "Blink & Flicker",
	version = "1.0.0",
	author = "Barkfart",
	description = "Drives opacity, glow strength, or tint amount for damage blinks and neon flicker.",
	targets = {
		value = bf.value { label = "Value" },
	},
	props = {
		rate = bf.number { default = 6.0, min = 0.5, max = 30.0, step = 0.5, label = "Blinks/sec" },
		duty = bf.number { default = 0.5, min = 0.05, max = 0.95, step = 0.05, label = "On share" },
		jitter = bf.number { default = 0.3, min = 0.0, max = 1.0, step = 0.05, label = "Randomness" },
		amount = bf.number { default = 1.0, min = 0.0, max = 4.0, step = 0.1, label = "Strength" },
	},

	step = function(_s, p, t)
		local phase = (t.elapsed * p.rate) % 1.0
		local on = if phase < p.duty then 1.0 else 0.0
		if t.random() < p.jitter * 0.45 then
			on = 1.0 - on
		end
		return { value = on * p.amount }
	end,
}
