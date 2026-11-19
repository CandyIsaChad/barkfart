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
        p.library[&root_id].layers.push(l);
        return Ok(Imported { layer: id, message: format!("Imported figure {name}"), approximations: rep.approximations });
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
        p.library[&root_id].layers.push(inst);
        return Ok(Imported { layer: lid, message: format!("Imported movieclip {name} ({} frames)", mc.frames.len()), approximations: rep.approximations });
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
            p.library[&root_id].layers.push(l);
        }
        return Ok(Imported {
            layer: last,
            message: format!("Imported {n} figures from {name} (its animation cannot be imported yet)"),
            approximations: rep.approximations,
        });
    }
    if lower.ends_with(".stk") {
        return Err(ImportError::Unsupported("Pivot figures (.stk) cannot be imported yet".into()));
    }
    Err(ImportError::Unsupported(format!("{file_name} is not a file Barkfart can import")))
}
