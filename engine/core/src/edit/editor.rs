//! Key editing (port of Java Editor and Edits): changing a property at the
//! playhead (keyed with auto-key), posing, and key selections (move, scale,
//! offset, delete, copy, paste, easing). Each call is one undo step.

use super::{fail, Document, EditResult, State};
use crate::model::*;
use indexmap::IndexMap;

/// A selected key: a property id and the exact time of one of its keys.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyRef {
    pub property: String,
    pub time: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Changed,
    /// Auto-key is off and the property has keys: nothing changed.
    RefusedNeedsAutoKey,
}

/// Copied keys: property id -> keys with times relative to the earliest copied key.
#[derive(Clone, Debug, Default)]
pub struct KeyClip {
    pub keys: IndexMap<String, Vec<Key>>,
}

#[derive(Clone, Debug)]
pub struct Editor {
    pub auto_key: bool,
    /// Interpolation for new keys when nothing better is known.
    pub default_interp: Interp,
}

impl Default for Editor {
    fn default() -> Self {
        Editor { auto_key: true, default_interp: Interp::Linear }
    }
}

fn prop<'a>(s: &'a mut State, id: &str) -> EditResult<&'a mut Property> {
    match s.project.property_mut(id) {
        Some(p) => Ok(p),
        None => fail(format!("no property {id}")),
    }
}

/// New keys take the interpolation of the key they replace or follow.
pub fn interp_at(p: &Property, t: f64, default: Interp) -> Interp {
    let tr = &p.track;
    if tr.is_empty() {
        return default;
    }
    match tr.index_of(t) {
        Ok(i) => tr.keys()[i].interp,
        Err(ins) => {
            if ins > 0 {
                tr.keys()[ins - 1].interp
            } else {
                tr.keys()[0].interp
            }
        }
    }
}

fn group(sel: &[KeyRef]) -> IndexMap<String, Vec<f64>> {
    let mut m: IndexMap<String, Vec<f64>> = IndexMap::new();
    for k in sel {
        let l = m.entry(k.property.clone()).or_default();
        if !l.contains(&k.time) {
            l.push(k.time);
        }
    }
    m
}

fn set_keys(p: &mut Property, keys: Vec<Key>) -> EditResult<()> {
    if keys.iter().any(|k| !p.ty.checked(&k.value)) {
        return fail(format!("{} cannot hold that value", p.id));
    }
    p.track.clear();
    for k in keys {
        p.track.put(k);
    }
    Ok(())
}

impl Editor {
    /// Rounds a time to the nearest frame.
    pub fn snap(fps: f32, t: f64) -> f64 {
        let f = fps as f64;
        (t * f).round() / f
    }

    /// Changes one property at time t: auto-key keys it; otherwise the
    /// static value changes when it has no keys, else nothing happens.
    pub fn change_in(&self, s: &mut State, id: &str, value: Value, t: f64) -> EditResult<Change> {
        let p = prop(s, id)?;
        if !p.ty.checked(&value) {
            return fail(format!("{id} cannot hold that value"));
        }
        if self.auto_key {
            let interp = interp_at(p, t, self.default_interp);
            p.track.put(Key::new(t, value, interp));
            return Ok(Change::Changed);
        }
        if p.track.is_empty() {
            p.value = value;
            return Ok(Change::Changed);
        }
        Ok(Change::RefusedNeedsAutoKey)
    }

    pub fn change(&self, d: &mut Document, id: &str, value: Value, t: f64) -> EditResult<Change> {
        d.edit(&format!("Change {id}"), |s| self.change_in(s, id, value, t))
    }

    /// Keys every node of a figure (rotation and length) at the frame, with
    /// overrides by node id and the current keys-only values for the rest.
    pub fn pose(&self, d: &mut Document, symbol: &str, layer: &str, rotations: &IndexMap<String, f32>, lengths: &IndexMap<String, f32>, frame_time: f64) -> EditResult<()> {
        let t = Editor::snap(d.project().settings.fps, frame_time);
        let name = d.project().symbol(symbol).and_then(|s| s.layer(layer)).and_then(|l| l.name.clone()).unwrap_or_default();
        let default = self.default_interp;
        d.edit(&format!("Pose {name}"), |s| {
            let Some(l) = s.project.layer_mut(symbol, layer) else { return fail("no such layer") };
            let LayerKind::Figure(f) = &mut l.kind else { return fail("not a figure") };
            for n in f.nodes_mut() {
                let rv = rotations.get(&n.id).map(|v| Value::F(*v)).unwrap_or_else(|| n.rotation.key_value(t));
                let ri = interp_at(&n.rotation, t, default);
                n.rotation.track.put(Key::new(t, rv, ri));
                let lv = lengths.get(&n.id).map(|v| Value::F(*v)).unwrap_or_else(|| n.length.key_value(t));
                let li = interp_at(&n.length, t, default);
                n.length.track.put(Key::new(t, lv, li));
            }
            Ok(())
        })
    }

    fn transform_times(&self, s: &mut State, sel: &[KeyRef], factor: f64, pivot: f64, delta: f64) -> EditResult<()> {
        for (id, times) in group(sel) {
            let p = prop(s, &id)?;
            let (mut moving, mut staying) = (Vec::new(), Vec::new());
            for k in p.track.keys() {
                if times.contains(&k.time) {
                    moving.push(*k);
                } else {
                    staying.push(*k);
                }
            }
            let mut out = Track::new(p.ty);
            for k in staying {
                out.put(k);
            }
            for k in moving {
                out.put(Key { time: pivot + (k.time - pivot) * factor + delta, ..k });
            }
            set_keys(p, out.keys().to_vec())?;
        }
        Ok(())
    }

    /// Moves keys by delta seconds (keys landing on others replace them).
    pub fn retime(&self, d: &mut Document, sel: &[KeyRef], delta: f64) -> EditResult<()> {
        d.edit("Move keys", |s| self.transform_times(s, sel, 1.0, 0.0, delta))
    }

    /// Scales key times around a pivot (factor > 0).
    pub fn scale_time(&self, d: &mut Document, sel: &[KeyRef], pivot: f64, factor: f64) -> EditResult<()> {
        if !(factor > 0.0) {
            return fail("factor must be > 0");
        }
        d.edit("Scale keys", |s| self.transform_times(s, sel, factor, pivot, 0.0))
    }

    /// Offset tool: along a chain, each group's keys move by index x frames.
    pub fn stagger(&self, d: &mut Document, chain: &[Vec<KeyRef>], frames: f64) -> EditResult<()> {
        let fps = d.project().settings.fps as f64;
        d.edit("Offset keys", |s| {
            for (i, g) in chain.iter().enumerate() {
                if i == 0 || g.is_empty() {
                    continue;
                }
                self.transform_times(s, g, 1.0, 0.0, i as f64 * frames / fps)?;
            }
            Ok(())
        })
    }

    pub fn delete(&self, d: &mut Document, sel: &[KeyRef]) -> EditResult<()> {
        d.edit("Delete keys", |s| {
            for (id, times) in group(sel) {
                let p = prop(s, &id)?;
                let keep: Vec<Key> = p.track.keys().iter().filter(|k| !times.contains(&k.time)).copied().collect();
                set_keys(p, keep)?;
            }
            Ok(())
        })
    }

    pub fn copy(&self, project: &Project, sel: &[KeyRef]) -> KeyClip {
        let t0 = sel.iter().map(|k| k.time).fold(f64::INFINITY, f64::min);
        let mut c = KeyClip::default();
        for (id, times) in group(sel) {
            let Some(p) = project.property(&id) else { continue };
            let out = p.track.keys().iter().filter(|k| times.contains(&k.time)).map(|k| Key { time: k.time - t0, ..*k }).collect();
            c.keys.insert(id, out);
        }
        c
    }

    /// Pastes at time t; ids map through id_map (onto another figure).
    /// Missing or mismatched targets are skipped. Returns keys pasted.
    pub fn paste(&self, d: &mut Document, c: &KeyClip, t: f64, id_map: &IndexMap<String, String>) -> EditResult<usize> {
        d.edit("Paste keys", |s| {
            let mut n = 0;
            for (id, keys) in &c.keys {
                let target = id_map.get(id).unwrap_or(id);
                let Some(p) = s.project.property_mut(target) else { continue };
                if keys.is_empty() || !p.ty.checked(&keys[0].value) {
                    continue;
                }
                let mut out = p.track.clone();
                for k in keys {
                    out.put(Key { time: t + k.time, ..*k });
                    n += 1;
                }
                set_keys(p, out.keys().to_vec())?;
            }
            Ok(n)
        })
    }

    /// Sets interpolation and easing on the selected keys.
    pub fn set_interp(&self, d: &mut Document, sel: &[KeyRef], interp: Interp, ease: Ease) -> EditResult<()> {
        d.edit("Easing", |s| self.set_interp_in(s, sel, interp, ease))
    }

    pub fn set_interp_in(&self, s: &mut State, sel: &[KeyRef], interp: Interp, ease: Ease) -> EditResult<()> {
        for (id, times) in group(sel) {
            let p = prop(s, &id)?;
            let ty = p.ty;
            let out: Vec<Key> = p
                .track
                .keys()
                .iter()
                .map(|k| if times.contains(&k.time) { Key { interp: if ty.continuous() { interp } else { Interp::Hold }, ease, ..*k } } else { *k })
                .collect();
            set_keys(p, out)?;
        }
        Ok(())
    }
}
