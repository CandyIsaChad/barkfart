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
