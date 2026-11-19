struct View {
    // world -> clip: x' = x * scale.x + offset.x
    scale: vec2<f32>,
    offset: vec2<f32>,
};
@group(0) @binding(0) var<uniform> view: View;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs(@location(0) p: vec2<f32>, @location(1) rgba: vec4<f32>) -> Out {
    var o: Out;
    o.pos = vec4<f32>(p * view.scale + view.offset, 0.0, 1.0);
    // straight -> premultiplied
    o.color = vec4<f32>(rgba.rgb * rgba.a, rgba.a);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return i.color;
}
