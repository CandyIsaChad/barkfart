//! Joining (port of Java JoinOps): attach a layer to a figure's joint (or the
//! figure, or the camera) and detach it, keeping it where it is: its
//! transform, static value and every key, is rewritten in the new frame.

use super::{fail, EditResult, State};
use crate::eval::{self, Affine};
use crate::model::*;

/// Attaches `child` to `target`'s node (None: to the figure itself), keeping
/// its current world placement (relative join).
pub fn join(st: &mut State, sym: &str, child: &str, target: &str, node: Option<&str>, rotate: bool, t: f64) -> EditResult<()> {
    join_mode(st, sym, child, target, node, rotate, false, t)
}

/// Attaches `child` to `target`'s node across the timeline. When `absolute` is
/// true, snaps `child`'s root `(0,0)` onto the target joint while preserving
/// its current world rotation and scale.
#[allow(clippy::too_many_arguments)]
pub fn join_mode(st: &mut State, sym: &str, child: &str, target: &str, node: Option<&str>, rotate: bool, absolute: bool, t: f64) -> EditResult<()> {
    {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        if target == child {
            return fail("a figure cannot join itself");
        }
        // Check for cycles across both static parent and all timed join switches.
        let mut stack = vec![target];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            if id == child {
                return fail("that would join a figure to its own joined figure");
            }
            if let Some(c) = s.layer(id) {
                if let Some(p) = c.parent.as_deref() {
                    stack.push(p);
                }
                for j in &c.joins {
                    if let Some(p) = j.parent.as_deref() {
                        stack.push(p);
                    }
                }
            }
        }
        let Some(f) = s.layer(target).and_then(|l| l.as_figure()) else { return fail("can only join to a figure") };
        if let Some(n) = node {
            if f.node(n).is_none() {
                return fail(format!("no node {n}"));
            }
        }
    }
    change_mode(st, sym, child, Some(target), node, rotate, absolute, t)
}

/// Detaches a layer from whatever it is joined to (all frames).
pub fn unjoin(st: &mut State, sym: &str, child: &str, t: f64) -> EditResult<()> {
    change_mode(st, sym, child, None, None, true, false, t)
}

/// Gives a layer any parent (or none), keeping it in place (camera locks).
pub fn reparent(st: &mut State, sym: &str, child: &str, parent: Option<&str>, t: f64) -> EditResult<()> {
    change_mode(st, sym, child, parent, None, true, false, t)
}

fn props(l: &Layer) -> [&Property; 5] {
    let tr = &l.transform;
    [&tr.x, &tr.y, &tr.rotation, &tr.scale_x, &tr.scale_y]
}

fn props_mut(l: &mut Layer) -> [&mut Property; 5] {
    let tr = &mut l.transform;
    [&mut tr.x, &mut tr.y, &mut tr.rotation, &mut tr.scale_x, &mut tr.scale_y]
}

/// One of x, y, rotation, scaleX, scaleY from a local matrix (no skew), with
/// the anchor accounted for; rotation stays continuous with `old`.
fn part(i: usize, ax: f64, ay: f64, old: f64, m: &Affine) -> f32 {
    let sx = m.a.hypot(m.b);
    let mut rot = m.b.atan2(m.a).to_degrees();
    let sy = if sx < 1e-12 { 0.0 } else { (m.a * m.d - m.b * m.c) / sx };
    let x = m.tx + m.a * ax + m.c * ay;
    let y = m.ty + m.b * ax + m.d * ay;
    rot += 360.0 * java_round_f64((old - rot) / 360.0);
    (match i {
        0 => x,
        1 => y,
        2 => rot,
        3 => sx,
        _ => sy,
    }) as f32
}

/// Snaps a local affine so local `(0,0)` maps to parent `(0,0)` while keeping
/// its linear part (rotation and scale) intact.
fn snap_origin(m: &Affine) -> Affine {
    Affine { tx: 0.0, ty: 0.0, ..*m }
}

#[allow(clippy::too_many_arguments)]
fn change_mode(st: &mut State, sym: &str, child: &str, parent: Option<&str>, node: Option<&str>, rotate: bool, absolute: bool, t: f64) -> EditResult<()> {
    // where the layer is at every time that matters, before the change
    let (times, world, template, keyed) = {
        let p = &st.project;
        let Some(s) = p.symbol(sym) else { return fail("no symbol") };
        let Some(l) = s.layer(child) else { return fail(format!("no layer {child}")) };
        let mut times: Vec<f64> = vec![t];
        for q in props(l) {
            for k in q.track.keys() {
                times.push(k.time);
            }
        }
        for j in &l.joins {
            times.push(j.time);
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        times.dedup();
        let world: Vec<Affine> = times.iter().map(|&tt| eval::parent_frame(p, s, l, tt).mul(&eval::local_transform(p, l, tt))).collect();
        let keyed = times.len() > 1 || props(l).iter().any(|q| !q.track.is_empty());
        // the first key at each time (any property) gives the interpolation
        let mut template: Vec<Key> = Vec::new();
        for q in props(l) {
            for k in q.track.keys() {
                if !template.iter().any(|x| x.time == k.time) {
                    template.push(*k);
                }
            }
        }
        let tw = p.settings.tween_interp;
        for j in &l.joins {
            if !template.iter().any(|x| (x.time - j.time).abs() < 1e-9) {
                template.push(Key::new(j.time, Value::F(0.0), tw));
            }
        }
        (times, world, template, keyed)
    };
    {
        let Some(l) = st.project.layer_mut(sym, child) else { return fail("no layer") };
        l.parent = parent.map(str::to_string);
        l.parent_node = node.map(str::to_string);
        l.parent_node_rotate = rotate;
        // a join for the whole timeline replaces any joins over time
        l.joins.clear();
    }
    // the same placement in the new frame (or snapped to joint origin for absolute join)
    let snap = absolute && parent.is_some();
    let (local, ax, ay, old_rot) = {
        let p = &st.project;
        let s = p.symbol(sym).expect("symbol");
        let l = s.layer(child).expect("layer");
        let local: Vec<Affine> = times
            .iter()
            .zip(&world)
            .map(|(&tt, w)| {
                let m = eval::parent_frame(p, s, l, tt).inverse().mul(w);
                if snap { snap_origin(&m) } else { m }
            })
            .collect();
        (local, l.transform.anchor_x.value.f() as f64, l.transform.anchor_y.value.f() as f64, l.transform.rotation.value.f() as f64)
    };
    let at = |time: f64| times.iter().position(|x| *x == time).expect("time listed");
    let l = st.project.layer_mut(sym, child).expect("layer");
    for (i, q) in props_mut(l).into_iter().enumerate() {
        q.track.clear();
        if keyed {
            for k in &template {
                q.track.put(Key { value: Value::F(part(i, ax, ay, old_rot, &local[at(k.time)])), ..*k });
            }
        }
        q.value = Value::F(part(i, ax, ay, old_rot, &local[at(t)]));
    }
    Ok(())
}

/// Detaches every layer in `sym` that references `deleted_id` in its static
/// `parent` or any timed `joins` entry, baking its world transform so nothing
/// jumps or holds a dangling parent reference after `deleted_id` is removed.
pub fn detach_deleted_parent(st: &mut State, sym: &str, deleted_id: &str, t: f64) -> EditResult<()> {
    let followers: Vec<String> = st
        .project
        .symbol(sym)
        .map(|s| {
            s.layers
                .iter()
                .filter(|x| x.id != deleted_id && (x.parent.as_deref() == Some(deleted_id) || x.joins.iter().any(|j| j.parent.as_deref() == Some(deleted_id))))
                .map(|x| x.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let eps = 1e-9;
    for fid in followers {
        let has_joins = st.project.symbol(sym).and_then(|s| s.layer(&fid)).is_some_and(|l| !l.joins.is_empty());
        if !has_joins {
            unjoin(st, sym, &fid, t)?;
            continue;
        }
        // Bake world transform at t, every key time, and every switch time before editing.
        let (times, world, template, keyed, ax, ay, old_rot) = {
            let p = &st.project;
            let Some(s) = p.symbol(sym) else { continue };
            let Some(l) = s.layer(&fid) else { continue };
            let mut times: Vec<f64> = vec![t];
            for q in props(l) {
                for k in q.track.keys() {
                    times.push(k.time);
                }
            }
            for j in &l.joins {
                times.push(j.time);
            }
            times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            times.dedup();
            let world: Vec<Affine> = times.iter().map(|&tt| eval::parent_frame(p, s, l, tt).mul(&eval::local_transform(p, l, tt))).collect();
            let keyed = times.len() > 1 || props(l).iter().any(|q| !q.track.is_empty());
            let mut template: Vec<Key> = Vec::new();
            for q in props(l) {
                for k in q.track.keys() {
                    if !template.iter().any(|x| (x.time - k.time).abs() < eps) {
                        template.push(*k);
                    }
                }
            }
            let tw = p.settings.tween_interp;
            for j in &l.joins {
                if !template.iter().any(|x| (x.time - j.time).abs() < eps) {
                    template.push(Key::new(j.time, Value::F(0.0), tw));
                }
            }
            (times, world, template, keyed, l.transform.anchor_x.value.f() as f64, l.transform.anchor_y.value.f() as f64, l.transform.rotation.value.f() as f64)
        };
        {
            let Some(l) = st.project.layer_mut(sym, &fid) else { continue };
            if l.parent.as_deref() == Some(deleted_id) {
                l.parent = None;
                l.parent_node = None;
            }
            for j in &mut l.joins {
                if j.parent.as_deref() == Some(deleted_id) {
                    j.parent = None;
                    j.node = None;
                }
            }
            // Prune consecutive redundant switches with identical (parent, node, rotate).
            let mut cleaned: Vec<JoinSwitch> = Vec::with_capacity(l.joins.len());
            let mut prev_p = l.parent.clone();
            let mut prev_n = l.parent_node.clone();
            let mut prev_r = l.parent_node_rotate;
            for j in l.joins.drain(..) {
                if j.parent == prev_p && j.node == prev_n && (j.parent.is_none() || j.rotate == prev_r) {
                    continue;
                }
                prev_p = j.parent.clone();
                prev_n = j.node.clone();
                prev_r = j.rotate;
                cleaned.push(j);
            }
            l.joins = cleaned;
        }
        // Recompute `before` for each surviving switch and local transforms for all `times`.
        let (new_befores, local) = {
            let p = &st.project;
            let s = p.symbol(sym).expect("symbol");
            let l = s.layer(&fid).expect("layer");
            let mut befores: Vec<[f32; 5]> = Vec::with_capacity(l.joins.len());
            for j in &l.joins {
                let idx = times.iter().position(|x| (*x - j.time).abs() < eps).unwrap_or(0);
                let pf_before = eval::parent_frame(p, s, l, j.time - 1e-6);
                let loc_before = pf_before.inverse().mul(&world[idx]);
                befores.push(std::array::from_fn(|i| part(i, ax, ay, old_rot, &loc_before)));
            }
            let local: Vec<Affine> = times.iter().zip(&world).map(|(&tt, w)| eval::parent_frame(p, s, l, tt).inverse().mul(w)).collect();
            (befores, local)
        };
        let at = |time: f64| times.iter().position(|x| (*x - time).abs() < eps).expect("time listed");
        let l = st.project.layer_mut(sym, &fid).expect("layer");
        for (j, b) in l.joins.iter_mut().zip(new_befores) {
            j.before = b;
        }
        for (i, q) in props_mut(l).into_iter().enumerate() {
            q.track.clear();
            if keyed {
                for k in &template {
                    q.track.put(Key { value: Value::F(part(i, ax, ay, old_rot, &local[at(k.time)])), ..*k });
                }
            }
            q.value = Value::F(part(i, ax, ay, old_rot, &local[at(t)]));
        }
    }
    Ok(())
}

/// Join-anchor flip: mirrors every layer joined at the anchor (a joint, or
/// None for the figure itself) across its axis: y, rotation and y scale change sign.
pub fn flip_joined(st: &mut State, sym: &str, anchor_layer: &str, node: Option<&str>) -> EditResult<()> {
    let ids: Vec<String> = {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        s.layers.iter().filter(|l| l.parent.as_deref() == Some(anchor_layer) && l.parent_node.as_deref() == node).map(|l| l.id.clone()).collect()
    };
    if ids.is_empty() {
        return fail("nothing is joined there");
    }
    for id in ids {
        let l = st.project.layer_mut(sym, &id).expect("layer");
        for q in [&mut l.transform.y, &mut l.transform.rotation, &mut l.transform.scale_y] {
            q.value = Value::F(-q.value.f());
            let ks: Vec<Key> = q.track.keys().to_vec();
            q.track.clear();
            for k in ks {
                q.track.put(Key { value: Value::F(-k.value.f()), ..k });
            }
        }
    }
    Ok(())
}

/// Break off (REDESIGN D3, F1): the branch from `node` down becomes its own
/// figure, joined to this one at the joint it hung from, so nothing moves on
/// any frame. The new figure is drawn just above this one. Figures joined to
/// a joint of the branch follow it to the new figure. Returns the new
/// figure's layer id.
pub fn break_off(st: &mut State, sym: &str, layer: &str, node: &str) -> EditResult<String> {
    detach_branch(st, sym, layer, node, false)
}

/// Separate joint (Round 3): extracts selected non-root `node` + full descendant
/// subtree into a new independent Stickfigure layer, removes the branch from the
/// original, preserves draw order & in-branch polyfills (dropping broken
/// cross-cut polyfills/connectors cleanly), re-targets any third-party figures
/// joined to nodes in the separated subtree (static AND timed joins), preserves
/// multi-frame animation continuity, and Absolute-Joins the new figure to its
/// former parent joint.
pub fn separate_node(st: &mut State, sym: &str, layer: &str, node: &str) -> EditResult<String> {
    detach_branch(st, sym, layer, node, true)
}

fn detach_branch(st: &mut State, sym: &str, layer: &str, node: &str, drop_cross: bool) -> EditResult<String> {
    let (src, sub, parent, absolute, index, rot_deltas, parent_conns, child_conns) = {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        let Some(l) = s.layer(layer) else { return fail(format!("no layer {layer}")) };
        let Some(f) = l.as_figure() else { return fail("only a figure's branch can break off") };
        let Some(n) = f.node(node) else { return fail(format!("no node {node}")) };
        let Some(parent) = n.parent.clone() else { return fail("The main joint is the whole figure; pick a joint further out.") };
        let sub = f.subtree(node);
        let inside = |id: &str| sub.iter().any(|x| x == id);
        // connectors across the break
        if !drop_cross {
            for (ci, (ei, _)) in crate::eval::pose::connector_links(f) {
                if inside(&f.nodes()[ci].id) != inside(&f.nodes()[ei].id) {
                    return fail("A connector ties this branch to the rest of the figure, so it can't break off.");
                }
            }
            for pf in &f.polyfills {
                let ids: Vec<&str> = std::iter::once(pf.anchor.as_str()).chain(pf.nodes.iter().map(String::as_str)).collect();
                let k = ids.iter().filter(|x| inside(x)).count();
                if k > 0 && k < ids.len() {
                    return fail("A polyfill spans this joint, so the branch can't break off.");
                }
            }
        }
        // Static parent + LOCK_RELATIVE: bake the static-chain base-angle delta into node.rotation
        // so detaching onto a non-static root preserves the exact world angle on every frame.
        let mut rot_deltas: Option<(f32, Vec<(f64, f32)>)> = None;
        if n.angle_lock == LOCK_RELATIVE && f.node(&parent).is_some_and(|p| p.is_static) {
            if let (Some(ni), Some(pi)) = (f.index_of(node), f.index_of(&parent)) {
                let mut r = crate::eval::Resolver::new(&st.project);
                let delta_at = |r: &mut crate::eval::Resolver, tt: f64| -> f32 {
                    let pose = crate::eval::FigurePose::compute(f, tt, r);
                    (pose.base_angle(f.nodes(), ni) - pose.angle[pi]) as f32
                };
                let d0 = delta_at(&mut r, 0.0);
                let ks: Vec<(f64, f32)> = n.rotation.track.keys().iter().map(|k| (k.time, delta_at(&mut r, k.time))).collect();
                rot_deltas = Some((d0, ks));
            }
        }
        // Partition legacy `fig.connections` on root into wholly-outside vs wholly-inside.
        let mut parent_conns: Option<String> = None;
        let mut child_conns: Option<String> = None;
        if let Some(raw) = f.root().and_then(|r| r.legacy_extras.get("fig.connections")) {
            let table = raw.strip_prefix("s:").unwrap_or(raw);
            let mut keep_p = Vec::new();
            let mut keep_c = Vec::new();
            for pair in table.split(';') {
                let Some((a, b)) = pair.split_once(',') else { continue };
                let ia = inside(&format!("n{a}"));
                let ib = inside(&format!("n{b}"));
                if ia && ib {
                    keep_c.push(pair);
                } else if !ia && !ib {
                    keep_p.push(pair);
                }
            }
            parent_conns = Some(keep_p.join(";"));
            if !keep_c.is_empty() {
                child_conns = Some(format!("s:{}", keep_c.join(";")));
            }
        }
        (l.clone(), sub, parent, n.angle_lock == LOCK_ABSOLUTE, s.layer_index(layer).unwrap_or(0), rot_deltas, parent_conns, child_conns)
    };
    let new_id = st.project.new_id("L");
    let mut ids = std::collections::HashMap::new();
    ids.insert(layer.to_string(), new_id.clone());
    let mut l2 = super::structure::copy_layer(&src, &ids)?;
    let name = src.name.clone().unwrap_or_else(|| src.id.clone());
    let part_name = src.as_figure().and_then(|f| f.node(node)).and_then(|n| n.name.clone()).filter(|s| !s.trim().is_empty());
    l2.name = Some(match part_name {
        Some(pn) if drop_cross => format!("{name} ({pn})"),
        _ => format!("{name} part"),
    });
    // the new figure: a main joint where the branch hung, then the branch
    {
        let LayerKind::Figure(f2) = &mut l2.kind else { return fail("not a figure") };
        let root_id = f2.root().map(|r| r.id.clone()).unwrap_or_else(|| "root".into());
        let mut new_root = Node::new(&new_id, &root_id, None, Shape::Line);
        if let Some(cc) = child_conns {
            new_root.legacy_extras.insert("fig.connections".into(), cc);
        }
        let mut list = vec![new_root];
        for id in &sub {
            let mut n = f2.node(id).expect("copied").clone();
            if id == node {
                n.parent = Some(root_id.clone());
                if let Some((d0, ref ks)) = rot_deltas {
                    n.rotation.value = Value::F(n.rotation.value.f() + d0);
                    let orig: Vec<Key> = n.rotation.track.keys().to_vec();
                    n.rotation.track.clear();
                    for k in orig {
                        let d = ks.iter().find(|(tt, _)| (*tt - k.time).abs() < 1e-9).map(|(_, v)| *v).unwrap_or(d0);
                        n.rotation.track.put(Key { value: Value::F(k.value.f() + d), ..k });
                    }
                }
            }
            if drop_cross {
                if let Some(to) = n.legacy_extras.get(crate::eval::pose::CONNECT_TO).and_then(|v| v.strip_prefix("s:")).map(str::to_string) {
                    if !sub.contains(&to) {
                        n.legacy_extras.shift_remove(crate::eval::pose::CONNECT_TO);
                        n.legacy_extras.shift_remove("connector");
                    }
                }
            }
            list.push(n);
        }
        f2.set_nodes(list).map_err(super::EditError)?;
        // Keep ONLY polyfills wholly inside `sub` (drops broken cross-cut polyfills!).
        let keep: Vec<Polyfill> = f2
            .polyfills
            .iter()
            .filter(|pf| sub.iter().any(|x| *x == pf.anchor) && pf.nodes.iter().all(|nid| sub.iter().any(|x| x == nid)))
            .cloned()
            .collect();
        f2.polyfills = keep;
        if let Some(b) = &f2.base {
            let b: indexmap::IndexMap<String, NodeBase> = b.iter().filter(|(k, _)| sub.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
            f2.base = Some(std::sync::Arc::new(b));
        }
    }
    // placed by the joint alone (Absolute Join: the joint's frame already carries
    // this figure's position, turn and scale; NEVER inherit `src.joins`!)
    {
        let tr = &mut l2.transform;
        for (q, v) in [(&mut tr.x, 0.0), (&mut tr.y, 0.0), (&mut tr.rotation, 0.0), (&mut tr.scale_x, 1.0), (&mut tr.scale_y, 1.0), (&mut tr.anchor_x, 0.0), (&mut tr.anchor_y, 0.0)] {
            q.track.clear();
            q.value = Value::F(v);
        }
        l2.joins.clear();
        l2.parent = Some(layer.to_string());
        l2.parent_node = Some(parent);
        // a branch locked to the stage's angle keeps it
        l2.parent_node_rotate = !absolute;
    }
    // the branch leaves this figure (keep only wholly-outside polyfills, clean base & connectors)
    {
        let Some(l) = st.project.layer_mut(sym, layer) else { return fail("no layer") };
        let LayerKind::Figure(f) = &mut l.kind else { return fail("not a figure") };
        f.remove_subtree(node).map_err(super::EditError)?;
        f.polyfills.retain(|pf| !sub.iter().any(|x| *x == pf.anchor) && !pf.nodes.iter().any(|nid| sub.iter().any(|x| x == nid)));
        if drop_cross {
            for nd in f.nodes_mut() {
                if let Some(to) = nd.legacy_extras.get(crate::eval::pose::CONNECT_TO).and_then(|v| v.strip_prefix("s:")).map(str::to_string) {
                    if sub.contains(&to) {
                        nd.legacy_extras.shift_remove(crate::eval::pose::CONNECT_TO);
                        nd.legacy_extras.shift_remove("connector");
                    }
                }
            }
        }
        if let Some(pc) = parent_conns {
            if let Some(root) = f.nodes_mut().first_mut() {
                if pc.is_empty() {
                    root.legacy_extras.shift_remove("fig.connections");
                } else {
                    root.legacy_extras.insert("fig.connections".into(), format!("s:{pc}"));
                }
            }
        }
        if let Some(b) = &mut f.base {
            let m = std::sync::Arc::make_mut(b);
            for id in &sub {
                m.shift_remove(id);
            }
        }
    }
    // figures joined to a joint of the branch (static parent OR timed joins!) go with it
    let followers: Vec<String> = st
        .project
        .symbol(sym)
        .map(|s| {
            s.layers
                .iter()
                .filter(|x| {
                    let static_hit = x.parent.as_deref() == Some(layer) && x.parent_node.as_ref().is_some_and(|n| sub.contains(n));
                    let timed_hit = x.joins.iter().any(|j| j.parent.as_deref() == Some(layer) && j.node.as_ref().is_some_and(|n| sub.contains(n)));
                    static_hit || timed_hit
                })
                .map(|x| x.id.clone())
                .collect()
        })
        .unwrap_or_default();
    for id in followers {
        if let Some(x) = st.project.layer_mut(sym, &id) {
            if x.parent.as_deref() == Some(layer) && x.parent_node.as_ref().is_some_and(|n| sub.contains(n)) {
                x.parent = Some(new_id.clone());
            }
            for j in &mut x.joins {
                if j.parent.as_deref() == Some(layer) && j.node.as_ref().is_some_and(|n| sub.contains(n)) {
                    j.parent = Some(new_id.clone());
                }
            }
        }
    }
    super::structure::add_layer(st, sym, l2, index + 1)?;
    Ok(new_id)
}


/// Joins over time (D8, I1): from `t` on, `child` is joined to `parent`'s
/// `node` (or the figure, or to nothing), until its next switch. Nothing
/// moves: the transform at the switch is kept as the end of the interval
/// before it, and the keys from `t` to the next switch are rewritten in the
/// new frame, with one on `t`.
pub fn switch_at(st: &mut State, sym: &str, child: &str, parent: Option<&str>, node: Option<&str>, rotate: bool, t: f64) -> EditResult<()> {
    switch_at_mode(st, sym, child, parent, node, rotate, false, t)
}

/// Timed join with optional `absolute` mode: when `absolute && parent.is_some()`,
/// frames before `t` keep `before` intact (zero jump on `[0, t)`) while `[t, next_switch)`
/// snaps the child's root `(0,0)` onto the target joint while preserving world rotation/scale.
#[allow(clippy::too_many_arguments)]
pub fn switch_at_mode(st: &mut State, sym: &str, child: &str, parent: Option<&str>, node: Option<&str>, rotate: bool, absolute: bool, t: f64) -> EditResult<()> {
    if let Some(target) = parent {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        if target == child {
            return fail("a figure cannot join itself");
        }
        // no loops through the target's own joins at this moment
        let mut cur = s.layer(target);
        let mut guard = s.layers.len() + 1;
        while let Some(c) = cur {
            if guard == 0 {
                break;
            }
            guard -= 1;
            if c.id == child {
                return fail("that would join a figure to its own joined figure");
            }
            cur = c.parent_at(t).0.and_then(|p| s.layer(p));
        }
        let Some(f) = s.layer(target).and_then(|l| l.as_figure()) else { return fail("can only join to a figure") };
        if let Some(n) = node {
            if f.node(n).is_none() {
                return fail(format!("no node {n}"));
            }
        }
    }
    let eps = 1e-9;
    // the interval the switch opens, and where the layer is over it (world)
    let (times, world, before, template, ax, ay, old_rot, same_as_prev) = {
        let p = &st.project;
        let Some(s) = p.symbol(sym) else { return fail("no symbol") };
        let Some(l) = s.layer(child) else { return fail(format!("no layer {child}")) };
        let prev = match l.joins.iter().rev().find(|j| j.time < t - eps) {
            Some(j) => (j.parent.as_deref(), j.node.as_deref(), j.rotate),
            None => (l.parent.as_deref(), l.parent_node.as_deref(), l.parent_node_rotate),
        };
        let same_as_prev = prev.0 == parent && prev.1 == node && (parent.is_none() || prev.2 == rotate);
        let end = l.joins.iter().find(|j| j.time > t + eps).map(|j| j.time);
        let mut times: Vec<f64> = vec![t];
        for q in props(l) {
            for k in q.track.keys() {
                if k.time > t + eps && end.is_none_or(|e| k.time < e - eps) {
                    times.push(k.time);
                }
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        times.dedup();
        let world: Vec<Affine> = times.iter().map(|&tt| eval::parent_frame(p, s, l, tt).mul(&eval::local_transform(p, l, tt))).collect();
        // the values at t in the frame before the switch
        let here = eval::local_transform(p, l, t);
        let (ax, ay, old_rot) = (l.transform.anchor_x.value.f() as f64, l.transform.anchor_y.value.f() as f64, l.transform.rotation.value.f() as f64);
        let before: [f32; 5] = std::array::from_fn(|i| part(i, ax, ay, old_rot, &here));
        let mut template: Vec<Key> = Vec::new();
        for q in props(l) {
            for k in q.track.keys() {
                if times.iter().any(|x| (x - k.time).abs() < eps) && !template.iter().any(|x| (x.time - k.time).abs() < eps) {
                    template.push(*k);
                }
            }
        }
        (times, world, before, template, ax, ay, old_rot, same_as_prev)
    };
    {
        let Some(l) = st.project.layer_mut(sym, child) else { return fail("no layer") };
        l.joins.retain(|j| (j.time - t).abs() > eps);
        if !same_as_prev {
            let at = l.joins.iter().position(|j| j.time > t).unwrap_or(l.joins.len());
            l.joins.insert(at, JoinSwitch { time: t, parent: parent.map(str::to_string), node: node.map(str::to_string), rotate, before });
        }
    }
    // the same places in the new frame (or snapped to joint origin if absolute)
    let snap = absolute && parent.is_some();
    let local: Vec<Affine> = {
        let p = &st.project;
        let s = p.symbol(sym).expect("symbol");
        let l = s.layer(child).expect("layer");
        times
            .iter()
            .zip(&world)
            .map(|(&tt, w)| {
                let m = eval::parent_frame(p, s, l, tt).inverse().mul(w);
                if snap { snap_origin(&m) } else { m }
            })
            .collect()
    };
    let l = st.project.layer_mut(sym, child).expect("layer");
    for (i, q) in props_mut(l).into_iter().enumerate() {
        for (j, &tt) in times.iter().enumerate() {
            let interp = template.iter().find(|k| (k.time - tt).abs() < eps).map_or(Interp::Linear, |k| k.interp);
            let k = q.track.keys().iter().find(|k| (k.time - tt).abs() < eps).copied().unwrap_or(Key::new(tt, Value::F(0.0), interp));
            q.track.put(Key { time: tt, value: Value::F(part(i, ax, ay, old_rot, &local[j])), ..k });
        }
    }
    Ok(())
}
