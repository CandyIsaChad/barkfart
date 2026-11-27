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

/// Whether `l` is shown on `frame` (cameras are always on).
pub fn shown_at(l: &Layer, frame: i32, fps: f64) -> bool {
    if matches!(l.kind, LayerKind::Camera(_)) {
        true
    } else {
        l.shown.key_value(frame as f64 / fps).b()
    }
}

fn set_shown_range(l: &mut Layer, from: i32, to: i32, fps: f64, want: bool) {
    if matches!(l.kind, LayerKind::Camera(_)) {
        return;
    }
    for f in from..to {
        if shown_at(l, f, fps) != want {
            l.shown.track.put(Key::new(f as f64 / fps, Value::B(want), Interp::Hold));
        }
    }
}

/// The frames a layer's pose is keyed on, in order.
pub fn pose_frames(l: &Layer, fps: f64) -> Vec<i32> {
    let mut v: Vec<i32> = pose_props_of(l).iter().flat_map(|q| q.track.keys().iter().map(|k| (k.time * fps).round() as i32)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Distinct pose start frames for block layout: `Camera` and `Text` are keyed on
/// every frame by `anchor_poses`, so equal consecutive frames merge into one span.
pub fn effective_pose_starts(l: &Layer, fps: f64) -> Vec<i32> {
    let starts = pose_frames(l, fps);
    if matches!(l.kind, LayerKind::Camera(_) | LayerKind::Text(_)) {
        let props = pose_props_of(l);
        let at = |f: i32| props.iter().map(|q| q.key_value(f as f64 / fps)).collect::<Vec<_>>();
        let mut kept: Vec<i32> = Vec::new();
        for f in starts {
            if kept.last().is_none_or(|&prev| at(prev) != at(f)) {
                kept.push(f);
            }
        }
        kept
    } else {
        starts
    }
}

fn push_shown_runs(l: &Layer, fps: f64, from: i32, to: i32, raw_key: Option<i32>, out: &mut Vec<(i32, i32, i32)>) {
    let mut f = from.max(0);
    while f < to {
        if !shown_at(l, f, fps) {
            f += 1;
            continue;
        }
        let a = f;
        while f < to && shown_at(l, f, fps) {
            f += 1;
        }
        let raw = match raw_key {
            Some(st) if a == from.max(0) => st,
            _ => a,
        };
        out.push((raw, a, f - a));
    }
}

/// Visible pose blocks on `[0, n)`: `(pose_key_frame, vis_start, vis_len)`.
/// Unposed layers (`pose_frames` empty) fall back to contiguous `shown == true` runs
/// so fresh/newly-added figures, pictures, movieclips, and textfields are extendable.
pub fn visible_blocks(l: &Layer, fps: f64, n: i32) -> Vec<(i32, i32, i32)> {
    if matches!(l.kind, LayerKind::Audio(_) | LayerKind::Script(_)) {
        return Vec::new();
    }
    let n = n.max(1);
    let starts = effective_pose_starts(l, fps);
    let mut out = Vec::new();
    if starts.is_empty() {
        push_shown_runs(l, fps, 0, n, None, &mut out);
        return out;
    }
    if starts[0] > 0 {
        push_shown_runs(l, fps, 0, starts[0].min(n), None, &mut out);
    }
    for (i, &st) in starts.iter().enumerate() {
        if st >= n {
            break;
        }
        let raw_end = starts.get(i + 1).copied().unwrap_or(n).min(n);
        push_shown_runs(l, fps, st, raw_end, Some(st), &mut out);
    }
    out
}

/// The visible block covering `frame` (or the pose span starting at/before `frame`):
/// `(pose_key_frame, vis_start, vis_len, next_bound_frame)`.
pub fn visible_block_at(l: &Layer, fps: f64, n: i32, frame: i32) -> Option<(i32, i32, i32, Option<i32>)> {
    let n = n.max(1);
    let vbs = visible_blocks(l, fps, n);
    if vbs.is_empty() {
        return None;
    }
    let idx = vbs
        .iter()
        .position(|&(rs, vs, vl)| (frame >= vs && frame < vs + vl) || rs == frame)
        .or_else(|| vbs.iter().rposition(|&(_, vs, _)| vs <= frame))?;
    let (raw_start, vis_start, vis_len) = vbs[idx];
    let next_vis = vbs.get(idx + 1).map(|&(_, vs, _)| vs);
    let next_pose = effective_pose_starts(l, fps).into_iter().find(|&k| k > raw_start.max(vis_start));
    let next = match (next_vis, next_pose) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    Some((raw_start, vis_start, vis_len, next))
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
/// a new block), with the pose it has there. Works on empty/runway frames too.
pub fn new_pose(st: &mut State, sym: &str, id: &str, frame: i32) -> EditResult<()> {
    if frame < 0 {
        return fail("Invalid frame.");
    }
    let fps = fps(st);
    let old_n = frame_count(st);
    let t = frame as f64 / fps;
    let tw = st.project.settings.tween_interp;
    {
        let l = layer(st, sym, id)?;
        let was_shown = frame < old_n && shown_at(l, frame, fps);
        if pose_frames(l, fps).contains(&frame) && was_shown {
            return fail("There is a pose on this frame already.");
        }
        if frame > old_n && !shown_at(l, old_n - 1, fps) {
            set_shown_range(l, old_n, frame, fps, false);
        }
        set_shown_range(l, frame, frame + 1, fps, true);
        for q in pose_props(l) {
            let v = q.key_value(t);
            let interp = crate::edit::editor::interp_at(q, t, tw);
            q.track.put(Key::new(t, v, interp));
        }
    }
    ensure_length(st, frame + 1);
    Ok(())
}

/// The last occupied frame (pose or shown) of any other drawn/timed layer in `sym` (0 when none).
fn others_last(st: &State, sym: &str, id: &str, fps: f64, n: i32) -> i32 {
    st.project
        .symbol(sym)
        .map(|s| {
            s.layers
                .iter()
                .filter(|x| x.id != id && matches!(x.kind, LayerKind::Figure(_) | LayerKind::Sprite(_) | LayerKind::Symbol(_) | LayerKind::Text(_)))
                .filter_map(|x| {
                    let last_pose = pose_frames(x, fps).into_iter().filter(|&f| shown_at(x, f, fps)).last();
                    let last_shown = (0..n.max(1)).rev().find(|&f| shown_at(x, f, fps));
                    match (last_pose, last_shown) {
                        (Some(a), Some(b)) => Some(a.max(b)),
                        (a, b) => a.or(b),
                    }
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// The block under `frame` lasts `frames` frames: the figure's later poses
/// move with its end, and `shown` updates across `[start .. start + frames)` so
/// newly added one-frame figures extend and shorten cleanly on any timeline.
pub fn set_block_length(st: &mut State, sym: &str, id: &str, frame: i32, frames: i32) -> EditResult<()> {
    if frames < 1 {
        return fail("A pose lasts at least one frame.");
    }
    let fps = fps(st);
    let n = frame_count(st);
    let (raw_start, vis_start, vis_len, next, last) = {
        let l = layer(st, sym, id)?;
        if let Some((rs, vs, vl, nx)) = visible_block_at(l, fps, n, frame) {
            let last_pose = pose_frames(l, fps).last().copied().unwrap_or(rs);
            let last_vis = (0..n).rev().find(|&f| shown_at(l, f, fps)).unwrap_or(last_pose);
            (rs, vs, vl, nx, last_pose.max(last_vis))
        } else {
            let Some((rs, nx)) = block_at(l, fps, frame) else { return fail("This figure has no pose here.") };
            let last_pose = pose_frames(l, fps).last().copied().unwrap_or(rs);
            (rs, rs, nx.unwrap_or(n).max(rs + 1) - rs, nx, last_pose)
        }
    };
    let _ = raw_start;
    let start = vis_start;
    let vis_end = start + vis_len;
    let new_vis_end = start + frames;
    if new_vis_end == vis_end {
        return Ok(());
    }
    let others = others_last(st, sym, id, fps, n);
    if next.is_none() {
        let final_n = if new_vis_end > n {
            ensure_length(st, new_vis_end);
            new_vis_end
        } else if vis_end >= n {
            // previously ran to the end of the animation: shrink animation down to other layers' end
            let to = new_vis_end.max(others + 1).max(start + 1).max(1);
            if let Some(r) = st.project.root_mut() {
                r.duration = to as f64 / fps;
            }
            to
        } else {
            // previously ended inside a longer timeline: keep timeline length, update shown
            n
        };
        let l = layer(st, sym, id)?;
        set_shown_range(l, start, new_vis_end, fps, true);
        if new_vis_end < final_n {
            set_shown_range(l, new_vis_end, final_n, fps, false);
        }
        return Ok(());
    }
    let end = next.unwrap();
    // If there is an unshown gap before `end` and `new_vis_end <= end`, fill/trim inside the gap
    // without disturbing later pose keys.
    if vis_end < end && new_vis_end <= end {
        let l = layer(st, sym, id)?;
        set_shown_range(l, start, new_vis_end, fps, true);
        set_shown_range(l, new_vis_end, end, fps, false);
        return Ok(());
    }
    let by = new_vis_end - end;
    let owns_end = last >= others;
    {
        let l = layer(st, sym, id)?;
        retime(l, fps, |fr| if fr >= end { fr + by } else { fr });
        if !l.shown.track.is_empty() {
            let old_shown: Vec<(i32, bool)> = (0..n).map(|f| (f, shown_at(l, f, fps))).collect();
            for (f, val) in old_shown {
                if f >= end {
                    let nf = f + by;
                    if nf >= 0 {
                        l.shown.track.put(Key::new(nf as f64 / fps, Value::B(val), Interp::Hold));
                    }
                }
            }
        }
        set_shown_range(l, start, new_vis_end, fps, true);
    }
    if owns_end {
        let to = (n + by).max(last + by + 1).max(others + 1).max(1);
        if let Some(r) = st.project.root_mut() {
            r.duration = to as f64 / fps;
        }
    } else {
        ensure_length(st, last + by + 1);
    }
    Ok(())
}

/// "Hold longer": the block under `frame` lasts one more frame.
pub fn hold_longer(st: &mut State, sym: &str, id: &str, frame: i32) -> EditResult<()> {
    let fps = fps(st);
    let n = frame_count(st);
    let vis_len = {
        let l = layer(st, sym, id)?;
        if let Some((_, _, vl, _)) = visible_block_at(l, fps, n, frame) {
            vl
        } else {
            let Some((start, next)) = block_at(l, fps, frame) else { return fail("This figure has no pose here.") };
            next.unwrap_or(n) - start
        }
    };
    set_block_length(st, sym, id, frame, vis_len + 1)
}

/// Extends a layer's visible block to cover `target_frame` (1-tap "Extend here" on empty/runway cells).
pub fn extend_to(st: &mut State, sym: &str, id: &str, target_frame: i32) -> EditResult<()> {
    if target_frame < 0 {
        return fail("Invalid frame.");
    }
    let fps = fps(st);
    let n = frame_count(st);
    let vbs = {
        let l = layer(st, sym, id)?;
        visible_blocks(l, fps, n)
    };
    if let Some(&(_, vis_start, vis_len)) = vbs.iter().rfind(|&&(_, vs, _)| vs <= target_frame) {
        if target_frame < vis_start + vis_len {
            return Ok(());
        }
        return set_block_length(st, sym, id, vis_start, target_frame - vis_start + 1);
    }
    if let Some(&(first_raw, first_vis_start, _)) = vbs.first() {
        // target_frame < first_vis_start: extend backwards to target_frame
        let l = layer(st, sym, id)?;
        set_shown_range(l, target_frame, first_vis_start, fps, true);
        if first_raw == first_vis_start && first_raw > 0 && target_frame > 0 && pose_frames(l, fps).contains(&first_raw) && !pose_frames(l, fps).contains(&target_frame) {
            retime(l, fps, |fr| if fr == first_raw { target_frame } else { fr });
        }
        return Ok(());
    }
    // Layer had no visible frames at all: show it on target_frame
    ensure_length(st, target_frame + 1);
    let new_n = frame_count(st);
    let l = layer(st, sym, id)?;
    if target_frame > n && !shown_at(l, n - 1, fps) {
        set_shown_range(l, n, target_frame, fps, false);
    }
    set_shown_range(l, target_frame, (target_frame + 1).min(new_n), fps, true);
    Ok(())
}

/// Moves the block under `frame` so it starts on `to`, between the poses
/// before and after it (blocks do not pass each other).
pub fn move_block(st: &mut State, sym: &str, id: &str, frame: i32, to: i32) -> EditResult<()> {
    let fps = fps(st);
    let n = frame_count(st);
    let vis_info = {
        let l = layer(st, sym, id)?;
        let keys = effective_pose_starts(l, fps);
        let (start, next, has_key) = if let Some((_, vs, vl, nx)) = visible_block_at(l, fps, n, frame) {
            if keys.is_empty() || !pose_frames(l, fps).contains(&vs) {
                (vs, nx, false)
            } else {
                let _ = vl;
                let nx_key = keys.iter().find(|k| **k > vs).copied();
                (vs, nx_key.or(nx), true)
            }
        } else {
            let Some((start, next)) = block_at(l, fps, frame) else { return fail("This figure has no pose here.") };
            (start, next, true)
        };
        if to == start {
            return Ok(());
        }
        if start == 0 {
            // every figure is keyed on the first frame (D25)
            return fail("The first pose stays on the first frame.");
        }
        let prev = visible_blocks(l, fps, n)
            .into_iter()
            .filter_map(|(_, vs, vl)| (vs + vl - 1 < start).then_some(if has_key { vs } else { vs + vl - 1 }))
            .last()
            .or_else(|| keys.iter().rev().find(|k| **k < start).copied());
        if to < 0 || prev.is_some_and(|p| to <= p) || next.is_some_and(|x| to >= x) || to >= n {
            return fail("A pose can move only as far as the poses on either side.");
        }
        let isolated = (!shown_at(l, start - 1, fps))
            .then(|| visible_block_at(l, fps, n, start))
            .flatten()
            .filter(|(_, vs, _, _)| *vs == start)
            .map(|(_, _, vl, _)| (vl, start + vl >= n || !shown_at(l, start + vl, fps)))
            .filter(|(_, after_off)| *after_off)
            .map(|(vl, _)| vl);
        if has_key {
            retime(l, fps, |fr| if fr == start { to } else { fr });
        }
        isolated.map(|vl| (start, vl))
    };
    if let Some((start, vl)) = vis_info {
        ensure_length(st, to + vl);
        let new_n = frame_count(st);
        let l = layer(st, sym, id)?;
        set_shown_range(l, start, (start + vl).min(new_n), fps, false);
        set_shown_range(l, to, (to + vl).min(new_n), fps, true);
    }
    Ok(())
}

/// "Delete this pose": the figure's key under `frame` goes, so the pose
/// before it lasts until the next. The first pose stays (delete the figure
/// instead).
pub fn delete_pose(st: &mut State, sym: &str, id: &str, frame: i32) -> EditResult<()> {
    let fps = fps(st);
    let l = layer(st, sym, id)?;
    let Some((start, _)) = block_at(l, fps, frame) else { return fail("This figure has no pose here.") };
    if start == 0 {
        return fail("The first pose stays; delete the figure to take it away.");
    }
    let t = start as f64 / fps;
    for q in pose_props(l) {
        if let Ok(i) = q.track.index_of(t) {
            q.track.remove_at(i);
        }
    }
    Ok(())
}

/// Moves a layer's pose key at `from` (>0) to any unoccupied frame `to` (>=1),
/// crossing other pose keys freely. Grows duration if `to >= frames`.
pub fn move_pose_key(st: &mut State, sym: &str, id: &str, from: i32, to: i32) -> EditResult<()> {
    if from == to {
        return Ok(());
    }
    if from == 0 {
        return fail("The first pose stays on the first frame.");
    }
    if to < 1 {
        return fail("A pose cannot move to the first frame.");
    }
    let fps = fps(st);
    let old_n = frame_count(st);
    let vis_info = {
        let l = layer(st, sym, id)?;
        let keys = pose_frames(l, fps);
        let has_key = keys.contains(&from);
        let isolated = (!shown_at(l, from - 1, fps))
            .then(|| visible_block_at(l, fps, old_n, from))
            .flatten()
            .filter(|(_, vs, _, _)| *vs == from)
            .map(|(_, _, vl, _)| (vl, from + vl >= old_n || !shown_at(l, from + vl, fps)))
            .filter(|(_, after_off)| *after_off)
            .map(|(vl, _)| vl);
        if !has_key && isolated.is_none() {
            return fail("There is no pose on this frame.");
        }
        if keys.contains(&to) || (!has_key && to < old_n && shown_at(l, to, fps) && !(to >= from && to < from + isolated.unwrap_or(1))) {
            return fail("There is already a pose on that frame.");
        }
        if has_key {
            retime(l, fps, |fr| if fr == from { to } else { fr });
        }
        isolated
    };
    if let Some(vl) = vis_info {
        ensure_length(st, to + vl);
        let new_n = frame_count(st);
        let l = layer(st, sym, id)?;
        if to > old_n && !shown_at(l, old_n - 1, fps) {
            set_shown_range(l, old_n, to, fps, false);
        }
        set_shown_range(l, from, (from + vl).min(new_n), fps, false);
        set_shown_range(l, to, (to + vl).min(new_n), fps, true);
    } else {
        ensure_length(st, to + 1);
    }
    Ok(())
}

/// Duplicates a layer's pose key at `from` (values, interp, ease, handles) to
/// `to` (or the first free frame > `from`). Returns the frame it landed on.
pub fn duplicate_pose_key(st: &mut State, sym: &str, id: &str, from: i32, to: Option<i32>) -> EditResult<i32> {
    let fps = fps(st);
    let tw = st.project.settings.tween_interp;
    let target = {
        let l = layer(st, sym, id)?;
        let keys = pose_frames(l, fps);
        if !keys.contains(&from) {
            return fail("There is no pose on this frame.");
        }
        let target = match to {
            Some(t) => {
                if t < 0 {
                    return fail("Invalid frame.");
                }
                if keys.contains(&t) {
                    return fail("There is already a pose on that frame.");
                }
                t
            }
            None => (from + 1..).find(|f| !keys.contains(f)).unwrap_or(from + 1),
        };
        let t_from = from as f64 / fps;
        let t_to = target as f64 / fps;
        for q in pose_props(l) {
            let k = q
                .track
                .keys()
                .iter()
                .find(|k| (k.time * fps).round() as i32 == from)
                .copied()
                .unwrap_or_else(|| {
                    let v = q.key_value(t_from);
                    let interp = crate::edit::editor::interp_at(q, t_from, tw);
                    Key::new(t_from, v, interp)
                });
            q.track.put(Key { time: t_to, ..k });
        }
        target
    };
    ensure_length(st, target + 1);
    Ok(target)
}

/// Moves one property's key at `from` to any unoccupied frame `to` (>=0).
pub fn move_prop_key(st: &mut State, prop_id: &str, from: i32, to: i32) -> EditResult<()> {
    if from == to {
        return Ok(());
    }
    if to < 0 {
        return fail("Frame must be at least 1.");
    }
    let fps = fps(st);
    {
        let Some(q) = st.project.property_mut(prop_id) else { return fail("There is no such value.") };
        let Some(idx) = q.track.keys().iter().position(|k| (k.time * fps).round() as i32 == from) else {
            return fail("There is no key on that frame.");
        };
        if q.track.keys().iter().any(|k| (k.time * fps).round() as i32 == to) {
            return fail("There is already a key on that frame.");
        }
        let k = q.track.keys()[idx];
        q.track.remove_at(idx);
        q.track.put(Key { time: to as f64 / fps, ..k });
    }
    ensure_length(st, to + 1);
    Ok(())
}

/// Duplicates one property's key at `from` to `to` (or the first free frame > `from`).
pub fn duplicate_prop_key(st: &mut State, prop_id: &str, from: i32, to: Option<i32>) -> EditResult<i32> {
    let fps = fps(st);
    let target = {
        let Some(q) = st.project.property_mut(prop_id) else { return fail("There is no such value.") };
        let Some(k) = q.track.keys().iter().find(|k| (k.time * fps).round() as i32 == from).copied() else {
            return fail("There is no key on that frame.");
        };
        let existing: Vec<i32> = q.track.keys().iter().map(|x| (x.time * fps).round() as i32).collect();
        let target = match to {
            Some(t) => {
                if t < 0 {
                    return fail("Frame must be at least 1.");
                }
                if existing.contains(&t) {
                    return fail("There is already a key on that frame.");
                }
                t
            }
            None => (from + 1..).find(|f| !existing.contains(f)).unwrap_or(from + 1),
        };
        q.track.put(Key { time: target as f64 / fps, ..k });
        target
    };
    ensure_length(st, target + 1);
    Ok(target)
}

/// Deletes one property's key at `frame`. If 1 or 0 keys remain, keeps the
/// surviving (or deleted) value as static so nothing jumps to an old default.
pub fn delete_prop_key(st: &mut State, prop_id: &str, frame: i32) -> EditResult<()> {
    let fps = fps(st);
    let Some(q) = st.project.property_mut(prop_id) else { return fail("There is no such value.") };
    let Some(idx) = q.track.keys().iter().position(|k| (k.time * fps).round() as i32 == frame) else {
        return fail("There is no key on that frame.");
    };
    let removed = q.track.keys()[idx];
    q.track.remove_at(idx);
    if q.track.len() == 1 {
        let v = q.track.keys()[0].value;
        q.track.clear();
        q.value = v;
    } else if q.track.is_empty() {
        q.value = removed.value;
    }
    Ok(())
}

/// Pastes a copied property key onto `prop_id` at `frame` (>=0).
pub fn paste_prop_key(st: &mut State, prop_id: &str, frame: i32, ty: ValueType, key: Key) -> EditResult<()> {
    if frame < 0 {
        return fail("Frame must be at least 1.");
    }
    let fps = fps(st);
    {
        let Some(q) = st.project.property_mut(prop_id) else { return fail("There is no such value.") };
        if q.ty != ty || !q.ty.checked(&key.value) {
            return fail("That copied key is a different kind of value.");
        }
        q.track.put(Key { time: frame as f64 / fps, ..key });
    }
    ensure_length(st, frame + 1);
    Ok(())
}

/// Shifts a batch of pose keys `(layer_id, frame)` and property keys `(prop_id, frame)`
/// by `by` frames. Frame 0 poses are skipped; poses clamp >= 1 and props >= 0.
pub fn shift_keys(st: &mut State, sym: &str, poses: &[(String, i32)], props: &[(String, i32)], by: i32) -> EditResult<()> {
    if by == 0 {
        return Ok(());
    }
    let fps = fps(st);
    let mut max_frame: Option<i32> = None;

    let mut by_layer: indexmap::IndexMap<&str, Vec<i32>> = indexmap::IndexMap::new();
    for (lid, fr) in poses {
        if *fr > 0 {
            let v = by_layer.entry(lid.as_str()).or_default();
            if !v.contains(fr) {
                v.push(*fr);
            }
        }
    }
    for (lid, mut sel) in by_layer {
        sel.sort_unstable();
        let Ok(l) = layer(st, sym, lid) else { continue };
        let existing = pose_frames(l, fps);
        let moving: Vec<i32> = sel.into_iter().filter(|f| existing.contains(f)).collect();
        if moving.is_empty() {
            continue;
        }
        let delta = by.max(1 - moving[0]);
        if delta == 0 {
            continue;
        }
        for &fr in &moving {
            let dst = fr + delta;
            if existing.contains(&dst) && !moving.contains(&dst) {
                return fail("Those poses would overlap another pose.");
            }
            max_frame = Some(max_frame.map_or(dst, |m| m.max(dst)));
        }
        retime(l, fps, |fr| if moving.contains(&fr) { fr + delta } else { fr });
    }

    let mut by_prop: indexmap::IndexMap<&str, Vec<i32>> = indexmap::IndexMap::new();
    for (pid, fr) in props {
        let v = by_prop.entry(pid.as_str()).or_default();
        if !v.contains(fr) {
            v.push(*fr);
        }
    }
    for (pid, mut sel) in by_prop {
        sel.sort_unstable();
        let Some(q) = st.project.property_mut(pid) else { continue };
        let existing: Vec<i32> = q.track.keys().iter().map(|k| (k.time * fps).round() as i32).collect();
        let moving: Vec<i32> = sel.into_iter().filter(|f| existing.contains(f)).collect();
        if moving.is_empty() {
            continue;
        }
        let delta = by.max(-moving[0]);
        if delta == 0 {
            continue;
        }
        for &fr in &moving {
            let dst = fr + delta;
            if existing.contains(&dst) && !moving.contains(&dst) {
                return fail("Those keys would overlap another key.");
            }
            max_frame = Some(max_frame.map_or(dst, |m| m.max(dst)));
        }
        let keys: Vec<Key> = q.track.keys().to_vec();
        q.track.clear();
        for k in keys {
            let fr = (k.time * fps).round() as i32;
            let nfr = if moving.contains(&fr) { fr + delta } else { fr };
            q.track.put(Key { time: nfr as f64 / fps, ..k });
        }
    }

    if let Some(m) = max_frame {
        ensure_length(st, m + 1);
    }
    Ok(())
}

/// Deletes a batch of pose keys (skipping frame 0) and property keys.
pub fn delete_keys(st: &mut State, sym: &str, poses: &[(String, i32)], props: &[(String, i32)]) -> EditResult<()> {
    let fps = fps(st);
    for (lid, fr) in poses {
        if *fr <= 0 {
            continue;
        }
        let Ok(l) = layer(st, sym, lid) else { continue };
        for q in pose_props(l) {
            if let Some(i) = q.track.keys().iter().position(|k| (k.time * fps).round() as i32 == *fr) {
                q.track.remove_at(i);
            }
        }
    }
    let mut by_prop: indexmap::IndexMap<&str, Vec<i32>> = indexmap::IndexMap::new();
    for (pid, fr) in props {
        let v = by_prop.entry(pid.as_str()).or_default();
        if !v.contains(fr) {
            v.push(*fr);
        }
    }
    for (pid, frames) in by_prop {
        let Some(q) = st.project.property_mut(pid) else { continue };
        let mut last_removed: Option<Value> = None;
        for fr in frames {
            if let Some(i) = q.track.keys().iter().position(|k| (k.time * fps).round() as i32 == fr) {
                last_removed = Some(q.track.keys()[i].value);
                q.track.remove_at(i);
            }
        }
        if last_removed.is_some() {
            if q.track.len() == 1 {
                let v = q.track.keys()[0].value;
                q.track.clear();
                q.value = v;
            } else if q.track.is_empty() {
                if let Some(v) = last_removed {
                    q.value = v;
                }
            }
        }
    }
    Ok(())
}

/// Sets easing on a batch of pose keys and property keys.
pub fn ease_keys(st: &mut State, sym: &str, poses: &[(String, i32)], props: &[(String, i32)], interp: Interp, ease: Ease) -> EditResult<()> {
    let fps = fps(st);
    for (lid, fr) in poses {
        let Ok(l) = layer(st, sym, lid) else { continue };
        for q in pose_props(l) {
            if !q.ty.continuous() {
                continue;
            }
            let keys: Vec<Key> = q
                .track
                .keys()
                .iter()
                .map(|k| if (k.time * fps).round() as i32 == *fr { Key { interp, ease, ..*k } } else { *k })
                .collect();
            q.track.clear();
            for k in keys {
                q.track.put(k);
            }
        }
    }
    for (pid, fr) in props {
        let Some(q) = st.project.property_mut(pid) else { continue };
        if !q.ty.continuous() {
            continue;
        }
        let keys: Vec<Key> = q
            .track
            .keys()
            .iter()
            .map(|k| if (k.time * fps).round() as i32 == *fr { Key { interp, ease, ..*k } } else { *k })
            .collect();
        q.track.clear();
        for k in keys {
            q.track.put(k);
        }
    }
    Ok(())
}

fn mirror_key_handles(k: Key) -> Key {
    if k.handle.stored() {
        Key { lh: (-k.rh.0, k.rh.1), rh: (-k.lh.0, k.lh.1), ..k }
    } else {
        k
    }
}

/// Reverses pose and animated property keys within `[from, to]` in place.
/// When `only_layer` is `None`, applies to every non-audio, non-script layer.
pub fn reverse_range(st: &mut State, sym: &str, only_layer: Option<&str>, from: i32, to: i32) -> EditResult<()> {
    if from < 0 || to <= from {
        return fail("Choose a range of at least two frames.");
    }
    let fps = fps(st);
    let tw = st.project.settings.tween_interp;
    let ids: Vec<String> = match only_layer {
        Some(id) => {
            if st.project.symbol(sym).and_then(|s| s.layer(id)).is_none() {
                return fail(format!("no layer {id}"));
            }
            vec![id.to_string()]
        }
        None => st
            .project
            .symbol(sym)
            .map(|s| s.layers.iter().filter(|l| !matches!(l.kind, LayerKind::Audio(_) | LayerKind::Script(_))).map(|l| l.id.clone()).collect())
            .unwrap_or_default(),
    };
    let t_to = to as f64 / fps;
    for id in ids {
        let Some(l) = st.project.layer_mut(sym, &id) else { continue };
        let pose_ids: std::collections::HashSet<String> = pose_props_of(l).into_iter().map(|q| q.id.clone()).collect();
        for q in l.properties_mut() {
            if q.track.is_empty() {
                continue;
            }
            let is_pose = pose_ids.contains(&q.id);
            let mut in_range: Vec<(i32, Key)> = Vec::new();
            let mut outside: Vec<Key> = Vec::new();
            for &k in q.track.keys() {
                let fr = (k.time * fps).round() as i32;
                if fr >= from && fr <= to {
                    in_range.push((fr, k));
                } else {
                    outside.push(k);
                }
            }
            if in_range.is_empty() {
                continue;
            }
            // If reversing from frame 0 and `to` had no explicit pose key, preserve the
            // pose visible at `to` so frame 0 receives it after mirroring.
            let need_end = is_pose && from == 0 && !in_range.iter().any(|(fr, _)| *fr == to);
            let end_key = need_end.then(|| {
                let v = q.key_value(t_to);
                let interp = crate::edit::editor::interp_at(q, t_to, tw);
                Key::new(t_to, v, interp)
            });
            q.track.clear();
            for k in outside {
                q.track.put(k);
            }
            for (fr, k) in in_range {
                let dst = from + to - fr;
                let mk = mirror_key_handles(k);
                q.track.put(Key { time: dst as f64 / fps, ..mk });
            }
            if let Some(ek) = end_key {
                q.track.put(Key { time: from as f64 / fps, ..ek });
            }
        }
    }
    ensure_length(st, to + 1);
    Ok(())
}

/// Appends the reversed `[from, to - 1]` range immediately after `to` (ping-pong),
/// shifting any later keys forward by `to - from` and growing duration cleanly.
pub fn ping_pong_range(st: &mut State, sym: &str, only_layer: Option<&str>, from: i32, to: i32) -> EditResult<()> {
    if from < 0 || to <= from {
        return fail("Choose a range of at least two frames.");
    }
    let fps = fps(st);
    let tw = st.project.settings.tween_interp;
    let added = to - from;
    let n = frame_count(st);
    let ids: Vec<String> = match only_layer {
        Some(id) => {
            if st.project.symbol(sym).and_then(|s| s.layer(id)).is_none() {
                return fail(format!("no layer {id}"));
            }
            vec![id.to_string()]
        }
        None => st
            .project
            .symbol(sym)
            .map(|s| s.layers.iter().filter(|l| !matches!(l.kind, LayerKind::Audio(_) | LayerKind::Script(_))).map(|l| l.id.clone()).collect())
            .unwrap_or_default(),
    };
    let t_to = to as f64 / fps;
    for id in ids {
        let Some(l) = st.project.layer_mut(sym, &id) else { continue };
        let pose_ids: std::collections::HashSet<String> = pose_props_of(l).into_iter().map(|q| q.id.clone()).collect();
        for q in l.properties_mut() {
            if q.track.is_empty() {
                continue;
            }
            let is_pose = pose_ids.contains(&q.id);
            let orig: Vec<Key> = q.track.keys().to_vec();
            let has_to = orig.iter().any(|k| (k.time * fps).round() as i32 == to);
            let turn_key = (is_pose && !has_to).then(|| {
                let v = q.key_value(t_to);
                let interp = crate::edit::editor::interp_at(q, t_to, tw);
                Key::new(t_to, v, interp)
            });
            let mut mirrored: Vec<Key> = Vec::new();
            q.track.clear();
            for k in orig {
                let fr = (k.time * fps).round() as i32;
                if fr > to {
                    q.track.put(Key { time: (fr + added) as f64 / fps, ..k });
                } else {
                    q.track.put(k);
                    if fr >= from && fr < to {
                        let dst = to + (to - fr);
                        let mk = mirror_key_handles(k);
                        mirrored.push(Key { time: dst as f64 / fps, ..mk });
                    }
                }
            }
            if let Some(tk) = turn_key {
                q.track.put(tk);
            }
            for k in mirrored {
                q.track.put(k);
            }
        }
    }
    let new_len = (n + added).max(to + added + 1);
    ensure_length(st, new_len);
    Ok(())
}
