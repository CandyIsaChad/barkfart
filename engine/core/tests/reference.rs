//! Checks against reference data exported from the Java version
//! (engine/testdata/ref, made by core/src/test/java/app/barkfart/RefDumpTest.java).

use std::path::PathBuf;

fn ref_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/ref")
}

fn cases() -> Vec<String> {
    let idx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(ref_dir().join("index.json")).unwrap()).unwrap();
    idx.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn every_reference_project_loads() {
    let names = cases();
    assert!(names.len() >= 80);
    for n in &names {
        let json = std::fs::read_to_string(ref_dir().join(n).join("project.json")).unwrap();
        let p = barkfart_core::codec::decode(&json).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert!(p.root().is_some(), "{n}");
    }
}
