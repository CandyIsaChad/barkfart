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
    n.own_color = b(m, "ownColor", false)?;
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
            fig.color = str_(m, "figureColor")?.and_then(|h| u32::from_str_radix(h.trim_start_matches('#'), 16).ok()).map(Color::rgba8888);
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
            t.align = i(m, "align", -1)?.clamp(-1, 2);
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
            read_prop(m, "frameDelay", &mut c.delay)?;
            read_prop(m, "repeatBack", &mut c.repeat_back)?;
            read_prop(m, "repeatTimes", &mut c.repeat_times)?;
            read_prop(m, "silence", &mut c.silence)?;
            read_prop(m, "slowMotion", &mut c.slow_motion)?;
            read_prop(m, "bgExpand", &mut c.bg_expand)?;
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
        s.layers.push(std::sync::Arc::new(layer(as_map(o, "layer")?)?));
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
        p.settings.tweening = b(s, "tweening", false)?;
        p.settings.tweened_frames = i(s, "tweenedFrames", DEFAULT_TWEENED_FRAMES)?.clamp(0, 8);
        p.settings.loop_tween = b(s, "loopTween", false)?;
        if let Some(t) = str_(s, "watermark")? {
            let c = str_(s, "watermarkColor")?.and_then(|h| u32::from_str_radix(h.trim_start_matches('#'), 16).ok()).map(Color::rgba8888).unwrap_or(Color::rgba8888(0xff3b30ff));
            p.settings.watermark = Some((t.to_string(), c));
        }
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
        let a = Asset { id: req(am, "id")?.into(), kind, path: req(am, "path")?.into(), name: str_(am, "name")?.map(Into::into) };
        p.assets.insert(a.id.clone(), a);
    }
    for o in list(m, "library")? {
        let s = symbol(as_map(o, "symbol")?)?;
        if p.library.contains_key(&s.id) {
            return err(format!("duplicate symbol {}", s.id));
        }
        p.library.insert(s.id.clone(), std::sync::Arc::new(s));
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

/// A .bark file: a ZIP with project.json and assets/<id>.<ext> (FORMAT.md).
pub struct BarkFile {
    pub project: Project,
    /// Asset bytes by archive path ("assets/<id>.<ext>").
    pub assets: indexmap::IndexMap<String, Vec<u8>>,
}

/// Reads a .bark file from bytes.
pub fn read_bark(bytes: &[u8]) -> Result<BarkFile, FormatError> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| FormatError(format!("not a .bark file: {e}")))?;
    let mut json = None;
    let mut assets = indexmap::IndexMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| FormatError(e.to_string()))?;
        let name = f.name().map_err(|e| FormatError(e.to_string()))?.to_string();
        if f.size() > 512 * 1024 * 1024 {
            return err(format!("{name} is too large"));
        }
        let mut data = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut data).map_err(|e| FormatError(e.to_string()))?;
        if name == "project.json" {
            json = Some(String::from_utf8(data).map_err(|_| FormatError("project.json is not UTF-8".into()))?);
        } else if name.starts_with("assets/") {
            assets.insert(name, data);
        }
    }
    let project = decode(&json.ok_or_else(|| FormatError("no project.json".into()))?)?;
    Ok(BarkFile { project, assets })
}

// ================================================================ encode

use crate::json::{self, Json, Obj as JObj};

fn color_hex(c: &Color) -> Json {
    Json::Str(format!("#{:08x}", c.to_rgba8888()))
}

fn value_json(v: &Value) -> Json {
    match v {
        Value::F(x) => json::f32(*x),
        Value::D(x) => json::f64(*x),
        Value::C(c) => color_hex(c),
        Value::B(b) => Json::Bool(*b),
    }
}

fn prop_json(p: &Property, with_type: bool) -> Json {
    let mut m = JObj::new();
    if with_type {
        m.put("t", json::s(p.ty.name()));
    }
    m.put("v", value_json(&p.value));
    if !p.track.is_empty() {
        let keys = p
            .track
            .keys()
            .iter()
            .map(|k| {
                let mut km = vec![json::f64(k.time), value_json(&k.value), json::s(match k.interp {
                    Interp::Hold => "H",
                    Interp::Linear => "L",
                    Interp::Bezier => "B",
                })];
                if k.interp == Interp::Bezier {
                    km.extend([json::f32(k.ease.x1), json::f32(k.ease.y1), json::f32(k.ease.x2), json::f32(k.ease.y2)]);
                    if k.ease.kind != EaseKind::Cubic {
                        km.push(json::s(k.ease.kind.name()));
                    }
                }
                Json::List(km)
            })
            .collect();
        m.put("k", Json::List(keys));
    }
    if !p.modifiers.is_empty() {
        m.put("m", Json::List(p.modifiers.iter().map(modifier_json).collect()));
    }
    m.done()
}

fn modifier_json(md: &Modifier) -> Json {
    let mut m = JObj::new();
    match &md.kind {
        ModifierKind::Wiggle { frequency, amplitude, seed, octaves } => {
            m.put("type", json::s("wiggle"))
                .put("frequency", json::f32(*frequency))
                .put("amplitude", json::f32(*amplitude))
                .put("seed", json::int(*seed as i64))
                .put("octaves", json::int(*octaves as i64));
        }
        ModifierKind::Sine { frequency, amplitude, phase } => {
            m.put("type", json::s("sine")).put("frequency", json::f32(*frequency)).put("amplitude", json::f32(*amplitude)).put("phase", json::f32(*phase));
        }
        ModifierKind::Loop { mode, start, end } => {
            m.put("type", json::s("loop"))
                .put("mode", json::s(if *mode == LoopMode::Cycle { "CYCLE" } else { "PING_PONG" }))
                .put("start", json::f64(*start))
                .put("end", json::f64(*end));
        }
        ModifierKind::DelayFollow { source, delay, scale, offset } => {
            m.put("type", json::s("delayFollow"))
                .put("source", json::s(source))
                .put("delay", json::f64(*delay))
                .put("scale", json::f32(*scale))
                .put("offset", json::f32(*offset));
        }
        ModifierKind::Link { source, scale, offset } => {
            m.put("type", json::s("link")).put("source", json::s(source)).put("scale", json::f32(*scale)).put("offset", json::f32(*offset));
        }
    }
    m.put("enabled", Json::Bool(md.enabled));
    m.done()
}

fn scope_json(s: &Scope) -> Json {
    let kind = match s.kind {
        ScopeKind::Node => "NODE",
        ScopeKind::NodeSet => "NODE_SET",
        ScopeKind::Figure => "FIGURE",
        ScopeKind::Layer => "LAYER",
        ScopeKind::SymbolInstance => "SYMBOL_INSTANCE",
        ScopeKind::Composition => "COMPOSITION",
    };
    let mut m = JObj::new();
    m.put("kind", json::s(kind)).put("target", json::opt(&s.target)).put("node", json::opt(&s.node));
    m.done()
}

fn stack_json(st: &EffectStack) -> Json {
    let nodes = st
        .nodes
        .values()
        .map(|e| {
            let mut params = JObj::new();
            for (k, p) in &e.params {
                params.put(k, prop_json(p, true));
            }
            let mut em = JObj::new();
            em.put("id", json::s(&e.id))
                .put("type", json::s(&e.ty))
                .put("enabled", Json::Bool(e.enabled))
                .put("mix", prop_json(&e.mix, false))
                .put("params", params.done())
                .put("inputs", Json::List(e.inputs.iter().map(|x| json::s(x)).collect()))
                .put("mask", e.mask.as_ref().map(scope_json).unwrap_or(Json::Null));
            em.done()
        })
        .collect();
    let mut m = JObj::new();
    m.put("output", json::opt(&st.output)).put("nodes", Json::List(nodes));
    m.done()
}

fn node_json(n: &Node) -> Json {
    let mut m = JObj::new();
    m.put("id", json::s(&n.id)).put("parent", json::opt(&n.parent)).put("shape", json::s(n.shape.name())).put("draw", json::int(n.draw_order as i64));
    if n.curve != 0.0 {
        m.put("curve", json::f32(n.curve));
    }
    if n.triangle_upside_down {
        m.put("upsideDown", Json::Bool(true));
    }
    if n.right_triangle != 0 {
        m.put("rightTriangle", json::int(n.right_triangle as i64));
    }
    if n.is_static {
        m.put("static", Json::Bool(true));
    }
    if n.own_color {
        m.put("ownColor", Json::Bool(true));
    }
    if n.stretchy {
        m.put("stretchy", Json::Bool(true));
    }
    if n.smart_stretch {
        m.put("smartStretch", Json::Bool(true));
    }
    if n.no_smart_stretch {
        m.put("noSmartStretch", Json::Bool(true));
    }
    if n.rest_length != 0.0 {
        m.put("restLength", json::f32(n.rest_length));
    }
    if n.circle_hollow {
        m.put("hollow", Json::Bool(true));
    }
    if n.circle_outline {
        m.put("outline", Json::Bool(true));
    }
    if !n.outline_color.track.is_empty() || n.outline_color.value != Value::C(Color::BLACK) {
        m.put("outlineColor", prop_json(&n.outline_color, false));
    }
    if n.half_arc != 0 {
        m.put("halfArc", json::int(n.half_arc as i64));
    }
    if n.polygon_sides != 0 {
        m.put("sides", json::int(n.polygon_sides as i64));
    }
    if n.trapezoid_ratio != 1.0 {
        m.put("trapezoidRatio", json::f32(n.trapezoid_ratio));
    }
    if n.trapezoid_round_start {
        m.put("roundStart", Json::Bool(true));
    }
    if n.trapezoid_round_end {
        m.put("roundEnd", Json::Bool(true));
    }
    if n.angle_lock != 0 {
        m.put("angleLock", json::int(n.angle_lock as i64));
    }
    if n.drag_lock != 0 {
        m.put("dragLock", json::int(n.drag_lock as i64));
    }
    if n.pinned {
        m.put("pinned", Json::Bool(true));
    }
    let grad_colour_set = !n.gradient_color.track.is_empty() || n.gradient_color.value != Value::C(Color::WHITE);
    if n.gradient || n.gradient_reverse || n.gradient_sideways || n.gradient_linear || grad_colour_set {
        let mut gm = JObj::new();
        gm.put("on", Json::Bool(n.gradient)).put("reverse", Json::Bool(n.gradient_reverse)).put("sideways", Json::Bool(n.gradient_sideways));
        if n.gradient_linear {
            gm.put("linear", Json::Bool(true));
        }
        gm.put("color", prop_json(&n.gradient_color, false));
        m.put("gradient", gm.done());
    }
    m.put("rotation", prop_json(&n.rotation, false))
        .put("length", prop_json(&n.length, false))
        .put("thickness", prop_json(&n.thickness, false))
        .put("scale", prop_json(&n.scale, false))
        .put("color", prop_json(&n.color, false))
        .put("shown", prop_json(&n.shown, false));
    if !n.legacy_extras.is_empty() {
        let mut ex = JObj::new();
        for (k, v) in &n.legacy_extras {
            ex.put(k, json::s(v));
        }
        m.put("legacy", ex.done());
    }
    m.done()
}

fn layer_json(l: &Layer) -> Json {
    let mut m = JObj::new();
    m.put("id", json::s(&l.id))
        .put("name", json::opt(&l.name))
        .put("type", json::s(l.kind.type_name()))
        .put("visible", Json::Bool(l.visible))
        .put("locked", Json::Bool(l.locked))
        .put("solo", Json::Bool(l.solo))
        .put("onion", Json::Bool(l.onion))
        .put("blend", json::s(l.blend.name()))
        .put("parent", json::opt(&l.parent));
    if let Some(pn) = &l.parent_node {
        m.put("parentNode", json::s(pn));
    }
    if !l.parent_node_rotate {
        m.put("parentNodeRotate", Json::Bool(false));
    }
    if l.inherit_effects {
        m.put("inheritEffects", Json::Bool(true));
    }
    if l.parallax != 0.0 {
        m.put("parallax", json::f32(l.parallax));
    }
    m.put("shown", prop_json(&l.shown, false)).put("opacity", prop_json(&l.opacity, false));
    let tr = &l.transform;
    let mut t = JObj::new();
    t.put("x", prop_json(&tr.x, false))
        .put("y", prop_json(&tr.y, false))
        .put("rotation", prop_json(&tr.rotation, false))
        .put("scaleX", prop_json(&tr.scale_x, false))
        .put("scaleY", prop_json(&tr.scale_y, false))
        .put("anchorX", prop_json(&tr.anchor_x, false))
        .put("anchorY", prop_json(&tr.anchor_y, false));
    m.put("transform", t.done()).put("effects", stack_json(&l.effects));
    match &l.kind {
        LayerKind::Figure(f) => {
            m.put("nodes", Json::List(f.nodes().iter().map(node_json).collect()));
            let fills = f
                .polyfills
                .iter()
                .map(|pf| {
                    let mut pm = JObj::new();
                    pm.put("id", json::s(&pf.id))
                        .put("anchor", json::s(&pf.anchor))
                        .put("nodes", Json::List(pf.nodes.iter().map(|x| json::s(x)).collect()))
                        .put("ownColor", Json::Bool(pf.use_own_color))
                        .put("color", prop_json(&pf.color, false));
                    pm.done()
                })
                .collect();
            m.put("polyfills", Json::List(fills));
            if let Some(c) = f.color {
                m.put("figureColor", json::s(&format!("#{:08x}", c.to_rgba8888())));
            }
        }
        LayerKind::Sprite(s) => {
            m.put("asset", json::opt(&s.asset)).put("width", json::f32(s.width)).put("height", json::f32(s.height)).put("tint", prop_json(&s.tint, false));
            if s.distorted() {
                m.put("distort", Json::List(s.distort.iter().map(|q| prop_json(q, false)).collect()));
            }
        }
        LayerKind::Text(t) => {
            m.put("text", json::opt(&t.text)).put("font", json::opt(&t.font)).put("size", json::f32(t.size)).put("color", prop_json(&t.color, false));
            if t.corner != 0 {
                m.put("corner", json::int(t.corner as i64));
            }
            if t.align >= 0 {
                m.put("align", json::int(t.align as i64));
            }
            if t.above_bars {
                m.put("aboveBars", Json::Bool(true));
            }
        }
        LayerKind::Symbol(s) => {
            m.put("symbol", json::s(&s.symbol))
                .put("timeOffset", prop_json(&s.time_offset, false))
                .put("speed", prop_json(&s.speed, false))
                .put(
                    "loop",
                    json::s(match s.loop_mode {
                        SymbolLoop::Once => "ONCE",
                        SymbolLoop::Loop => "LOOP",
                        SymbolLoop::PingPong => "PING_PONG",
                    }),
                )
                .put("recursionLimit", json::int(s.recursion_limit as i64));
        }
        LayerKind::Audio(clips) => {
            let cl = clips
                .iter()
                .map(|c| {
                    let mut cm = JObj::new();
                    cm.put("id", json::s(&c.id))
                        .put("asset", json::opt(&c.asset))
                        .put("start", json::f64(c.start))
                        .put("in", json::f64(c.in_))
                        .put("out", json::f64(c.out))
                        .put("loop", Json::Bool(c.loop_))
                        .put("fadeIn", json::f64(c.fade_in))
                        .put("fadeOut", json::f64(c.fade_out));
                    if c.pitch != 1.0 {
                        cm.put("pitch", json::f64(c.pitch));
                    }
                    cm.put("volume", prop_json(&c.volume, false)).put("pan", prop_json(&c.pan, false));
                    cm.done()
                })
                .collect();
            m.put("clips", Json::List(cl));
        }
        LayerKind::Camera(c) => {
            m.put("camX", prop_json(&c.x, false)).put("camY", prop_json(&c.y, false)).put("zoom", prop_json(&c.zoom, false)).put("camRotation", prop_json(&c.rotation, false));
            if !c.background.track.is_empty() || c.background.value.color().a > 0.0 {
                m.put("background", prop_json(&c.background, false));
            }
            if !c.background2.track.is_empty() || c.background2.value.color().a > 0.0 {
                m.put("background2", prop_json(&c.background2, false));
            }
            for (k, q) in [("frameDelay", &c.delay), ("repeatBack", &c.repeat_back), ("repeatTimes", &c.repeat_times), ("silence", &c.silence), ("slowMotion", &c.slow_motion), ("bgExpand", &c.bg_expand)] {
                if !q.track.is_empty() || q.value != Value::F(0.0) {
                    m.put(k, prop_json(q, false));
                }
            }
        }
        LayerKind::Adjustment => {}
    }
    m.done()
}

fn symbol_json(s: &Symbol) -> Json {
    let mut m = JObj::new();
    m.put("id", json::s(&s.id)).put("name", json::opt(&s.name)).put("duration", json::f64(s.duration));
    m.put("layers", Json::List(s.layers.iter().map(|l| layer_json(l)).collect()));
    let markers = s
        .markers
        .iter()
        .map(|mk| {
            let mut mm = JObj::new();
            mm.put("time", json::f64(mk.time)).put("name", json::opt(&mk.name)).put("color", color_hex(&mk.color));
            mm.done()
        })
        .collect();
    m.put("markers", Json::List(markers));
    let scoped = s
        .scoped_effects
        .iter()
        .map(|ss| {
            let mut sm = JObj::new();
            sm.put("scope", scope_json(&ss.scope)).put("stack", stack_json(&ss.stack));
            sm.done()
        })
        .collect();
    m.put("scopedEffects", Json::List(scoped));
    m.done()
}

/// project.json for a project (the same text the Java version writes).
pub fn encode(p: &Project) -> String {
    let mut m = JObj::new();
    m.put("format", json::s("barkfart")).put("formatVersion", json::int(FORMAT_VERSION)).put("idCounter", json::int(p.id_counter as i64));
    let st = &p.settings;
    let mut s = JObj::new();
    s.put("fps", json::f32(st.fps)).put("width", json::int(st.width as i64)).put("height", json::int(st.height as i64)).put("background", color_hex(&st.background));
    if st.letterbox != 0.0 {
        s.put("letterbox", json::f32(st.letterbox));
    }
    // written only when not the default, so older project texts stay the same
    if st.tweening {
        s.put("tweening", Json::Bool(true));
    }
    if st.tweened_frames != DEFAULT_TWEENED_FRAMES {
        s.put("tweenedFrames", json::int(st.tweened_frames as i64));
    }
    if st.loop_tween {
        s.put("loopTween", Json::Bool(true));
    }
    if let Some((t, c)) = &st.watermark {
        s.put("watermark", json::s(t)).put("watermarkColor", json::s(&format!("#{:08x}", c.to_rgba8888())));
    }
    m.put("settings", s.done());
    let v = &p.view;
    let is_default = v.zoom == 1.0 && v.pan_x == 0.0 && v.pan_y == 0.0 && v.frame == 0 && v.onion_before == 2 && v.onion_after == 1 && !v.onion_in_front;
    if !is_default {
        let mut vm = JObj::new();
        vm.put("zoom", json::f32(v.zoom))
            .put("panX", json::f32(v.pan_x))
            .put("panY", json::f32(v.pan_y))
            .put("frame", json::int(v.frame as i64))
            .put("onionBefore", json::int(v.onion_before as i64))
            .put("onionAfter", json::int(v.onion_after as i64));
        if v.onion_in_front {
            vm.put("onionInFront", Json::Bool(true));
        }
        m.put("view", vm.done());
    }
    m.put("rootSymbol", json::s(&p.root_symbol));
    m.put("library", Json::List(p.library.values().map(|s| symbol_json(s)).collect()));
    let assets = p
        .assets
        .values()
        .map(|a| {
            let mut am = JObj::new();
            am.put("id", json::s(&a.id))
                .put(
                    "kind",
                    json::s(match a.kind {
                        AssetKind::Image => "IMAGE",
                        AssetKind::Audio => "AUDIO",
                        AssetKind::Font => "FONT",
                    }),
                )
                .put("path", json::s(&a.path));
            if let Some(n) = &a.name {
                am.put("name", json::s(n));
            }
            am.done()
        })
        .collect();
    m.put("assets", Json::List(assets));
    let sets = p
        .node_sets
        .values()
        .map(|ns| {
            let mut nm = JObj::new();
            nm.put("id", json::s(&ns.id))
                .put("name", json::opt(&ns.name))
                .put("members", Json::List(ns.members.iter().map(|(a, b)| Json::List(vec![json::s(a), json::s(b)])).collect()));
            nm.done()
        })
        .collect();
    m.put("nodeSets", Json::List(sets));
    json::write(&m.done())
}

/// A .bark file: project.json plus the asset entries (stored, they are
/// already compressed), in the Java writer's order.
pub fn write_bark(p: &Project, assets: &indexmap::IndexMap<String, Vec<u8>>) -> Result<Vec<u8>, FormatError> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("project.json", opts).map_err(|e| FormatError(e.to_string()))?;
    zip.write_all(encode(p).as_bytes()).map_err(|e| FormatError(e.to_string()))?;
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for a in p.assets.values() {
        if let Some(b) = assets.get(&a.path) {
            zip.start_file(a.path.as_str(), stored).map_err(|e| FormatError(e.to_string()))?;
            zip.write_all(b).map_err(|e| FormatError(e.to_string()))?;
        }
    }
    Ok(zip.finish().map_err(|e| FormatError(e.to_string()))?.into_inner())
}
