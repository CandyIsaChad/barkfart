//! Text glyphs: the bundled Inter font rasterized once at BASE pixels into
//! one atlas page ("glyphs:0"): Latin, Latin-1 and common punctuation
//! (like the Java GlyphAtlas).

use barkfart_core::{Glyph, GlyphSource};
use std::collections::HashMap;

pub const BASE: f32 = 64.0;
pub const PAGE: &str = "glyphs:0";
const EXTRA: &str = "ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿŒœŠšŸŽžƒ€‘’“”•…–—™¡¿«»°±²³µ¶·¹º¼½¾§©®";
static FONT: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");

pub struct GlyphAtlas {
    glyphs: HashMap<char, Glyph>,
    missing: Option<Glyph>,
    line_height: f32,
    /// The atlas page: white with coverage in alpha, rows top-down.
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl GlyphAtlas {
    pub fn new() -> GlyphAtlas {
        let font = fontdue::Font::from_bytes(FONT, fontdue::FontSettings::default()).expect("bundled font");
        let line_height = font.horizontal_line_metrics(BASE).map(|m| m.new_line_size).unwrap_or(BASE * 1.2);
        let chars: Vec<char> = (32u8..127).map(char::from).chain(EXTRA.chars()).chain(['\u{FFFD}']).collect();
        let (w, h) = (1024u32, 1024u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for p in rgba.chunks_exact_mut(4) {
            p[0] = 255;
            p[1] = 255;
            p[2] = 255;
        }
        let mut glyphs = HashMap::new();
        let (mut x, mut y, mut row) = (1u32, 1u32, 0u32);
        for c in chars {
            let (m, bitmap) = font.rasterize(c, BASE);
            let (gw, gh) = (m.width as u32, m.height as u32);
            if x + gw + 1 > w {
                x = 1;
                y += row + 1;
                row = 0;
            }
            if y + gh + 1 > h {
                break; // page full (does not happen for this set at 64 px)
            }
            for j in 0..gh {
                for i in 0..gw {
                    rgba[(((y + j) * w + x + i) * 4 + 3) as usize] = bitmap[(j * gw + i) as usize];
                }
            }
            let g = Glyph {
                texture: PAGE.into(),
                u0: x as f32 / w as f32,
                v0: (y + gh) as f32 / h as f32,
                u1: (x + gw) as f32 / w as f32,
                v1: y as f32 / h as f32,
                x: m.xmin as f32,
                y: m.ymin as f32,
                w: gw as f32,
                h: gh as f32,
                advance: m.advance_width,
            };
            glyphs.insert(c, g);
            x += gw + 1;
            row = row.max(gh);
        }
        let missing = glyphs.get(&'\u{FFFD}').cloned().or_else(|| glyphs.get(&'?').cloned());
        GlyphAtlas { glyphs, missing, line_height, width: w, height: h, rgba }
    }
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphSource for GlyphAtlas {
    fn base_px(&self) -> f32 {
        BASE
    }

    fn line_height(&self) -> f32 {
        self.line_height
    }

    fn glyph(&self, c: char) -> Option<Glyph> {
        self.glyphs.get(&c).cloned().or_else(|| self.missing.clone())
    }
}
