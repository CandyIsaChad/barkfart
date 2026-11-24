//! New figures don't land exactly on one already there (the bughunt,
//! 2026-11-24: imported figures stacked, their joints overlapping).

use barkfart_core::import::import_file;
use barkfart_core::session::new_editor_project;

#[test]
fn an_imported_figure_steps_aside_from_one_already_there() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/assets/stock/figures/Stickfigure.nodes")).unwrap();
    let mut p = new_editor_project();
    let x_of = |p: &barkfart_core::model::Project, id: &str| p.root().unwrap().layer(id).unwrap().transform.x.key_value(0.0).f();
    let a = import_file(&mut p, "Stickfigure.nodes", &bytes).unwrap().layer;
    let b = import_file(&mut p, "Stickfigure.nodes", &bytes).unwrap().layer;
    let c = import_file(&mut p, "Stickfigure.nodes", &bytes).unwrap().layer;
    let xs = [x_of(&p, &a), x_of(&p, &b), x_of(&p, &c)];
    for i in 0..3 {
        for j in i + 1..3 {
            assert!((xs[i] - xs[j]).abs() >= 40.0, "{xs:?}");
        }
    }
    // and none on the starter figure either
    let starter = p.root().unwrap().layers.iter().find(|l| l.as_figure().is_some()).unwrap().transform.x.key_value(0.0).f();
    assert!(xs.iter().all(|x| (x - starter).abs() >= 40.0), "{xs:?} vs {starter}");
}
