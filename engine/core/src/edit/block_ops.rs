//! Blocks (REDESIGN D2, F4): with the whole-figure key rule (D25) a figure's
//! pose is keyed as a whole, and a block is one such key: it lasts from its
//! frame until the figure's next key. These commands change one figure's
//! row only; "Add frame" (frame_ops::insert_after) is the bridge that acts on
//! every row at once.

use super::{fail, EditResult, State};
use crate::model::*;

/// A layer's pose: its transform, and every joint's angle and length (a
/// figure) or the view (the camera). The whole-figure rule keys these together.
pub fn pose_props(l: &mut Layer) -> Vec<&mut Property> {
    let tr = &mut l.transform;
    let mut ps: Vec<&mut Property> = vec![&mut tr.x, &mut tr.y, &mut tr.rotation, &mut tr.scale_x, &mut tr.scale_y];
    match &mut l.kind {
        LayerKind::Figure(f) => {
            for nd in f.nodes_mut() {
                ps.push(&mut nd.rotation);
                ps.push(&mut nd.length);
            }
        }
        LayerKind::Camera(c) => {
            ps.push(&mut c.x);
            ps.push(&mut c.y);
            ps.push(&mut c.zoom);
            ps.push(&mut c.rotation);
        }
        _ => {}
    }
    ps
}

/// The same, to read.
pub fn pose_props_of(l: &Layer) -> Vec<&Property> {
    let tr = &l.transform;
    let mut ps: Vec<&Property> = vec![&tr.x, &tr.y, &tr.rotation, &tr.scale_x, &tr.scale_y];
    match &l.kind {
        LayerKind::Figure(f) => {
            for nd in f.nodes() {
                ps.push(&nd.rotation);
                ps.push(&nd.length);
            }
        }
        LayerKind::Camera(c) => {
            ps.extend([&c.x, &c.y, &c.zoom, &c.rotation]);
        }
        _ => {}
    }
    ps
}

fn fps(st: &State) -> f64 {
    let f = st.project.settings.fps as f64;
    if f > 0.0 {
        f
    } else {
        30.0
    }
}

fn frame_count(st: &State) -> i32 {
    let d = st.project.root().map(|s| s.duration).unwrap_or(0.0);
    1.max((d * fps(st)).round() as i32)
}

fn layer<'a>(st: &'a mut State, sym: &str, id: &str) -> EditResult<&'a mut Layer> {
    match st.project.layer_mut(sym, id) {
        Some(l) if !matches!(l.kind, LayerKind::Camera(_)) => Ok(l),
        Some(_) => fail("The camera is set frame by frame."),
        None => fail(format!("no layer {id}")),
    }
}

/// The frames a layer's pose is keyed on, in order.
pub fn pose_frames(l: &Layer, fps: f64) -> Vec<i32> {
    let mut v: Vec<i32> = pose_props_of(l).iter().flat_map(|q| q.track.keys().iter().map(|k| (k.time * fps).round() as i32)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The block under `frame`: (its key's frame, the next key's frame, if any).
pub fn block_at(l: &Layer, fps: f64, frame: i32) -> Option<(i32, Option<i32>)> {
    let keys = pose_frames(l, fps);
    let start = keys.iter().rev().find(|k| **k <= frame).copied()?;
    let next = keys.iter().find(|k| **k > frame).copied();
    Some((start, next))
}

/// Rewrites every pose key of a layer through `f(frame) -> new frame`.
fn retime(l: &mut Layer, fps: f64, f: impl Fn(i32) -> i32) {
    for q in pose_props(l) {
        let keys: Vec<Key> = q.track.keys().to_vec();
        q.track.clear();
        for k in keys {
            let fr = (k.time * fps).round() as i32;
            q.track.put(Key { time: f(fr) as f64 / fps, ..k });
        }
    }
}

/// Makes the animation at least `frames` long.
fn ensure_length(st: &mut State, frames: i32) {
    let fps = fps(st);
    if frame_count(st) < frames {
        if let Some(r) = st.project.root_mut() {
            r.duration = frames as f64 / fps;
        }
    }
}

/// "New pose": the figure gets a key of its own on this frame (the start of
/// a new block), with the pose it has there.
pub fn new_pose(st: &mut State, sym: &str, id: &str, frame: i32) -> EditResult<()> {
    let fps = fps(st);
    let t = frame as f64 / fps;
    let l = layer(st, sym, id)?;
    if pose_frames(l, fps).contains(&frame) {
        return fail("There is a pose on this frame already.");
    }
    for q in pose_props(l) {
        let v = q.key_value(t);
        let interp = crate::edit::editor::interp_at(q, t, Interp::Linear);
        q.track.put(Key::new(t, v, interp));
    }
    Ok(())
}

/// The block under `frame` lasts `frames` frames: the figure's later poses
/// move with its end. A last block grows the animation when it needs to.
pub fn set_block_length(st: &mut State, sym: &str, id: &str, frame: i32, frames: i32) -> EditResult<()> {
    if frames < 1 {
        return fail("A pose lasts at least one frame.");
    }
    let fps = fps(st);
    let n = frame_count(st);
    let (start, next, last) = {
        let l = layer(st, sym, id)?;
        let Some((start, next)) = block_at(l, fps, frame) else { return fail("This figure has no pose here.") };
        (start, next, pose_frames(l, fps).last().copied().unwrap_or(start))
    };
    let end = next.unwrap_or(n); // first frame after the block
    let by = start + frames - end;
    if by == 0 {
        return Ok(());
    }
    if next.is_none() {
        // the last block: it runs to the end of the animation
        ensure_length(st, start + frames);
        return Ok(());
    }
    let l = layer(st, sym, id)?;
    retime(l, fps, |fr| if fr >= end { fr + by } else { fr });
    ensure_length(st, last + by + 1);
    Ok(())
}

/// "Hold longer": the block under `frame` lasts one more frame.
pub fn hold_longer(st: &mut State, sym: &str, id: &str, frame: i32) -> EditResult<()> {
    let fps = fps(st);
    let n = frame_count(st);
    let (start, next) = {
        let l = layer(st, sym, id)?;
        let Some(b) = block_at(l, fps, frame) else { return fail("This figure has no pose here.") };
        b
    };
    set_block_length(st, sym, id, frame, next.unwrap_or(n) - start + 1)
}

/// Moves the block under `frame` so it starts on `to`, between the poses
/// before and after it (blocks do not pass each other).
pub fn move_block(st: &mut State, sym: &str, id: &str, frame: i32, to: i32) -> EditResult<()> {
    let fps = fps(st);
    let n = frame_count(st);
    let l = layer(st, sym, id)?;
    let keys = pose_frames(l, fps);
    let Some((start, next)) = block_at(l, fps, frame) else { return fail("This figure has no pose here.") };
    if to == start {
        return Ok(());
    }
    if start == 0 {
        // every figure is keyed on the first frame (D25)
        return fail("The first pose stays on the first frame.");
    }
    let prev = keys.iter().rev().find(|k| **k < start).copied();
    if to < 0 || prev.is_some_and(|p| to <= p) || next.is_some_and(|x| to >= x) || to >= n {
        return fail("A pose can move only as far as the poses on either side.");
    }
    retime(l, fps, |fr| if fr == start { to } else { fr });
    Ok(())
}
