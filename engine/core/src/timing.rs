//! Playback timing: frame delays and "repeat series of frames" turn the
//! frame strip into a schedule of slots in wall-clock seconds. The picture,
//! the sound and exports all read the same schedule, so they stay together.
//! A project without delays or repeats maps one to one.

use crate::audio::AudioPlan;
use crate::model::*;

/// One played frame: `frame` shows from `start` for `len` seconds; the first
/// `delay` seconds hold its pose, the rest tweens like any frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub frame: i32,
    pub start: f64,
    pub len: f64,
    pub delay: f64,
    /// Sounds still playing stop when this slot starts.
    pub silence: bool,
}

#[derive(Clone, Debug)]
pub struct Schedule {
    pub slots: Vec<Slot>,
    pub fps: f64,
    /// Wall seconds of one pass.
    pub total: f64,
    /// No delays and no repeats: wall time is project time.
    pub identity: bool,
}

/// More slots than this (runaway repeats) are cut off.
pub const MAX_SLOTS: usize = 100_000;

/// The longest frame delay, in seconds.
pub const MAX_DELAY: f32 = 600.0;

/// A frame setting: the exact key at the frame's time, no key meaning 0.
pub fn setting(p: &Property, fps: f64, frame: i32) -> f32 {
    let (t, eps) = (frame as f64 / fps, 0.25 / fps);
    p.track.keys().iter().find(|k| (k.time - t).abs() < eps).and_then(|k| if let Value::F(v) = k.value { Some(v) } else { None }).filter(|v| v.is_finite()).unwrap_or(0.0)
}

/// Whether a frame silences the sounds playing when it starts.
pub fn frame_silences(p: &Project, frame: i32) -> bool {
    let fps = fps_of(p);
    p.root().and_then(|s| s.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None })).is_some_and(|c| setting(&c.silence, fps, frame) > 0.5)
}

/// (delay seconds, go back frames, times) of a frame.
pub fn frame_settings(p: &Project, frame: i32) -> (f32, i32, i32) {
    let fps = fps_of(p);
    let cam = p.root().and_then(|s| s.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None }));
    let Some(c) = cam else { return (0.0, 0, 0) };
    let delay = setting(&c.delay, fps, frame).clamp(0.0, MAX_DELAY);
    let back = setting(&c.repeat_back, fps, frame).round().max(0.0) as i32;
    let times = setting(&c.repeat_times, fps, frame).round().max(0.0) as i32;
    (delay, back, if back > 0 { times } else { 0 })
}

fn fps_of(p: &Project) -> f64 {
    if p.settings.fps > 0.0 {
        p.settings.fps as f64
    } else {
        30.0
    }
}

pub fn frame_count(p: &Project) -> i32 {
    let d = p.root().map(|s| s.duration).unwrap_or(0.0);
    1.max((d * fps_of(p)).round() as i32)
}

impl Schedule {
    pub fn build(p: &Project) -> Schedule {
        let fps = fps_of(p);
        let n = frame_count(p);
        let sets: Vec<(f32, i32, i32)> = (0..n).map(|f| frame_settings(p, f)).collect();
        let silences: Vec<bool> = (0..n).map(|f| frame_silences(p, f)).collect();
        let identity = sets.iter().all(|s| s.0 == 0.0 && s.2 == 0) && !silences.iter().any(|x| *x);
        let mut slots = Vec::with_capacity(n as usize);
        let mut done = vec![0i32; n as usize];
        let (mut f, mut at) = (0i32, 0f64);
        while f < n && slots.len() < MAX_SLOTS {
            let (delay, back, times) = sets[f as usize];
            let len = 1.0 / fps + delay as f64;
            slots.push(Slot { frame: f, start: at, len, delay: delay as f64, silence: silences[f as usize] });
            at += len;
            if times > 0 && done[f as usize] < times {
                done[f as usize] += 1;
                let to = (f - back).max(0);
                // repeats inside the series start over on every pass
                for g in to..f {
                    done[g as usize] = 0;
                }
                f = to;
            } else {
                f += 1;
            }
        }
        Schedule { slots, fps, total: at, identity }
    }

    /// The slot playing at wall time w (clamped into one pass).
    pub fn slot_at(&self, w: f64) -> &Slot {
        let i = self.slots.partition_point(|s| s.start <= w).saturating_sub(1);
        &self.slots[i.min(self.slots.len() - 1)]
    }

    /// Project time shown at wall time w, one picture per 1/rate seconds of
    /// tweening (rate = fps: one picture per frame).
    pub fn time_at(&self, w: f64, rate: f64) -> f64 {
        if self.slots.is_empty() || !w.is_finite() {
            return 0.0;
        }
        let s = self.slot_at(w.max(0.0));
        let base = s.frame as f64 / self.fps;
        let local = w - s.start - s.delay;
        if !(local > 0.0) {
            return base;
        }
        let rate = if rate > 0.0 { rate } else { self.fps };
        let step = ((local * rate).floor() / rate).min((rate / self.fps).ceil().max(1.0) / rate - 1.0 / rate);
        base + step.max(0.0)
    }

    /// Only the slots of frames [first, first + frames), played back to back
    /// (what an export of that range shows and sounds like).
    pub fn subset(&self, first: i32, frames: i32) -> Schedule {
        let mut at = 0.0;
        let mut slots = Vec::new();
        for s in self.slots.iter().filter(|s| s.frame >= first && s.frame < first + frames) {
            slots.push(Slot { start: at, ..*s });
            at += s.len;
        }
        // retime() maps every slot by its own start, so the result is exact
        Schedule { slots, fps: self.fps, total: at, identity: false }
    }

    /// Wall time a frame first starts at.
    pub fn wall_of(&self, frame: i32) -> f64 {
        self.slots.iter().find(|s| s.frame == frame).map(|s| s.start).unwrap_or(0.0)
    }

    /// The sound in wall time: a clip starting on a frame starts again on
    /// every slot of that frame, offset by the same amount.
    pub fn retime(&self, plan: &AudioPlan) -> AudioPlan {
        if self.identity {
            return plan.clone();
        }
        let mut out = AudioPlan { entries: Vec::new(), end: self.total, diagnostics: plan.diagnostics.clone() };
        for e in &plan.entries {
            let f = (e.start * self.fps + 1e-6).floor() as i32;
            let off = e.start - f as f64 / self.fps;
            for s in self.slots.iter().filter(|s| s.frame == f) {
                let mut c = e.clone();
                c.start = s.start + off;
                c.end = if e.loop_ { self.total.max(c.start) } else { c.start + (e.end - e.start) };
                // a later silencing frame cuts it off
                if let Some(cut) = self.slots.iter().find(|x| x.silence && x.start > s.start + 1e-9).map(|x| x.start) {
                    c.end = c.end.min(cut);
                }
                if c.end > c.start {
                    out.entries.push(c);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{tone, Entry};
    use std::sync::Arc;

    #[test]
    fn sounds_start_on_every_slot_of_their_frame() {
        let fps = 10.0;
        let slot = |frame, start| Slot { frame, start, len: 0.1, delay: 0.0, silence: false };
        // frames 0 1 2, frame 0 pauses 1 s, then 1 2 repeat once
        let sch = Schedule {
            slots: vec![Slot { frame: 0, start: 0.0, len: 1.1, delay: 1.0, silence: false }, slot(1, 1.1), slot(2, 1.2), slot(1, 1.3), slot(2, 1.4)],
            fps,
            total: 1.5,
            identity: false,
        };
        let e = Entry { pcm: Arc::new(tone(440.0, 0.05, 8000, 0.5)), start: 0.15, end: 0.2, in_: 0.0, out: 0.05, loop_: false, fade_in: 0.0, fade_out: 0.0, pitch: 1.0, volume: vec![1.0], pan: vec![0.0] };
        let plan = AudioPlan { entries: vec![e], end: 0.3, diagnostics: Vec::new() };
        let out = sch.retime(&plan);
        let starts: Vec<f64> = out.entries.iter().map(|e| (e.start * 1000.0).round() / 1000.0).collect();
        assert_eq!(starts, vec![1.15, 1.35]);
        assert!((out.entries[0].end - out.entries[0].start - 0.05).abs() < 1e-9);
        assert_eq!(out.end, 1.5);
        // the picture holds frame 0 during its pause
        assert_eq!(sch.time_at(0.5, fps), 0.0);
        assert_eq!(sch.slot_at(1.32).frame, 1);
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;

    #[test]
    fn exports_hold_delays_and_play_repeats() {
        let fps = 10.0;
        let slot = |frame, start, delay: f64| Slot { frame, start, len: 0.1 + delay, delay, silence: false };
        let sch = Schedule { slots: vec![slot(0, 0.0, 0.0), slot(1, 0.1, 0.3), slot(0, 0.5, 0.0), slot(1, 0.6, 0.3)], fps, total: 1.0, identity: false };
        // GIF: the delayed picture carries its extra time
        let gif = crate::export::pictures(&sch, 0, 2, fps, false);
        assert_eq!(gif.len(), 4);
        assert!((gif[1].1 - 0.3).abs() < 1e-9 && gif[0].1 == 0.0);
        // PNG frames: three more copies of the held picture
        let png = crate::export::pictures(&sch, 0, 2, fps, true);
        assert_eq!(png.len(), 2 * (1 + 4));
        // a range keeps only its frames, back to back
        let part = sch.subset(1, 1);
        assert_eq!(part.slots.len(), 2);
        assert!((part.slots[1].start - 0.4).abs() < 1e-9);
        assert!((part.total - 0.8).abs() < 1e-9);
    }
}
