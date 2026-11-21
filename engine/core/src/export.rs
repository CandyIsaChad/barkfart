//! Export writers (port of Java app.barkfart.export): animated GIF (fixed
//! colour cube, ordered dithering, LZW), PNG frames in a ZIP, and the frame
//! timing of an export range. Frames are straight RGBA8, rows top-down.

use std::io::{Seek, Write};

const BAYER: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Animated GIF writer: 6x7x6 colour cube (252 colours) with 4x4 ordered
/// dithering, transparent index 255 for pixels under half alpha, frame
/// delays in hundredths with the rounding error carried (never under 2).
pub struct Gif<W: Write> {
    out: W,
    w: u16,
    h: u16,
    frame_seconds: f64,
    owed: f64,
}

/// Palette entry i: the cube for i < 252, black otherwise.
pub fn gif_colour(i: u32) -> [u8; 3] {
    if i >= 252 {
        return [0, 0, 0];
    }
    let (r, g, b) = (i / 42, (i / 6) % 7, i % 6);
    [(r * 255 / 5) as u8, (g * 255 / 6) as u8, (b * 255 / 5) as u8]
}

fn q(v: u8, levels: u32, threshold: f32) -> u32 {
    let s = v as f32 * levels as f32 / 255.0;
    let lo = s as u32;
    levels.min(lo + if s - lo as f32 > threshold { 1 } else { 0 })
}

impl<W: Write> Gif<W> {
    pub fn new(mut out: W, w: u16, h: u16, fps: f64, looping: bool) -> std::io::Result<Gif<W>> {
        out.write_all(b"GIF89a")?;
        out.write_all(&w.to_le_bytes())?;
        out.write_all(&h.to_le_bytes())?;
        out.write_all(&[0xF7, 0, 0])?;
        for i in 0..256 {
            out.write_all(&gif_colour(i))?;
        }
        if looping {
            out.write_all(&[0x21, 0xFF, 0x0B])?;
            out.write_all(b"NETSCAPE2.0")?;
            out.write_all(&[3, 1, 0, 0, 0])?;
        }
        Ok(Gif { out, w, h, frame_seconds: 1.0 / if fps > 0.0 { fps } else { 30.0 }, owed: 0.0 })
    }

    fn next_delay(&mut self) -> u32 {
        self.owed += self.frame_seconds * 100.0;
        let mut d = (self.owed + 1e-9).floor() as i64;
        if d < 2 {
            d = 2;
        }
        self.owed -= d as f64;
        d as u32
    }

    /// Adds a frame that shows `extra` seconds longer than the others (a
    /// frame delay).
    pub fn frame_held(&mut self, rgba: &[u8], extra: f64) -> std::io::Result<()> {
        if extra.is_finite() && extra > 0.0 {
            self.owed += extra.min(600.0) * 100.0;
        }
        self.frame(rgba)
    }

    /// Adds a frame (straight RGBA8, w * h * 4 bytes).
    pub fn frame(&mut self, rgba: &[u8]) -> std::io::Result<()> {
        let (w, h) = (self.w as usize, self.h as usize);
        let delay = self.next_delay();
        self.out.write_all(&[0x21, 0xF9, 4, 0x09, delay as u8, (delay >> 8) as u8, 255, 0])?;
        self.out.write_all(&[0x2C, 0, 0, 0, 0])?;
        self.out.write_all(&self.w.to_le_bytes())?;
        self.out.write_all(&self.h.to_le_bytes())?;
        self.out.write_all(&[0])?;
        let mut idx = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let p = (y * w + x) * 4;
                if rgba[p + 3] < 128 {
                    idx[y * w + x] = 255;
                    continue;
                }
                let th = (BAYER[y & 3][x & 3] as f32 + 0.5) / 16.0;
                let (r, g, b) = (q(rgba[p], 5, th), q(rgba[p + 1], 6, th), q(rgba[p + 2], 5, th));
                idx[y * w + x] = (r * 42 + g * 6 + b) as u8;
            }
        }
        self.lzw(&idx)
    }

    pub fn finish(mut self) -> std::io::Result<W> {
        self.out.write_all(&[0x3B])?;
        self.out.flush()?;
        Ok(self.out)
    }

    fn lzw(&mut self, data: &[u8]) -> std::io::Result<()> {
        const MIN: u32 = 8;
        let (clear, eoi) = (1u32 << MIN, (1u32 << MIN) + 1);
        self.out.write_all(&[MIN as u8])?;
        let mut bits = Bits { buf: 0, count: 0, block: Vec::with_capacity(255) };
        let mut dict: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        let (mut next, mut width) = (eoi + 1, MIN + 1);
        bits.emit(&mut self.out, clear, width)?;
        let mut prefix: i64 = if data.is_empty() { -1 } else { data[0] as i64 };
        for &c in data.iter().skip(1) {
            let key = ((prefix as u32) << 8) | c as u32;
            if let Some(&code) = dict.get(&key) {
                prefix = code as i64;
                continue;
            }
            bits.emit(&mut self.out, prefix as u32, width)?;
            if next < 4096 {
                dict.insert(key, next);
                next += 1;
                if next > (1 << width) && width < 12 {
                    width += 1;
                }
            } else {
                bits.emit(&mut self.out, clear, width)?;
                dict.clear();
                next = eoi + 1;
                width = MIN + 1;
            }
            prefix = c as i64;
        }
        if prefix >= 0 {
            bits.emit(&mut self.out, prefix as u32, width)?;
        }
        bits.emit(&mut self.out, eoi, width)?;
        if bits.count > 0 {
            bits.byte(&mut self.out, (bits.buf & 0xff) as u8)?;
        }
        if !bits.block.is_empty() {
            bits.flush(&mut self.out)?;
        }
        self.out.write_all(&[0])
    }
}

struct Bits {
    buf: u32,
    count: u32,
    block: Vec<u8>,
}

impl Bits {
    fn emit<W: Write>(&mut self, out: &mut W, code: u32, width: u32) -> std::io::Result<()> {
        self.buf |= code << self.count;
        self.count += width;
        while self.count >= 8 {
            self.byte(out, (self.buf & 0xff) as u8)?;
            self.buf >>= 8;
            self.count -= 8;
        }
        Ok(())
    }

    fn byte<W: Write>(&mut self, out: &mut W, b: u8) -> std::io::Result<()> {
        self.block.push(b);
        if self.block.len() == 255 {
            self.flush(out)?;
        }
        Ok(())
    }

    fn flush<W: Write>(&mut self, out: &mut W) -> std::io::Result<()> {
        out.write_all(&[self.block.len() as u8])?;
        out.write_all(&self.block)?;
        self.block.clear();
        Ok(())
    }
}

/// PNG of straight RGBA8 pixels; RGB only when alpha is false.
pub fn png(rgba: &[u8], w: u32, h: u32, alpha: bool) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(if alpha { png::ColorType::Rgba } else { png::ColorType::Rgb });
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut wr = enc.write_header().expect("png header");
        if alpha {
            wr.write_image_data(&rgba[..(w * h * 4) as usize]).expect("png data");
        } else {
            let rgb: Vec<u8> = rgba.chunks_exact(4).take((w * h) as usize).flat_map(|p| [p[0], p[1], p[2]]).collect();
            wr.write_image_data(&rgb).expect("png data");
        }
    }
    out
}

/// PNG frames in a ZIP ("frame_0001.png", ...), like the Java export.
pub struct PngZip<W: Write + Seek> {
    zip: zip::ZipWriter<W>,
    w: u32,
    h: u32,
    alpha: bool,
    n: u32,
}

impl<W: Write + Seek> PngZip<W> {
    pub fn new(out: W, w: u32, h: u32, alpha: bool) -> PngZip<W> {
        PngZip { zip: zip::ZipWriter::new(out), w, h, alpha, n: 0 }
    }

    pub fn frame(&mut self, rgba: &[u8]) -> std::io::Result<()> {
        self.n += 1;
        // PNG is already compressed: store it
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        self.zip.start_file(format!("frame_{:04}.png", self.n), opts).map_err(std::io::Error::other)?;
        self.zip.write_all(&png(rgba, self.w, self.h, self.alpha))
    }

    pub fn finish(self) -> std::io::Result<W> {
        self.zip.finish().map_err(std::io::Error::other)
    }
}

/// Time of the n-th exported frame (0-based) of a range starting at start_frame.
/// The pictures of an export of frames [first, first + frames) in playing
/// order: (project time, extra seconds it shows for). Frame delays hold the
/// slot's first picture; repeats play their frames again. `rate` pictures
/// per second (the fps, or more with tweening on); with `hold_as_copies` a
/// delay becomes copies of the picture (PNG frames have no durations).
pub fn pictures(sch: &crate::timing::Schedule, first: i32, frames: i32, rate: f64, hold_as_copies: bool) -> Vec<(f64, f64)> {
    let rate = if rate > 0.0 { rate } else { sch.fps };
    let per = ((rate / sch.fps).round() as i32).max(1);
    let mut out = Vec::new();
    for s in sch.slots.iter().filter(|s| s.frame >= first && s.frame < first + frames) {
        let base = s.frame as f64 / sch.fps;
        for k in 0..per {
            let t = base + k as f64 / rate;
            if k == 0 && s.delay > 0.0 {
                if hold_as_copies {
                    for _ in 0..(s.delay * rate).round() as i64 {
                        out.push((t, 0.0));
                    }
                    out.push((t, 0.0));
                } else {
                    out.push((t, s.delay));
                }
            } else {
                out.push((t, 0.0));
            }
            if out.len() >= 1_000_000 {
                return out;
            }
        }
    }
    out
}

pub fn frame_time(start_frame: u32, n: u32, fps: f64) -> f64 {
    let fps = if fps > 0.0 { fps } else { 30.0 };
    (start_frame + n) as f64 / fps
}
