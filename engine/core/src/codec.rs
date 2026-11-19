//! Reading project.json (port of Java `ProjectCodec.decode`; FORMAT.md rules:
//! optional fields default to the model's values, unknown fields are ignored,
//! a newer formatVersion is refused).

use crate::model::*;
use serde_json::{Map, Value as J};

#[derive(Debug, Clone, PartialEq)]
pub struct FormatError(pub String);

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormatError {}

type R<T> = Result<T, FormatError>;

fn err<T>(s: impl Into<String>) -> R<T> {
    Err(FormatError(s.into()))
}

type Obj = Map<String, J>;

fn as_map<'a>(o: &'a J, what: &str) -> R<&'a Obj> {
    o.as_object().ok_or_else(|| FormatError(format!("{what} must be an object")))
}

fn list<'a>(m: &'a Obj, k: &str) -> R<&'a [J]> {
    match m.get(k) {
        None | Some(J::Null) => Ok(&[]),
        Some(J::Array(a)) => Ok(a),
        _ => err(format!("{k} must be a list")),
    }
}

fn str_<'a>(m: &'a Obj, k: &str) -> R<Option<&'a str>> {
    match m.get(k) {
        None | Some(J::Null) => Ok(None),
        Some(J::String(s)) => Ok(Some(s)),
        _ => err(format!("{k} must be a string")),
    }
}

fn req<'a>(m: &'a Obj, k: &str) -> R<&'a str> {
    str_(m, k)?.ok_or_else(|| FormatError(format!("missing {k}")))
}

fn num_text(o: Option<&J>, k: &str) -> R<String> {
    match o {
        Some(J::Number(n)) => Ok(n.to_string()),
        _ => err(format!("{k} must be a number")),
    }
}

fn parse_f(o: Option<&J>, k: &str) -> R<f32> {
    num_text(o, k)?.parse::<f32>().map_err(|e| FormatError(format!("{k}: {e}")))
}

fn parse_d(o: Option<&J>, k: &str) -> R<f64> {
    num_text(o, k)?.parse::<f64>().map_err(|e| FormatError(format!("{k}: {e}")))
}

fn parse_i(o: Option<&J>, k: &str) -> R<i32> {
    num_text(o, k)?.parse::<i32>().map_err(|e| FormatError(format!("{k}: {e}")))
}

fn has(m: &Obj, k: &str) -> bool {
    m.contains_key(k)
}

fn f(m: &Obj, k: &str, def: f32) -> R<f32> {
    if has(m, k) {
        parse_f(m.get(k), k)
    } else {
        Ok(def)
    }
}

fn d(m: &Obj, k: &str, def: f64) -> R<f64> {
    if has(m, k) {
        parse_d(m.get(k), k)
    } else {
        Ok(def)
    }
}

fn i(m: &Obj, k: &str, def: i32) -> R<i32> {
    if has(m, k) {
        parse_i(m.get(k), k)
    } else {
        Ok(def)
    }
}

fn b(m: &Obj, k: &str, def: bool) -> R<bool> {
    match m.get(k) {
        None => Ok(def),
        Some(J::Bool(v)) => Ok(*v),
        _ => err(format!("{k} must be a boolean")),
    }
}

fn parse_color(s: &str) -> R<Color> {
    if s.len() != 9 || !s.starts_with('#') {
        return err(format!("bad colour {s}"));
    }
    let v = u32::from_str_radix(&s[1..], 16).map_err(|_| FormatError(format!("bad colour {s}")))?;
    Ok(Color::rgba8888(v))
}

fn parse_value(t: ValueType, o: Option<&J>) -> R<Value> {
    Ok(match t {
        ValueType::Float | ValueType::Angle => Value::F(parse_f(o, "value")?),
        ValueType::Double => Value::D(parse_d(o, "value")?),
        ValueType::Color => match o {
            Some(J::String(s)) => Value::C(parse_color(s)?),
            _ => return err("colour must be a string"),
        },
        ValueType::Bool => match o {
            Some(J::Bool(v)) => Value::B(*v),
            _ => return err("bool value expected"),
        },
    })
}

fn read_prop(owner: &Obj, k: &str, p: &mut Property) -> R<()> {
    let Some(o) = owner.get(k) else { return Ok(()) };
    if o.is_null() {
        return Ok(());
    }
    let m = as_map(o, k)?;
    p.value = parse_value(p.ty, m.get("v"))?;
    fill_prop(m, p)
}

fn fill_prop(m: &Obj, p: &mut Property) -> R<()> {
    for ko in list(m, "k")? {
        let k = ko.as_array().ok_or_else(|| FormatError("key must be a list".into()))?;
        let time = parse_d(k.first(), "key time")?;
        if !time.is_finite() {
            return err(format!("bad key time {time}"));
        }
        let v = parse_value(p.ty, k.get(1))?;
        let interp = match k.get(2).and_then(J::as_str) {
            Some("H") => Interp::Hold,
            Some("B") => Interp::Bezier,
            _ => Interp::Linear,
        };
        let mut ease = Ease::LINEAR;
        if interp == Interp::Bezier {
            ease = Ease::new(parse_f(k.get(3), "x1")?, parse_f(k.get(4), "y1")?, parse_f(k.get(5), "x2")?, parse_f(k.get(6), "y2")?);
            if let Some(J::String(name)) = k.get(7) {
                if let Some(kind) = EaseKind::from_name(name) {
                    ease = Ease::named(kind);
                }
            }
        }
        if !p.ty.checked(&v) {
            return err("key value type");
        }
        p.track.put(Key { time, value: v, interp, ease });
    }
    for mo in list(m, "m")? {
        let mm = as_map(mo, "modifier")?;
        p.add_modifier(parse_modifier(mm)?).map_err(FormatError)?;
    }
    Ok(())
}

fn parse_modifier(m: &Obj) -> R<Modifier> {
    let kind = match req(m, "type")? {
        "wiggle" => ModifierKind::Wiggle {
            frequency: f(m, "frequency", 1.0)?,
            amplitude: f(m, "amplitude", 0.0)?,
            seed: i(m, "seed", 0)?,
            octaves: i(m, "octaves", 1)?,
        },
        "sine" => ModifierKind::Sine { frequency: f(m, "frequency", 1.0)?, amplitude: f(m, "amplitude", 0.0)?, phase: f(m, "phase", 0.0)? },
        "loop" => ModifierKind::Loop {
            mode: match str_(m, "mode")? {
                None | Some("CYCLE") => LoopMode::Cycle,
                Some("PING_PONG") => LoopMode::PingPong,
                Some(x) => return err(format!("loop mode {x}")),
            },
            start: d(m, "start", 0.0)?,
            end: d(m, "end", 1.0)?,
        },
        "delayFollow" => ModifierKind::DelayFollow {
            source: req(m, "source")?.into(),
            delay: d(m, "delay", 0.0)?,
            scale: f(m, "scale", 1.0)?,
            offset: f(m, "offset", 0.0)?,
        },
        "link" => ModifierKind::Link { source: req(m, "source")?.into(), scale: f(m, "scale", 1.0)?, offset: f(m, "offset", 0.0)? },
        other => return err(format!("unknown modifier {other}")),
    };
    let mut md = Modifier::new(kind);
    md.enabled = b(m, "enabled", true)?;
    Ok(md)
}

fn scope(m: &Obj) -> R<Scope> {
    let kind = match req(m, "kind")? {
        "NODE" => ScopeKind::Node,
        "NODE_SET" => ScopeKind::NodeSet,
        "FIGURE" => ScopeKind::Figure,
        "LAYER" => ScopeKind::Layer,
        "SYMBOL_INSTANCE" => ScopeKind::SymbolInstance,
        "COMPOSITION" => ScopeKind::Composition,
        x => return err(format!("scope kind {x}")),
    };
    Ok(Scope { kind, target: str_(m, "target")?.map(Into::into), node: str_(m, "node")?.map(Into::into) })
}

fn read_stack(m: &Obj, st: &mut EffectStack, owner: &str) -> R<()> {
    for o in list(m, "nodes")? {
        let em = as_map(o, "effect")?;
        let mut e = EffectNode::new(owner, req(em, "id")?, req(em, "type")?);
        e.enabled = b(em, "enabled", true)?;
        read_prop(em, "mix", &mut e.mix)?;
        if let Some(params) = em.get("params") {
            for (name, pv) in as_map(params, "params")? {
                let pm = as_map(pv, "param")?;
                let vt = ValueType::from_name(req(pm, "t")?).ok_or_else(|| FormatError("param type".into()))?;
                let v = parse_value(vt, pm.get("v"))?;
                let p = e.param(owner, name, vt, v);
                fill_prop(pm, p)?;
            }
        }
        for inp in list(em, "inputs")? {
            if let Some(s) = inp.as_str() {
                e.inputs.push(s.into());
            }
        }
        if let Some(mask) = em.get("mask") {
            if !mask.is_null() {
                e.mask = Some(scope(as_map(mask, "mask")?)?);
            }
        }
        if st.nodes.contains_key(&e.id) {
            return err(format!("duplicate effect {}", e.id));
        }
        st.nodes.insert(e.id.clone(), e);
    }
    st.output = str_(m, "output")?.map(Into::into);
    Ok(())
}

fn node(layer_id: &str, m: &Obj) -> R<Node> {
    let shape = match str_(m, "shape")? {
        None => Shape::Line,
        Some(s) => Shape::from_name(s).ok_or_else(|| FormatError(format!("shape {s}")))?,
    };
    let mut n = Node::new(layer_id, req(m, "id")?, str_(m, "parent")?, shape);
    n.draw_order = i(m, "draw", UNASSIGNED)?;
    n.curve = f(m, "curve", 0.0)?;
    n.triangle_upside_down = b(m, "upsideDown", false)?;
    n.right_triangle = i(m, "rightTriangle", 0)?;
    n.is_static = b(m, "static", false)?;
    n.stretchy = b(m, "stretchy", false)?;
    n.smart_stretch = b(m, "smartStretch", false)?;
    n.no_smart_stretch = b(m, "noSmartStretch", false)?;
    n.rest_length = f(m, "restLength", 0.0)?;
    n.circle_hollow = b(m, "hollow", false)?;
    n.circle_outline = b(m, "outline", false)?;
    read_prop(m, "outlineColor", &mut n.outline_color)?;
    n.half_arc = i(m, "halfArc", 0)?;
    n.polygon_sides = i(m, "sides", 0)?;
    n.trapezoid_ratio = f(m, "trapezoidRatio", 1.0)?;
    n.trapezoid_round_start = b(m, "roundStart", false)?;
    n.trapezoid_round_end = b(m, "roundEnd", false)?;
    n.angle_lock = i(m, "angleLock", 0)?;
    n.drag_lock = i(m, "dragLock", 0)?;
    n.pinned = b(m, "pinned", false)?;
    if let Some(g) = m.get("gradient") {
        let gm = as_map(g, "gradient")?;
        n.gradient = b(gm, "on", true)?;
        n.gradient_reverse = b(gm, "reverse", false)?;
        n.gradient_sideways = b(gm, "sideways", false)?;
        n.gradient_linear = b(gm, "linear", false)?;
        read_prop(gm, "color", &mut n.gradient_color)?;
    }
    read_prop(m, "rotation", &mut n.rotation)?;
    read_prop(m, "length", &mut n.length)?;
    read_prop(m, "thickness", &mut n.thickness)?;
    read_prop(m, "scale", &mut n.scale)?;
    read_prop(m, "color", &mut n.color)?;
    read_prop(m, "shown", &mut n.shown)?;
    if let Some(ex) = m.get("legacy") {
        for (k, v) in as_map(ex, "legacy")? {
            let s = v.as_str().ok_or_else(|| FormatError("legacy extra".into()))?;
            if s.len() < 2 || !matches!(s.as_bytes()[0], b'i' | b'f' | b'b' | b's') {
                return err(format!("bad legacy extra {s}"));
            }
            n.legacy_extras.insert(k.clone(), s.to_string());
        }
    }
    Ok(n)
}

fn layer(m: &Obj) -> R<Layer> {
    let id = req(m, "id")?;
    let name = str_(m, "name")?;
    let mut l = match req(m, "type")? {
        "FIGURE" => {
            let mut l = Layer::figure(id, name);
            let LayerKind::Figure(fig) = &mut l.kind else { unreachable!() };
            for o in list(m, "nodes")? {
                fig.add(node(id, as_map(o, "node")?)?).map_err(FormatError)?;
            }
            for o in list(m, "polyfills")? {
                let pm = as_map(o, "polyfill")?;
                let mut pf = Polyfill::new(id, req(pm, "id")?);
                pf.anchor = req(pm, "anchor")?.into();
                for nid in list(pm, "nodes")? {
                    pf.nodes.push(nid.as_str().ok_or_else(|| FormatError("polyfill node".into()))?.into());
                }
                pf.use_own_color = b(pm, "ownColor", false)?;
                read_prop(pm, "color", &mut pf.color)?;
                fig.polyfills.push(pf);
            }
            l
        }
        "SPRITE" => {
            let mut l = Layer::sprite(id, name);
            let LayerKind::Sprite(s) = &mut l.kind else { unreachable!() };
            s.asset = str_(m, "asset")?.map(Into::into);
            s.width = f(m, "width", 0.0)?;
            s.height = f(m, "height", 0.0)?;
            read_prop(m, "tint", &mut s.tint)?;
            if let Some(J::Array(dl)) = m.get("distort") {
                for (k, dv) in dl.iter().take(8).enumerate() {
                    if let J::Object(dm) = dv {
                        s.distort[k].value = parse_value(ValueType::Float, dm.get("v"))?;
                        fill_prop(dm, &mut s.distort[k])?;
                    }
                }
            }
            l
        }
        "TEXT" => {
            let mut l = Layer::text(id, name);
            let LayerKind::Text(t) = &mut l.kind else { unreachable!() };
            t.text = str_(m, "text")?.map(Into::into);
            t.font = str_(m, "font")?.map(Into::into);
            t.size = f(m, "size", t.size)?;
            read_prop(m, "color", &mut t.color)?;
            t.corner = i(m, "corner", 0)?.clamp(0, 4);
            t.above_bars = b(m, "aboveBars", false)?;
            l
        }
        "SYMBOL" => {
            let mut l = Layer::symbol(id, name, req(m, "symbol")?);
            let LayerKind::Symbol(s) = &mut l.kind else { unreachable!() };
            read_prop(m, "timeOffset", &mut s.time_offset)?;
            read_prop(m, "speed", &mut s.speed)?;
            s.loop_mode = match str_(m, "loop")? {
                None => s.loop_mode,
                Some("ONCE") => SymbolLoop::Once,
                Some("LOOP") => SymbolLoop::Loop,
                Some("PING_PONG") => SymbolLoop::PingPong,
                Some(x) => return err(format!("loop {x}")),
            };
            s.recursion_limit = i(m, "recursionLimit", s.recursion_limit)?;
            l
        }
        "ADJUSTMENT" => Layer::adjustment(id, name),
        "AUDIO" => {
            let mut l = Layer::audio(id, name);
            let LayerKind::Audio(clips) = &mut l.kind else { unreachable!() };
            for o in list(m, "clips")? {
                let cm = as_map(o, "clip")?;
                let mut c = Clip::new(id, req(cm, "id")?, str_(cm, "asset")?.map(Into::into), d(cm, "start", 0.0)?, d(cm, "in", 0.0)?, d(cm, "out", 0.0)?);
                c.pitch = d(cm, "pitch", 1.0)?;
                c.loop_ = b(cm, "loop", false)?;
                c.fade_in = d(cm, "fadeIn", 0.0)?;
                c.fade_out = d(cm, "fadeOut", 0.0)?;
                read_prop(cm, "volume", &mut c.volume)?;
                read_prop(cm, "pan", &mut c.pan)?;
                clips.push(c);
            }
            l
        }
        "CAMERA" => {
            let mut l = Layer::camera(id, name);
            let LayerKind::Camera(c) = &mut l.kind else { unreachable!() };
            read_prop(m, "camX", &mut c.x)?;
            read_prop(m, "camY", &mut c.y)?;
            read_prop(m, "zoom", &mut c.zoom)?;
            read_prop(m, "camRotation", &mut c.rotation)?;
            read_prop(m, "background", &mut c.background)?;
            read_prop(m, "background2", &mut c.background2)?;
            l
        }
        other => return err(format!("unknown layer type {other}")),
    };
    l.visible = b(m, "visible", true)?;
    l.locked = b(m, "locked", false)?;
    l.solo = b(m, "solo", false)?;
    l.onion = b(m, "onion", true)?;
    l.blend = match str_(m, "blend")? {
        None => Blend::Normal,
        Some(s) => Blend::from_name(s).ok_or_else(|| FormatError(format!("blend {s}")))?,
    };
    l.parent = str_(m, "parent")?.map(Into::into);
    l.parent_node = str_(m, "parentNode")?.map(Into::into);
    l.parent_node_rotate = b(m, "parentNodeRotate", true)?;
    l.inherit_effects = b(m, "inheritEffects", false)?;
    l.parallax = f(m, "parallax", 0.0)?;
    read_prop(m, "shown", &mut l.shown)?;
    read_prop(m, "opacity", &mut l.opacity)?;
    if let Some(t) = m.get("transform") {
        let t = as_map(t, "transform")?;
        let tr = &mut l.transform;
        read_prop(t, "x", &mut tr.x)?;
        read_prop(t, "y", &mut tr.y)?;
        read_prop(t, "rotation", &mut tr.rotation)?;
        read_prop(t, "scaleX", &mut tr.scale_x)?;
        read_prop(t, "scaleY", &mut tr.scale_y)?;
        read_prop(t, "anchorX", &mut tr.anchor_x)?;
        read_prop(t, "anchorY", &mut tr.anchor_y)?;
    }
    if let Some(e) = m.get("effects") {
        let owner = l.id.clone();
        read_stack(as_map(e, "effects")?, &mut l.effects, &owner)?;
    }
    Ok(l)
}

fn symbol(m: &Obj) -> R<Symbol> {
    let mut s = Symbol::new(req(m, "id")?, str_(m, "name")?, d(m, "duration", 1.0)?);
    for o in list(m, "layers")? {
        s.layers.push(layer(as_map(o, "layer")?)?);
    }
    for o in list(m, "markers")? {
        let mm = as_map(o, "marker")?;
        let color = match str_(mm, "color")? {
            Some(c) => parse_color(c)?,
            None => Color::WHITE,
        };
        s.markers.push(Marker { time: d(mm, "time", 0.0)?, name: str_(mm, "name")?.map(Into::into), color });
    }
    let n = s.scoped_effects.len();
    for (k, o) in list(m, "scopedEffects")?.iter().enumerate() {
        let sm = as_map(o, "scopedEffect")?;
        let sc = scope(as_map(sm.get("scope").unwrap_or(&J::Null), "scope")?)?;
        let mut st = EffectStack::default();
        read_stack(as_map(sm.get("stack").unwrap_or(&J::Null), "stack")?, &mut st, &format!("{}/fx{}", s.id, n + k))?;
        s.scoped_effects.push(ScopedStack { scope: sc, stack: st });
    }
    Ok(s)
}

/// Parses project.json.
pub fn decode(json: &str) -> R<Project> {
    let root: J = serde_json::from_str(json).map_err(|e| FormatError(format!("invalid project: {e}")))?;
    let m = as_map(&root, "project")?;
    if m.get("format").and_then(J::as_str) != Some("barkfart") {
        return err("not a Barkfart project");
    }
    let v = parse_i(m.get("formatVersion"), "formatVersion")? as i64;
    if v > FORMAT_VERSION {
        return err(format!("project format {v} is newer than this app ({FORMAT_VERSION})"));
    }
    if v < 1 {
        return err(format!("no migration from version {v}"));
    }
    let mut p = Project::default();
    let def = Settings::default();
    if let Some(s) = m.get("settings") {
        let s = as_map(s, "settings")?;
        p.settings.fps = f(s, "fps", def.fps)?;
        p.settings.width = i(s, "width", def.width)?;
        p.settings.height = i(s, "height", def.height)?;
        if let Some(bg) = str_(s, "background")? {
            p.settings.background = parse_color(bg)?;
        }
        p.settings.letterbox = f(s, "letterbox", 0.0)?;
    }
    if let Some(vw) = m.get("view") {
        let vw = as_map(vw, "view")?;
        p.view.zoom = f(vw, "zoom", 1.0)?;
        p.view.pan_x = f(vw, "panX", 0.0)?;
        p.view.pan_y = f(vw, "panY", 0.0)?;
        p.view.frame = i(vw, "frame", 0)?;
        p.view.onion_before = i(vw, "onionBefore", 2)?.clamp(0, 10);
        p.view.onion_after = i(vw, "onionAfter", 1)?.clamp(0, 10);
        p.view.onion_in_front = b(vw, "onionInFront", false)?;
    }
    p.root_symbol = req(m, "rootSymbol")?.into();
    for o in list(m, "assets")? {
        let am = as_map(o, "asset")?;
        let kind = match req(am, "kind")? {
            "IMAGE" => AssetKind::Image,
            "AUDIO" => AssetKind::Audio,
            "FONT" => AssetKind::Font,
            x => return err(format!("asset kind {x}")),
        };
        let a = Asset { id: req(am, "id")?.into(), kind, path: req(am, "path")?.into() };
        p.assets.insert(a.id.clone(), a);
    }
    for o in list(m, "library")? {
        let s = symbol(as_map(o, "symbol")?)?;
        if p.library.contains_key(&s.id) {
            return err(format!("duplicate symbol {}", s.id));
        }
        p.library.insert(s.id.clone(), s);
    }
    for o in list(m, "nodeSets")? {
        let nm = as_map(o, "nodeSet")?;
        let mut ns = NodeSet { id: req(nm, "id")?.into(), name: str_(nm, "name")?.map(Into::into), members: Vec::new() };
        for pair in list(nm, "members")? {
            let a = pair.as_array().ok_or_else(|| FormatError("member".into()))?;
            let get = |k: usize| a.get(k).and_then(J::as_str).map(str::to_string).ok_or_else(|| FormatError("member".into()));
            ns.members.push((get(0)?, get(1)?));
        }
        p.node_sets.insert(ns.id.clone(), ns);
    }
    if !p.library.contains_key(&p.root_symbol) {
        return err("root symbol missing");
    }
    p.id_counter = i(m, "idCounter", 1)?.max(1);
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_file_loads_with_defaults() {
        let json = r#"{"format": "barkfart", "formatVersion": 1, "rootSymbol": "S1",
            "library": [{"id": "S1", "layers": [{"id": "L1", "type": "FIGURE",
            "nodes": [{"id": "r"}, {"id": "a", "parent": "r", "length": {"v": 20.0}}]}]}]}"#;
        let p = decode(json).unwrap();
        let l = &p.root().unwrap().layers[0];
        assert!(l.visible);
        assert_eq!(l.blend, Blend::Normal);
        assert_eq!(p.settings.fps, 30.0);
        assert_eq!(p.root().unwrap().duration, 1.0);
        assert_eq!(l.as_figure().unwrap().node("a").unwrap().shape, Shape::Line);
    }

    #[test]
    fn newer_versions_are_refused() {
        let json = r#"{"format": "barkfart", "formatVersion": 2, "rootSymbol": "S1", "library": [{"id": "S1"}]}"#;
        assert!(decode(json).unwrap_err().0.contains("newer"));
    }
}
