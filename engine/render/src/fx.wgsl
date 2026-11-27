// Full-screen passes for isolates and effects (mirror barkfart_core::fx).
// All textures are premultiplied RGBA8, rows top-down, the size of the frame;
// texels outside a texture read as transparent.

struct P { v: array<vec4<f32>, 40> };
@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var t0: texture_2d<f32>;
@group(0) @binding(2) var t1: texture_2d<f32>;

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(i & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn ld0(x: i32, y: i32) -> vec4<f32> {
    let d = vec2<i32>(textureDimensions(t0));
    if (x < 0 || y < 0 || x >= d.x || y >= d.y) { return vec4<f32>(0.0); }
    return textureLoad(t0, vec2<i32>(x, y), 0);
}

fn ld1(x: i32, y: i32) -> vec4<f32> {
    let d = vec2<i32>(textureDimensions(t1));
    if (x < 0 || y < 0 || x >= d.x || y >= d.y) { return vec4<f32>(0.0); }
    return textureLoad(t1, vec2<i32>(x, y), 0);
}

fn px(pos: vec4<f32>) -> vec2<i32> {
    return vec2<i32>(floor(pos.xy));
}

// v0.x = opacity
@fragment
fn fs_copy(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    return ld0(q.x, q.y) * p.v[0].x;
}

// mix(t0, t1, v0.x)
@fragment
fn fs_lerp(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    return mix(ld0(q.x, q.y), ld1(q.x, q.y), p.v[0].x);
}

// v0.xy = direction, v0.z = sigma (pixels), v0.w = radius in taps
@fragment
fn fs_blur(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let dir = vec2<i32>(p.v[0].xy);
    let sigma = p.v[0].z;
    let r = i32(p.v[0].w);
    var acc = vec4<f32>(0.0);
    var ws = 0.0;
    for (var i = -r; i <= r; i++) {
        let x = f32(i);
        let w = exp(-(x * x) / (2.0 * sigma * sigma));
        ws += w;
        let o = q + dir * i;
        acc += ld0(o.x, o.y) * w;
    }
    return acc / ws;
}

// the source's alpha moved by v0.xy whole pixels (rows down)
@fragment
fn fs_shift(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let d = vec2<i32>(p.v[0].xy);
    return vec4<f32>(0.0, 0.0, 0.0, ld0(q.x - d.x, q.y - d.y).a);
}

// source (t0) over the coloured blurred alpha (t1): v0 = colour, v1.x = strength
@fragment
fn fs_under(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let src = ld0(q.x, q.y);
    let c = p.v[0];
    let sa = min(1.0, ld1(q.x, q.y).a * p.v[1].x) * c.a;
    return vec4<f32>(src.rgb + c.rgb * sa * (1.0 - src.a), src.a + sa * (1.0 - src.a));
}

@fragment
fn fs_invert(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let c = ld0(q.x, q.y);
    return vec4<f32>(c.a - c.rgb, c.a);
}

// rows of the colour matrix in v1..v3 (xyz), brightness v0.x
@fragment
fn fs_adjust(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let c = ld0(q.x, q.y);
    if (c.a <= 0.0) { return c; }
    let s = c.rgb / c.a;
    let b = p.v[0].x;
    let rgb = clamp(vec3<f32>(dot(p.v[1].xyz, s), dot(p.v[2].xyz, s), dot(p.v[3].xyz, s)) + vec3<f32>(b), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb * c.a, c.a);
}

// v0 = colour, v1.x = amount
@fragment
fn fs_tint(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let c = ld0(q.x, q.y);
    return vec4<f32>(c.rgb + (p.v[0].rgb * c.a - c.rgb) * p.v[1].x, c.a);
}

// v0 = (view x0, view y0, pixels per unit, block size in view units)
@fragment
fn fs_pixelate(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let x0 = p.v[0].x;
    let y0 = p.v[0].y;
    let s = p.v[0].z;
    let size = p.v[0].w;
    let vx = x0 + pos.x / s;
    let vy = y0 - pos.y / s;
    let cx = (floor(vx / size) + 0.5) * size;
    let cy = (floor(vy / size) + 0.5) * size;
    return ld0(i32(floor((cx - x0) * s)), i32(floor((y0 - cy) * s)));
}

// v0 = colour; v1..v16 = 32 offsets (two per vec4, y up)
@fragment
fn fs_outline(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let src = ld0(q.x, q.y);
    var a = src.a;
    for (var k = 0; k < 16; k++) {
        let o = vec4<i32>(p.v[1 + k]);
        a = max(a, ld0(q.x + o.x, q.y - o.y).a);
        a = max(a, ld0(q.x + o.z, q.y - o.w).a);
    }
    let c = p.v[0];
    let sa = a * c.a;
    return vec4<f32>(src.rgb + c.rgb * sa * (1.0 - src.a), src.a + sa * (1.0 - src.a));
}

// v1..v8 = 16 offsets (two per vec4, y up)
@fragment
fn fs_motion(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    var acc = vec4<f32>(0.0);
    for (var k = 0; k < 8; k++) {
        let o = vec4<i32>(p.v[1 + k]);
        acc += ld0(q.x + o.x, q.y - o.y);
        acc += ld0(q.x + o.z, q.y - o.w);
    }
    return acc / 16.0;
}

// Bilinear sample in pixel-index space (texel (i, j) sits at (i, j)).
fn sample_bilinear(x: f32, y: f32) -> vec4<f32> {
    let fx = floor(x);
    let fy = floor(y);
    let tx = x - fx;
    let ty = y - fy;
    let ix = i32(fx);
    let iy = i32(fy);
    let c00 = ld0(ix, iy);
    let c10 = ld0(ix + 1, iy);
    let c01 = ld0(ix, iy + 1);
    let c11 = ld0(ix + 1, iy + 1);
    let top = mix(c00, c10, tx);
    let bot = mix(c01, c11, tx);
    return mix(top, bot, ty);
}

// Deterministic 32-bit integer hash (matches barkfart_core::fx::hash_u32).
fn hash_u32(v_in: u32) -> u32 {
    var x = v_in;
    x = (x ^ 61u) ^ (x >> 16u);
    x = x + (x << 3u);
    x = x ^ (x >> 4u);
    x = x * 0x27d4eb2du;
    x = x ^ (x >> 15u);
    return x;
}

// v0.xy = (dx, dy) in pixels (y down)
@fragment
fn fs_chromatic(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let fx = f32(q.x);
    let fy = f32(q.y);
    let dx = p.v[0].x;
    let dy = p.v[0].y;
    let cr = sample_bilinear(fx + dx, fy + dy);
    let cg = ld0(q.x, q.y);
    let cb = sample_bilinear(fx - dx, fy - dy);
    let a = max(cr.a, max(cg.a, cb.a));
    return vec4<f32>(cr.r, cg.g, cb.b, a);
}

// v0 = color, v1 = (amount, softness, full_frame_flag, 0), v2 = frame_rect (rx, ry, rw, rh)
@fragment
fn fs_vignette(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let src = ld0(q.x, q.y);
    let rx = p.v[2].x;
    let ry = p.v[2].y;
    let rw = max(p.v[2].z, 1.0);
    let rh = max(p.v[2].w, 1.0);
    let uv = (vec2<f32>(q) + vec2<f32>(0.5) - vec2<f32>(rx, ry)) / vec2<f32>(rw, rh) * 2.0 - vec2<f32>(1.0);
    let dist = length(uv) * 0.70710678;
    let amount = p.v[1].x;
    let softness = p.v[1].y;
    let full_frame = p.v[1].z;
    let inner = 0.95 - softness * 0.85;
    let t = smoothstep(inner, 1.05, dist);
    let c = p.v[0];
    let va = t * amount * c.a;
    if (full_frame > 0.5) {
        let k = 1.0 - va;
        return vec4<f32>(c.rgb * va + src.rgb * k, va + src.a * k);
    }
    return vec4<f32>(src.rgb + (c.rgb * src.a - src.rgb) * va, src.a);
}

// v0 = (x0, y0, scale, amp_px), v1 = (k_freq, phase_rad, cos_a, sin_a)
@fragment
fn fs_wave(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let x0 = p.v[0].x;
    let y0 = p.v[0].y;
    let s = max(p.v[0].z, 1e-6);
    let amp_px = p.v[0].w;
    let k_freq = p.v[1].x;
    let phase_rad = p.v[1].y;
    let cos_a = p.v[1].z;
    let sin_a = p.v[1].w;
    let vx = x0 + (f32(q.x) + 0.5) / s;
    let vy = y0 - (f32(q.y) + 0.5) / s;
    let along = vx * (-sin_a) + vy * cos_a;
    let off = sin(along * k_freq + phase_rad) * amp_px;
    return sample_bilinear(f32(q.x) + cos_a * off, f32(q.y) - sin_a * off);
}

// v0 = (amount, size, seed_i, full_frame_flag)
@fragment
fn fs_grain(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let src = ld0(q.x, q.y);
    let amount = p.v[0].x;
    let size = max(p.v[0].y, 1.0);
    let sd = bitcast<u32>(i32(p.v[0].z));
    let full_frame = p.v[0].w;
    let gx = bitcast<u32>(i32(floor(f32(q.x) / size)));
    let gy = bitcast<u32>(i32(floor(f32(q.y) / size)));
    let h = hash_u32(gx ^ hash_u32(gy ^ hash_u32(sd * 0x9e3779b9u + 1u)));
    let n = f32(h & 0xffffu) / 32767.5 - 1.0;
    var rgb = vec3<f32>(0.0);
    if (src.a > 0.0) {
        rgb = clamp(src.rgb / src.a + vec3<f32>(n * amount * 0.5), vec3<f32>(0.0), vec3<f32>(1.0)) * src.a;
    }
    if (full_frame > 0.5) {
        let ga = abs(n) * amount * 0.22 * (1.0 - src.a);
        let gc = select(0.0, ga, n > 0.0);
        return vec4<f32>(rgb + vec3<f32>(gc), src.a + ga);
    }
    return vec4<f32>(rgb, src.a);
}

// v0 = (exp_mul, contrast, inv_gamma, 0)
@fragment
fn fs_grade(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let src = ld0(q.x, q.y);
    if (src.a <= 0.0) { return src; }
    let exp_mul = p.v[0].x;
    let contrast = p.v[0].y;
    let inv_g = p.v[0].z;
    var s = clamp(src.rgb / src.a, vec3<f32>(0.0), vec3<f32>(1.0));
    s = s * exp_mul;
    s = (s - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5);
    s = clamp(s, vec3<f32>(0.0), vec3<f32>(1.0));
    let powered = pow(max(s, vec3<f32>(1e-6)), vec3<f32>(inv_g));
    s = select(powered, vec3<f32>(0.0), s <= vec3<f32>(0.0));
    return vec4<f32>(clamp(s, vec3<f32>(0.0), vec3<f32>(1.0)) * src.a, src.a);
}

// v0 = (amount, lines, offset_px, phase_i), v1 = (phase_f, full_frame_flag, ry, rh)
@fragment
fn fs_scanlines(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = px(pos);
    let amount = p.v[0].x;
    let lines = p.v[0].y;
    let offset_px = p.v[0].z;
    let sd = bitcast<u32>(i32(p.v[0].w));
    let phase_f = p.v[1].x;
    let full_frame = p.v[1].y;
    let ry = p.v[1].z;
    let rh = max(p.v[1].w, 1.0);
    let local_y = f32(q.y) - ry;
    let band_h = max(2.0, rh / max(lines * 0.25, 4.0));
    let band = bitcast<u32>(i32(floor(local_y / band_h)));
    let hb = hash_u32(band ^ hash_u32(sd * 0x9e3779b9u + 7u));
    let u = f32(hb & 0xffffu) / 65535.0;
    var shift_px = 0.0;
    if (offset_px > 0.0 && amount > 0.0 && u > (1.0 - amount * 0.55)) {
        let dir = f32((hb >> 16u) & 0xffffu) / 32767.5 - 1.0;
        shift_px = dir * offset_px;
    }
    var c: vec4<f32>;
    if (abs(shift_px) > 0.01) {
        let fx = f32(q.x);
        let fy = f32(q.y);
        let cr = sample_bilinear(fx + shift_px * 1.25, fy);
        let cg = sample_bilinear(fx + shift_px, fy);
        let cb = sample_bilinear(fx + shift_px * 0.75, fy);
        c = vec4<f32>(cr.r, cg.g, cb.b, max(cr.a, max(cg.a, cb.a)));
    } else {
        c = ld0(q.x, q.y);
    }
    let y_norm = (local_y + 0.5) / rh;
    let scan = 0.5 + 0.5 * sin((y_norm * lines + phase_f * 0.25) * 6.2831853);
    let darken = (1.0 - scan) * amount * 0.65;
    let rgb = c.rgb * (1.0 - darken);
    if (full_frame > 0.5) {
        return vec4<f32>(rgb, c.a + darken * 0.45 * (1.0 - c.a));
    }
    return vec4<f32>(rgb, c.a);
}
