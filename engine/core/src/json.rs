//! A small JSON writer in the same layout as the Java version's (two-space
//! indent, short lists inline, Java number spelling: "1.0", "1.0E-4"), so a
//! saved project reads the same whichever version wrote it.

/// A JSON value with numbers kept as text.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    List(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

/// Java's spelling of a decimal: shortest digits, ".0" for whole numbers,
/// scientific ("1.5E-4") below 1e-3 and from 1e7.
fn java_style(shortest_sci: String, plain: String, v: f64) -> String {
    let a = v.abs();
    if a != 0.0 && !(1e-3..1e7).contains(&a) {
        // shortest_sci like "1.5e-4" or "1e7"
        let (m, e) = shortest_sci.split_once('e').unwrap_or((&shortest_sci, "0"));
        let m = if m.contains('.') { m.to_string() } else { format!("{m}.0") };
        return format!("{m}E{e}");
    }
    if plain.contains('.') || plain.contains("inf") || plain.contains("NaN") {
        plain
    } else {
        format!("{plain}.0")
    }
}

pub fn f32(v: f32) -> Json {
    if !v.is_finite() {
        return Json::Num("0".into());
    }
    let s = if v == 0.0 && v.is_sign_negative() { "-0.0".to_string() } else { java_style(format!("{v:e}"), format!("{v}"), v as f64) };
    Json::Num(s)
}

pub fn f64(v: f64) -> Json {
    if !v.is_finite() {
        return Json::Num("0".into());
    }
    let s = if v == 0.0 && v.is_sign_negative() { "-0.0".to_string() } else { java_style(format!("{v:e}"), format!("{v}"), v) };
    Json::Num(s)
}

pub fn int(v: i64) -> Json {
    Json::Num(v.to_string())
}

pub fn s(v: &str) -> Json {
    Json::Str(v.to_string())
}

pub fn opt(v: &Option<String>) -> Json {
    match v {
        Some(x) => Json::Str(x.clone()),
        None => Json::Null,
    }
}

/// Object builder keeping insertion order.
#[derive(Default)]
pub struct Obj(pub Vec<(String, Json)>);

impl Obj {
    pub fn new() -> Obj {
        Obj(Vec::new())
    }

    pub fn put(&mut self, k: &str, v: Json) -> &mut Obj {
        self.0.push((k.to_string(), v));
        self
    }

    pub fn done(self) -> Json {
        Json::Obj(self.0)
    }
}

pub fn write(v: &Json) -> String {
    let mut sb = String::new();
    write_into(v, &mut sb, 0);
    sb.push('\n');
    sb
}

fn nl(sb: &mut String, indent: usize) {
    sb.push('\n');
    for _ in 0..indent {
        sb.push_str("  ");
    }
}

fn is_deep(l: &[Json]) -> bool {
    l.iter().any(|o| matches!(o, Json::Obj(_) | Json::List(_)))
}

fn write_into(v: &Json, sb: &mut String, indent: usize) {
    match v {
        Json::Null => sb.push_str("null"),
        Json::Bool(b) => sb.push_str(if *b { "true" } else { "false" }),
        Json::Num(t) => sb.push_str(t),
        Json::Str(t) => string(t, sb),
        Json::Obj(m) => {
            if m.is_empty() {
                sb.push_str("{}");
                return;
            }
            sb.push('{');
            for (i, (k, val)) in m.iter().enumerate() {
                if i > 0 {
                    sb.push(',');
                }
                nl(sb, indent + 1);
                string(k, sb);
                sb.push_str(": ");
                write_into(val, sb, indent + 1);
            }
            nl(sb, indent);
            sb.push('}');
        }
        Json::List(l) => {
            let flat = l.iter().all(|o| !matches!(o, Json::Obj(_)) && !matches!(o, Json::List(x) if !x.is_empty() && is_deep(x)));
            sb.push('[');
            for (i, o) in l.iter().enumerate() {
                if i > 0 {
                    sb.push_str(if flat { ", " } else { "," });
                }
                if !flat {
                    nl(sb, indent + 1);
                }
                write_into(o, sb, indent + 1);
            }
            if !flat && !l.is_empty() {
                nl(sb, indent);
            }
            sb.push(']');
        }
    }
}

fn string(s: &str, sb: &mut String) {
    sb.push('"');
    // Java walks UTF-16 code units; control characters are escaped the same way
    for c in s.chars() {
        match c {
            '"' => sb.push_str("\\\""),
            '\\' => sb.push_str("\\\\"),
            '\n' => sb.push_str("\\n"),
            '\r' => sb.push_str("\\r"),
            '\t' => sb.push_str("\\t"),
            c if (c as u32) < 0x20 => sb.push_str(&format!("\\u{:04x}", c as u32)),
            c => sb.push(c),
        }
    }
    sb.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_spelled_like_java() {
        let t = |j: Json| match j {
            Json::Num(s) => s,
            _ => unreachable!(),
        };
        assert_eq!(t(f32(1.0)), "1.0");
        assert_eq!(t(f32(0.5)), "0.5");
        assert_eq!(t(f32(-100.0)), "-100.0");
        assert_eq!(t(f32(1.0e-4)), "1.0E-4");
        assert_eq!(t(f32(1.5e-4)), "1.5E-4");
        assert_eq!(t(f32(1.0e7)), "1.0E7");
        assert_eq!(t(f32(0.001)), "0.001");
        assert_eq!(t(f64(0.37)), "0.37");
        assert_eq!(t(f64(2.0)), "2.0");
        assert_eq!(t(f32(f32::NAN)), "0");
        assert_eq!(t(int(24)), "24");
    }
}
