//! Legacy import against the Java importer (engine/testdata/legacy):
//! synthetic samples (committed) and, when present, local samples from the
//! original app (never committed).

use std::path::{Path, PathBuf};

fn check_dir(dir: &Path) -> (usize, Vec<String>) {
    let mut failures = Vec::new();
    let mut checked = 0;
    let Ok(rd) = std::fs::read_dir(dir) else { return (0, failures) };
    let mut inputs: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_lowercase();
            [".nodes", ".nodemc", ".stknds", ".wav", ".png", ".gif"].iter().any(|e| n.ends_with(e)) && !n.contains(".export")
        })
        .collect();
    inputs.sort();
    for input in inputs {
        let name = input.file_name().unwrap().to_string_lossy().to_string();
        let side = |suffix: &str| dir.join(format!("{name}{suffix}"));
        let bytes = std::fs::read(&input).unwrap();
        let mut p = barkfart_core::codec::decode(&std::fs::read_to_string(side(".before.json")).unwrap()).unwrap();
        let r = barkfart_core::import::import_file(&mut p, &name, &bytes);
        checked += 1;
        if side(".unsupported.txt").exists() || side(".error.txt").exists() {
            if r.is_ok() {
                failures.push(format!("{name}: Java refused it, Rust imported it"));
            }
            continue;
        }
        if let Err(e) = r {
            failures.push(format!("{name}: {e}"));
            continue;
        }
        let want = std::fs::read_to_string(side(".after.json")).unwrap();
        let got = barkfart_core::codec::encode(&p);
        if want != got {
            let line = want.lines().zip(got.lines()).position(|(a, b)| a != b).unwrap_or(0);
            failures.push(format!(
                "{name}: project differs at line {}\n    java: {:?}\n    rust: {:?}",
                line + 1,
                want.lines().nth(line).unwrap_or(""),
                got.lines().nth(line).unwrap_or("")
            ));
            continue;
        }
        // re-export each figure: same bytes as the Java exporter
        let mut k = 0;
        for s in p.library.values() {
            for l in &s.layers {
                if l.as_figure().is_none() {
                    continue;
                }
                let want = std::fs::read(side(&format!(".export{k}.nodes"))).unwrap();
                let fig = barkfart_core::legacy::to_legacy(l).unwrap();
                let got = barkfart_core::legacy::write_nodes(&fig);
                let same = if fig.compressed {
                    // compressed bytes depend on the zlib implementation: compare the bodies
                    barkfart_core::legacy::write_nodes(&barkfart_core::legacy::read_nodes(&want).unwrap()) == got
                        && barkfart_core::legacy::read_nodes(&got).is_ok()
                } else {
                    want == got
                };
                if !same {
                    failures.push(format!("{name}: figure {k} re-exports differently"));
                }
                k += 1;
            }
        }
    }
    (checked, failures)
}

#[test]
fn synthetic_samples_import_like_java() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/legacy/synthetic");
    let (n, failures) = check_dir(&dir);
    assert!(n >= 8);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn original_samples_import_like_java_when_present() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/legacy/local");
    let (n, failures) = check_dir(&dir);
    eprintln!("{n} local samples checked");
    assert!(failures.is_empty(), "{} of {n} differ:\n{}", failures.len(), failures.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

/// A movieclip saved by Stick Nodes Pro 4.3.1 (version 431): the IK figure with
/// one foot pulled in (local file, made on the user's phone 2026-11-21).
#[test]
fn imports_a_version_431_movieclip() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/barkfart/research/local/zz_ik_probe.nodemc");
    let Ok(bytes) = std::fs::read(path) else { return };
    let mut p = barkfart_core::session::new_project();
    p.root_mut().unwrap().layers.clear();
    let got = barkfart_core::import::import_file(&mut p, "zz_ik_probe.nodemc", &bytes).expect("version 431 reads");
    let _ = got;
    let sym = p.library.values().find(|s| s.id != p.root_symbol).expect("the movieclip symbol");
    assert!((sym.duration - 0.4).abs() < 1e-6, "4 frames at 10 fps");
    let l = sym.layers.iter().find(|l| l.as_figure().is_some()).unwrap();
    let f = l.as_figure().unwrap();
    let mut r = barkfart_core::eval::Resolver::new(&p);
    let t = 0.1; // frame 2
    assert!((r.f(&f.node("n30").unwrap().length, t) - 176.7).abs() < 0.1);
    let pose = barkfart_core::eval::FigurePose::compute(f, t, &mut r);
    let at = |n: &str| { let i = f.index_of(n).unwrap(); (pose.x[i], pose.y[i]) };
    let d = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
    assert!((d(at("n13"), at("n0")) - 141.0).abs() < 0.3, "the knee point as the original placed it");
    assert!((d(at("n27"), at("n10")) - 148.3).abs() < 0.3);
}
