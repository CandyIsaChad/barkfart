//! Image assets: decoding PNG / JPEG bytes into straight RGBA8.

/// A decoded image: straight RGBA8, rows top-down.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

/// Largest side accepted (memory guard; matches the GIF importer).
pub const MAX_SIDE: u32 = 8192;

/// Decodes PNG or JPEG; None for anything else, broken or too large.
pub fn decode(bytes: &[u8]) -> Option<Rgba> {
    let fmt = image::guess_format(bytes).ok()?;
    if !matches!(fmt, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return None;
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), fmt);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().ok()?.into_rgba8();
    Some(Rgba { w: img.width(), h: img.height(), px: img.into_raw() })
}

/// Image bytes for an asset id from a .bark file's entries.
pub fn asset_bytes<'a>(project: &crate::model::Project, entries: &'a indexmap::IndexMap<String, Vec<u8>>, id: &str) -> Option<&'a [u8]> {
    let a = project.assets.get(id)?;
    entries.get(&a.path).or_else(|| entries.get(&format!("assets/{}", a.path))).map(Vec::as_slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_and_refuses_junk() {
        let mut png = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png, 2, 1);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&[255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
        }
        let img = decode(&png).unwrap();
        assert_eq!((img.w, img.h), (2, 1));
        assert_eq!(img.px, vec![255, 0, 0, 255, 0, 0, 255, 128]);
        assert!(decode(b"not an image").is_none());
        assert!(decode(&png[..20]).is_none());
    }
}

// ---------------------------------------------------------------- GIF

/// One GIF frame: the whole canvas (straight RGBA8, rows top-down) and how long it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct GifFrame {
    pub rgba: Vec<u8>,
    pub delay: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gif {
    pub w: u32,
    pub h: u32,
    pub frames: Vec<GifFrame>,
}

pub const GIF_MAX_FRAMES: usize = 500;
pub const GIF_MAX_SIDE: u32 = 4096;

/// Animated GIF (port of Java GifDecoder): colour tables, LZW, interlacing,
/// transparency, delays (0-1 hundredths show as 0.1 s, like browsers) and
/// disposal (keep, clear, restore previous). None for anything unreadable.
pub fn decode_gif(b: &[u8]) -> Option<Gif> {
    let u8at = |p: usize| b.get(p).copied();
    let u16at = |p: usize| Some(u8at(p)? as u32 | (u8at(p + 1)? as u32) << 8);
    if b.len() < 13 || &b[0..3] != b"GIF" {
        return None;
    }
    let (w, h, flags) = (u16at(6)?, u16at(8)?, u8at(10)? as u32);
    if w == 0 || h == 0 || w > GIF_MAX_SIDE || h > GIF_MAX_SIDE {
        return None;
    }
    let mut p = 13usize;
    let palette = |p: usize, n: usize| -> Option<Vec<u32>> { (0..n).map(|i| Some((u8at(p + i * 3)? as u32) << 16 | (u8at(p + i * 3 + 1)? as u32) << 8 | u8at(p + i * 3 + 2)? as u32)).collect() };
    let mut global = None;
    if flags & 0x80 != 0 {
        let n = 2usize << (flags & 7);
        global = Some(palette(p, n)?);
        p += 3 * n;
    }
    let (wu, hu) = (w as usize, h as usize);
    let mut out = Gif { w, h, frames: Vec::new() };
    let mut canvas = vec![0u8; wu * hu * 4];
    let (mut delay, mut transparent, mut disposal) = (0u32, -1i32, 0u32);
    while p < b.len() && out.frames.len() < GIF_MAX_FRAMES {
        let block = u8at(p)?;
        p += 1;
        if block == 0x3B {
            break;
        }
        if block == 0x21 {
            let label = u8at(p)?;
            p += 1;
            if label == 0xF9 && u8at(p)? >= 4 {
                let packed = u8at(p + 1)? as u32;
                disposal = (packed >> 2) & 7;
                delay = u16at(p + 2)?;
                transparent = if packed & 1 != 0 { u8at(p + 4)? as i32 } else { -1 };
            }
            while p < b.len() {
                let len = u8at(p)? as usize;
                p += 1;
                if len == 0 {
                    break;
                }
                p += len;
            }
            continue;
        }
        if block != 0x2C {
            break; // unknown: stop with what we have
        }
        let (fx, fy, fw, fh, ff) = (u16at(p)? as usize, u16at(p + 2)? as usize, u16at(p + 4)? as usize, u16at(p + 6)? as usize, u8at(p + 8)? as u32);
        p += 9;
        let mut pal = global.clone();
        if ff & 0x80 != 0 {
            let n = 2usize << (ff & 7);
            pal = Some(palette(p, n)?);
            p += 3 * n;
        }
        let interlaced = ff & 0x40 != 0;
        let min_code = u8at(p)? as u32;
        p += 1;
        let mut data = Vec::new();
        while p < b.len() {
            let len = u8at(p)? as usize;
            p += 1;
            if len == 0 {
                break;
            }
            let end = (p + len).min(b.len());
            data.extend_from_slice(&b[p..end]);
            p += len;
        }
        let idx = lzw(&data, min_code, fw * fh);
        let before = if disposal == 3 { Some(canvas.clone()) } else { None };
        if let Some(pal) = &pal {
            for i in 0..fw * fh {
                let (row, col) = (i / fw, i % fw);
                let y = if interlaced { deinterlace(row, fh) } else { row };
                let (cx, cy) = (fx + col, fy + y);
                if cx >= wu || cy >= hu {
                    continue;
                }
                let c = idx[i] as usize;
                if c as i32 == transparent || c >= pal.len() {
                    continue;
                }
                let (o, rgb) = ((cy * wu + cx) * 4, pal[c]);
                canvas[o] = (rgb >> 16) as u8;
                canvas[o + 1] = (rgb >> 8) as u8;
                canvas[o + 2] = rgb as u8;
                canvas[o + 3] = 255;
            }
        }
        let seconds = if delay <= 1 { 0.1 } else { delay as f64 / 100.0 };
        out.frames.push(GifFrame { rgba: canvas.clone(), delay: seconds });
        if disposal == 2 {
            for y in fy..hu.min(fy + fh) {
                for x in fx..wu.min(fx + fw) {
                    let o = (y * wu + x) * 4;
                    canvas[o..o + 4].fill(0);
                }
            }
        } else if let (3, Some(b4)) = (disposal, before) {
            canvas = b4;
        }
        delay = 0;
        transparent = -1;
        disposal = 0;
    }
    if out.frames.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Row in the image for the n-th stored row of an interlaced frame.
fn deinterlace(mut n: usize, h: usize) -> usize {
    let (start, step) = ([0usize, 4, 2, 1], [8usize, 8, 4, 2]);
    for pass in 0..4 {
        let rows = if h > start[pass] { (h - start[pass]).div_ceil(step[pass]) } else { 0 };
        if n < rows {
            return start[pass] + n * step[pass];
        }
        n -= rows;
    }
    h.saturating_sub(1)
}

/// Variable-width LZW into at most `count` indices (short data leaves zeros).
fn lzw(data: &[u8], min_code: u32, count: usize) -> Vec<u8> {
    let mut out = vec![0u8; count];
    let min_code = if (2..=11).contains(&min_code) { min_code } else { 8 };
    let (clear, eoi) = (1u32 << min_code, (1u32 << min_code) + 1);
    let mut prefix = vec![0u32; 4096];
    let mut suffix = vec![0u8; 4096];
    for i in 0..clear as usize {
        suffix[i] = i as u8;
    }
    let (mut width, mut next, mut prev, mut o) = (min_code + 1, eoi + 1, -1i64, 0usize);
    let (mut bits, mut buf) = (0u32, 0u32);
    let mut stack = vec![0u8; 4097];
    for &byte in data {
        if o >= count {
            break;
        }
        buf |= (byte as u32) << bits;
        bits += 8;
        while bits >= width && o < count {
            let code = buf & ((1 << width) - 1);
            buf >>= width;
            bits -= width;
            if code == clear {
                width = min_code + 1;
                next = eoi + 1;
                prev = -1;
                continue;
            }
            if code == eoi {
                return out;
            }
            let mut c = code;
            if code >= next {
                if prev < 0 {
                    return out;
                }
                c = prev as u32;
            }
            let mut sp = 0;
            while c >= clear && sp < 4096 {
                stack[sp] = suffix[c as usize];
                sp += 1;
                c = prefix[c as usize];
            }
            stack[sp] = c as u8;
            sp += 1;
            let first = c as u8;
            for k in (0..sp).rev() {
                if o >= count {
                    break;
                }
                out[o] = stack[k];
                o += 1;
            }
            if code >= next && o < count {
                out[o] = first;
                o += 1;
            }
            if prev >= 0 && next < 4096 {
                prefix[next as usize] = prev as u32;
                suffix[next as usize] = first;
                next += 1;
                if next == (1 << width) && width < 12 {
                    width += 1;
                }
            }
            prev = code as i64;
        }
    }
    out
}

/// Width and height of a PNG, JPEG or GIF without decoding it.
pub fn size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() >= 10 && &bytes[0..3] == b"GIF" {
        return Some((bytes[6] as u32 | (bytes[7] as u32) << 8, bytes[8] as u32 | (bytes[9] as u32) << 8));
    }
    let fmt = ::image::guess_format(bytes).ok()?;
    ::image::ImageReader::with_format(std::io::Cursor::new(bytes), fmt).into_dimensions().ok()
}
