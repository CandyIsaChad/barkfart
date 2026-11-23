//! Groups of layers (REDESIGN K, D33). A group is a record in its symbol
//! (`Symbol::groups`) plus a `group` id on each member layer. One level only;
//! members sit next to each other in row order; camera, sound and adjustment
//! layers are never members. `tidy` restores these rules after any edit.

use super::{fail, EditResult, State};
use crate::model::*;
use std::sync::Arc;

/// Whether a layer may be in a group.
pub fn groupable(l: &Layer) -> bool {
    !matches!(l.kind, LayerKind::Camera(_) | LayerKind::Audio(_) | LayerKind::Adjustment)
}

/// Makes every symbol keep the group rules: memberships only of groupable
/// layers and known groups; each group's members next to each other
/// (gathered at its frontmost member, keeping their order); no empty groups.
pub fn tidy(st: &mut State) {
    for sym in st.project.library.values_mut() {
        // symbols stay shared unless they have groups to look after
        if !sym.groups.is_empty() || sym.layers.iter().any(|l| l.group.is_some()) {
            tidy_symbol(Arc::make_mut(sym));
        }
    }
}

fn tidy_symbol(s: &mut Symbol) {
    // memberships that break the rules go
    for i in 0..s.layers.len() {
        let bad = match &s.layers[i].group {
            Some(g) => !groupable(&s.layers[i]) || !s.groups.iter().any(|x| &x.id == g),
            None => false,
        };
        if bad {
            Arc::make_mut(&mut s.layers[i]).group = None;
        }
    }
    // each group's members next to each other
    let ids: Vec<String> = s.groups.iter().map(|g| g.id.clone()).collect();
    for g in &ids {
        let at: Vec<usize> = (0..s.layers.len()).filter(|&i| s.layers[i].group.as_deref() == Some(g)).collect();
        if at.len() < 2 || at.last().unwrap() - at.first().unwrap() + 1 == at.len() {
            continue;
        }
        gather(s, &at);
    }
    // groups with no members go
    let used: std::collections::HashSet<String> = s.layers.iter().filter_map(|l| l.group.clone()).collect();
    s.groups.retain(|g| used.contains(&g.id));
}

/// Moves the layers at `at` (ascending) together at the frontmost one,
/// keeping their order; the layers that sat between them go behind.
fn gather(s: &mut Symbol, at: &[usize]) {
    let front = *at.last().expect("members");
    let moved: Vec<Arc<Layer>> = at.iter().rev().map(|&i| s.layers.remove(i)).collect::<Vec<_>>().into_iter().rev().collect();
    let pos = front + 1 - moved.len();
    for (k, l) in moved.into_iter().enumerate() {
        s.layers.insert(pos + k, l);
    }
}

fn symbol<'a>(st: &'a mut State, id: &str) -> EditResult<&'a mut Symbol> {
    match st.project.symbol_mut(id) {
        Some(s) => Ok(s),
        None => fail(format!("no symbol {id}")),
    }
}

/// "Group": the layers become one group (leaving any group they were in),
/// gathered at the frontmost of them. Returns the group's id.
pub fn group(st: &mut State, sym: &str, layers: &[String], name: Option<&str>) -> EditResult<String> {
    let gid = st.project.new_id("G");
    let s = symbol(st, sym)?;
    let at: Vec<usize> = (0..s.layers.len()).filter(|&i| layers.contains(&s.layers[i].id) && groupable(&s.layers[i])).collect();
    if at.is_empty() {
        return fail("Only figures, pictures, text and movieclips go in a group.");
    }
    let n = s.groups.len() + 1;
    let name = name.map(str::to_string).unwrap_or_else(|| format!("Group {n}"));
    s.groups.push(LayerGroup::new(&gid, &name));
    for &i in &at {
        Arc::make_mut(&mut s.layers[i]).group = Some(gid.clone());
    }
    if at.len() > 1 {
        gather(s, &at);
    }
    tidy_symbol(s);
    Ok(gid)
}

/// "Ungroup": the members stay where they are, on their own.
pub fn ungroup(st: &mut State, sym: &str, gid: &str) -> EditResult<()> {
    let s = symbol(st, sym)?;
    if !s.groups.iter().any(|g| g.id == gid) {
        return fail("There is no such group.");
    }
    for l in s.layers.iter_mut() {
        if l.group.as_deref() == Some(gid) {
            Arc::make_mut(l).group = None;
        }
    }
    s.groups.retain(|g| g.id != gid);
    Ok(())
}

/// "Delete group": the group and every layer in it.
pub fn delete(st: &mut State, sym: &str, gid: &str) -> EditResult<()> {
    let s = symbol(st, sym)?;
    if !s.groups.iter().any(|g| g.id == gid) {
        return fail("There is no such group.");
    }
    let gone: Vec<String> = s.layers.iter().filter(|l| l.group.as_deref() == Some(gid)).map(|l| l.id.clone()).collect();
    s.layers.retain(|l| l.group.as_deref() != Some(gid));
    s.groups.retain(|g| g.id != gid);
    // figures joined to a deleted layer are on their own now
    for l in s.layers.iter_mut() {
        if l.parent.as_ref().is_some_and(|p| gone.contains(p)) || l.joins.iter().any(|j| j.parent.as_ref().is_some_and(|p| gone.contains(p))) {
            let l = Arc::make_mut(l);
            if l.parent.as_ref().is_some_and(|p| gone.contains(p)) {
                l.parent = None;
                l.parent_node = None;
            }
            for j in l.joins.iter_mut() {
                if j.parent.as_ref().is_some_and(|p| gone.contains(p)) {
                    j.parent = None;
                    j.node = None;
                }
            }
        }
    }
    Ok(())
}

/// The group page's settings: name, opacity, open (the folder row).
pub fn set(st: &mut State, sym: &str, gid: &str, name: Option<&str>, opacity: Option<f32>, open: Option<bool>) -> EditResult<()> {
    let s = symbol(st, sym)?;
    let Some(g) = s.groups.iter_mut().find(|g| g.id == gid) else { return fail("There is no such group.") };
    if let Some(n) = name {
        let n = n.trim();
        if n.is_empty() {
            return fail("A group needs a name.");
        }
        g.name = n.to_string();
    }
    if let Some(o) = opacity {
        g.opacity = o.clamp(0.0, 1.0);
    }
    if let Some(o) = open {
        g.open = o;
    }
    Ok(())
}
