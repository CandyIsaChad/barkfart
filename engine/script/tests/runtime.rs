//! SCRIPTING.md ground rules: a sandbox, a time budget, a memory cap, and
//! plugin definitions read into typed props.

use barkfart_script::*;
use std::time::Instant;

const STUTTER: &str = r#"
--!strict
local bf = require("@barkfart")
return bf.modifier {
	id = "yeah.stutter", name = "Stutter", version = "1.0.0", author = "yeah",
	props = {
		every = bf.number { default = 3, min = 1, step = 1, label = "Every" },
		jumpy = bf.toggle { default = true },
		tint = bf.colour { default = bf.rgb(1, 0.8, 0.2) },
		shape = bf.choice { "Ripple", "Twist", "Bulge", default = "Twist" },
	},
	value = function(v, p, t)
		return v:at(math.floor(t.frame / p.every) * p.every)  -- holds every Nth frame
	end,
}
"#;

#[test]
fn a_modifier_loads_with_typed_props() {
    let host = Host::new().unwrap();
    let p = host.load(STUTTER, "stutter").unwrap();
    assert_eq!(p.def.id, "yeah.stutter");
    assert_eq!(p.def.kind, Kind::Modifier);
    assert_eq!(p.def.author.as_deref(), Some("yeah"));
    let names: Vec<&str> = p.def.props.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["every", "jumpy", "shape", "tint"]);
    assert_eq!(p.def.props[0].kind, PropKind::Number { default: 3.0, min: Some(1.0), max: None, step: Some(1.0) });
    assert_eq!(p.def.props[0].label.as_deref(), Some("Every"));
    assert_eq!(p.def.props[1].kind, PropKind::Toggle { default: true });
    assert_eq!(p.def.props[2].kind, PropKind::Choice { options: vec!["Ripple".into(), "Twist".into(), "Bulge".into()], default: 1 });
    assert_eq!(p.def.props[3].kind, PropKind::Colour { default: [1.0, 0.8, 0.2, 1.0] });
}

#[test]
fn a_modifier_drives_a_value() {
    let host = Host::new().unwrap();
    let p = host.load(STUTTER, "stutter").unwrap();
    // the keyed value: 10 per frame; the stutter holds every third frame
    let keyed = |f: f64| f * 10.0;
    let props = vec![("every".to_string(), PropValue::Number(3.0))];
    let at = |frame: f64| host.modifier_value(&p, "L1/x", keyed(frame), &props, Time { frame, seconds: frame / 12.0, fps: 12.0 }, &keyed).unwrap();
    assert_eq!([at(0.0), at(1.0), at(2.0), at(3.0), at(5.0), at(6.0)], [0.0, 0.0, 0.0, 30.0, 30.0, 60.0]);
}

#[test]
fn a_runaway_plugin_is_stopped() {
    let host = Host::new().unwrap();
    let start = Instant::now();
    let e = host.load("while true do end", "loop").err().unwrap();
    assert_eq!(e, ScriptError::TooSlow);
    assert!(start.elapsed().as_millis() < 2000, "stopped on time: {:?}", start.elapsed());
    // a modifier that loops is stopped on each call, and the runtime still works
    let p = host.load("local bf = require('@barkfart') return bf.modifier { id = 'x.spin', version = '1', value = function(v, p, t) while true do end end }", "spin").unwrap();
    let e = host.modifier_value(&p, "L1/x", 1.0, &[], Time { frame: 0.0, seconds: 0.0, fps: 12.0 }, &|f| f).err().unwrap();
    assert_eq!(e, ScriptError::TooSlow);
    let ok = host.load(STUTTER, "stutter").unwrap();
    let props = vec![("every".to_string(), PropValue::Number(3.0))];
    assert_eq!(host.modifier_value(&ok, "L1/x", 5.0, &props, Time { frame: 4.0, seconds: 0.0, fps: 12.0 }, &|f| f * 2.0), Ok(6.0), "the runtime carries on after a stop");
}

#[test]
fn a_greedy_plugin_is_stopped() {
    let host = Host::new().unwrap();
    let e = host.load("local t = {} for i = 1, 1e9 do t[i] = string.rep('x', 1024) end", "greedy").err().unwrap();
    assert!(matches!(e, ScriptError::TooMuchMemory | ScriptError::TooSlow), "{e:?}");
}

#[test]
fn the_sandbox_has_no_way_out() {
    let host = Host::new().unwrap();
    for (what, code) in [("io", "return io"), ("os", "return os"), ("debug", "return debug"), ("loading bytecode", "return string.dump")] {
        let r = host.load(&format!("local x = (function() {code} end)() assert(x == nil, '{what} is there') local bf = require('@barkfart') return bf.tool {{ id = 'x.t', version = '1' }}"), what);
        assert!(r.is_ok(), "{what}: {:?}", r.err());
    }
    // the libraries can't be changed for other plugins
    let r = host.load("math.floor = function() return 0 end", "meddle");
    assert!(r.is_err());
    assert!(host.load(STUTTER, "stutter").is_ok());
}

#[test]
fn mistakes_are_told_plainly() {
    let host = Host::new().unwrap();
    assert!(matches!(host.load("return {", "broken").err().unwrap(), ScriptError::Load(_)));
    assert!(matches!(host.load("return 3", "number").err().unwrap(), ScriptError::Invalid(_)));
    let e = host.load("local bf = require('@barkfart') return bf.modifier { version = '1', value = function() return 0 end }", "noid").err().unwrap();
    assert_eq!(e, ScriptError::Invalid("it needs id = \"...\"".into()));
    let e = host.load("local bf = require('@barkfart') return bf.modifier { id = 'x.m', version = '1' }", "novalue").err().unwrap();
    assert!(matches!(e, ScriptError::Invalid(ref m) if m.contains("value = function")), "{e:?}");
    let e = host.load("local bf = require('@barkfart') return bf.effect { id = 'x y', version = '1' }", "badid").err().unwrap();
    assert!(matches!(e, ScriptError::Invalid(ref m) if m.contains("letters")), "{e:?}");
    assert!(e.to_string().starts_with("This isn't a Barkfart plugin"));
}

#[test]
fn plugins_keep_to_themselves() {
    let host = Host::new().unwrap();
    // one plugin's globals never reach another
    host.load("leak = 1 local bf = require('@barkfart') return bf.tool { id = 'x.a', version = '1' }", "a").unwrap();
    let r = host.load("assert(leak == nil, 'leaked') local bf = require('@barkfart') return bf.tool { id = 'x.b', version = '1' }", "b");
    assert!(r.is_ok(), "{:?}", r.err());
    // and none can break the API for the rest
    assert!(host.load("require('@barkfart').number = nil", "break").is_err());
    let r = host.load("local bf = require('@barkfart') assert(bf.number ~= nil) return bf.tool { id = 'x.c', version = '1' }", "c");
    assert!(r.is_ok(), "{:?}", r.err());
}

#[test]
fn randomness_is_the_same_whatever_order_frames_are_drawn_in() {
    let host = Host::new().unwrap();
    let p = host.load("local bf = require('@barkfart') assert(math.random == nil) return bf.modifier { id = 'x.jitter', version = '1', value = function(v, p, t) return v.value + t.random(-1, 1) + t.random(-1, 1) end }", "jitter").unwrap();
    let at = |frame: f64, key: &str| host.modifier_value(&p, key, 0.0, &[], Time { frame, seconds: frame / 12.0, fps: 12.0 }, &|f| f).unwrap();
    let forward: Vec<f64> = (0..8).map(|f| at(f as f64, "L1/x")).collect();
    let backward: Vec<f64> = (0..8).rev().map(|f| at(f as f64, "L1/x")).collect::<Vec<_>>().into_iter().rev().collect();
    assert_eq!(forward, backward);
    assert!(forward.windows(2).any(|w| w[0] != w[1]), "it does jitter: {forward:?}");
    assert!(forward.iter().all(|v| v.abs() <= 2.0));
    assert_ne!(at(3.0, "L1/x"), at(3.0, "L1/y"), "each value gets its own numbers");
}

const CAPE: &str = r#"
local bf = require("@barkfart")
return bf.script {
	id = "yeah.cape", name = "Cape sway", version = "1.0.0", author = "yeah",
	targets = { cape = bf.figure(), glow = bf.value { label = "Glow" } },
	props = { stiffness = bf.number { default = 0.4, min = 0, max = 1 } },
	start = function(s)
		s.swing, s.speed = 0, 0
	end,
	step = function(s, p, t, scene)
		local push = -scene.cape.velocity.x * t.dt * 0.05
		s.speed = (s.speed + push - s.swing * p.stiffness) * 0.9
		s.swing += s.speed
		return {
			cape = { rotate = { root = s.swing, arm = t.random(-1, 1) }, move = { y = t.elapsed } },
			glow = scene.glow + 1,
		}
	end,
}
"#;

fn cape_scene(frames: usize) -> Vec<Vec<(String, Seen)>> {
    (0..frames)
        .map(|k| {
            vec![
                ("cape".to_string(), Seen::Figure { x: k as f64 * 10.0, y: 0.0, rotation: 0.0, vx: 300.0, vy: 0.0, joints: vec![("root".into(), 0.0, 0.0, 0.0)] }),
                ("glow".to_string(), Seen::Value(k as f64)),
            ]
        })
        .collect()
}

#[test]
fn a_script_runs_forward_with_its_own_state() {
    let host = Host::new().unwrap();
    let p = host.load(CAPE, "cape").unwrap();
    assert_eq!(p.def.kind, Kind::Script);
    let names: Vec<(&str, TargetKind)> = p.def.targets.iter().map(|t| (t.name.as_str(), t.kind)).collect();
    assert_eq!(names, [("cape", TargetKind::Figure), ("glow", TargetKind::Value)]);
    let scene = cape_scene(20);
    let props = [("stiffness".to_string(), PropValue::Number(0.4))];
    let out = host.script_run(&p, &Run { seed: 1, props: &props, fps: 24.0, first_frame: 5, scene: &scene }).unwrap();
    assert_eq!(out.len(), 20);
    // the state carries over: the swing builds up from 0 and doesn't stay put
    let swing: Vec<f64> = out.iter().map(|f| f[0].1.rotate.iter().find(|r| r.0 == "root").unwrap().1).collect();
    assert!(swing[0] < 0.0 && swing[3] < swing[0], "{swing:?}");
    assert!(swing.windows(2).any(|w| w[1] > w[0]), "it swings back: {swing:?}");
    // t.elapsed starts at 0 on the block's first frame; values see the scene
    assert_eq!(out[0][0].1.y, Some(0.0));
    assert!((out[12][0].1.y.unwrap() - 0.5).abs() < 1e-9);
    assert_eq!(out[7][1].1.value, Some(8.0));
}

#[test]
fn a_run_is_repeatable_and_each_seed_differs() {
    let host = Host::new().unwrap();
    let p = host.load(CAPE, "cape").unwrap();
    let scene = cape_scene(10);
    let props = [("stiffness".to_string(), PropValue::Number(0.4))];
    let arm = |seed: u64| -> Vec<f64> {
        let out = host.script_run(&p, &Run { seed, props: &props, fps: 24.0, first_frame: 0, scene: &scene }).unwrap();
        out.iter().map(|f| f[0].1.rotate.iter().find(|r| r.0 == "arm").unwrap().1).collect()
    };
    assert_eq!(arm(7), arm(7));
    // another host, same numbers
    let host2 = Host::new().unwrap();
    let p2 = host2.load(CAPE, "cape").unwrap();
    let again = host2.script_run(&p2, &Run { seed: 7, props: &props, fps: 24.0, first_frame: 0, scene: &scene }).unwrap();
    assert_eq!(arm(7), again.iter().map(|f| f[0].1.rotate.iter().find(|r| r.0 == "arm").unwrap().1).collect::<Vec<_>>());
    assert_ne!(arm(7), arm(8));
}

#[test]
fn a_runaway_script_is_stopped() {
    let host = Host::new().unwrap();
    let src = r#"
local bf = require("@barkfart")
return bf.script { id = "x.loop", version = "1", targets = { a = bf.value() },
	step = function(s, p, t) while true do end end }
"#;
    let p = host.load(src, "loop").unwrap();
    let scene = vec![vec![("a".to_string(), Seen::Value(0.0))]; 3];
    let at = Instant::now();
    assert_eq!(host.script_run(&p, &Run { seed: 1, props: &[], fps: 24.0, first_frame: 0, scene: &scene }).err(), Some(ScriptError::TooSlow));
    assert!(at.elapsed().as_millis() < 500);
    // and a wrong answer is told plainly
    let src = r#"
local bf = require("@barkfart")
return bf.script { id = "x.bad", version = "1", targets = { a = bf.value() },
	step = function() return { a = "lots" } end }
"#;
    let p = host.load(src, "bad").unwrap();
    match host.script_run(&p, &Run { seed: 1, props: &[], fps: 24.0, first_frame: 0, scene: &scene }) {
        Err(ScriptError::Runtime(m)) => assert!(m.contains("a must be a number"), "{m}"),
        other => panic!("{other:?}"),
    }
    // a script without targets or step isn't one
    assert!(matches!(host.load("local bf = require('@barkfart') return bf.script { id = 'x.y', version = '1', step = function() end }", "n"), Err(ScriptError::Invalid(_))));
    assert!(matches!(host.load("local bf = require('@barkfart') return bf.script { id = 'x.y', version = '1', targets = { a = bf.value() } }", "n"), Err(ScriptError::Invalid(_))));
}

#[test]
fn every_stock_workshop_plugin_loads_and_runs_cleanly() {
    let host = Host::new().unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/barkfart/plugins");
    let mut scripts = 0;
    let mut modifiers = 0;
    let mut effects = 0;
    for entry in std::fs::read_dir(&dir).expect("docs/barkfart/plugins exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("luau") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        let src = std::fs::read_to_string(&path).unwrap();
        let plugin = host.load(&src, name).unwrap_or_else(|e| panic!("{name} failed to load: {e}"));
        let props: Vec<(String, PropValue)> = plugin
            .def
            .props
            .iter()
            .map(|p| {
                let v = match &p.kind {
                    PropKind::Number { default, .. } | PropKind::Angle { default } => PropValue::Number(*default),
                    PropKind::Toggle { default } => PropValue::Toggle(*default),
                    PropKind::Colour { default } => PropValue::Colour(*default),
                    PropKind::Choice { options, default } => PropValue::Text(options.get(*default).cloned().unwrap_or_default()),
                    PropKind::Text { default } => PropValue::Text(default.clone()),
                    PropKind::Point { default } => PropValue::Point(default.0, default.1),
                };
                (p.name.clone(), v)
            })
            .collect();
        match plugin.def.kind {
            Kind::Script => {
                let scene: Vec<Vec<(String, Seen)>> = (0..12)
                    .map(|k| {
                        plugin
                            .def
                            .targets
                            .iter()
                            .map(|tg| {
                                let seen = match tg.kind {
                                    TargetKind::Figure => Seen::Figure {
                                        x: k as f64 * 12.0,
                                        y: (k as f64).sin() * 5.0,
                                        rotation: 0.0,
                                        vx: 240.0,
                                        vy: 0.0,
                                        joints: vec![("root".into(), 0.0, 0.0, 0.0)],
                                    },
                                    TargetKind::Value => Seen::Value(k as f64 * 0.5),
                                };
                                (tg.name.clone(), seen)
                            })
                            .collect()
                    })
                    .collect();
                let out = host.script_run(&plugin, &Run { seed: 42, props: &props, fps: 24.0, first_frame: 0, scene: &scene }).unwrap_or_else(|e| panic!("{name} script_run failed: {e}"));
                assert_eq!(out.len(), 12, "{name}");
                scripts += 1;
            }
            Kind::Modifier => {
                let sample = |f: f64| f * 5.0;
                for frame in [0.0, 1.0, 5.0, 12.0] {
                    let v = host.modifier_value(&plugin, "L1/x", sample(frame), &props, Time { frame, seconds: frame / 24.0, fps: 24.0 }, &sample).unwrap_or_else(|e| panic!("{name} modifier failed: {e}"));
                    assert!(v.is_finite(), "{name} non-finite output {v}");
                }
                modifiers += 1;
            }
            Kind::Effect => {
                assert!(plugin.def.shader.is_some(), "{name} missing shader");
                effects += 1;
            }
            Kind::Tool => {}
        }
    }
    assert!(scripts >= 8, "expected >= 8 stock scripts, got {scripts}");
    assert!(modifiers >= 3, "expected >= 3 stock modifiers, got {modifiers}");
    assert!(effects >= 3, "expected >= 3 stock effects, got {effects}");
}
