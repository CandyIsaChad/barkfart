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
    let at = |frame: f64| host.modifier_value(&p, keyed(frame), &props, Time { frame, seconds: frame / 12.0, fps: 12.0 }, &keyed).unwrap();
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
    let e = host.modifier_value(&p, 1.0, &[], Time { frame: 0.0, seconds: 0.0, fps: 12.0 }, &|f| f).err().unwrap();
    assert_eq!(e, ScriptError::TooSlow);
    let ok = host.load(STUTTER, "stutter").unwrap();
    let props = vec![("every".to_string(), PropValue::Number(3.0))];
    assert_eq!(host.modifier_value(&ok, 5.0, &props, Time { frame: 4.0, seconds: 0.0, fps: 12.0 }, &|f| f * 2.0), Ok(6.0), "the runtime carries on after a stop");
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
