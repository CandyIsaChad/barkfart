//! Importing files into a project (port of the Java Importer): Stick Nodes
//! figures, movieclips and project figure libraries; sounds; images and
//! animated GIFs come next.

use crate::legacy::{self, Report};
use crate::model::*;

#[derive(Debug, Clone, PartialEq)]
pub enum ImportError {
    /// A file Barkfart cannot import (yet), with a message for the user.
    Unsupported(String),
    /// A damaged or unreadable file.
    Broken(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Unsupported(s) | ImportError::Broken(s) => f.write_str(s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Imported {
    /// The layer added last (to select it).
    pub layer: String,
    pub message: String,
    pub approximations: Vec<String>,
    /// New asset files to store with the project, by archive path.
    pub assets: Vec<(String, Vec<u8>)>,
}

fn name_of(file_name: &str) -> &str {
    match file_name.rfind('.') {
        Some(i) if i > 0 => &file_name[..i],
        _ => file_name,
    }
}

fn wrap(name: &str, e: legacy::LegacyError) -> ImportError {
    if legacy::is_unsupported(&e) {
        ImportError::Unsupported(format!("{name}: {e}"))
    } else {
        ImportError::Broken(e.0)
    }
}

/// Imports a Stick Nodes file into the project's root symbol.
pub fn import_file(p: &mut Project, file_name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let lower = file_name.to_lowercase();
    let name = name_of(file_name).to_string();
    let root_id = p.root_symbol.clone();
    if !p.library.contains_key(&root_id) {
        return Err(ImportError::Broken("the project has no root symbol".into()));
    }
    if lower.ends_with(".nodes") {
        let fig = legacy::read_nodes(bytes).map_err(|e| wrap(&name, e))?;
        let mut rep = Report::default();
        let l = legacy::to_layer(p, &fig, &name, &mut rep);
        let id = l.id.clone();
        p.root_mut().expect("root").layers.push(std::sync::Arc::new(l));
        return Ok(Imported { layer: id, message: format!("Imported figure {name}"), approximations: rep.approximations, assets: Vec::new() });
    }
    if lower.ends_with(".nodemc") {
        let mc = legacy::read_movieclip(bytes).map_err(|e| wrap(&name, e))?;
        let mut rep = Report::default();
        let sid = legacy::movieclip_to_symbol(p, &mc, &mut rep);
        let lid = legacy::new_id(p, "L");
        let mut inst = Layer::symbol(&lid, Some(&name), &sid);
        if let LayerKind::Symbol(s) = &mut inst.kind {
            s.loop_mode = SymbolLoop::Loop;
        }
        p.root_mut().expect("root").layers.push(std::sync::Arc::new(inst));
        return Ok(Imported { layer: lid, message: format!("Imported movieclip {name} ({} frames)", mc.frames.len()), approximations: rep.approximations, assets: Vec::new() });
    }
    if lower.ends_with(".stknds") {
        let h = legacy::read_stknds_header(bytes).map_err(|e| wrap(&name, e))?;
        if h.figures.is_empty() {
            return Err(ImportError::Unsupported(format!("{name} has no figures to import")));
        }
        let mut rep = Report::default();
        let n = h.figures.len();
        let mut last = String::new();
        for (i, nf) in h.figures.iter().enumerate() {
            let mut l = legacy::to_layer(p, &nf.figure, &nf.name, &mut rep);
            l.transform.x.value = Value::F((i as f32 - (n - 1) as f32 / 2.0) * 160.0);
            last = l.id.clone();
            p.root_mut().expect("root").layers.push(std::sync::Arc::new(l));
        }
        return Ok(Imported {
            layer: last,
            message: format!("Imported {n} figures from {name} (its animation cannot be imported yet)"),
            approximations: rep.approximations,
            assets: Vec::new(),
        });
    }
    let ext = match lower.rfind('.') {
        Some(i) => lower[i..].to_string(),
        None => String::new(),
    };
    if crate::files::SOUND_EXTENSIONS.contains(&ext.as_str()) {
        let Some(pcm) = crate::audio::decode_sound(bytes) else {
            return Err(ImportError::Unsupported(format!("Can't play {file_name}. Use mp3, m4a, ogg, flac or wav.")));
        };
        let aid = legacy::new_id(p, "A");
        let path = format!("assets/{aid}{ext}");
        p.assets.insert(aid.clone(), Asset { id: aid.clone(), kind: AssetKind::Audio, path: path.clone() });
        let lid = legacy::new_id(p, "L");
        let mut al = Layer::audio(&lid, Some(&name));
        let cid = legacy::new_id(p, "C");
        if let LayerKind::Audio(clips) = &mut al.kind {
            clips.push(Clip::new(&lid, &cid, Some(aid.clone()), 0.0, 0.0, pcm.seconds()));
        }
        p.root_mut().expect("root").layers.push(std::sync::Arc::new(al));
        return Ok(Imported {
            layer: lid,
            message: format!("Added sound {name} ({:.1} s) at 0.00 s", pcm.seconds()),
            approximations: Vec::new(),
            assets: vec![(path, bytes.to_vec())],
        });
    }
    if matches!(ext.as_str(), ".png" | ".jpg" | ".jpeg" | ".gif") {
        let mut bytes = bytes.to_vec();
        let mut ext = ext;
        if bytes.starts_with(b"GIF") {
            let Some(gif) = crate::image::decode_gif(&bytes) else {
                return Err(ImportError::Unsupported(format!("Can't read {file_name}.")));
            };
            if gif.frames.len() > 1 {
                return Ok(import_gif(p, &root_id, &name, &gif));
            }
            bytes = crate::export::png(&gif.frames[0].rgba, gif.w, gif.h, true);
            ext = ".png".into();
        }
        let Some((w, h)) = crate::image::size(&bytes).filter(|(w, h)| *w > 0 && *h > 0) else {
            return Err(ImportError::Unsupported(format!("Can't read {file_name} (use png or jpg).")));
        };
        let aid = legacy::new_id(p, "A");
        let path = format!("assets/{aid}{ext}");
        p.assets.insert(aid.clone(), Asset { id: aid.clone(), kind: AssetKind::Image, path: path.clone() });
        let lid = legacy::new_id(p, "L");
        let mut sl = Layer::sprite(&lid, Some(&name));
        let fit = 1f32.min((p.settings.width as f32 * 0.6 / w as f32).min(p.settings.height as f32 * 0.6 / h as f32));
        if let LayerKind::Sprite(s) = &mut sl.kind {
            s.asset = Some(aid.clone());
            s.width = w as f32 * fit;
            s.height = h as f32 * fit;
            sl.transform.anchor_x.value = Value::F(s.width / 2.0);
            sl.transform.anchor_y.value = Value::F(s.height / 2.0);
        }
        p.root_mut().expect("root").layers.push(std::sync::Arc::new(sl));
        return Ok(Imported { layer: lid, message: format!("Added image {name} ({w} × {h})"), approximations: Vec::new(), assets: vec![(path, bytes)] });
    }
    if lower.ends_with(".stk") {
        return Err(ImportError::Unsupported("Pivot figures (.stk) cannot be imported yet".into()));
    }
    Err(ImportError::Unsupported(format!("{file_name} is not a file Barkfart can import")))
}

/// An animated GIF as a looping movieclip: one sprite per frame, each shown
/// only during its own time (Stick Nodes 4.2.7).
fn import_gif(p: &mut Project, root_id: &str, name: &str, gif: &crate::image::Gif) -> Imported {
    let fit = 1f32.min((p.settings.width as f32 * 0.6 / gif.w as f32).min(p.settings.height as f32 * 0.6 / gif.h as f32));
    let total: f64 = gif.frames.iter().map(|f| f.delay).sum();
    let sid = legacy::new_id(p, "S");
    let mut sym = Symbol::new(&sid, Some(name), total);
    let mut assets = Vec::new();
    let mut at = 0.0;
    for (k, fr) in gif.frames.iter().enumerate() {
        let aid = legacy::new_id(p, "A");
        let path = format!("assets/{aid}.png");
        p.assets.insert(aid.clone(), Asset { id: aid.clone(), kind: AssetKind::Image, path: path.clone() });
        assets.push((path, crate::export::png(&fr.rgba, gif.w, gif.h, true)));
        let lid = legacy::new_id(p, "L");
        let mut sl = Layer::sprite(&lid, Some(&format!("{name} {}", k + 1)));
        if let LayerKind::Sprite(s) = &mut sl.kind {
            s.asset = Some(aid);
            s.width = gif.w as f32 * fit;
            s.height = gif.h as f32 * fit;
            sl.transform.anchor_x.value = Value::F(s.width / 2.0);
            sl.transform.anchor_y.value = Value::F(s.height / 2.0);
        }
        sl.shown.value = Value::B(false);
        if at > 0.0 {
            sl.shown.track.put(Key::new(0.0, Value::B(false), Interp::Hold));
        }
        sl.shown.track.put(Key::new(at, Value::B(true), Interp::Hold));
        if at + fr.delay < total - 1e-9 {
            sl.shown.track.put(Key::new(at + fr.delay, Value::B(false), Interp::Hold));
        }
        sym.layers.push(std::sync::Arc::new(sl));
        at += fr.delay;
    }
    p.add_symbol(sym);
    let lid = legacy::new_id(p, "L");
    let mut inst = Layer::symbol(&lid, Some(name), &sid);
    if let LayerKind::Symbol(s) = &mut inst.kind {
        s.loop_mode = SymbolLoop::Loop;
    }
    p.symbol_mut(root_id).expect("root").layers.push(std::sync::Arc::new(inst));
    Imported {
        layer: lid,
        message: format!("Added GIF {name} ({} frames, {total:.2} s, as a movieclip)", gif.frames.len()),
        approximations: Vec::new(),
        assets,
    }
}
