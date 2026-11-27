--!strict
-- Trail & Ghost Echo: directional fading afterimage copies behind a figure.
local bf = require("@barkfart")

return bf.effect {
	id = "barkfart.trail_echo",
	name = "Trail & Ghost Echo",
	version = "1.0.0",
	author = "Barkfart",
	description = "Draws tinted speed-afterimage echoes trailing behind a figure or dash.",
	props = {
		copies = bf.number { default = 5, min = 1, max = 10, step = 1, label = "Echoes" },
		spacing = bf.number { default = 0.018, min = 0.002, max = 0.1, step = 0.004, label = "Spacing" },
		angle = bf.angle { default = 180.0, label = "Trail angle" },
		fade = bf.number { default = 0.65, min = 0.0, max = 1.0, step = 0.05, label = "Opacity" },
		tint = bf.colour { default = bf.rgb(0.22, 0.76, 1.0), label = "Echo tint" },
	},
	shader = bf.wgsl([[
fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> {
    let rad = radians(u.angle);
    let step_uv = vec2<f32>(cos(rad), -sin(rad)) * u.spacing;
    var acc = vec4<f32>(0.0);
    let n = clamp(u.copies, 1.0, 10.0);
    for (var i = 10; i >= 1; i--) {
        let fi = f32(i);
        if (fi > n) { continue; }
        let s = layer(uv + step_uv * fi);
        let w = (1.0 - (fi - 1.0) / (n + 0.5)) * u.fade * 0.55;
        let tinted = vec4<f32>(mix(s.rgb, u.tint.rgb * s.a, 0.7) * w, s.a * w);
        acc = vec4<f32>(tinted.rgb + acc.rgb * (1.0 - tinted.a), tinted.a + acc.a * (1.0 - tinted.a));
    }
    let src = layer(uv);
    return vec4<f32>(src.rgb + acc.rgb * (1.0 - src.a), src.a + acc.a * (1.0 - src.a));
}
]]),
}
