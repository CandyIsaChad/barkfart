--!strict
-- Pulse: any value that can be keyed (an effect's strength, a segment's
-- length, a plugin effect's setting) rises and falls on top of its keys.
local bf = require("@barkfart")

return bf.script {
	id = "barkfart.pulse", name = "Pulse", version = "1.0.0", author = "Barkfart",
	description = "Makes a value rise and fall: point it at a glow's strength or a segment's length.",
	targets = { value = bf.value { label = "Value" } },
	props = {
		amount = bf.number { default = 5, min = 0, max = 100, label = "Amount" },
		rate = bf.number { default = 1, min = 0.05, max = 10, label = "Pulses a second" },
	},

	step = function(s, p, t)
		return { value = math.abs(math.sin(t.elapsed * p.rate * math.pi)) * p.amount }
	end,
}
