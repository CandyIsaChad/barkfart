// ---- shapes (draw-list batches) ----

struct View {
    // world -> clip: x' = x * scale.x + offset.x
    scale: vec2<f32>,
    offset: vec2<f32>,
};
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs(@location(0) p: vec2<f32>, @location(1) rgba: vec4<f32>, @location(2) uv: vec2<f32>) -> Out {
    var o: Out;
    o.pos = vec4<f32>(p * view.scale + view.offset, 0.0, 1.0);
    o.color = rgba;
    o.uv = uv;
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    // straight colour interpolated, then premultiplied (like the reference)
    let c = i.color * textureSample(tex, samp, i.uv);
    return vec4<f32>(c.rgb * c.a, c.a);
}
