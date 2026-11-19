//! Editing (port of Java app.barkfart.edit). Undo works by snapshots: the
//! project shares its symbols and layers (Arc), so keeping the state before
//! a step costs only pointer copies, and every operation is a plain function
//! that changes the project. One begin/commit is one undo step.

pub mod editor;

use crate::model::Project;
use indexmap::IndexMap;
use std::sync::Arc;

/// Everything an undo step restores: the project and its asset files.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub project: Project,
    /// Asset bytes by archive path ("assets/<id>.<ext>").
    pub assets: IndexMap<String, Arc<Vec<u8>>>,
}

impl State {
    /// True when nothing an undo step would restore differs.
    fn same(&self, o: &State) -> bool {
        let (a, b) = (&self.project, &o.project);
        a.library.len() == b.library.len()
            && a.library.iter().zip(b.library.iter()).all(|((ka, va), (kb, vb))| ka == kb && Arc::ptr_eq(va, vb))
            && a.settings == b.settings
            && a.view == b.view
            && a.root_symbol == b.root_symbol
            && a.assets == b.assets
            && a.node_sets == b.node_sets
            && self.assets.len() == o.assets.len()
            && self.assets.iter().zip(o.assets.iter()).all(|((ka, va), (kb, vb))| ka == kb && Arc::ptr_eq(va, vb))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditError(pub String);

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EditError {}

pub type EditResult<T> = Result<T, EditError>;

pub fn fail<T>(s: impl Into<String>) -> EditResult<T> {
    Err(EditError(s.into()))
}

/// The project and its history.
pub struct Document {
    pub state: State,
    undo: Vec<(String, State)>,
    redo: Vec<(String, State)>,
    open: Option<(String, State, u32)>,
    limit: usize,
    revision: u64,
    saved: u64,
}

pub const DEFAULT_HISTORY: usize = 200;

impl Document {
    pub fn new(project: Project) -> Document {
        Document::with_assets(project, IndexMap::new())
    }

    pub fn with_assets(project: Project, assets: IndexMap<String, Arc<Vec<u8>>>) -> Document {
        Document { state: State { project, assets }, undo: Vec::new(), redo: Vec::new(), open: None, limit: DEFAULT_HISTORY, revision: 0, saved: 0 }
    }

    pub fn project(&self) -> &Project {
        &self.state.project
    }

    pub fn project_mut(&mut self) -> &mut Project {
        &mut self.state.project
    }

    /// Changes every time the document changes (redraw when it moves on).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Starts grouping changes into one undo step; nested calls join the open one.
    pub fn begin(&mut self, label: &str) {
        match &mut self.open {
            Some((_, _, depth)) => *depth += 1,
            None => self.open = Some((label.to_string(), self.state.clone(), 0)),
        }
    }

    /// Ends the group; a group that changed nothing leaves no history entry.
    pub fn commit(&mut self) {
        let Some((label, before, depth)) = self.open.take() else { return };
        if depth > 0 {
            self.open = Some((label, before, depth - 1));
            return;
        }
        if before.same(&self.state) {
            return;
        }
        self.undo.push((label, before));
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
    }

    /// Reverts everything in the open group (a cancelled gesture).
    pub fn cancel(&mut self) {
        if let Some((_, before, _)) = self.open.take() {
            let counter = self.state.project.id_counter;
            self.state = before;
            self.state.project.id_counter = self.state.project.id_counter.max(counter);
            self.revision += 1;
        }
    }

    /// Runs `f` as one undo step; an error reverts it.
    pub fn edit<T>(&mut self, label: &str, f: impl FnOnce(&mut State) -> EditResult<T>) -> EditResult<T> {
        self.begin(label);
        match f(&mut self.state) {
            Ok(v) => {
                self.commit();
                Ok(v)
            }
            Err(e) => {
                // an error inside a nested group reverts the whole group
                self.open.as_mut().map(|o| o.2 = 0);
                self.cancel();
                Err(e)
            }
        }
    }

    pub fn in_transaction(&self) -> bool {
        self.open.is_some()
    }

    pub fn can_undo(&self) -> bool {
        self.open.is_none() && !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        self.open.is_none() && !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(l, _)| l.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(l, _)| l.as_str())
    }

    /// Ids keep counting up across undo and redo, so a new id never meets an old one.
    fn swap_in(&mut self, s: State) -> State {
        let counter = self.state.project.id_counter;
        let old = std::mem::replace(&mut self.state, s);
        self.state.project.id_counter = self.state.project.id_counter.max(counter);
        self.revision += 1;
        old
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        let (label, s) = self.undo.pop().expect("checked");
        let cur = self.swap_in(s);
        self.redo.push((label, cur));
        true
    }

    pub fn redo(&mut self) -> bool {
        if !self.can_redo() {
            return false;
        }
        let (label, s) = self.redo.pop().expect("checked");
        let cur = self.swap_in(s);
        self.undo.push((label, cur));
        true
    }

    /// Marks the current state as saved.
    pub fn mark_saved(&mut self) {
        self.saved = self.revision;
    }

    /// True when there are changes since the last save.
    pub fn dirty(&self) -> bool {
        self.saved != self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn doc() -> Document {
        let mut p = Project { root_symbol: "S1".into(), id_counter: 2, ..Default::default() };
        p.add_symbol(Symbol::new("S1", Some("root"), 1.0));
        Document::new(p)
    }

    #[test]
    fn undo_redo_and_empty_steps() {
        let mut d = doc();
        d.edit("Add", |s| {
            let id = s.project.new_id("L");
            s.project.root_mut().unwrap().layers.push(Arc::new(Layer::figure(&id, Some("f"))));
            Ok(())
        })
        .unwrap();
        assert!(d.can_undo() && d.dirty());
        assert_eq!(d.project().root().unwrap().layers.len(), 1);
        // a step that changes nothing leaves no history entry
        d.edit("Nothing", |_| Ok(())).unwrap();
        assert_eq!(d.undo_label(), Some("Add"));
        assert!(d.undo());
        assert_eq!(d.project().root().unwrap().layers.len(), 0);
        assert!(d.redo());
        assert_eq!(d.project().root().unwrap().layers.len(), 1);
        // a failing step changes nothing
        let r: EditResult<()> = d.edit("Bad", |s| {
            s.project.root_mut().unwrap().layers.clear();
            fail("refused")
        });
        assert!(r.is_err());
        assert_eq!(d.project().root().unwrap().layers.len(), 1);
        // nested groups are one step
        d.begin("Outer");
        d.edit("Inner", |s| {
            s.project.settings.fps = 12.0;
            Ok(())
        })
        .unwrap();
        d.commit();
        assert_eq!(d.undo_label(), Some("Outer"));
        d.mark_saved();
        assert!(!d.dirty());
    }

    #[test]
    fn snapshots_share_untouched_layers() {
        let mut d = doc();
        d.edit("Add", |s| {
            for _ in 0..3 {
                let id = s.project.new_id("L");
                s.project.root_mut().unwrap().layers.push(Arc::new(Layer::figure(&id, None)));
            }
            Ok(())
        })
        .unwrap();
        let before: Vec<Arc<Layer>> = d.project().root().unwrap().layers.clone();
        d.edit("Rename", |s| {
            let id = s.project.root().unwrap().layers[1].id.clone();
            s.project.layer_mut("S1", &id).unwrap().name = Some("x".into());
            Ok(())
        })
        .unwrap();
        let after = &d.project().root().unwrap().layers;
        assert!(Arc::ptr_eq(&before[0], &after[0]) && Arc::ptr_eq(&before[2], &after[2]), "untouched layers shared");
        assert!(!Arc::ptr_eq(&before[1], &after[1]), "the edited layer copied");
    }
}
