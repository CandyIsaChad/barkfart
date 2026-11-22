//! Text glyphs: fonts rasterized once at BASE pixels into atlas pages. The
//! default (Inter) is page "glyphs:0", built at start (like the Java
//! GlyphAtlas); other fonts, bundled or imported into a project, get a page
//! each ("glyphs:<key>") the first time a textfield uses them. Latin,
//! Latin-1 and common punctuation; a character a font lacks comes from the
//! default font.

use barkfart_core::{Glyph, GlyphSource};
use std::collections::HashMap;

pub const BASE: f32 = 64.0;
pub const PAGE: &str = "glyphs:0";
const EXTRA: &str = "ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿŒœŠšŸŽžƒ€‘’“”•…–—™¡¿«»°±²³µ¶·¹º¼½¾§©®";
static FONT: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");

/// Fonts that ship with Barkfart (all openly licensed; licences beside them
/// in assets/fonts). The key is what a textfield stores.
pub const BUNDLED: &[(&str, &[u8])] = &[
    ("Bangers", include_bytes!("../../../assets/fonts/Bangers-Regular.ttf")),
    ("Permanent Marker", include_bytes!("../../../assets/fonts/PermanentMarker-Regular.ttf")),
    ("Press Start 2P", include_bytes!("../../../assets/fonts/PressStart2P-Regular.ttf")),
    ("Lobster", include_bytes!("../../../assets/fonts/Lobster-Regular.ttf")),
];

struct Face {
    glyphs: HashMap<char, Glyph>,
    missing: Option<Glyph>,
    line_height: f32,
}

/// A rasterized font page: its texture id, size and RGBA (white, coverage
/// in alpha, rows top-down), for the renderer.
pub struct Page {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn rasterize(bytes: &[u8], page: &str) -> Option<(Face, Page)> {
    let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
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
        // characters the font does not have come from the default font
        if c != ' ' && font.lookup_glyph_index(c) == 0 {
            continue;
        }
        let (m, bitmap) = font.rasterize(c, BASE);
        let (gw, gh) = (m.width as u32, m.height as u32);
        if x + gw + 1 > w {
            x = 1;
            y += row + 1;
            row = 0;
        }
        if y + gh + 1 > h {
            break;
        }
        for j in 0..gh {
            for i in 0..gw {
                rgba[(((y + j) * w + x + i) * 4 + 3) as usize] = bitmap[(j * gw + i) as usize];
            }
        }
        glyphs.insert(
            c,
            Glyph {
                texture: page.into(),
                u0: x as f32 / w as f32,
                v0: (y + gh) as f32 / h as f32,
                u1: (x + gw) as f32 / w as f32,
                v1: y as f32 / h as f32,
                x: m.xmin as f32,
                y: m.ymin as f32,
                w: gw as f32,
                h: gh as f32,
                advance: m.advance_width,
            },
        );
        x += gw + 1;
        row = row.max(gh);
    }
    if glyphs.len() < 10 {
        return None; // not a usable text font
    }
    let missing = glyphs.get(&'\u{FFFD}').cloned().or_else(|| glyphs.get(&'?').cloned());
    Some((Face { glyphs, missing, line_height }, Page { id: page.into(), width: w, height: h, rgba }))
}

pub struct GlyphAtlas {
    default: Face,
    fonts: HashMap<String, Face>,
    /// The default page.
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl GlyphAtlas {
    pub fn new() -> GlyphAtlas {
        let (default, page) = rasterize(FONT, PAGE).expect("bundled font");
        GlyphAtlas { default, fonts: HashMap::new(), width: page.width, height: page.height, rgba: page.rgba }
    }

    pub fn has_font(&self, key: &str) -> bool {
        self.fonts.contains_key(key)
    }

    /// Rasterizes a font under `key` (a bundled font's name, or a project's
    /// font asset id) and returns its page to upload; None when the bytes are
    /// not a usable font.
    pub fn add_font(&mut self, key: &str, bytes: &[u8]) -> Option<Page> {
        let (face, page) = rasterize(bytes, &format!("glyphs:{key}"))?;
        self.fonts.insert(key.into(), face);
        Some(page)
    }

    /// A bundled font by name, rasterized; None when it is not bundled.
    pub fn add_bundled(&mut self, name: &str) -> Option<Page> {
        let bytes = BUNDLED.iter().find(|(n, _)| *n == name)?.1;
        self.add_font(name, bytes)
    }

    pub fn remove_font(&mut self, key: &str) {
        self.fonts.remove(key);
    }

    /// Character advances and line height per unit of text size (textfield
    /// boxes); None for a font that is not loaded.
    pub fn metrics(&self, font: Option<&str>) -> Option<barkfart_core::session::FontMetrics> {
        let f = match font {
            Some(k) => self.fonts.get(k)?,
            None => &self.default,
        };
        let mut advance: HashMap<char, f32> = self.default.glyphs.iter().map(|(c, g)| (*c, g.advance / BASE)).collect();
        for (c, g) in &f.glyphs {
            advance.insert(*c, g.advance / BASE);
        }
        Some(barkfart_core::session::FontMetrics { line: f.line_height / BASE, advance })
    }

    fn face(&self, font: Option<&str>) -> &Face {
        font.and_then(|k| self.fonts.get(k)).unwrap_or(&self.default)
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
        self.default.line_height
    }

    fn glyph(&self, c: char) -> Option<Glyph> {
        self.default.glyphs.get(&c).cloned().or_else(|| self.default.missing.clone())
    }

    fn line_height_in(&self, font: Option<&str>) -> f32 {
        self.face(font).line_height
    }

    fn glyph_in(&self, font: Option<&str>, c: char) -> Option<Glyph> {
        let f = self.face(font);
        f.glyphs.get(&c).cloned().or_else(|| self.default.glyphs.get(&c).cloned()).or_else(|| f.missing.clone())
    }
}
