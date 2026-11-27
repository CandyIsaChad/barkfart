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
    if matches!(l.kind, LayerKind::Camera(_)) || from >= to {
        return;
    }
    // what `to` shows now: the range's keys hold on past it, so it gets its
    // own key back (review, 2026-11-28: a moved block stayed shown after it)
    let after = shown_at(l, to, fps);
    for f in from..to {
        if shown_at(l, f, fps) != want {
            l.shown.track.put(Key::new(f as f64 / fps, Value::B(want), Interp::Hold));
        }
    }
    if shown_at(l, to, fps) != after {
        l.shown.track.put(Key::new(to as f64 / fps, Value::B(after), Interp::Hold));
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
                .filter(|x| x.id != id && matches!(x.kind, LayerKind::Figure(_) | LayerKind::Sprite(_) | LayerKind::Symbol(_) | LayerKind::Text(_) | LayerKind::Adjustment))
                .filter_map(|x| {
                    let last_pose = pose_frames(x, fps).into_iter().filter(|&f| shown_at(x, f, fps)).last();
                    // a layer merely shown on through the end isn't holding it
                    // (review, 2026-11-28): only a shown run that ends before
                    // the end counts, or any shown frame of a layer with no poses
                    let last_shown = if pose_frames(x, fps).is_empty() {
                        (0..n.max(1)).rev().find(|&f| shown_at(x, f, fps))
                    } else {
                        (0..n - 1).rev().find(|&f| shown_at(x, f, fps) && !shown_at(x, f + 1, fps))
                    };
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

/// Captures/bakes the exact current interpolated pose of `id` at `frame` into
/// pose keys on all `pose_props(l)` (if not already keyed there), so editing a
/// figure mid-tween or mid-hold splits the block cleanly with zero snap-back on
/// unedited joints.
pub fn bake_pose_at(st: &mut State, sym: &str, id: &str, frame: i32) -> EditResult<()> {
    if frame < 0 {
        return Ok(());
    }
    let fps = fps(st);
    let eps = 0.25 / fps;
    let t = frame as f64 / fps;
    let tw = st.project.settings.tween_interp;
    // Pre-sample join-interval local transform if this layer has timed joins.
    let join_local: Option<[f32; 5]> = {
        let Some(l) = st.project.symbol(sym).and_then(|s| s.layer(id)) else { return Ok(()) };
        if matches!(l.kind, LayerKind::Camera(_) | LayerKind::Audio(_) | LayerKind::Script(_) | LayerKind::Adjustment) {
            return Ok(());
        }
        let all_keyed = pose_props_of(l).iter().all(|q| q.track.keys().iter().any(|k| (k.time - t).abs() < eps));
        if all_keyed {
            return Ok(());
        }
        if !l.joins.is_empty() {
            let m = crate::eval::local_transform(&st.project, l, t);
            let ax = l.transform.anchor_x.value.f() as f64;
            let ay = l.transform.anchor_y.value.f() as f64;
            let old_rot = l.transform.rotation.key_value(t).f() as f64;
            let sx = m.a.hypot(m.b);
            let mut rot = m.b.atan2(m.a).to_degrees();
            let sy = if sx < 1e-12 { 0.0 } else { (m.a * m.d - m.b * m.c) / sx };
            let x = m.tx + m.a * ax + m.c * ay;
            let y = m.ty + m.b * ax + m.d * ay;
            rot += 360.0 * java_round_f64((old_rot - rot) / 360.0);
            Some([x as f32, y as f32, rot as f32, sx as f32, sy as f32])
        } else {
            None
        }
    };
    let Some(l) = st.project.layer_mut(sym, id) else { return Ok(()) };
    for (i, q) in pose_props(l).into_iter().enumerate() {
        if q.track.keys().iter().any(|k| (k.time - t).abs() < eps) {
            continue;
        }
        let v = if i < 5 {
            if let Some(jl) = join_local { Value::F(jl[i]) } else { q.key_value(t) }
        } else {
            q.key_value(t)
        };
        let interp = crate::edit::editor::interp_at(q, t, tw);
        q.track.put(Key::new(t, v, interp));
    }
    Ok(())
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
    move_block_ex(st, sym, id, frame, to, false)
}

/// The left grip: the pose starts at `to`, its end stays (a block on its
/// own, hidden before and after, grows or shrinks from the left; in a run of
/// poses the key moves, which is the same thing).
pub fn move_block_start(st: &mut State, sym: &str, id: &str, frame: i32, to: i32) -> EditResult<()> {
    move_block_ex(st, sym, id, frame, to, true)
}

fn move_block_ex(st: &mut State, sym: &str, id: &str, frame: i32, to: i32, keep_end: bool) -> EditResult<()> {
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
        if keep_end && isolated.is_some_and(|vl| to > start + vl - 1) {
            return fail("A pose lasts at least one frame.");
        }
        if has_key {
            retime(l, fps, |fr| if fr == start { to } else { fr });
        }
        isolated.map(|vl| (start, vl))
    };
    if let (true, Some((start, _))) = (keep_end, vis_info) {
        let l = layer(st, sym, id)?;
        if to < start {
            set_shown_range(l, to, start, fps, true);
        } else {
            set_shown_range(l, start, to, fps, false);
        }
        return Ok(());
    }
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
/// by `by` frames. Frame 0 poses stay pinned at 0; poses clamp >= 1 and props >= 0.
pub fn shift_keys(st: &mut State, sym: &str, poses: &[(String, i32)], props: &[(String, i32)], by: i32) -> EditResult<()> {
    shift_keys_ex(st, sym, poses, props, by, true).map(|_| ())
}

/// Shifts a batch of pose & property keys by one shared group-clamped `delta`
/// (so tracks never shear when one hits frame 1/0), handling isolated `shown`-only
/// blocks cleanly, and returning `(moved_poses, moved_props)`.
pub fn shift_keys_ex(
    st: &mut State,
    sym: &str,
    poses: &[(String, i32)],
    props: &[(String, i32)],
    by: i32,
    overwrite: bool,
) -> EditResult<(Vec<(String, i32)>, Vec<(String, i32)>)> {
    let fps = fps(st);
    let old_n = frame_count(st);
    if by == 0 {
        return Ok((poses.to_vec(), props.to_vec()));
    }

    // 1. Collect movable pose frames per layer (preserving frame 0 as pinned) and prop frames per prop.
    let mut pinned_poses: Vec<(String, i32)> = Vec::new();
    let mut by_layer: indexmap::IndexMap<&str, Vec<i32>> = indexmap::IndexMap::new();
    for (lid, fr) in poses {
        if *fr <= 0 {
            if *fr == 0 && !pinned_poses.iter().any(|(x, f)| x == lid && *f == 0) {
                pinned_poses.push((lid.clone(), 0));
            }
            continue;
        }
        let v = by_layer.entry(lid.as_str()).or_default();
        if !v.contains(fr) {
            v.push(*fr);
        }
    }
    let mut by_prop: indexmap::IndexMap<&str, Vec<i32>> = indexmap::IndexMap::new();
    for (pid, fr) in props {
        if *fr < 0 {
            continue;
        }
        let v = by_prop.entry(pid.as_str()).or_default();
        if !v.contains(fr) {
            v.push(*fr);
        }
    }

    // Resolve actual existing movable frames per layer (including shown-only visible blocks).
    struct LayerPlan {
        lid: String,
        moving_keys: Vec<i32>,
        isolated_spans: Vec<(i32, i32)>,
        existing_keys: Vec<i32>,
    }
    let mut layer_plans: Vec<LayerPlan> = Vec::new();
    let mut min_allowed = i32::MIN;
    let mut any_movable = false;

    for (lid, mut sel) in by_layer {
        sel.sort_unstable();
        let Some(l) = st.project.symbol(sym).and_then(|s| s.layer(lid)) else { continue };
        if matches!(l.kind, LayerKind::Camera(_) | LayerKind::Audio(_) | LayerKind::Script(_)) {
            continue;
        }
        let existing_keys = pose_frames(l, fps);
        let vbs = visible_blocks(l, fps, old_n);
        let mut moving_keys: Vec<i32> = Vec::new();
        let mut isolated_spans: Vec<(i32, i32)> = Vec::new();
        for fr in sel {
            let has_key = existing_keys.contains(&fr);
            if has_key {
                moving_keys.push(fr);
            }
            // Detect isolated visible span starting at `fr` (e.g. newly added/imported mid-timeline or shown-only layer)
            if let Some(&(_, vs, vl)) = vbs.iter().find(|&&(_, vs, _)| vs == fr) {
                let before_off = vs > 0 && !shown_at(l, vs - 1, fps);
                let after_off = vs + vl >= old_n || !shown_at(l, vs + vl, fps);
                if before_off && after_off {
                    isolated_spans.push((vs, vl));
                }
            }
        }
        let mut all_fr: Vec<i32> = moving_keys.iter().copied().chain(isolated_spans.iter().map(|(s, _)| *s)).collect();
        all_fr.sort_unstable();
        all_fr.dedup();
        if let Some(&first) = all_fr.first() {
            any_movable = true;
            min_allowed = min_allowed.max(1 - first);
            layer_plans.push(LayerPlan { lid: lid.to_string(), moving_keys, isolated_spans, existing_keys });
        }
    }

    struct PropPlan {
        pid: String,
        moving: Vec<i32>,
        existing: Vec<i32>,
    }
    let mut prop_plans: Vec<PropPlan> = Vec::new();
    for (pid, mut sel) in by_prop {
        sel.sort_unstable();
        let Some(q) = st.project.property(pid) else { continue };
        let existing: Vec<i32> = q.track.keys().iter().map(|k| (k.time * fps).round() as i32).collect();
        let moving: Vec<i32> = sel.into_iter().filter(|f| existing.contains(f)).collect();
        if let Some(&first) = moving.first() {
            any_movable = true;
            min_allowed = min_allowed.max(-first);
            prop_plans.push(PropPlan { pid: pid.to_string(), moving, existing });
        }
    }

    if !any_movable {
        return Ok((poses.to_vec(), props.to_vec()));
    }
    let delta = by.max(min_allowed);
    if delta == 0 {
        return Ok((poses.to_vec(), props.to_vec()));
    }

    // 2. Validate overlaps if !overwrite before mutating anything.
    if !overwrite {
        for lp in &layer_plans {
            for &fr in &lp.moving_keys {
                let dst = fr + delta;
                if lp.existing_keys.contains(&dst) && !lp.moving_keys.contains(&dst) {
                    return fail("Those poses would overlap another pose.");
                }
            }
        }
        for pp in &prop_plans {
            for &fr in &pp.moving {
                let dst = fr + delta;
                if pp.existing.contains(&dst) && !pp.moving.contains(&dst) {
                    return fail("Those keys would overlap another key.");
                }
            }
        }
    }

    // 3. Apply retiming cleanly across all planned layers and properties.
    let mut max_frame: Option<i32> = None;
    let mut moved_poses = pinned_poses;
    let mut moved_props: Vec<(String, i32)> = Vec::new();

    for lp in layer_plans {
        let mut all_moved_fr: Vec<i32> = lp.moving_keys.iter().copied().chain(lp.isolated_spans.iter().map(|(s, _)| *s)).collect();
        all_moved_fr.sort_unstable();
        all_moved_fr.dedup();
        for &fr in &all_moved_fr {
            let dst = fr + delta;
            moved_poses.push((lp.lid.clone(), dst));
            max_frame = Some(max_frame.map_or(dst, |m| m.max(dst)));
        }
        for &(vs, vl) in &lp.isolated_spans {
            let end_f = vs + delta + vl - 1;
            max_frame = Some(max_frame.map_or(end_f, |m| m.max(end_f)));
        }
        if !lp.moving_keys.is_empty() {
            let dsts: std::collections::HashSet<i32> = lp.moving_keys.iter().map(|&fr| fr + delta).collect();
            let l = layer(st, sym, &lp.lid)?;
            for q in pose_props(l) {
                let orig: Vec<Key> = q.track.keys().to_vec();
                q.track.clear();
                let mut moved = Vec::new();
                for k in orig {
                    let fr = (k.time * fps).round() as i32;
                    if lp.moving_keys.contains(&fr) {
                        moved.push(Key { time: (fr + delta) as f64 / fps, ..k });
                    } else if !overwrite || !dsts.contains(&fr) {
                        q.track.put(k);
                    }
                }
                for k in moved {
                    q.track.put(k);
                }
            }
        }
        if !lp.isolated_spans.is_empty() {
            let max_end = lp.isolated_spans.iter().map(|&(vs, vl)| vs + delta + vl).max().unwrap_or(old_n);
            ensure_length(st, max_end);
            let new_n = frame_count(st);
            let l = layer(st, sym, &lp.lid)?;
            for &(vs, vl) in &lp.isolated_spans {
                set_shown_range(l, vs, (vs + vl).min(new_n), fps, false);
            }
            for &(vs, vl) in &lp.isolated_spans {
                let dst = vs + delta;
                if dst > old_n && !shown_at(l, old_n - 1, fps) {
                    set_shown_range(l, old_n, dst, fps, false);
                }
                set_shown_range(l, dst, (dst + vl).min(new_n), fps, true);
            }
        }
    }

    for pp in prop_plans {
        let dsts: std::collections::HashSet<i32> = pp.moving.iter().map(|&fr| fr + delta).collect();
        for &fr in &pp.moving {
            let dst = fr + delta;
            moved_props.push((pp.pid.clone(), dst));
            max_frame = Some(max_frame.map_or(dst, |m| m.max(dst)));
        }
        let Some(q) = st.project.property_mut(&pp.pid) else { continue };
        let orig: Vec<Key> = q.track.keys().to_vec();
        q.track.clear();
        let mut moved = Vec::new();
        for k in orig {
            let fr = (k.time * fps).round() as i32;
            if pp.moving.contains(&fr) {
                moved.push(Key { time: (fr + delta) as f64 / fps, ..k });
            } else if !overwrite || !dsts.contains(&fr) {
                q.track.put(k);
            }
        }
        for k in moved {
            q.track.put(k);
        }
    }

    if let Some(m) = max_frame {
        ensure_length(st, m + 1);
    }
    Ok((moved_poses, moved_props))
}

/// Maps sorted unique frame indices around `anchor` by `factor`, clamping `>= min_frame`
/// and enforcing strict monotonic spacing `>= 1` frame so distinct keys NEVER collapse.
fn scale_frames_monotonic(sorted: &[i32], anchor: i32, factor: f64, min_frame: i32) -> Vec<(i32, i32)> {
    let k = sorted.len();
    if k == 0 {
        return Vec::new();
    }
    let mut dst: Vec<i32> = sorted.iter().map(|&f| (anchor as f64 + (f - anchor) as f64 * factor).round() as i32).collect();
    // If anchoring on the right edge, push left first so the right anchor stays pinned.
    if anchor >= sorted[k - 1] {
        dst[k - 1] = dst[k - 1].max(min_frame + (k as i32 - 1));
        for i in (0..k - 1).rev() {
            dst[i] = dst[i].min(dst[i + 1] - 1);
        }
    }
    // Forward pass enforces `>= min_frame` and strict `+1` separation.
    dst[0] = dst[0].max(min_frame);
    for i in 1..k {
        dst[i] = dst[i].max(dst[i - 1] + 1);
    }
    sorted.iter().copied().zip(dst).collect()
}

/// Proportionally scales selected pose & property keys around `anchor` by `factor` (> 0).
/// Frame 0 poses stay pinned at 0; poses clamp >= 1, props >= 0; strict per-track monotonic
/// spacing prevents distinct keys from ever collapsing together; stored Bezier/Smooth
/// handle time offsets scale with `factor`. Returns `(moved_poses, moved_props)`.
pub fn scale_keys(
    st: &mut State,
    sym: &str,
    poses: &[(String, i32)],
    props: &[(String, i32)],
    anchor: i32,
    factor: f64,
    overwrite: bool,
) -> EditResult<(Vec<(String, i32)>, Vec<(String, i32)>)> {
    if !(factor.is_finite() && factor > 0.0) {
        return fail("Scale factor must be positive.");
    }
    let factor = factor.clamp(0.05, 20.0);
    let fps = fps(st);
    let old_n = frame_count(st);

    let mut pinned_poses: Vec<(String, i32)> = Vec::new();
    let mut by_layer: indexmap::IndexMap<&str, Vec<i32>> = indexmap::IndexMap::new();
    for (lid, fr) in poses {
        if *fr <= 0 {
            if *fr == 0 && !pinned_poses.iter().any(|(x, f)| x == lid && *f == 0) {
                pinned_poses.push((lid.clone(), 0));
            }
            continue;
        }
        let v = by_layer.entry(lid.as_str()).or_default();
        if !v.contains(fr) {
            v.push(*fr);
        }
    }
    let mut by_prop: indexmap::IndexMap<&str, Vec<i32>> = indexmap::IndexMap::new();
    for (pid, fr) in props {
        if *fr < 0 {
            continue;
        }
        let v = by_prop.entry(pid.as_str()).or_default();
        if !v.contains(fr) {
            v.push(*fr);
        }
    }

    let mut max_frame: Option<i32> = None;
    let mut moved_poses = pinned_poses;
    let mut moved_props: Vec<(String, i32)> = Vec::new();

    for (lid, mut sel) in by_layer {
        sel.sort_unstable();
        let Some(l_ro) = st.project.symbol(sym).and_then(|s| s.layer(lid)) else { continue };
        if matches!(l_ro.kind, LayerKind::Camera(_) | LayerKind::Audio(_) | LayerKind::Script(_)) {
            continue;
        }
        let existing_keys = pose_frames(l_ro, fps);
        let vbs = visible_blocks(l_ro, fps, old_n);
        let mut movable: Vec<i32> = Vec::new();
        let mut isolated_map: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
        for fr in sel {
            let has_key = existing_keys.contains(&fr);
            let iso = vbs.iter().find(|&&(_, vs, _)| vs == fr).and_then(|&(_, vs, vl)| {
                let before_off = vs > 0 && !shown_at(l_ro, vs - 1, fps);
                let after_off = vs + vl >= old_n || !shown_at(l_ro, vs + vl, fps);
                (before_off && after_off).then_some(vl)
            });
            if has_key || iso.is_some() {
                movable.push(fr);
                if let Some(vl) = iso {
                    isolated_map.insert(fr, vl);
                }
            }
        }
        if movable.is_empty() {
            continue;
        }
        let mapping = scale_frames_monotonic(&movable, anchor, factor, 1);
        let src_to_dst: std::collections::HashMap<i32, i32> = mapping.iter().copied().collect();
        let dsts: std::collections::HashSet<i32> = mapping.iter().map(|&(_, d)| d).collect();
        if !overwrite {
            for &(_, dst) in &mapping {
                if existing_keys.contains(&dst) && !src_to_dst.contains_key(&dst) {
                    return fail("Those poses would overlap another pose.");
                }
            }
        }
        for &(_, dst) in &mapping {
            moved_poses.push((lid.to_string(), dst));
            max_frame = Some(max_frame.map_or(dst, |m| m.max(dst)));
        }
        if mapping.iter().any(|&(s, d)| s != d) {
            let l = layer(st, sym, lid)?;
            for q in pose_props(l) {
                let orig: Vec<Key> = q.track.keys().to_vec();
                q.track.clear();
                let mut moved = Vec::new();
                for k in orig {
                    let fr = (k.time * fps).round() as i32;
                    if let Some(&dst) = src_to_dst.get(&fr) {
                        let nk = if k.handle.stored() {
                            Key { time: dst as f64 / fps, lh: (k.lh.0 * factor, k.lh.1), rh: (k.rh.0 * factor, k.rh.1), ..k }
                        } else {
                            Key { time: dst as f64 / fps, ..k }
                        };
                        moved.push(nk);
                    } else if !overwrite || !dsts.contains(&fr) {
                        q.track.put(k);
                    }
                }
                for k in moved {
                    q.track.put(k);
                }
            }
            if !isolated_map.is_empty() {
                let max_end = mapping
                    .iter()
                    .filter_map(|&(s, d)| isolated_map.get(&s).map(|&vl| d + vl))
                    .max()
                    .unwrap_or(old_n);
                max_frame = Some(max_frame.map_or(max_end - 1, |m| m.max(max_end - 1)));
                ensure_length(st, max_end);
                let new_n = frame_count(st);
                let l = layer(st, sym, lid)?;
                for (&s, &vl) in &isolated_map {
                    set_shown_range(l, s, (s + vl).min(new_n), fps, false);
                }
                for &(s, d) in &mapping {
                    if let Some(&vl) = isolated_map.get(&s) {
                        if d > old_n && !shown_at(l, old_n - 1, fps) {
                            set_shown_range(l, old_n, d, fps, false);
                        }
                        set_shown_range(l, d, (d + vl).min(new_n), fps, true);
                    }
                }
            }
        }
    }

    for (pid, mut sel) in by_prop {
        sel.sort_unstable();
        let Some(q) = st.project.property(pid) else { continue };
        let existing: Vec<i32> = q.track.keys().iter().map(|k| (k.time * fps).round() as i32).collect();
        let movable: Vec<i32> = sel.into_iter().filter(|f| existing.contains(f)).collect();
        if movable.is_empty() {
            continue;
        }
        let mapping = scale_frames_monotonic(&movable, anchor, factor, 0);
        let src_to_dst: std::collections::HashMap<i32, i32> = mapping.iter().copied().collect();
        let dsts: std::collections::HashSet<i32> = mapping.iter().map(|&(_, d)| d).collect();
        if !overwrite {
            for &(_, dst) in &mapping {
                if existing.contains(&dst) && !src_to_dst.contains_key(&dst) {
                    return fail("Those keys would overlap another key.");
                }
            }
        }
        for &(_, dst) in &mapping {
            moved_props.push((pid.to_string(), dst));
            max_frame = Some(max_frame.map_or(dst, |m| m.max(dst)));
        }
        if mapping.iter().any(|&(s, d)| s != d) {
            let Some(q) = st.project.property_mut(pid) else { continue };
            let orig: Vec<Key> = q.track.keys().to_vec();
            q.track.clear();
            let mut moved = Vec::new();
            for k in orig {
                let fr = (k.time * fps).round() as i32;
                if let Some(&dst) = src_to_dst.get(&fr) {
                    let nk = if k.handle.stored() {
                        Key { time: dst as f64 / fps, lh: (k.lh.0 * factor, k.lh.1), rh: (k.rh.0 * factor, k.rh.1), ..k }
                    } else {
                        Key { time: dst as f64 / fps, ..k }
                    };
                    moved.push(nk);
                } else if !overwrite || !dsts.contains(&fr) {
                    q.track.put(k);
                }
            }
            for k in moved {
                q.track.put(k);
            }
        }
    }

    if let Some(m) = max_frame {
        ensure_length(st, m + 1);
    }
    Ok((moved_poses, moved_props))
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
