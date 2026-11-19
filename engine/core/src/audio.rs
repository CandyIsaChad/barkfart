//! Audio (port of Java app.barkfart.audio): decoded sounds, WAV read/write,
//! the frozen per-clip plan, the mixer and the offline bounce. Pure; the
//! mixer is safe to call from an audio thread.

use crate::eval::{scene::DiagnosticKind, Diagnostic, Resolver};
use crate::model::{LayerKind, Project, Symbol};
use std::sync::Arc;

/// Decoded audio: 16-bit samples, interleaved when stereo.
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub rate: i32,
    pub channels: i32,
    pub frames: i32,
    pub data: Vec<i16>,
}

/// Longest sound kept in memory (seconds).
pub const MAX_SECONDS: i32 = 600;

impl Pcm {
    pub fn new(rate: i32, channels: i32, data: Vec<i16>) -> Option<Pcm> {
        if rate <= 0 || !(channels == 1 || channels == 2) {
            return None;
        }
        Some(Pcm { rate, channels, frames: data.len() as i32 / channels, data })
    }

    pub fn seconds(&self) -> f64 {
        self.frames as f64 / self.rate as f64
    }

    /// Float sample (-1..1) to 16 bits, clamped (NaN = 0).
    pub fn s16(v: f32) -> i16 {
        if v.is_nan() {
            return 0;
        }
        crate::model::java_round(v.clamp(-1.0, 1.0) * 32767.0) as i16
    }
}

/// A sine tone (test tone), mono 16-bit, with 10 ms ramps.
pub fn tone(hz: f64, seconds: f64, rate: i32, level: f32) -> Pcm {
    let n = (seconds * rate as f64).round() as i32;
    let d = (0..n)
        .map(|i| {
            let env = 1f64.min((i.min(n - i)) as f64 / (0.01 * rate as f64));
            Pcm::s16((level as f64 * env * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin()) as f32)
        })
        .collect();
    Pcm::new(rate, 1, d).expect("tone")
}

// ---------------------------------------------------------------- WAV

pub fn looks_like_wav(b: &[u8]) -> bool {
    b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WAVE"
}

fn u16le(b: &[u8], o: usize) -> Option<u32> {
    Some(*b.get(o)? as u32 | (*b.get(o + 1)? as u32) << 8)
}

fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u16le(b, o)? | u16le(b, o + 2)? << 16)
}

/// RIFF/WAVE: integer PCM (8/16/24/32 bit) and 32-bit float, any channel
/// count (more than two downmixed to the first two). None for anything else.
pub fn decode_wav(b: &[u8]) -> Option<Pcm> {
    if !looks_like_wav(b) {
        return None;
    }
    let (mut format, mut channels, mut rate, mut bits) = (0u32, 0u32, 0u32, 0u32);
    let mut data: Option<(usize, usize)> = None;
    let mut p = 12usize;
    while p + 8 <= b.len() {
        let id = &b[p..p + 4];
        let len = u32le(b, p + 4)? as u64;
        let body = p + 8;
        if id == b"fmt " && body + 16 <= b.len() {
            format = u16le(b, body)?;
            channels = u16le(b, body + 2)?;
            rate = u32le(b, body + 4)?;
            bits = u16le(b, body + 14)?;
            if format == 0xFFFE && len >= 26 && body + 26 <= b.len() {
                format = u16le(b, body + 24)?;
            }
        } else if id == b"data" {
            data = Some((body, (len as usize).min(b.len() - body)));
            break;
        }
        p = (b.len() as u64).min(body as u64 + len + (len & 1)) as usize;
    }
    let (at, len) = data?;
    if channels == 0 || channels > 32 || rate == 0 || rate > 384_000 {
        return None;
    }
    let is_float = format == 3 && bits == 32;
    if !(format == 1 && matches!(bits, 8 | 16 | 24 | 32)) && !is_float {
        return None;
    }
    let bytes = (bits / 8) as usize;
    let stride = bytes * channels as usize;
    let frames = (len / stride).min(rate as usize * MAX_SECONDS as usize);
    let out = if channels >= 2 { 2 } else { 1 };
    let mut d = Vec::with_capacity(frames * out);
    for i in 0..frames {
        for c in 0..out {
            let o = at + i * stride + c * bytes;
            let v = if bits == 16 && !is_float {
                u16le(b, o)? as u16 as i16
            } else {
                let s = if is_float {
                    f32::from_bits(u32le(b, o)?)
                } else {
                    match bits {
                        8 => (b[o] as i32 - 128) as f32 / 128.0,
                        24 => ((b[o] as i32) | (b[o + 1] as i32) << 8 | ((b[o + 2] as i8 as i32) << 16)) as f32 / 8_388_608.0,
                        _ => (u32le(b, o)? as i32) as f32 / 2_147_483_648.0,
                    }
                };
                Pcm::s16(s)
            };
            d.push(v);
        }
    }
    Pcm::new(rate as i32, out as i32, d)
}

/// 16-bit PCM as a WAV file.
pub fn encode_wav(pcm: &Pcm) -> Vec<u8> {
    let data_len = pcm.data.len() as u32 * 2;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&(pcm.channels as u16).to_le_bytes());
    b.extend_from_slice(&(pcm.rate as u32).to_le_bytes());
    b.extend_from_slice(&((pcm.rate * pcm.channels * 2) as u32).to_le_bytes());
    b.extend_from_slice(&((pcm.channels * 2) as u16).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for v in &pcm.data {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

// ---------------------------------------------------------------- plan

/// Volume and pan are sampled this many times per second.
pub const CURVE_RATE: i32 = 100;

#[derive(Clone, Debug)]
pub struct Entry {
    pub pcm: Arc<Pcm>,
    /// Timeline seconds the clip starts and stops.
    pub start: f64,
    pub end: f64,
    /// Source seconds played: [in, out).
    pub in_: f64,
    pub out: f64,
    pub loop_: bool,
    pub fade_in: f64,
    pub fade_out: f64,
    /// Source seconds per timeline second.
    pub pitch: f64,
    /// Samples at start + k / CURVE_RATE.
    pub volume: Vec<f32>,
    pub pan: Vec<f32>,
}

/// Everything the audio thread needs, frozen.
#[derive(Clone, Debug, Default)]
pub struct AudioPlan {
    pub entries: Vec<Entry>,
    /// Timeline length (looping clips stop here).
    pub end: f64,
    pub diagnostics: Vec<Diagnostic>,
}

fn finite(v: f64) -> bool {
    v.is_finite()
}

impl AudioPlan {
    /// Builds the plan for a symbol's audio layers; `sounds` decodes an asset id.
    pub fn build(p: &Project, s: &Symbol, sounds: &dyn Fn(&str) -> Option<Arc<Pcm>>) -> AudioPlan {
        let mut r = Resolver::new(p);
        let mut out = Vec::new();
        let end = if s.duration > 0.0 && s.duration < 1e6 { s.duration } else { 0.0 };
        let any_solo = s.layers.iter().any(|l| l.solo);
        for l in &s.layers {
            let LayerKind::Audio(clips) = &l.kind else { continue };
            if !l.visible || (any_solo && !l.solo) {
                continue;
            }
            for c in clips {
                let pcm = c.asset.as_deref().and_then(sounds);
                let Some(pcm) = pcm.filter(|p| p.frames > 0) else {
                    r.diag(DiagnosticKind::MissingRef, format!("{}/{} (sound)", l.id, c.id));
                    continue;
                };
                let src = pcm.seconds();
                let in_ = if finite(c.in_) { c.in_.min(src).max(0.0) } else { 0.0 };
                let o = if finite(c.out) && c.out > 0.0 { c.out.min(src) } else { src };
                if !(o > in_ + 1e-6) || !finite(c.start) || c.start.abs() > 1e6 {
                    continue;
                }
                let start = c.start;
                let pitch = if finite(c.pitch) { c.pitch.min(4.0).max(0.25) } else { 1.0 };
                let stop = if c.loop_ { start.max(end) } else { start + (o - in_) / pitch };
                if !(stop > start) {
                    continue;
                }
                let n = ((CURVE_RATE as i64 * 3600).min(((stop - start) * CURVE_RATE as f64).ceil() as i64 + 1)) as usize;
                let n = n.max(1);
                let mut vol = vec![0f32; n];
                let mut pan = vec![0f32; n];
                for k in 0..n {
                    let t = start + k as f64 / CURVE_RATE as f64;
                    let v = r.f(&c.volume, t) * r.f(&l.opacity, t);
                    let pn = r.f(&c.pan, t);
                    vol[k] = if v.is_nan() { 0.0 } else { v.min(4.0).max(0.0) };
                    pan[k] = if pn.is_nan() { 0.0 } else { pn.min(1.0).max(-1.0) };
                }
                out.push(Entry {
                    pcm,
                    start,
                    end: stop,
                    in_,
                    out: o,
                    loop_: c.loop_,
                    fade_in: if finite(c.fade_in) { c.fade_in.max(0.0) } else { 0.0 },
                    fade_out: if finite(c.fade_out) { c.fade_out.max(0.0) } else { 0.0 },
                    pitch,
                    volume: vol,
                    pan,
                });
            }
        }
        AudioPlan { entries: out, end, diagnostics: r.diagnostics }
    }
}

// ---------------------------------------------------------------- mixer

fn curve(c: &[f32], local: f64) -> f32 {
    let x = local * CURVE_RATE as f64;
    if !(x > 0.0) {
        return c[0];
    }
    let k = x as usize;
    if k >= c.len() - 1 {
        return c[c.len() - 1];
    }
    let f = (x - k as f64) as f32;
    c[k] + (c[k + 1] - c[k]) * f
}

fn gain(e: &Entry, t: f64, local: f64) -> f32 {
    let mut g = curve(&e.volume, local);
    if e.fade_in > 0.0 && local < e.fade_in {
        g *= (local / e.fade_in) as f32;
    }
    let left = e.end - t;
    if e.fade_out > 0.0 && left < e.fade_out {
        g *= (0f64).max(left / e.fade_out) as f32;
    }
    g
}

/// Fills out[0 .. 2 frames) (interleaved L, R) with the plan at timeline
/// time t0 and the given output rate. Overwrites out.
pub fn mix(plan: &AudioPlan, t0: f64, frames: usize, rate: i32, out: &mut [f32]) {
    out[..frames * 2].fill(0.0);
    if rate <= 0 || t0.is_nan() || t0.abs() > 1e7 {
        return;
    }
    let dt = 1.0 / rate as f64;
    let t1 = t0 + frames as f64 * dt;
    for e in &plan.entries {
        if e.end <= t0 || e.start >= t1 {
            continue;
        }
        let p = &*e.pcm;
        let len = e.out - e.in_;
        let i0 = 0f64.max(((e.start - t0) * rate as f64 - 1e-9).ceil()) as usize;
        let i1 = (frames as f64).min(((e.end - t0) * rate as f64 - 1e-9).ceil()).max(0.0) as usize;
        for i in i0..i1 {
            let t = t0 + i as f64 * dt;
            let local = t - e.start;
            let played = local * e.pitch;
            let src_t = if e.loop_ { e.in_ + (played % len) } else { e.in_ + played };
            if src_t >= e.out || src_t < e.in_ {
                continue;
            }
            let pos = src_t * p.rate as f64;
            let k = pos as i32;
            let f = (pos - k as f64) as f32;
            if k >= p.frames {
                continue;
            }
            let k2 = (p.frames - 1).min(k + 1);
            let g = gain(e, t, local);
            if g <= 0.0 {
                continue;
            }
            let pan = curve(&e.pan, local);
            let (k, k2) = (k as usize, k2 as usize);
            if p.channels == 1 {
                let s = (p.data[k] as f32 + (p.data[k2] as f32 - p.data[k] as f32) * f) / 32768.0 * g;
                let a = (pan as f64 + 1.0) * std::f64::consts::PI / 4.0;
                out[i * 2] += s * a.cos() as f32 * 1.414_213_56;
                out[i * 2 + 1] += s * a.sin() as f32 * 1.414_213_56;
            } else {
                let l = (p.data[k * 2] as f32 + (p.data[k2 * 2] as f32 - p.data[k * 2] as f32) * f) / 32768.0 * g;
                let r = (p.data[k * 2 + 1] as f32 + (p.data[k2 * 2 + 1] as f32 - p.data[k * 2 + 1] as f32) * f) / 32768.0 * g;
                out[i * 2] += l * 1f32.min(1.0 - pan);
                out[i * 2 + 1] += r * 1f32.min(1.0 + pan);
            }
        }
    }
    for v in &mut out[..frames * 2] {
        *v = if *v > 1.0 {
            1.0
        } else if *v < -1.0 {
            -1.0
        } else if v.is_nan() {
            0.0
        } else {
            *v
        };
    }
}

/// The plan mixed from `from`, `seconds` long, as a stereo 44.1 kHz WAV
/// (an export's soundtrack); None when the plan is silent.
pub fn bounce(plan: &AudioPlan, from: f64, seconds: f64) -> Option<Vec<u8>> {
    if plan.entries.is_empty() || !(seconds > 0.0) {
        return None;
    }
    let rate = 44100;
    let frames = ((rate as i64 * MAX_SECONDS as i64).min((seconds * rate as f64).round() as i64)) as usize;
    let mut d = vec![0i16; frames * 2];
    let mut block = vec![0f32; 4096 * 2];
    let mut at = 0;
    while at < frames {
        let n = 4096.min(frames - at);
        mix(plan, from + at as f64 / rate as f64, n, rate, &mut block);
        for i in 0..n * 2 {
            d[at * 2 + i] = Pcm::s16(block[i]);
        }
        at += 4096;
    }
    Some(encode_wav(&Pcm::new(rate, 2, d)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_round_trip_and_garbage() {
        let t = tone(440.0, 0.1, 22050, 0.5);
        let back = decode_wav(&encode_wav(&t)).unwrap();
        assert_eq!(back, t);
        assert!(decode_wav(b"RIFF____WAVEjunkjunkjunk").is_none());
        assert!(decode_wav(&[0; 3]).is_none());
    }
}
