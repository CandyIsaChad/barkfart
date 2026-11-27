--!strict
-- Spring Overshoot: curve modifier that adds elastic follow-through to any keyed value.
local bf = require("@barkfart")

return bf.modifier {
	id = "barkfart.spring_overshoot",
	name = "Spring Overshoot",
	version = "1.0.0",
	author = "Barkfart",
	description = "Adds bouncy elastic follow-through after fast keyframe changes in the graph.",
	props = {
		amount = bf.number { default = 0.45, min = 0.0, max = 2.0, step = 0.05, label = "Overshoot" },
		frequency = bf.number { default = 3.5, min = 0.5, max = 15.0, step = 0.25, label = "Frequency" },
		damping = bf.number { default = 3.0, min = 0.5, max = 12.0, step = 0.25, label = "Damping" },
	},

	value = function(v, p, t)
		local dt = 1.0 / math.max(t.fps, 1.0)
		local acc = 0.0
		local w = p.frequency * math.pi * 2
		for k = 1, 10 do
			local f1 = t.frame - k
			local f0 = f1 - 1
			local dv = v:at(f1) - v:at(f0)
			local age = k * dt
			acc += dv * math.exp(-age * p.damping) * math.sin(age * w)
		end
		return v.value + acc * p.amount
	end,
}
