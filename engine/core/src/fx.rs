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

/// Bilinear sample in pixel-index space (texel `(i, j)` sits at `(i, j)`),
/// out-of-bounds reads transparent. Matches `sample_bilinear` in `fx.wgsl`.
fn sample_bilinear(im: &Image, x: f32, y: f32) -> [f32; 4] {
    let fx = x.floor();
    let fy = y.floor();
    let tx = x - fx;
    let ty = y - fy;
    let ix = fx as i32;
    let iy = fy as i32;
    let c00 = at(im, ix, iy);
    let c10 = at(im, ix + 1, iy);
    let c01 = at(im, ix, iy + 1);
    let c11 = at(im, ix + 1, iy + 1);
    let mut out = [0f32; 4];
    for c in 0..4 {
        let top = c00[c] + (c10[c] - c00[c]) * tx;
        let bot = c01[c] + (c11[c] - c01[c]) * tx;
        out[c] = top + (bot - top) * ty;
    }
    out
}

/// Deterministic 32-bit integer hash (identical bitwise math to `fx.wgsl`).
#[inline]
fn hash_u32(mut x: u32) -> u32 {
    x = (x ^ 61) ^ (x >> 16);
    x = x.wrapping_add(x << 3);
    x = x ^ (x >> 4);
    x = x.wrapping_mul(0x27d4_eb2d);
    x = x ^ (x >> 15);
    x
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp01((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Chromatic aberration: shifts R forward and B backward along `angle_deg` by
/// `amount_px` pixels (y up in angle space, y down in pixel space).
pub fn chromatic(src: &Image, amount_px: f32, angle_deg: f32) -> Image {
    if !(amount_px > 0.01) {
        return src.clone();
    }
    let rad = (angle_deg as f64).to_radians();
    let dx = (rad.cos() * amount_px as f64) as f32;
    let dy = -(rad.sin() * amount_px as f64) as f32;
    let mut out = src.blank();
    for y in 0..src.h {
        let fy = y as f32;
        for x in 0..src.w {
            let fx = x as f32;
            let cr = sample_bilinear(src, fx + dx, fy + dy);
            let cg = at(src, x as i32, y as i32);
            let cb = sample_bilinear(src, fx - dx, fy - dy);
            let a = cr[3].max(cg[3].max(cb[3]));
            let o = (y * src.w + x) * 4;
            out.px[o] = cr[0];
            out.px[o + 1] = cg[1];
            out.px[o + 2] = cb[2];
            out.px[o + 3] = a;
        }
    }
    out
}

/// Cinematic vignette. On an adjustment layer (`full_frame = true`) it shades
/// transparent canvas too so the stage background darkens cleanly; on a normal
/// layer it masks to `src.a` so no bounding-box rectangle appears.
/// `frame_rect` is `(x0, y0, w, h)` in pixels (camera shot rect or full image).
pub fn vignette(src: &Image, amount: f32, softness: f32, cr: f32, cg: f32, cb: f32, ca: f32, full_frame: bool, frame_rect: [f32; 4]) -> Image {
    if !(amount > 0.0) || !(ca > 0.0) {
        return src.clone();
    }
    let mut out = src.blank();
    let (rx, ry, rw, rh) = (frame_rect[0], frame_rect[1], frame_rect[2].max(1.0), frame_rect[3].max(1.0));
    let inner = 0.95 - softness * 0.85;
    let outer = 1.05;
    for y in 0..src.h {
        let v = (y as f32 + 0.5 - ry) / rh * 2.0 - 1.0;
        for x in 0..src.w {
            let u = (x as f32 + 0.5 - rx) / rw * 2.0 - 1.0;
            let dist = (u * u + v * v).sqrt() * 0.70710678;
            let t = smoothstep(inner, outer, dist);
            let va = t * amount * ca;
            let o = (y * src.w + x) * 4;
            let (sr, sg, sb, sa) = (src.px[o], src.px[o + 1], src.px[o + 2], src.px[o + 3]);
            if full_frame {
                let k = 1.0 - va;
                out.px[o] = cr * va + sr * k;
                out.px[o + 1] = cg * va + sg * k;
                out.px[o + 2] = cb * va + sb * k;
                out.px[o + 3] = va + sa * k;
            } else {
                out.px[o] = sr + (cr * sa - sr) * va;
                out.px[o + 1] = sg + (cg * sa - sg) * va;
                out.px[o + 2] = sb + (cb * sa - sb) * va;
                out.px[o + 3] = sa;
            }
        }
    }
    out
}

/// Sinusoidal wave distortion anchored in view units; keyframe `phase_deg` for
/// continuous looping ripples.
pub fn wave(src: &Image, amplitude: f32, frequency: f32, phase_deg: f32, angle_deg: f32) -> Image {
    let amp_px = amplitude * src.scale;
    if !(amp_px > 0.01) {
        return src.clone();
    }
    let rad = (angle_deg as f64).to_radians();
    let cos_a = rad.cos() as f32;
    let sin_a = rad.sin() as f32;
    let k_freq = frequency * std::f32::consts::TAU / 400.0;
    let phase_rad = (phase_deg as f64).to_radians() as f32;
    let inv_s = 1.0 / src.scale.max(1e-6);
    let mut out = src.blank();
    for y in 0..src.h {
        let vy = src.y0 - (y as f32 + 0.5) * inv_s;
        for x in 0..src.w {
            let vx = src.x0 + (x as f32 + 0.5) * inv_s;
            let along = vx * (-sin_a) + vy * cos_a;
            let off = (along * k_freq + phase_rad).sin() * amp_px;
            let c = sample_bilinear(src, x as f32 + cos_a * off, y as f32 - sin_a * off);
            let o = (y * src.w + x) * 4;
            out.px[o..o + 4].copy_from_slice(&c);
        }
    }
    out
}

/// Film grain / procedural noise (deterministic integer hash; matches `fx.wgsl`).
pub fn grain(src: &Image, amount: f32, size: f32, seed: f32, full_frame: bool) -> Image {
    if !(amount > 0.0) {
        return src.clone();
    }
    let size = size.max(1.0);
    let sd = (crate::model::java_round(seed)) as u32;
    let sd_h = hash_u32(sd.wrapping_mul(0x9e37_79b9).wrapping_add(1));
    let mut out = src.blank();
    for y in 0..src.h {
        let gy = ((y as f32 / size).floor() as i32) as u32;
        let gy_h = hash_u32(gy ^ sd_h);
        for x in 0..src.w {
            let gx = ((x as f32 / size).floor() as i32) as u32;
            let h = hash_u32(gx ^ gy_h);
            let n = (h & 0xffff) as f32 / 32767.5 - 1.0;
            let o = (y * src.w + x) * 4;
            let (sr, sg, sb, sa) = (src.px[o], src.px[o + 1], src.px[o + 2], src.px[o + 3]);
            let (mut rr, mut rg, mut rb) = (0.0f32, 0.0f32, 0.0f32);
            if sa > 0.0 {
                let d = n * amount * 0.5;
                rr = clamp01(sr / sa + d) * sa;
                rg = clamp01(sg / sa + d) * sa;
                rb = clamp01(sb / sa + d) * sa;
            }
            if full_frame {
                let ga = n.abs() * amount * 0.22 * (1.0 - sa);
                let gc = if n > 0.0 { ga } else { 0.0 };
                out.px[o] = rr + gc;
                out.px[o + 1] = rg + gc;
                out.px[o + 2] = rb + gc;
                out.px[o + 3] = sa + ga;
            } else {
                out.px[o] = rr;
                out.px[o + 1] = rg;
                out.px[o + 2] = rb;
                out.px[o + 3] = sa;
            }
        }
    }
    out
}

/// Exposure (EV stops), contrast (around 0.5), and midtone gamma.
pub fn grade(im: &mut Image, exposure: f32, contrast: f32, gamma: f32) {
    let exp_mul = 2.0f32.powf(exposure);
    let inv_g = 1.0 / gamma.max(0.1);
    for p in im.px.chunks_exact_mut(4) {
        let a = p[3];
        if a <= 0.0 {
            continue;
        }
        for c in 0..3 {
            let mut s = clamp01(p[c] / a);
            s *= exp_mul;
            s = (s - 0.5) * contrast + 0.5;
            s = clamp01(s);
            s = if s <= 0.0 { 0.0 } else { s.powf(inv_g) };
            p[c] = clamp01(s) * a;
        }
    }
}

/// CRT scanlines with optional horizontal glitch slice displacement & RGB split.
pub fn scanlines(src: &Image, amount: f32, lines: f32, offset_px: f32, phase: f32, full_frame: bool, frame_rect: [f32; 4]) -> Image {
    if !(amount > 0.0) && !(offset_px > 0.0) {
        return src.clone();
    }
    let ry = frame_rect[1];
    let rh = frame_rect[3].max(1.0);
    let band_h = (rh / (lines * 0.25).max(4.0)).max(2.0);
    let sd = (crate::model::java_round(phase)) as u32;
    let sd_h = hash_u32(sd.wrapping_mul(0x9e37_79b9).wrapping_add(7));
    let mut out = src.blank();
    for y in 0..src.h {
        let local_y = y as f32 - ry;
        let band = ((local_y / band_h).floor() as i32) as u32;
        let hb = hash_u32(band ^ sd_h);
        let u = (hb & 0xffff) as f32 / 65535.0;
        let shift_px = if offset_px > 0.0 && amount > 0.0 && u > (1.0 - amount * 0.55) {
            let dir = ((hb >> 16) & 0xffff) as f32 / 32767.5 - 1.0;
            dir * offset_px
        } else {
            0.0
        };
        let y_norm = (local_y + 0.5) / rh;
        let scan = 0.5 + 0.5 * ((y_norm * lines + phase * 0.25) * std::f32::consts::TAU).sin();
        let darken = (1.0 - scan) * amount * 0.65;
        let keep = 1.0 - darken;
        for x in 0..src.w {
            let c = if shift_px.abs() > 0.01 {
                let fx = x as f32;
                let fy = y as f32;
                let cr = sample_bilinear(src, fx + shift_px * 1.25, fy);
                let cg = sample_bilinear(src, fx + shift_px, fy);
                let cb = sample_bilinear(src, fx + shift_px * 0.75, fy);
                [cr[0], cg[1], cb[2], cr[3].max(cg[3].max(cb[3]))]
            } else {
                at(src, x as i32, y as i32)
            };
            let o = (y * src.w + x) * 4;
            out.px[o] = c[0] * keep;
            out.px[o + 1] = c[1] * keep;
            out.px[o + 2] = c[2] * keep;
            out.px[o + 3] = if full_frame { c[3] + darken * 0.45 * (1.0 - c[3]) } else { c[3] };
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
pub const CHROMATIC: &str = "chromatic.aberration";
pub const VIGNETTE: &str = "vignette";
pub const WAVE: &str = "distort.wave";
pub const GRAIN: &str = "noise.grain";
pub const GRADE: &str = "color.grade";
pub const SCANLINES: &str = "stylize.scanlines";
pub const ALL: [&str; 15] = [BLUR, GLOW, SHADOW, INVERT, ADJUST, TINT, PIXELATE, OUTLINE, MOTION, CHROMATIC, VIGNETTE, WAVE, GRAIN, GRADE, SCANLINES];
/// Largest radius or offset, project pixels.
pub const MAX_LENGTH: f32 = 400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    Float,
    Angle,
    Color,
}

#[derive(Clone, Copy, Debug)]
pub struct ParamMeta {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub default_f: f32,
    pub default_c: Color,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl ParamMeta {
    const fn float(name: &'static str, label: &'static str, default_f: f32, min: f32, max: f32, step: f32) -> ParamMeta {
        ParamMeta { name, label, kind: ParamKind::Float, default_f, default_c: Color::WHITE, min, max, step }
    }
    const fn angle(name: &'static str, label: &'static str, default_f: f32, min: f32, max: f32, step: f32) -> ParamMeta {
        ParamMeta { name, label, kind: ParamKind::Angle, default_f, default_c: Color::WHITE, min, max, step }
    }
    const fn color(name: &'static str, label: &'static str, default_c: Color) -> ParamMeta {
        ParamMeta { name, label, kind: ParamKind::Color, default_f: 0.0, default_c, min: 0.0, max: 1.0, step: 0.0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EffectMeta {
    pub ty: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub params: &'static [ParamMeta],
}

pub const CATEGORIES: [&str; 5] = ["Blur", "Light & Glow", "Color", "Distort", "Stylize"];

const P_BLUR: &[ParamMeta] = &[ParamMeta::float("radius", "Radius", 6.0, 0.0, MAX_LENGTH, 0.5)];
const P_MOTION: &[ParamMeta] = &[
    ParamMeta::float("distance", "Distance", 30.0, 0.0, MAX_LENGTH, 2.0),
    ParamMeta::angle("angle", "Angle", 0.0, -3600.0, 3600.0, 5.0),
];
const P_GLOW: &[ParamMeta] = &[
    ParamMeta::color("color", "Color", Color::WHITE),
    ParamMeta::float("radius", "Radius", 8.0, 0.0, MAX_LENGTH, 1.0),
    ParamMeta::float("strength", "Strength", 1.0, 0.0, 4.0, 0.1),
];
const P_SHADOW: &[ParamMeta] = &[
    ParamMeta::color("color", "Color", SHADOW_COLOR),
    ParamMeta::float("dx", "Offset X", 8.0, -MAX_LENGTH, MAX_LENGTH, 1.0),
    ParamMeta::float("dy", "Offset Y", -8.0, -MAX_LENGTH, MAX_LENGTH, 1.0),
    ParamMeta::float("radius", "Blur", 6.0, 0.0, MAX_LENGTH, 0.5),
];
const P_VIGNETTE: &[ParamMeta] = &[
    ParamMeta::float("amount", "Amount", 0.65, 0.0, 1.0, 0.05),
    ParamMeta::float("softness", "Softness", 0.55, 0.05, 1.0, 0.05),
    ParamMeta::color("color", "Color", Color::BLACK),
];
const P_ADJUST: &[ParamMeta] = &[
    ParamMeta::angle("hue", "Hue", 0.0, -3600.0, 3600.0, 5.0),
    ParamMeta::float("saturation", "Saturation", 1.0, 0.0, 4.0, 0.05),
    ParamMeta::float("brightness", "Brightness", 0.0, -1.0, 1.0, 0.05),
];
const P_GRADE: &[ParamMeta] = &[
    ParamMeta::float("exposure", "Exposure", 0.0, -4.0, 4.0, 0.1),
    ParamMeta::float("contrast", "Contrast", 1.0, 0.0, 4.0, 0.05),
    ParamMeta::float("gamma", "Gamma", 1.0, 0.1, 4.0, 0.05),
];
const P_TINT: &[ParamMeta] = &[
    ParamMeta::color("color", "Color", TINT_COLOR),
    ParamMeta::float("amount", "Amount", 0.5, 0.0, 1.0, 0.05),
];
const P_INVERT: &[ParamMeta] = &[];
const P_CHROMATIC: &[ParamMeta] = &[
    ParamMeta::float("amount", "Amount", 8.0, 0.0, 120.0, 1.0),
    ParamMeta::angle("angle", "Angle", 0.0, -3600.0, 3600.0, 5.0),
];
const P_WAVE: &[ParamMeta] = &[
    ParamMeta::float("amplitude", "Amplitude", 12.0, 0.0, 160.0, 1.0),
    ParamMeta::float("frequency", "Frequency", 3.0, 0.1, 40.0, 0.25),
    ParamMeta::angle("phase", "Phase", 0.0, -3600.0, 3600.0, 15.0),
    ParamMeta::angle("angle", "Angle", 0.0, -3600.0, 3600.0, 5.0),
];
const P_OUTLINE: &[ParamMeta] = &[
    ParamMeta::color("color", "Color", Color::BLACK),
    ParamMeta::float("width", "Width", 4.0, 0.0, 64.0, 1.0),
];
const P_PIXELATE: &[ParamMeta] = &[ParamMeta::float("size", "Size", 8.0, 1.0, MAX_LENGTH, 1.0)];
const P_GRAIN: &[ParamMeta] = &[
    ParamMeta::float("amount", "Amount", 0.25, 0.0, 1.0, 0.05),
    ParamMeta::float("size", "Size", 2.0, 1.0, 24.0, 0.5),
    ParamMeta::float("seed", "Seed", 0.0, -10000.0, 10000.0, 1.0),
];
const P_SCANLINES: &[ParamMeta] = &[
    ParamMeta::float("amount", "Amount", 0.45, 0.0, 1.0, 0.05),
    ParamMeta::float("lines", "Lines", 80.0, 8.0, 400.0, 4.0),
    ParamMeta::float("offset", "Glitch", 6.0, 0.0, 80.0, 1.0),
    ParamMeta::float("phase", "Phase", 0.0, -10000.0, 10000.0, 1.0),
];

const CATALOG: [EffectMeta; 15] = [
    EffectMeta { ty: BLUR, label: "Gaussian blur", category: "Blur", params: P_BLUR },
    EffectMeta { ty: MOTION, label: "Motion blur", category: "Blur", params: P_MOTION },
    EffectMeta { ty: GLOW, label: "Glow", category: "Light & Glow", params: P_GLOW },
    EffectMeta { ty: SHADOW, label: "Drop shadow", category: "Light & Glow", params: P_SHADOW },
    EffectMeta { ty: VIGNETTE, label: "Vignette", category: "Light & Glow", params: P_VIGNETTE },
    EffectMeta { ty: ADJUST, label: "Hue & saturation", category: "Color", params: P_ADJUST },
    EffectMeta { ty: GRADE, label: "Exposure & contrast", category: "Color", params: P_GRADE },
    EffectMeta { ty: TINT, label: "Tint", category: "Color", params: P_TINT },
    EffectMeta { ty: INVERT, label: "Invert", category: "Color", params: P_INVERT },
    EffectMeta { ty: CHROMATIC, label: "Chromatic aberration", category: "Distort", params: P_CHROMATIC },
    EffectMeta { ty: WAVE, label: "Wave distort", category: "Distort", params: P_WAVE },
    EffectMeta { ty: OUTLINE, label: "Outline", category: "Stylize", params: P_OUTLINE },
    EffectMeta { ty: PIXELATE, label: "Pixelate", category: "Stylize", params: P_PIXELATE },
    EffectMeta { ty: GRAIN, label: "Film grain", category: "Stylize", params: P_GRAIN },
    EffectMeta { ty: SCANLINES, label: "Scanlines & glitch", category: "Stylize", params: P_SCANLINES },
];

pub fn catalog() -> &'static [EffectMeta] {
    &CATALOG
}

pub fn meta(ty: &str) -> Option<&'static EffectMeta> {
    CATALOG.iter().find(|m| m.ty == ty)
}

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
        Some(ParamValue::C(col)) => Color {
            r: clampf(col.r, 0.0, 1.0, def.r),
            g: clampf(col.g, 0.0, 1.0, def.g),
            b: clampf(col.b, 0.0, 1.0, def.b),
            a: clampf(col.a, 0.0, 1.0, def.a),
        },
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
        CHROMATIC => f(e, "amount", 8.0, 0.0, 120.0) + 1.0,
        WAVE => f(e, "amplitude", 12.0, 0.0, 160.0) + 1.0,
        SCANLINES => f(e, "offset", 6.0, 0.0, 80.0) + 1.0,
        _ => 0.0,
    }
}

/// The effect's mix, clamped (NaN = 1).
pub fn mix(e: &ResolvedEffect) -> f32 {
    clampf(e.mix, 0.0, 1.0, 1.0)
}

pub const SHADOW_COLOR: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.6 };
pub const TINT_COLOR: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

/// Runs one effect on a normal layer (mix applied).
pub fn apply(e: &ResolvedEffect, input: Image) -> Image {
    let rect = [0.0, 0.0, input.w as f32, input.h as f32];
    apply_mode(e, input, false, rect)
}

/// Runs one effect on an adjustment layer (full-frame effects like Vignette,
/// Film grain, and Scanlines cover the canvas too).
pub fn apply_adjustment(e: &ResolvedEffect, input: Image) -> Image {
    let rect = [0.0, 0.0, input.w as f32, input.h as f32];
    apply_mode(e, input, true, rect)
}

fn apply_mode(e: &ResolvedEffect, input: Image, full_frame: bool, frame_rect: [f32; 4]) -> Image {
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
        TINT => {
            let c = c(e, "color", TINT_COLOR);
            let mut o = input.clone();
            tint(&mut o, c.r, c.g, c.b, f(e, "amount", 0.5, 0.0, 1.0) * c.a);
            o
        }
        PIXELATE => pixelate(&input, f(e, "size", 8.0, 1.0, MAX_LENGTH)),
        OUTLINE => {
            let c = c(e, "color", Color::BLACK);
            outline(&input, f(e, "width", 4.0, 0.0, 64.0) * s, c.r, c.g, c.b, c.a)
        }
        MOTION => smear(&input, f(e, "angle", 0.0, -3600.0, 3600.0), f(e, "distance", 30.0, 0.0, MAX_LENGTH) * s),
        CHROMATIC => chromatic(&input, f(e, "amount", 8.0, 0.0, 120.0) * s, f(e, "angle", 0.0, -3600.0, 3600.0)),
        VIGNETTE => {
            let c = c(e, "color", Color::BLACK);
            vignette(&input, f(e, "amount", 0.65, 0.0, 1.0), f(e, "softness", 0.55, 0.05, 1.0), c.r, c.g, c.b, c.a, full_frame, frame_rect)
        }
        WAVE => wave(
            &input,
            f(e, "amplitude", 12.0, 0.0, 160.0),
            f(e, "frequency", 3.0, 0.1, 40.0),
            f(e, "phase", 0.0, -3600.0, 3600.0),
            f(e, "angle", 0.0, -3600.0, 3600.0),
        ),
        GRAIN => grain(&input, f(e, "amount", 0.25, 0.0, 1.0), f(e, "size", 2.0, 1.0, 24.0), f(e, "seed", 0.0, -10000.0, 10000.0), full_frame),
        GRADE => {
            let mut o = input.clone();
            grade(&mut o, f(e, "exposure", 0.0, -4.0, 4.0), f(e, "contrast", 1.0, 0.0, 4.0), f(e, "gamma", 1.0, 0.1, 4.0));
            o
        }
        SCANLINES => scanlines(
            &input,
            f(e, "amount", 0.45, 0.0, 1.0),
            f(e, "lines", 80.0, 8.0, 400.0),
            f(e, "offset", 6.0, 0.0, 80.0) * s,
            f(e, "phase", 0.0, -10000.0, 10000.0),
            full_frame,
            frame_rect,
        ),
        _ => input.clone(),
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
                    let (rx0, ry0, rx1, ry1) = if !child.empty() {
                        let x0 = ((child.min_x - canvas.x0) * canvas.scale).floor().clamp(0.0, canvas.w as f32) as usize;
                        let x1 = ((child.max_x - canvas.x0) * canvas.scale).ceil().clamp(0.0, canvas.w as f32) as usize;
                        let y0 = ((canvas.y0 - child.max_y) * canvas.scale).floor().clamp(0.0, canvas.h as f32) as usize;
                        let y1 = ((canvas.y0 - child.min_y) * canvas.scale).ceil().clamp(0.0, canvas.h as f32) as usize;
                        if x1 <= x0 || y1 <= y0 {
                            continue;
                        }
                        (x0, y0, x1, y1)
                    } else {
                        (0, 0, canvas.w, canvas.h)
                    };
                    let frame_rect = [rx0 as f32, ry0 as f32, (rx1 - rx0) as f32, (ry1 - ry0) as f32];
                    let mut img = canvas.blank();
                    for y in ry0..ry1 {
                        let row_start = (y * canvas.w + rx0) * 4;
                        let row_end = (y * canvas.w + rx1) * 4;
                        img.px[row_start..row_end].copy_from_slice(&canvas.px[row_start..row_end]);
                    }
                    for e in &child.effects {
                        img = apply_mode(e, img, true, frame_rect);
                    }
                    let op = clampf(child.opacity, 0.0, 1.0, 1.0);
                    for y in ry0..ry1 {
                        for x in rx0..rx1 {
                            let o = (y * canvas.w + x) * 4;
                            for c in 0..4 {
                                canvas.px[o + c] += (img.px[o + c] - canvas.px[o + c]) * op;
                            }
                        }
                    }
                    continue;
                }
                let mut buf = canvas.blank();
                draw(dl, ci, &mut buf);
                for e in &child.effects {
                    buf = apply(e, buf);
                }
                composite(canvas, &buf, clampf(child.opacity, 0.0, 1.0, 1.0), child.blend);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image() -> Image {
        let mut im = Image::new(64, 64, -32.0, 32.0, 1.0);
        for y in 16..48 {
            for x in 16..48 {
                let o = (y * im.w + x) * 4;
                let a = if x == 16 || x == 47 { 0.5 } else { 1.0 };
                im.px[o] = 0.8 * a;
                im.px[o + 1] = 0.4 * a;
                im.px[o + 2] = 0.2 * a;
                im.px[o + 3] = a;
            }
        }
        im
    }

    fn check_premultiplied(im: &Image, label: &str) {
        for (i, p) in im.px.chunks_exact(4).enumerate() {
            for c in 0..4 {
                assert!(p[c].is_finite(), "{label} px {i} ch {c} non-finite: {}", p[c]);
                assert!(p[c] >= -1e-5 && p[c] <= 1.0 + 1e-5, "{label} px {i} ch {c} out of range: {}", p[c]);
            }
            assert!(
                p[0] <= p[3] + 1e-4 && p[1] <= p[3] + 1e-4 && p[2] <= p[3] + 1e-4,
                "{label} px {i} breaks premultiplied alpha: {p:?}"
            );
        }
    }

    #[test]
    fn catalog_covers_all_known_effects_and_categories() {
        assert_eq!(catalog().len(), ALL.len());
        for &ty in &ALL {
            let m = meta(ty).unwrap_or_else(|| panic!("missing catalog meta for {ty}"));
            assert!(CATEGORIES.contains(&m.category), "unknown category {}", m.category);
            assert!(!m.label.is_empty());
        }
    }

    #[test]
    fn all_new_effects_preserve_premultiplied_alpha_and_clamp_nan() {
        let src = test_image();
        let cases: Vec<ResolvedEffect> = vec![
            ResolvedEffect { ty: CHROMATIC.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(6.0)), ("angle".into(), ParamValue::F(25.0))] },
            ResolvedEffect { ty: VIGNETTE.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.7)), ("softness".into(), ParamValue::F(0.5)), ("color".into(), ParamValue::C(Color::BLACK))] },
            ResolvedEffect { ty: WAVE.into(), mix: 1.0, params: vec![("amplitude".into(), ParamValue::F(6.0)), ("frequency".into(), ParamValue::F(4.0)), ("phase".into(), ParamValue::F(90.0)), ("angle".into(), ParamValue::F(15.0))] },
            ResolvedEffect { ty: GRAIN.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.4)), ("size".into(), ParamValue::F(2.0)), ("seed".into(), ParamValue::F(7.0))] },
            ResolvedEffect { ty: GRADE.into(), mix: 1.0, params: vec![("exposure".into(), ParamValue::F(0.6)), ("contrast".into(), ParamValue::F(1.4)), ("gamma".into(), ParamValue::F(0.8))] },
            ResolvedEffect { ty: SCANLINES.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.6)), ("lines".into(), ParamValue::F(32.0)), ("offset".into(), ParamValue::F(5.0)), ("phase".into(), ParamValue::F(2.0))] },
            // NaN parameter stress test
            ResolvedEffect { ty: GRADE.into(), mix: f32::NAN, params: vec![("exposure".into(), ParamValue::F(f32::NAN)), ("contrast".into(), ParamValue::F(f32::INFINITY)), ("gamma".into(), ParamValue::F(-5.0))] },
            ResolvedEffect { ty: WAVE.into(), mix: f32::NAN, params: vec![("amplitude".into(), ParamValue::F(f32::NAN)), ("frequency".into(), ParamValue::F(f32::NAN))] },
        ];
        for e in &cases {
            let normal = apply(e, src.clone());
            check_premultiplied(&normal, &format!("normal {}", e.ty));
            let adj = apply_adjustment(e, src.clone());
            check_premultiplied(&adj, &format!("adj {}", e.ty));
        }
        // Normal vignette must NOT darken transparent corner pixels (no bbox artifact)
        let vig = ResolvedEffect { ty: VIGNETTE.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.9)), ("softness".into(), ParamValue::F(0.6))] };
        let n = apply(&vig, src.clone());
        assert_eq!(n.px[3], 0.0, "normal layer vignette keeps transparent corners clear");
        let a = apply_adjustment(&vig, src);
        assert!(a.px[3] > 0.5, "adjustment layer vignette darkens corners over background");
    }
}
