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
