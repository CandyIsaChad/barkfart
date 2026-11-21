//! The frame strip (port of Java FrameOps): frames are slots at the project
//! fps over the root symbol's keyed tracks. Each call is one undo step for
//! the caller.

use super::editor::{interp_at, Editor, KeyRef};
use super::{EditResult, State};
use crate::model::*;

fn fps(st: &State) -> f64 {
    st.project.settings.fps as f64
}

pub fn frame_count(st: &State) -> i32 {
    let d = st.project.root().map(|s| s.duration).unwrap_or(0.0);
    1.max((d * fps(st)).round() as i32)
}

pub fn time_of(fps: f64, frame: i32) -> f64 {
    frame as f64 / fps
}

pub fn time(st: &State, frame: i32) -> f64 {
    frame as f64 / fps(st)
}

pub fn frame_at(st: &State, t: f64) -> i32 {
    ((t * fps(st)).round() as i32).clamp(0, frame_count(st) - 1)
}

/// (layer id, property id) of every property of the root symbol, in order.
fn props(st: &State) -> Vec<(String, String)> {
    let Some(s) = st.project.root() else { return Vec::new() };
    s.layers.iter().flat_map(|l| l.properties().into_iter().map(|p| (l.id.clone(), p.id.clone())).collect::<Vec<_>>()).collect()
}

/// Rewrites one property's keys with `f(track) -> new keys`.
fn rewrite(st: &mut State, layer: &str, id: &str, f: impl FnOnce(&Property) -> Option<Vec<Key>>) {
    let root = st.project.root_symbol.clone();
    let Some(l) = st.project.layer_mut(&root, layer) else { return };
    let Some(p) = l.properties_mut().into_iter().find(|p| p.id == id) else { return };
    if let Some(keys) = f(p) {
        p.track.clear();
        for k in keys {
            p.track.put(k);
        }
    }
}

/// The audio layer holding the frames' own sounds (Frame Tools > Sounds).
pub const FRAME_SOUNDS: &str = "Frame sounds";

/// The frames' own sounds follow their frames like keys: `f(start)` gives
/// the new start, or None to remove the clip. (Other audio layers stay on
/// the timeline, like the Java editor.)
fn move_clips(st: &mut State, f: impl Fn(f64) -> Option<f64>) {
    let root = st.project.root_symbol.clone();
    let ids: Vec<String> = st.project.root().map(|s| s.layers.iter().filter(|l| matches!(l.kind, LayerKind::Audio(_)) && l.name.as_deref() == Some(FRAME_SOUNDS)).map(|l| l.id.clone()).collect()).unwrap_or_default();
    for id in ids {
        let Some(l) = st.project.layer_mut(&root, &id) else { continue };
        let LayerKind::Audio(clips) = &mut l.kind else { continue };
        clips.retain_mut(|c| match f(c.start) {
            Some(v) => {
                c.start = v;
                true
            }
            None => false,
        });
    }
}

fn set_duration(st: &mut State, seconds: f64) {
    if let Some(s) = st.project.root_mut() {
        s.duration = seconds;
    }
}

pub fn has_keys(st: &State, frame: i32) -> bool {
    let (t, eps) = (time(st, frame), 0.25 / fps(st));
    let Some(s) = st.project.root() else { return false };
    s.layers.iter().any(|l| l.properties().iter().any(|p| p.track.keys().iter().any(|k| (k.time - t).abs() < eps)))
}

/// Inserts a frame after `frame`, shifting later keys; with copy_pose the
/// new frame repeats the pose of `frame`.
pub fn insert_after(st: &mut State, frame: i32, copy_pose: bool) -> EditResult<()> {
    let (t, dt, eps) = (time(st, frame), 1.0 / fps(st), 0.25 / fps(st));
    let n = frame_count(st);
    set_duration(st, (n + 1) as f64 / fps(st));
    move_clips(st, |c| Some(if c > t + eps { c + dt } else { c }));
    for (l, id) in props(st) {
        rewrite(st, &l, &id, |p| {
            if p.track.is_empty() {
                return None;
            }
            let mut out = Track::new(p.ty);
            for k in p.track.keys() {
                out.put(if k.time > t + eps { Key { time: k.time + dt, ..*k } } else { *k });
            }
            if copy_pose {
                out.put(Key::new(t + dt, p.key_value(t), interp_at(p, t, Interp::Linear)));
            }
            Some(out.keys().to_vec())
        });
    }
    Ok(())
}

/// Deletes a frame: its keys go, later keys move one frame earlier. The last frame stays.
pub fn delete(st: &mut State, frame: i32) -> EditResult<bool> {
    if frame_count(st) <= 1 {
        return Ok(false);
    }
    let (t, dt, eps) = (time(st, frame), 1.0 / fps(st), 0.25 / fps(st));
    // a sound starting on the deleted frame goes with it
    move_clips(st, |c| if c > t - eps && c < t + dt - eps { None } else if c > t { Some(c - dt) } else { Some(c) });
    for (l, id) in props(st) {
        rewrite(st, &l, &id, |p| {
            if p.track.is_empty() {
                return None;
            }
            let mut out = Track::new(p.ty);
            for k in p.track.keys() {
                if (k.time - t).abs() < eps {
                    continue;
                }
                out.put(if k.time > t { Key { time: k.time - dt, ..*k } } else { *k });
            }
            Some(out.keys().to_vec())
        });
    }
    let n = frame_count(st);
    set_duration(st, (n - 1) as f64 / fps(st));
    Ok(true)
}

/// Every key on this frame tweens to the next (LINEAR) or snaps (HOLD).
pub fn set_tween(ed: &Editor, st: &mut State, frame: i32, tween: bool) -> EditResult<()> {
    let (t, eps) = (time(st, frame), 0.25 / fps(st));
    let Some(s) = st.project.root() else { return Ok(()) };
    let sel: Vec<KeyRef> = s
        .layers
        .iter()
        .flat_map(|l| l.properties().into_iter().flat_map(|p| p.track.keys().iter().filter(|k| (k.time - t).abs() < eps).map(|k| KeyRef { property: p.id.clone(), time: k.time }).collect::<Vec<_>>()).collect::<Vec<_>>())
        .collect();
    if sel.is_empty() {
        return Ok(());
    }
    ed.set_interp_in(st, &sel, if tween { Interp::Linear } else { Interp::Hold }, Ease::LINEAR)
}

/// Frame delay and repeat (camera properties) belong to one frame; tweened
/// frames do not inherit them.
pub fn is_frame_setting(id: &str) -> bool {
    id.ends_with("/cam/delay") || id.ends_with("/cam/repeatBack") || id.ends_with("/cam/repeatTimes") || id.ends_with("/cam/silence") || id.ends_with("/cam/slowMotion")
}

/// "Add tweened frame": keys every animated property (all layers, or one)
/// at t with the value it has there. Nothing moves. Returns keys added.
pub fn key_pose(st: &mut State, only: Option<&str>, t: f64) -> EditResult<usize> {
    let mut added = 0;
    for (l, id) in props(st) {
        if only.is_some_and(|o| o != l) || is_frame_setting(&id) {
            continue;
        }
        rewrite(st, &l, &id, |p| {
            if p.track.is_empty() || p.track.index_of(t).is_ok() {
                return None;
            }
            let mut out = p.track.clone();
            out.put(Key::new(t, p.key_value(t), interp_at(p, t, Interp::Linear)));
            added += 1;
            Some(out.keys().to_vec())
        });
    }
    Ok(added)
}

/// The tweened pose a fraction of the way from t0 to t1, as keys (10-90% buttons).
pub fn key_pose_between(st: &mut State, only: Option<&str>, t0: f64, t1: f64, fraction: f64) -> EditResult<usize> {
    let f = fraction.clamp(0.0, 1.0);
    key_pose(st, only, t0 + (t1 - t0) * f)
}

/// Holds a frame's pose for `frames` more frames (frame delay); with
/// camera_moves the camera keeps tweening through the hold.
pub fn hold(st: &mut State, frame: i32, frames: i32, camera_moves: bool) -> EditResult<()> {
    if frames <= 0 {
        return Ok(());
    }
    let (t, dt, eps) = (time(st, frame), frames as f64 / fps(st), 0.25 / fps(st));
    let cams: std::collections::HashSet<String> = if camera_moves {
        st.project.root().map(|s| s.layers.iter().filter(|l| matches!(l.kind, LayerKind::Camera(_))).flat_map(|l| l.properties().into_iter().map(|p| p.id.clone()).collect::<Vec<_>>()).collect()).unwrap_or_default()
    } else {
        Default::default()
    };
    let n = frame_count(st);
    set_duration(st, (n + frames) as f64 / fps(st));
    move_clips(st, |c| Some(if c > t + eps { c + dt } else { c }));
    for (l, id) in props(st) {
        let is_cam = cams.contains(&id);
        rewrite(st, &l, &id, |p| {
            if p.track.is_empty() {
                return None;
            }
            let v = p.key_value(t);
            let interp = interp_at(p, t, Interp::Linear);
            let mut out = Track::new(p.ty);
            for k in p.track.keys() {
                out.put(if k.time > t + eps { Key { time: k.time + dt, ..*k } } else { *k });
            }
            if !is_cam {
                if out.index_of(t).is_err() {
                    out.put(Key::new(t, v, interp));
                }
                out.put(Key::new(t + dt, v, interp));
            }
            Some(out.keys().to_vec())
        });
    }
    Ok(())
}

/// Adds tweened frames after a frame: later keys move back, so the tween to
/// the next key runs over more frames.
pub fn insert_tweened(st: &mut State, frame: i32, frames: i32) -> EditResult<()> {
    if frames <= 0 {
        return Ok(());
    }
    let (t, dt, eps) = (time(st, frame), frames as f64 / fps(st), 0.25 / fps(st));
    let n = frame_count(st);
    set_duration(st, (n + frames) as f64 / fps(st));
    move_clips(st, |c| Some(if c > t + eps { c + dt } else { c }));
    for (l, id) in props(st) {
        rewrite(st, &l, &id, |p| {
            if p.track.is_empty() {
                return None;
            }
            let mut out = Track::new(p.ty);
            for k in p.track.keys() {
                out.put(if k.time > t + eps { Key { time: k.time + dt, ..*k } } else { *k });
            }
            Some(out.keys().to_vec())
        });
    }
    Ok(())
}

/// Slow motion: the tween from this frame to the next keyed one takes
/// `factor` times as many frames. Returns the frames added.
pub fn slow_motion(st: &mut State, frame: i32, factor: i32) -> EditResult<i32> {
    if factor <= 1 {
        return Ok(0);
    }
    let (t, eps) = (time(st, frame), 0.25 / fps(st));
    let next = st
        .project
        .root()
        .map(|s| s.layers.iter().flat_map(|l| l.properties().into_iter().flat_map(|p| p.track.keys().iter().map(|k| k.time).collect::<Vec<_>>()).collect::<Vec<_>>()).filter(|kt| *kt > t + eps).fold(f64::INFINITY, f64::min))
        .unwrap_or(f64::INFINITY);
    if next.is_infinite() {
        return Ok(0);
    }
    let gap = ((next - t) * fps(st)).round() as i32;
    let add = (gap * (factor - 1)).min(100_000);
    insert_tweened(st, frame, add)?;
    Ok(add)
}
