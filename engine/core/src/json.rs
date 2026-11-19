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
fn java_style(shortest_sci: String, _plain: String, v: f64) -> String {
    let (m, e) = shortest_sci.split_once('e').unwrap_or((&shortest_sci, "0"));
    let neg = m.starts_with('-');
    let digits: String = m.trim_start_matches('-').chars().filter(|c| *c != '.').collect();
    let exp: i32 = e.parse().unwrap_or(0);
    spell(neg, &digits, exp, v)
}

/// Writes sign, significant digits and decimal exponent (value = d.ddd x 10^exp) the Java way.
fn spell(neg: bool, digits: &str, exp: i32, v: f64) -> String {
    let a = v.abs();
    let sign = if neg { "-" } else { "" };
    if a != 0.0 && !(1e-3..1e7).contains(&a) {
        let (head, tail) = digits.split_at(1);
        let tail = if tail.is_empty() { "0" } else { tail };
        return format!("{sign}{head}.{tail}E{exp}");
    }
    if exp >= 0 {
        let e = exp as usize;
        let int: String = if digits.len() > e { digits[..e + 1].to_string() } else { format!("{digits}{}", "0".repeat(e + 1 - digits.len())) };
        let frac = if digits.len() > e + 1 { &digits[e + 1..] } else { "0" };
        format!("{sign}{int}.{frac}")
    } else {
        format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
    }
}

/// Java's Float.toString breaks exact ties between the two shortest
/// spellings toward the even digit ("-3.5507812" for -3.55078125), where
/// Rust's shortest form rounds up; both read back to the same float.
fn float_digits(v: f32) -> (bool, String, i32) {
    let sci = format!("{v:e}");
    let (m, e) = sci.split_once('e').unwrap_or((&sci, "0"));
    let neg = m.starts_with('-');
    let mut digits: String = m.trim_start_matches('-').chars().filter(|c| *c != '.').collect();
    let mut exp: i32 = e.parse().unwrap_or(0);
    let n = digits.len();
    // one more digit, exactly: is the value a tie at n digits?
    let longer = format!("{:.*e}", n, (v as f64).abs());
    let (lm, le) = longer.split_once('e').unwrap_or((&longer, "0"));
    let ldigits: String = lm.chars().filter(|c| *c != '.').collect();
    let exact = longer.parse::<f64>().map(|x| x == (v as f64).abs()).unwrap_or(false);
    if exact && ldigits.ends_with('5') && ldigits.len() == n + 1 {
        let le: i32 = le.parse().unwrap_or(0);
        let base = &ldigits[..n];
        let last = base.as_bytes()[n - 1] - b'0';
        let candidate = if last % 2 == 0 { base.to_string() } else { increment(base) };
        let (cd, ce) = if candidate.len() > n { (candidate[..n].to_string(), le + 1) } else { (candidate, le) };
        // keep it only if it still names the same float
        let text = format!("{}{}.{}e{}", if neg { "-" } else { "" }, &cd[..1], if cd.len() > 1 { &cd[1..] } else { "0" }, ce);
        if text.parse::<f32>().map(|x| x == v).unwrap_or(false) {
            digits = cd;
            exp = ce;
        }
    }
    (neg, digits, exp)
}

fn increment(d: &str) -> String {
    let mut b: Vec<u8> = d.bytes().collect();
    let mut i = b.len();
    while i > 0 {
        i -= 1;
        if b[i] == b'9' {
            b[i] = b'0';
        } else {
            b[i] += 1;
            return String::from_utf8(b).expect("digits");
        }
    }
    format!("1{}", String::from_utf8(b).expect("digits"))
}

/// Java's Float.toString (NaN, Infinity, -0.0 included).
pub fn java_float(v: f32) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == 0.0 && v.is_sign_negative() {
        return "-0.0".into();
    }
    let (neg, digits, exp) = float_digits(v);
    spell(neg, &digits, exp, v as f64)
}

pub fn f32(v: f32) -> Json {
    if !v.is_finite() {
        return Json::Num("0".into());
    }
    Json::Num(java_float(v))
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
        assert_eq!(t(f32(-3.550_781_25)), "-3.5507812", "ties go to the even digit");
        assert_eq!(t(f32(1558.906_25)), "1558.9062");
        assert_eq!(t(f32(100.0)), "100.0");
        assert_eq!(t(f32(123456.7)), "123456.7");
    }
}
