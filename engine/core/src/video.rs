//! Video export: H.264 (OpenH264, built from source) in a QuickTime movie,
//! with the soundtrack as 16-bit PCM. Apple's players and Photos read it
//! as is, and so do the usual desktop players. Pictures arrive one by one
//! with how long each shows, like the GIF writer.

use openh264::encoder::{Encoder, EncoderConfig, IntraFramePeriod, QpRange};
use openh264::formats::{RgbaSliceU8, YUVBuffer};
use openh264::OpenH264API;
use std::io::{Seek, SeekFrom, Write};

const TIMESCALE: u32 = 90_000;

pub struct Mov<W: Write + Seek> {
    out: W,
    w: u32,
    h: u32,
    enc: Encoder,
    sps: Vec<u8>,
    pps: Vec<u8>,
    /// Bytes written into mdat so far (after its 16-byte header).
    pos: u64,
    mdat_at: u64,
    sizes: Vec<u32>,
    sync: Vec<u32>,
    durations: Vec<u32>,
    /// Picture start times in seconds, for rounding without drift.
    elapsed: f64,
    ticks: u64,
}

fn io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

impl<W: Write + Seek> Mov<W> {
    /// w and h are rounded down to even numbers (the encoder needs them).
    pub fn new(mut out: W, w: u32, h: u32, rate: f64) -> std::io::Result<Mov<W>> {
        let (w, h) = (w & !1, h & !1);
        if w < 16 || h < 16 {
            return Err(io("the video is too small"));
        }
        let period = ((rate.max(1.0) * 2.0).round() as u32).max(1);
        let cfg = EncoderConfig::new().skip_frames(false).qp(QpRange::new(16, 30)).intra_frame_period(IntraFramePeriod::from_num_frames(period));
        let enc = Encoder::with_api_config(OpenH264API::from_source(), cfg).map_err(io)?;
        // ftyp, then an mdat with a 64-bit size patched in finish()
        let mut head = Vec::new();
        atom(&mut head, b"ftyp", |b| {
            b.extend_from_slice(b"qt  ");
            b.extend_from_slice(&0x0000_0200u32.to_be_bytes());
            b.extend_from_slice(b"qt  ");
        });
        out.write_all(&head)?;
        let mdat_at = out.stream_position()?;
        out.write_all(&1u32.to_be_bytes())?;
        out.write_all(b"mdat")?;
        out.write_all(&0u64.to_be_bytes())?;
        Ok(Mov { out, w, h, enc, sps: Vec::new(), pps: Vec::new(), pos: 0, mdat_at, sizes: Vec::new(), sync: Vec::new(), durations: Vec::new(), elapsed: 0.0, ticks: 0 })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    /// One picture (RGBA, the size given to new) shown for `seconds`.
    pub fn frame(&mut self, rgba: &[u8], seconds: f64) -> std::io::Result<()> {
        let need = (self.w * self.h * 4) as usize;
        if rgba.len() < need {
            return Err(io("picture smaller than the video"));
        }
        let yuv = YUVBuffer::from_rgb8_source(RgbaSliceU8::new(&rgba[..need], (self.w as usize, self.h as usize)));
        let bs = self.enc.encode(&yuv).map_err(io)?;
        let mut sample = Vec::new();
        let mut idr = false;
        for l in 0..bs.num_layers() {
            let Some(layer) = bs.layer(l) else { continue };
            for n in 0..layer.nal_count() {
                let Some(unit) = layer.nal_unit(n) else { continue };
                let body = strip_start_code(unit);
                if body.is_empty() {
                    continue;
                }
                match body[0] & 0x1f {
                    7 => self.sps = body.to_vec(),
                    8 => self.pps = body.to_vec(),
                    t => {
                        idr |= t == 5;
                        sample.extend_from_slice(&(body.len() as u32).to_be_bytes());
                        sample.extend_from_slice(body);
                    }
                }
            }
        }
        if sample.is_empty() {
            return Ok(());
        }
        self.out.write_all(&sample)?;
        self.pos += sample.len() as u64;
        self.sizes.push(sample.len() as u32);
        if idr {
            self.sync.push(self.sizes.len() as u32);
        }
        // durations from rounded start times, so they never drift
        self.elapsed += seconds.max(0.0);
        let end = (self.elapsed * TIMESCALE as f64).round() as u64;
        self.durations.push((end - self.ticks).max(1) as u32);
        self.ticks = end.max(self.ticks + 1);
        Ok(())
    }

    /// Writes the soundtrack (interleaved stereo at `rate`, if any) and the
    /// index; returns the writer.
    pub fn finish(mut self, sound: Option<(&[i16], u32)>) -> std::io::Result<W> {
        if self.sizes.is_empty() || self.sps.is_empty() || self.pps.is_empty() {
            return Err(io("no pictures"));
        }
        let video_at = self.mdat_at + 16;
        let mut audio = None;
        if let Some((pcm, rate)) = sound.filter(|(p, r)| p.len() >= 2 && *r > 0) {
            let at = video_at + self.pos;
            let mut bytes = Vec::with_capacity(pcm.len() * 2);
            for s in pcm {
                bytes.extend_from_slice(&s.to_le_bytes());
            }
            self.out.write_all(&bytes)?;
            self.pos += bytes.len() as u64;
            audio = Some((at, (pcm.len() / 2) as u32, rate));
        }
        let end = self.out.stream_position()?;
        self.out.seek(SeekFrom::Start(self.mdat_at + 8))?;
        self.out.write_all(&(16 + self.pos).to_be_bytes())?;
        self.out.seek(SeekFrom::Start(end))?;

        let secs = self.ticks as f64 / TIMESCALE as f64;
        let movie_ms = |s: f64| (s * 1000.0).round() as u32;
        let audio_secs = audio.map_or(0.0, |(_, n, r)| n as f64 / r as f64);
        let mut moov = Vec::new();
        atom(&mut moov, b"moov", |m| {
            atom(m, b"mvhd", |b| {
                full(b, 0, 0);
                b.extend_from_slice(&[0; 8]);
                b.extend_from_slice(&1000u32.to_be_bytes());
                b.extend_from_slice(&movie_ms(secs.max(audio_secs)).to_be_bytes());
                b.extend_from_slice(&0x0001_0000u32.to_be_bytes());
                b.extend_from_slice(&0x0100u16.to_be_bytes());
                b.extend_from_slice(&[0; 10]);
                matrix(b);
                b.extend_from_slice(&[0; 24]);
                b.extend_from_slice(&3u32.to_be_bytes());
            });
            // video
            atom(m, b"trak", |t| {
                tkhd(t, 1, movie_ms(secs), false, self.w, self.h);
                atom(t, b"mdia", |d| {
                    mdhd(d, TIMESCALE, self.ticks as u32);
                    hdlr(d, b"mhlr", b"vide", "VideoHandler");
                    atom(d, b"minf", |i| {
                        atom(i, b"vmhd", |b| {
                            full(b, 0, 1);
                            b.extend_from_slice(&[0; 8]);
                        });
                        hdlr(i, b"dhlr", b"url ", "DataHandler");
                        dinf(i);
                        atom(i, b"stbl", |s| {
                            atom(s, b"stsd", |b| {
                                full(b, 0, 0);
                                b.extend_from_slice(&1u32.to_be_bytes());
                                atom(b, b"avc1", |e| {
                                    e.extend_from_slice(&[0; 6]);
                                    e.extend_from_slice(&1u16.to_be_bytes());
                                    e.extend_from_slice(&[0; 16]);
                                    e.extend_from_slice(&(self.w as u16).to_be_bytes());
                                    e.extend_from_slice(&(self.h as u16).to_be_bytes());
                                    e.extend_from_slice(&0x0048_0000u32.to_be_bytes());
                                    e.extend_from_slice(&0x0048_0000u32.to_be_bytes());
                                    e.extend_from_slice(&0u32.to_be_bytes());
                                    e.extend_from_slice(&1u16.to_be_bytes());
                                    e.extend_from_slice(&[0; 32]);
                                    e.extend_from_slice(&0x0018u16.to_be_bytes());
                                    e.extend_from_slice(&0xffffu16.to_be_bytes());
                                    atom(e, b"avcC", |c| {
                                        c.extend_from_slice(&[1, self.sps[1], self.sps[2], self.sps[3], 0xff, 0xe1]);
                                        c.extend_from_slice(&(self.sps.len() as u16).to_be_bytes());
                                        c.extend_from_slice(&self.sps);
                                        c.push(1);
                                        c.extend_from_slice(&(self.pps.len() as u16).to_be_bytes());
                                        c.extend_from_slice(&self.pps);
                                    });
                                });
                            });
                            atom(s, b"stts", |b| {
                                full(b, 0, 0);
                                let mut runs: Vec<(u32, u32)> = Vec::new();
                                for d in &self.durations {
                                    match runs.last_mut() {
                                        Some((n, v)) if *v == *d => *n += 1,
                                        _ => runs.push((1, *d)),
                                    }
                                }
                                b.extend_from_slice(&(runs.len() as u32).to_be_bytes());
                                for (n, v) in runs {
                                    b.extend_from_slice(&n.to_be_bytes());
                                    b.extend_from_slice(&v.to_be_bytes());
                                }
                            });
                            atom(s, b"stss", |b| {
                                full(b, 0, 0);
                                b.extend_from_slice(&(self.sync.len() as u32).to_be_bytes());
                                for k in &self.sync {
                                    b.extend_from_slice(&k.to_be_bytes());
                                }
                            });
                            one_chunk(s, self.sizes.len() as u32);
                            atom(s, b"stsz", |b| {
                                full(b, 0, 0);
                                b.extend_from_slice(&0u32.to_be_bytes());
                                b.extend_from_slice(&(self.sizes.len() as u32).to_be_bytes());
                                for z in &self.sizes {
                                    b.extend_from_slice(&z.to_be_bytes());
                                }
                            });
                            co64(s, video_at);
                        });
                    });
                });
            });
            // sound
            if let Some((at, frames, rate)) = audio {
                atom(m, b"trak", |t| {
                    tkhd(t, 2, movie_ms(audio_secs), true, 0, 0);
                    atom(t, b"mdia", |d| {
                        mdhd(d, rate, frames);
                        hdlr(d, b"mhlr", b"soun", "SoundHandler");
                        atom(d, b"minf", |i| {
                            atom(i, b"smhd", |b| {
                                full(b, 0, 0);
                                b.extend_from_slice(&[0; 4]);
                            });
                            hdlr(i, b"dhlr", b"url ", "DataHandler");
                            dinf(i);
                            atom(i, b"stbl", |s| {
                                atom(s, b"stsd", |b| {
                                    full(b, 0, 0);
                                    b.extend_from_slice(&1u32.to_be_bytes());
                                    atom(b, b"sowt", |e| {
                                        e.extend_from_slice(&[0; 6]);
                                        e.extend_from_slice(&1u16.to_be_bytes());
                                        e.extend_from_slice(&[0; 8]);
                                        e.extend_from_slice(&2u16.to_be_bytes());
                                        e.extend_from_slice(&16u16.to_be_bytes());
                                        e.extend_from_slice(&[0; 4]);
                                        e.extend_from_slice(&(rate << 16).to_be_bytes());
                                    });
                                });
                                atom(s, b"stts", |b| {
                                    full(b, 0, 0);
                                    b.extend_from_slice(&1u32.to_be_bytes());
                                    b.extend_from_slice(&frames.to_be_bytes());
                                    b.extend_from_slice(&1u32.to_be_bytes());
                                });
                                one_chunk(s, frames);
                                atom(s, b"stsz", |b| {
                                    full(b, 0, 0);
                                    b.extend_from_slice(&4u32.to_be_bytes());
                                    b.extend_from_slice(&frames.to_be_bytes());
                                });
                                co64(s, at);
                            });
                        });
                    });
                });
            }
        });
        self.out.write_all(&moov)?;
        self.out.flush()?;
        Ok(self.out)
    }
}

fn strip_start_code(u: &[u8]) -> &[u8] {
    if u.starts_with(&[0, 0, 0, 1]) {
        &u[4..]
    } else if u.starts_with(&[0, 0, 1]) {
        &u[3..]
    } else {
        u
    }
}

fn atom(out: &mut Vec<u8>, kind: &[u8; 4], body: impl FnOnce(&mut Vec<u8>)) {
    let at = out.len();
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(kind);
    body(out);
    let n = (out.len() - at) as u32;
    out[at..at + 4].copy_from_slice(&n.to_be_bytes());
}

fn full(b: &mut Vec<u8>, version: u8, flags: u32) {
    b.push(version);
    b.extend_from_slice(&flags.to_be_bytes()[1..]);
}

fn matrix(b: &mut Vec<u8>) {
    for v in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        b.extend_from_slice(&v.to_be_bytes());
    }
}

fn tkhd(t: &mut Vec<u8>, id: u32, ms: u32, sound: bool, w: u32, h: u32) {
    atom(t, b"tkhd", |b| {
        full(b, 0, 3);
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&id.to_be_bytes());
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&ms.to_be_bytes());
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&(if sound { 0x0100u16 } else { 0 }).to_be_bytes());
        b.extend_from_slice(&[0; 2]);
        matrix(b);
        b.extend_from_slice(&(w << 16).to_be_bytes());
        b.extend_from_slice(&(h << 16).to_be_bytes());
    });
}

fn mdhd(d: &mut Vec<u8>, scale: u32, duration: u32) {
    atom(d, b"mdhd", |b| {
        full(b, 0, 0);
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&scale.to_be_bytes());
        b.extend_from_slice(&duration.to_be_bytes());
        b.extend_from_slice(&0x55c4u16.to_be_bytes());
        b.extend_from_slice(&0u16.to_be_bytes());
    });
}

fn hdlr(d: &mut Vec<u8>, component: &[u8; 4], kind: &[u8; 4], name: &str) {
    atom(d, b"hdlr", |b| {
        full(b, 0, 0);
        b.extend_from_slice(component);
        b.extend_from_slice(kind);
        b.extend_from_slice(&[0; 12]);
        b.push(name.len() as u8);
        b.extend_from_slice(name.as_bytes());
    });
}

fn dinf(i: &mut Vec<u8>) {
    atom(i, b"dinf", |d| {
        atom(d, b"dref", |b| {
            full(b, 0, 0);
            b.extend_from_slice(&1u32.to_be_bytes());
            atom(b, b"url ", |u| full(u, 0, 1));
        });
    });
}

fn one_chunk(s: &mut Vec<u8>, samples: u32) {
    atom(s, b"stsc", |b| {
        full(b, 0, 0);
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&samples.to_be_bytes());
        b.extend_from_slice(&1u32.to_be_bytes());
    });
}

fn co64(s: &mut Vec<u8>, offset: u64) {
    atom(s, b"co64", |b| {
        full(b, 0, 0);
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&offset.to_be_bytes());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_movie_with_pictures_and_sound() {
        let (w, h) = (64u32, 48u32);
        let mut mov = Mov::new(std::io::Cursor::new(Vec::new()), w, h, 10.0).unwrap();
        for f in 0..12u32 {
            let mut px = vec![255u8; (w * h * 4) as usize];
            for y in 10..30 {
                for x in f * 3..f * 3 + 12 {
                    let i = ((y * w + x) * 4) as usize;
                    px[i] = 200;
                    px[i + 1] = 20;
                    px[i + 2] = 20;
                }
            }
            mov.frame(&px, if f == 5 { 0.6 } else { 0.1 }).unwrap();
        }
        let pcm: Vec<i16> = (0..44100 * 2).map(|i| ((i as f32 * 0.03).sin() * 8000.0) as i16).collect();
        let bytes = mov.finish(Some((&pcm, 44100))).unwrap().into_inner();
        let has = |k: &[u8]| bytes.windows(4).any(|w| w == k);
        for k in [b"ftyp", b"mdat", b"moov", b"avc1", b"avcC", b"sowt", b"stss", b"co64"] {
            assert!(has(k), "{}", String::from_utf8_lossy(k));
        }
        // the mdat size covers the pictures and the sound
        let mdat = bytes.windows(4).position(|w| w == b"mdat").unwrap() - 4;
        let size = u64::from_be_bytes(bytes[mdat + 8..mdat + 16].try_into().unwrap());
        assert!(size > (pcm.len() * 2) as u64);
        assert_eq!(&bytes[mdat + size as usize + 4..mdat + size as usize + 8], b"moov");
    }
}
