//! Barkfart engine core: the project model, evaluator and tessellator.
//!
//! Pure Rust, no GPU and no platform code, so everything here runs in plain
//! `cargo test` on any machine. This is the start of the port from the Java
//! version (git tag `java-reference`), which stays the reference until this
//! crate matches its outputs.

pub mod audio;
pub mod codec;
pub mod edit;
pub mod eval;
pub mod export;
pub mod figures;
pub mod files;
pub mod fx;
pub mod image;
pub mod import;
pub mod json;
pub mod legacy;
pub mod model;
pub mod session;
pub mod tess;

pub use tess::{Batch, DrawList, Glyph, GlyphSource};
