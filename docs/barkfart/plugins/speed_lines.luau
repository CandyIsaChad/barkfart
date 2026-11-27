--!strict
-- Anime Speed Lines: radial manga/anime action lines animated over time.
local bf = require("@barkfart")

return bf.effect {
	id = "barkfart.speed_lines",
	name = "Speed Lines",
	version = "1.0.0",
	author = "Barkfart",
	description = "Radial anime action lines converging toward the center. Best on an Effect layer!",
	props = {
		density = bf.number { default = 52.0, min = 12.0, max = 160.0, step = 4.0, label = "Density" },
		intensity = bf.number { default = 0.78, min = 0.0, max = 1.0, step = 0.05, label = "Intensity" },
		inner = bf.number { default = 0.30, min = 0.05, max = 0.85, step = 0.05, label = "Center clear" },
		speed = bf.number { default = 14.0, min = 0.0, max = 60.0, step = 2.0, label = "Flicker speed" },
		color = bf.colour { default = bf.rgb(1.0, 1.0, 1.0), label = "Line color" },
	},
	shader = bf.wgsl([[
fn sl_hash(n: u32) -> f32 {
    var x = (n ^ 61u) ^ (n >> 16u);
    x = x + (x << 3u);
    x = x ^ (x >> 4u);
    x = x * 0x27d4eb2du;
    x = x ^ (x >> 15u);
    return f32(x & 0xffffu) / 65535.0;
}

fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> {
    let src = layer(uv);
    let p = uv * 2.0 - vec2<f32>(1.0);
    let r = length(p);
    let ang = atan2(p.y, p.x);
    let norm_a = (ang + 3.14159265) / 6.2831853;
    let sector = floor(norm_a * u.density);
    let local_a = fract(norm_a * u.density);
    let tick = u32(floor(u.time * max(u.speed, 0.0)));
    let h = sl_hash(u32(sector) ^ (tick * 0x9e3779b9u + 13u));
    let h2 = sl_hash(u32(sector) * 3u + 7u ^ (tick * 0x85ebca6bu));
    let reach = u.inner + h * 0.48;
    let radial = smoothstep(reach, reach + 0.32, r);
    let spike = smoothstep(0.32, 0.04, abs(local_a - 0.5)) * step(0.38, h2);
    let la = clamp(radial * spike * u.intensity * u.color.a, 0.0, 1.0);
    let k = 1.0 - la;
    return vec4<f32>(u.color.rgb * la + src.rgb * k, la + src.a * k);
}
]]),
}
