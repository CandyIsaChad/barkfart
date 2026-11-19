//! Effects: the software reference renderer (port of Java fx.Image, Kernels,
//! EffectTypes, Compositor). The GPU renderer must match this.
//! Images are premultiplied RGBA f32, row 0 at the top.

use crate::eval::{ParamValue, ResolvedEffect};
use crate::model::{Blend, Color};
use crate::tess::{Batch, DrawList, IsoItem};

/// Premultiplied RGBA float image. Pixel (i, j) covers view
/// x = x0 + (i + 0.5) / scale, y = y0 - (j + 0.5) / scale.
#[derive(Clone, Debug)]
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
    pub x0: f32,
    pub y0: f32,
    pub scale: f32,
}

impl Image {
    pub fn new(w: usize, h: usize, x0: f32, y0: f32, scale: f32) -> Image {
        Image { w, h, px: vec![0.0; w * h * 4], x0, y0, scale }
    }

    pub fn blank(&self) -> Image {
        Image::new(self.w, self.h, self.x0, self.y0, self.scale)
    }

    /// Straight RGBA8888 of a pixel.
    pub fn rgba(&self, x: usize, y: usize) -> u32 {
        let o = (y * self.w + x) * 4;
        let a = self.px[o + 3];
        if a <= 0.0 {
            return 0;
        }
        let ch = |v: f32| crate::model::java_round(clamp01(v) * 255.0) as u32;
        (ch(self.px[o] / a) << 24) | (ch(self.px[o + 1] / a) << 16) | (ch(self.px[o + 2] / a) << 8) | ch(a)
    }

    /// Premultiplied RGBA8, rounded.
    pub fn to_premultiplied_u8(&self) -> Vec<u8> {
        self.px.iter().map(|v| crate::model::java_round(v.clamp(0.0, 1.0) * 255.0) as u8).collect()
    }
}

pub fn clamp01(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

// ---------------------------------------------------------------- kernels

/// Largest blur sigma in pixels.
pub const MAX_SIGMA: f32 = 64.0;

/// Normalised Gaussian taps for sigma (pixels): radius ceil(3 sigma).
pub fn gaussian(sigma: f32) -> Vec<f32> {
    let sigma = sigma.min(MAX_SIGMA);
    let r = (3.0 * sigma).ceil() as i32;
    let mut k: Vec<f32> = (-r..=r).map(|i| (-((i * i) as f64) / (2.0 * sigma as f64 * sigma as f64)).exp() as f32).collect();
    let mut sum = 0f32;
    for v in &k {
        sum += v;
    }
    for v in &mut k {
        *v /= sum;
    }
    k
}

/// Separable Gaussian blur in place; edges read as transparent.
pub fn blur(im: &mut Image, sigma: f32) {
    if !(sigma > 0.05) {
        return;
    }
    let k = gaussian(sigma);
    let r = (k.len() / 2) as i32;
    let (w, h) = (im.w as i32, im.h as i32);
    let mut tmp = vec![0f32; im.px.len()];
    for y in 0..h {
        for x in 0..w {
            let mut a = [0f32; 4];
            for t in -r..=r {
                let xx = x + t;
                if xx < 0 || xx >= w {
                    continue;
                }
                let o = ((y * w + xx) * 4) as usize;
                let kw = k[(t + r) as usize];
                for c in 0..4 {
                    a[c] += im.px[o + c] * kw;
                }
            }
            let o = ((y * w + x) * 4) as usize;
            tmp[o..o + 4].copy_from_slice(&a);
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut a = [0f32; 4];
            for t in -r..=r {
                let yy = y + t;
                if yy < 0 || yy >= h {
                    continue;
                }
                let o = ((yy * w + x) * 4) as usize;
                let kw = k[(t + r) as usize];
                for c in 0..4 {
                    a[c] += tmp[o + c] * kw;
                }
            }
            let o = ((y * w + x) * 4) as usize;
            im.px[o..o + 4].copy_from_slice(&a);
        }
    }
}

/// Shadow or glow: the source's alpha shifted (pixels, y down), blurred,
/// coloured and strengthened, drawn under the source.
#[allow(clippy::too_many_arguments)]
pub fn under(src: &Image, dx: f32, dy: f32, sigma: f32, r: f32, g: f32, b: f32, a: f32, strength: f32) -> Image {
    let mut s = src.blank();
    let (w, h) = (src.w as i32, src.h as i32);
    let (ix, iy) = (crate::model::java_round(dx), crate::model::java_round(dy));
    for y in 0..h {
        let sy = y - iy;
        if sy < 0 || sy >= h {
            continue;
        }
        for x in 0..w {
            let sx = x - ix;
            if sx < 0 || sx >= w {
                continue;
            }
            s.px[((y * w + x) * 4 + 3) as usize] = src.px[((sy * w + sx) * 4 + 3) as usize];
        }
    }
    blur(&mut s, sigma);
    let mut out = src.blank();
    for i in 0..src.w * src.h {
        let sa = (s.px[i * 4 + 3] * strength).min(1.0) * a;
        let top = src.px[i * 4 + 3];
        out.px[i * 4] = src.px[i * 4] + r * sa * (1.0 - top);
        out.px[i * 4 + 1] = src.px[i * 4 + 1] + g * sa * (1.0 - top);
        out.px[i * 4 + 2] = src.px[i * 4 + 2] + b * sa * (1.0 - top);
        out.px[i * 4 + 3] = top + sa * (1.0 - top);
    }
    out
}

pub fn invert(im: &mut Image) {
    for p in im.px.chunks_exact_mut(4) {
        let a = p[3];
        p[0] = a - p[0];
        p[1] = a - p[1];
        p[2] = a - p[2];
    }
}

/// Hue (degrees about the luma axis) then saturation, as a row-major 3x3.
pub fn hue_saturation(hue_deg: f32, s: f32) -> [f32; 9] {
    let a = (hue_deg as f64).to_radians();
    let (c, sn) = (a.cos(), a.sin());
    let k = ((1.0 - c) / 3.0) as f32;
    let q = (sn / 3f64.sqrt()) as f32;
    let cf = c as f32;
    let h = [cf + k, k - q, k + q, k + q, cf + k, k - q, k - q, k + q, cf + k];
    let (lr, lg, lb) = (0.2126f32, 0.7152f32, 0.0722f32);
    let sm = [
        lr + (1.0 - lr) * s,
        lg - lg * s,
        lb - lb * s,
        lr - lr * s,
        lg + (1.0 - lg) * s,
        lb - lb * s,
        lr - lr * s,
        lg - lg * s,
        lb + (1.0 - lb) * s,
    ];
    let mut out = [0f32; 9];
    for r in 0..3 {
        for col in 0..3 {
            let mut v = 0f32;
            for j in 0..3 {
                v += sm[r * 3 + j] * h[j * 3 + col];
            }
            out[r * 3 + col] = v;
        }
    }
    out
}

pub fn adjust(im: &mut Image, hue: f32, saturation: f32, brightness: f32) {
    let m = hue_saturation(hue, saturation);
    for p in im.px.chunks_exact_mut(4) {
        let a = p[3];
        if a <= 0.0 {
            continue;
        }
        let (r, g, b) = (p[0] / a, p[1] / a, p[2] / a);
        let nr = m[0] * r + m[1] * g + m[2] * b + brightness;
        let ng = m[3] * r + m[4] * g + m[5] * b + brightness;
        let nb = m[6] * r + m[7] * g + m[8] * b + brightness;
        p[0] = clamp01(nr) * a;
        p[1] = clamp01(ng) * a;
        p[2] = clamp01(nb) * a;
    }
}

pub fn tint(im: &mut Image, r: f32, g: f32, b: f32, amount: f32) {
    for p in im.px.chunks_exact_mut(4) {
        let a = p[3];
        p[0] += (r * a - p[0]) * amount;
        p[1] += (g * a - p[1]) * amount;
        p[2] += (b * a - p[2]) * amount;
    }
}

/// a + (b - a) * t (the effect's mix).
pub fn lerp(a: &Image, b: Image, t: f32) -> Image {
    if t >= 1.0 {
        return b;
    }
    let mut out = a.blank();
    for i in 0..a.px.len() {
        out.px[i] = a.px[i] + (b.px[i] - a.px[i]) * t;
    }
    out
}

/// src over dst with an opacity (premultiplied).
pub fn over(dst: &mut Image, src: &Image, opacity: f32) {
    for i in 0..dst.w * dst.h {
        let sa = src.px[i * 4 + 3] * opacity;
        let k = 1.0 - sa;
        for c in 0..3 {
            dst.px[i * 4 + c] = src.px[i * 4 + c] * opacity + dst.px[i * 4 + c] * k;
        }
        dst.px[i * 4 + 3] = sa + dst.px[i * 4 + 3] * k;
    }
}

fn at(im: &Image, x: i32, y: i32) -> [f32; 4] {
    if x < 0 || y < 0 || x >= im.w as i32 || y >= im.h as i32 {
        return [0.0; 4];
    }
    let o = (y as usize * im.w + x as usize) * 4;
    [im.px[o], im.px[o + 1], im.px[o + 2], im.px[o + 3]]
}

/// Blocks of `size` view units anchored to the view origin; each takes the
/// pixel at its centre.
pub fn pixelate(im: &Image, size: f32) -> Image {
    if !(size * im.scale >= 1.5) {
        return im.clone();
    }
    let mut out = im.blank();
    let (scale, size) = (im.scale as f64, size as f64);
    for j in 0..im.h {
        for i in 0..im.w {
            let vx = im.x0 as f64 + (i as f64 + 0.5) / scale;
            let vy = im.y0 as f64 - (j as f64 + 0.5) / scale;
            let cx = ((vx / size).floor() + 0.5) * size;
            let cy = ((vy / size).floor() + 0.5) * size;
            let si = ((cx - im.x0 as f64) * scale).floor() as i32;
            let sj = ((im.y0 as f64 - cy) * scale).floor() as i32;
            let c = at(im, si, sj);
            let o = (j * im.w + i) * 4;
            out.px[o..o + 4].copy_from_slice(&c);
        }
    }
    out
}

/// Outline taps (pixels, y up): 16 directions at r and r / 2.
pub fn ring_offsets(r: f32) -> [i32; 64] {
    let mut o = [0i32; 64];
    for k in 0..32 {
        let rad = if k < 16 { r as f64 } else { r as f64 / 2.0 };
        let a = 2.0 * std::f64::consts::PI * (k % 16) as f64 / 16.0;
        o[k * 2] = (a.cos() * rad + 0.5).floor() as i32;
        o[k * 2 + 1] = (a.sin() * rad + 0.5).floor() as i32;
    }
    o
}

pub fn outline(src: &Image, r: f32, cr: f32, cg: f32, cb: f32, ca: f32) -> Image {
    let mut out = src.blank();
    let off = ring_offsets(r);
    let (w, h) = (src.w as i32, src.h as i32);
    for y in 0..h {
        for x in 0..w {
            let o = ((y * w + x) * 4) as usize;
            let mut a = src.px[o + 3];
            for k in 0..32 {
                let (sx, sy) = (x + off[k * 2], y - off[k * 2 + 1]);
                if sx < 0 || sy < 0 || sx >= w || sy >= h {
                    continue;
                }
                a = a.max(src.px[((sy * w + sx) * 4 + 3) as usize]);
            }
            let (sa, top) = (a * ca, src.px[o + 3]);
            out.px[o] = src.px[o] + cr * sa * (1.0 - top);
            out.px[o + 1] = src.px[o + 1] + cg * sa * (1.0 - top);
            out.px[o + 2] = src.px[o + 2] + cb * sa * (1.0 - top);
            out.px[o + 3] = top + sa * (1.0 - top);
        }
    }
    out
}

/// Motion blur taps (pixels, y up): 16 spread over distance, centred.
pub fn smear_offsets(angle_deg: f32, distance: f32) -> [i32; 32] {
    let mut o = [0i32; 32];
    let a = (angle_deg as f64).to_radians();
    for k in 0..16 {
        let d = distance as f64 * (k as f64 / 15.0 - 0.5);
        o[k * 2] = (a.cos() * d + 0.5).floor() as i32;
        o[k * 2 + 1] = (a.sin() * d + 0.5).floor() as i32;
    }
    o
}

pub fn smear(src: &Image, angle_deg: f32, distance: f32) -> Image {
    let mut out = src.blank();
    let off = smear_offsets(angle_deg, distance);
    for y in 0..src.h as i32 {
        for x in 0..src.w as i32 {
            let mut acc = [0f32; 4];
            for k in 0..16 {
                let c = at(src, x + off[k * 2], y - off[k * 2 + 1]);
                for i in 0..4 {
                    acc[i] += c[i];
                }
            }
            let o = (y as usize * src.w + x as usize) * 4;
            for i in 0..4 {
                out.px[o + i] = acc[i] / 16.0;
            }
        }
    }
    out
}

// ---------------------------------------------------------------- effect types

pub const BLUR: &str = "blur.gaussian";
pub const GLOW: &str = "glow";
pub const SHADOW: &str = "shadow.drop";
pub const INVERT: &str = "invert";
pub const ADJUST: &str = "color.adjust";
pub const TINT: &str = "tint";
pub const PIXELATE: &str = "pixelate";
pub const OUTLINE: &str = "outline";
pub const MOTION: &str = "blur.motion";
pub const ALL: [&str; 9] = [BLUR, GLOW, SHADOW, INVERT, ADJUST, TINT, PIXELATE, OUTLINE, MOTION];
/// Largest radius or offset, project pixels.
pub const MAX_LENGTH: f32 = 400.0;

pub fn known(t: &str) -> bool {
    ALL.contains(&t)
}

/// Blur sigma for a radius (the radius covers about two sigmas).
pub fn sigma(radius: f32) -> f32 {
    radius / 2.0
}

fn clampf(v: f32, min: f32, max: f32, def: f32) -> f32 {
    if v.is_nan() {
        return def;
    }
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// A float parameter (default when missing, not a number, or NaN), clamped.
pub fn f(e: &ResolvedEffect, name: &str, def: f32, min: f32, max: f32) -> f32 {
    let v = match e.params.iter().find(|(k, _)| k == name).map(|(_, v)| v) {
        Some(ParamValue::F(x)) => *x,
        Some(ParamValue::D(x)) => *x as f32,
        _ => def,
    };
    clampf(v, min, max, def)
}

pub fn c(e: &ResolvedEffect, name: &str, def: Color) -> Color {
    match e.params.iter().find(|(k, _)| k == name).map(|(_, v)| v) {
        Some(ParamValue::C(c)) => *c,
        _ => def,
    }
}

/// View units the effect can reach beyond the content.
pub fn padding(e: &ResolvedEffect) -> f32 {
    match e.ty.as_str() {
        BLUR => 1.5 * f(e, "radius", 6.0, 0.0, MAX_LENGTH),
        GLOW => 1.5 * f(e, "radius", 8.0, 0.0, MAX_LENGTH),
        SHADOW => {
            1.5 * f(e, "radius", 6.0, 0.0, MAX_LENGTH)
                + f(e, "dx", 8.0, -MAX_LENGTH, MAX_LENGTH).abs().max(f(e, "dy", -8.0, -MAX_LENGTH, MAX_LENGTH).abs())
        }
        OUTLINE => f(e, "width", 4.0, 0.0, 64.0) + 1.0,
        MOTION => f(e, "distance", 30.0, 0.0, MAX_LENGTH) / 2.0 + 1.0,
        _ => 0.0,
    }
}

/// The effect's mix, clamped (NaN = 1).
pub fn mix(e: &ResolvedEffect) -> f32 {
    clampf(e.mix, 0.0, 1.0, 1.0)
}

pub const SHADOW_COLOR: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.6 };
pub const TINT_COLOR: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

/// Runs one effect (mix applied).
pub fn apply(e: &ResolvedEffect, input: Image) -> Image {
    let m = mix(e);
    if m <= 0.0 || !known(&e.ty) {
        return input;
    }
    let s = input.scale;
    let out = match e.ty.as_str() {
        BLUR => {
            let mut o = input.clone();
            blur(&mut o, sigma(f(e, "radius", 6.0, 0.0, MAX_LENGTH)) * s);
            o
        }
        GLOW => {
            let c = c(e, "color", Color::WHITE);
            under(&input, 0.0, 0.0, sigma(f(e, "radius", 8.0, 0.0, MAX_LENGTH)) * s, c.r, c.g, c.b, c.a, f(e, "strength", 1.0, 0.0, 4.0))
        }
        SHADOW => {
            let c = c(e, "color", SHADOW_COLOR);
            let dx = f(e, "dx", 8.0, -MAX_LENGTH, MAX_LENGTH) * s;
            let dy = -f(e, "dy", -8.0, -MAX_LENGTH, MAX_LENGTH) * s;
            under(&input, dx, dy, sigma(f(e, "radius", 6.0, 0.0, MAX_LENGTH)) * s, c.r, c.g, c.b, c.a, 1.0)
        }
        INVERT => {
            let mut o = input.clone();
            invert(&mut o);
            o
        }
        ADJUST => {
            let mut o = input.clone();
            adjust(&mut o, f(e, "hue", 0.0, -3600.0, 3600.0), f(e, "saturation", 1.0, 0.0, 4.0), f(e, "brightness", 0.0, -1.0, 1.0));
            o
        }
        PIXELATE => pixelate(&input, f(e, "size", 8.0, 1.0, MAX_LENGTH)),
        OUTLINE => {
            let c = c(e, "color", Color::BLACK);
            outline(&input, f(e, "width", 4.0, 0.0, 64.0) * s, c.r, c.g, c.b, c.a)
        }
        MOTION => smear(&input, f(e, "angle", 0.0, -3600.0, 3600.0), f(e, "distance", 30.0, 0.0, MAX_LENGTH) * s),
        _ => {
            let c = c(e, "color", TINT_COLOR);
            let mut o = input.clone();
            tint(&mut o, c.r, c.g, c.b, f(e, "amount", 0.5, 0.0, 1.0) * c.a);
            o
        }
    };
    lerp(&input, out, m)
}

// ---------------------------------------------------------------- compositor

/// Largest buffer the reference renderer allocates.
pub const MAX_PIXELS: usize = 4096 * 4096;

/// Renders a layered draw list over target.
pub fn render(dl: &DrawList, target: &mut Image) {
    if dl.isolates.is_empty() || target.w * target.h > MAX_PIXELS {
        return;
    }
    let mut layer = target.blank();
    draw(dl, 0, &mut layer);
    over(target, &layer, 1.0);
}

fn draw(dl: &DrawList, iso: usize, canvas: &mut Image) {
    for item in &dl.isolates[iso].items {
        match *item {
            IsoItem::Batch(b) => fill(&dl.batches[b], canvas),
            IsoItem::Isolate(ci) => {
                let child = &dl.isolates[ci];
                if child.adjustment {
                    let mut img = canvas.clone();
                    for e in &child.effects {
                        img = apply(e, img);
                    }
                    let mixed = lerp(canvas, img, child.opacity);
                    canvas.px.copy_from_slice(&mixed.px);
                    continue;
                }
                let mut buf = canvas.blank();
                draw(dl, ci, &mut buf);
                for e in &child.effects {
                    buf = apply(e, buf);
                }
                composite(canvas, &buf, child.opacity, child.blend);
            }
        }
    }
}

/// Premultiplied composite with the four blend modes.
pub fn composite(dst: &mut Image, src: &Image, op: f32, blend: Blend) {
    if blend == Blend::Normal {
        over(dst, src, op);
        return;
    }
    for i in 0..dst.w * dst.h {
        let sa = src.px[i * 4 + 3] * op;
        for c in 0..3 {
            let (s, d) = (src.px[i * 4 + c] * op, dst.px[i * 4 + c]);
            let v = match blend {
                Blend::Add => d + s,
                Blend::Multiply => s * d + d * (1.0 - sa),
                _ => s + d - s * d,
            };
            dst.px[i * 4 + c] = v.min(1.0);
        }
        let da = dst.px[i * 4 + 3];
        dst.px[i * 4 + 3] = if blend == Blend::Add { (da + sa).min(1.0) } else { sa + da * (1.0 - sa) };
    }
}

fn ch(rgba: u32, shift: u32) -> f32 {
    ((rgba >> shift) & 0xff) as f32 / 255.0
}

/// Triangles sampled at pixel centres (one sample), vertex colours interpolated.
pub fn fill(b: &Batch, im: &mut Image) {
    let n = b.vertex_count() / 3 * 3;
    let s = im.scale;
    let mut t = 0;
    while t < n {
        let (ax, ay) = ((b.xy[t * 2] - im.x0) * s, (im.y0 - b.xy[t * 2 + 1]) * s);
        let (bx, by) = ((b.xy[t * 2 + 2] - im.x0) * s, (im.y0 - b.xy[t * 2 + 3]) * s);
        let (cx, cy) = ((b.xy[t * 2 + 4] - im.x0) * s, (im.y0 - b.xy[t * 2 + 5]) * s);
        let area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
        let (ca, cb, cc) = (b.rgba[t], b.rgba[t + 1], b.rgba[t + 2]);
        t += 3;
        if !(area.abs() > 1e-9) {
            continue;
        }
        let x0 = 0.max(ax.min(bx.min(cx)).floor() as i32);
        let x1 = (im.w as i32 - 1).min(ax.max(bx.max(cx)).ceil() as i32);
        let y0 = 0.max(ay.min(by.min(cy)).floor() as i32);
        let y1 = (im.h as i32 - 1).min(ay.max(by.max(cy)).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let px = x as f32 + 0.5 + 1.3e-4;
                let py = y as f32 + 0.5 + 0.7e-4;
                let w0 = ((bx - px) * (cy - py) - (by - py) * (cx - px)) / area;
                let w1 = ((cx - px) * (ay - py) - (cy - py) * (ax - px)) / area;
                let w2 = 1.0 - w0 - w1;
                if !(w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) {
                    continue;
                }
                let r = ch(ca, 24) * w0 + ch(cb, 24) * w1 + ch(cc, 24) * w2;
                let g = ch(ca, 16) * w0 + ch(cb, 16) * w1 + ch(cc, 16) * w2;
                let bl = ch(ca, 8) * w0 + ch(cb, 8) * w1 + ch(cc, 8) * w2;
                let a = (ch(ca, 0) * w0 + ch(cb, 0) * w1 + ch(cc, 0) * w2) * 4.0 / 4.0;
                blend_pixel(im, (y as usize * im.w + x as usize) * 4, r, g, bl, a, b.blend());
            }
        }
    }
}

fn blend_pixel(im: &mut Image, o: usize, r: f32, g: f32, b: f32, a: f32, mode: Blend) {
    let p = &mut im.px;
    let (sr, sg, sb) = (r * a, g * a, b * a);
    match mode {
        Blend::Add => {
            p[o] = (p[o] + sr).min(1.0);
            p[o + 1] = (p[o + 1] + sg).min(1.0);
            p[o + 2] = (p[o + 2] + sb).min(1.0);
            p[o + 3] = (p[o + 3] + a).min(1.0);
        }
        Blend::Multiply => {
            let da = p[o + 3];
            p[o] = sr * p[o] + p[o] * (1.0 - a);
            p[o + 1] = sg * p[o + 1] + p[o + 1] * (1.0 - a);
            p[o + 2] = sb * p[o + 2] + p[o + 2] * (1.0 - a);
            p[o + 3] = a + da * (1.0 - a);
        }
        Blend::Screen => {
            p[o] = sr + p[o] - sr * p[o];
            p[o + 1] = sg + p[o + 1] - sg * p[o + 1];
            p[o + 2] = sb + p[o + 2] - sb * p[o + 2];
            p[o + 3] = a + p[o + 3] * (1.0 - a);
        }
        Blend::Normal => {
            p[o] = sr + p[o] * (1.0 - a);
            p[o + 1] = sg + p[o + 1] * (1.0 - a);
            p[o + 2] = sb + p[o + 2] * (1.0 - a);
            p[o + 3] = a + p[o + 3] * (1.0 - a);
        }
    }
}
