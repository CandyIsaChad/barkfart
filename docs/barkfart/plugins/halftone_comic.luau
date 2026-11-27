--!strict
-- Comic Halftone: crisp manga/comic screentone dot shade preserving alpha.
local bf = require("@barkfart")

return bf.effect {
	id = "barkfart.halftone_comic",
	name = "Comic Halftone",
	version = "1.0.0",
	author = "Barkfart",
	description = "Turns shading into crisp manga/comic halftone dots.",
	props = {
		scale = bf.number { default = 6.0, min = 2.0, max = 24.0, step = 1.0, label = "Dot size" },
		contrast = bf.number { default = 1.25, min = 0.2, max = 3.0, step = 0.05, label = "Contrast" },
		amount = bf.number { default = 0.8, min = 0.0, max = 1.0, step = 0.05, label = "Amount" },
	},
	shader = bf.wgsl([[
fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> {
    let src = layer(uv);
    if (src.a <= 0.0) { return src; }
    let rgb = clamp(src.rgb / src.a, vec3<f32>(0.0), vec3<f32>(1.0));
    let luma = clamp((dot(rgb, vec3<f32>(0.299, 0.587, 0.114)) - 0.5) * u.contrast + 0.5, 0.0, 1.0);
    let cell = max(u.scale, 2.0);
    let px = uv * u.size;
    let rot = vec2<f32>(px.x * 0.866 - px.y * 0.5, px.x * 0.5 + px.y * 0.866);
    let g = fract(rot / cell) - vec2<f32>(0.5);
    let dist = length(g);
    let rad = sqrt(max(1.0 - luma, 0.0)) * 0.62;
    let dot_mask = smoothstep(rad + 0.06, rad - 0.06, dist);
    let toned = mix(rgb, rgb * 0.18, dot_mask);
    let out_rgb = mix(rgb, toned, clamp(u.amount, 0.0, 1.0));
    return vec4<f32>(out_rgb * src.a, src.a);
}
]]),
}
