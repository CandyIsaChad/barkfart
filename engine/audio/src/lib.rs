//! Sound output: plays an AudioPlan on the device (cpal: CoreAudio on Apple,
//! AAudio / OpenSL on Android, ALSA / PulseAudio / PipeWire on Linux,
//! WASAPI on Windows). The mixing itself is barkfart_core::audio::mix.

use barkfart_core::audio::{mix, AudioPlan};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// What the audio thread plays.
#[derive(Clone, Debug)]
pub struct State {
    pub plan: Arc<AudioPlan>,
    pub playing: bool,
    /// Timeline time of the next frame to play.
    pub t: f64,
    /// Playback wraps from loop_end back to loop_start (loop_end <= loop_start: no loop).
    pub loop_start: f64,
    pub loop_end: f64,
}

impl Default for State {
    fn default() -> Self {
        State { plan: Arc::new(AudioPlan::default()), playing: false, t: 0.0, loop_start: 0.0, loop_end: 0.0 }
    }
}

/// Fills `out` (interleaved, `channels` per frame) from the state at the
/// given rate, advancing and looping its time. Pure apart from `state`.
pub fn fill(state: &mut State, out: &mut [f32], channels: usize, rate: u32, scratch: &mut Vec<f32>) {
    out.fill(0.0);
    let channels = channels.max(1);
    let frames = out.len() / channels;
    if !state.playing || frames == 0 || rate == 0 {
        return;
    }
    scratch.resize(frames * 2, 0.0);
    let looping = state.loop_end > state.loop_start;
    let mut done = 0;
    while done < frames {
        if looping && state.t >= state.loop_end {
            state.t = state.loop_start + (state.t - state.loop_end).min(state.loop_end - state.loop_start);
            if state.t >= state.loop_end {
                state.t = state.loop_start;
            }
        }
        // stop at the loop end inside this block
        let mut n = frames - done;
        if looping {
            let left = ((state.loop_end - state.t) * rate as f64).ceil().max(1.0) as usize;
            n = n.min(left);
        }
        mix(&state.plan, state.t, n, rate as i32, &mut scratch[..n * 2]);
        for i in 0..n {
            let o = (done + i) * channels;
            out[o] = scratch[i * 2];
            if channels >= 2 {
                out[o + 1] = scratch[i * 2 + 1];
            } else {
                out[o] = (scratch[i * 2] + scratch[i * 2 + 1]) * 0.5;
            }
        }
        state.t += n as f64 / rate as f64;
        done += n;
    }
}

/// The device output. Without a usable device every call still works and
/// nothing is heard.
pub struct Player {
    state: Arc<Mutex<State>>,
    stream: Option<cpal::Stream>,
    pub rate: u32,
    pub device: String,
}

impl Player {
    pub fn new() -> Player {
        let state = Arc::new(Mutex::new(State::default()));
        match Self::open(state.clone()) {
            Ok((stream, rate, device)) => Player { state, stream: Some(stream), rate, device },
            Err(e) => Player { state, stream: None, rate: 44100, device: format!("no sound output ({e})") },
        }
    }

    fn open(state: Arc<Mutex<State>>) -> Result<(cpal::Stream, u32, String), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no output device")?;
        let name = device.id().map(|d| d.to_string()).unwrap_or_else(|_| "output".into());
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            // f32 is the common case on every target; others can be added
            return Err(format!("unsupported sample format {}", supported.sample_format()));
        }
        let config: cpal::StreamConfig = supported.into();
        let rate = config.sample_rate;
        let channels = config.channels as usize;
        let mut scratch = Vec::new();
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // never wait for the UI thread: a busy lock means one block of silence
                    match state.try_lock() {
                        Ok(mut s) => fill(&mut s, data, channels, rate, &mut scratch),
                        Err(_) => data.fill(0.0),
                    }
                },
                |e| eprintln!("[barkfart audio] {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((stream, rate, name))
    }

    pub fn available(&self) -> bool {
        self.stream.is_some()
    }

    fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s)
    }

    pub fn set_plan(&self, plan: AudioPlan) {
        self.with(|s| s.plan = Arc::new(plan));
    }

    /// Plays from timeline time t, looping [loop_start, loop_end) when loop_end > loop_start.
    pub fn play(&self, t: f64, loop_start: f64, loop_end: f64) {
        self.with(|s| {
            s.t = t;
            s.loop_start = loop_start;
            s.loop_end = loop_end;
            s.playing = true;
        });
    }

    pub fn stop(&self) {
        self.with(|s| s.playing = false);
    }

    /// The timeline time being played, or None when stopped.
    pub fn position(&self) -> Option<f64> {
        self.with(|s| if s.playing { Some(s.t) } else { None })
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use barkfart_core::audio::{tone, AudioPlan, Entry};

    fn plan() -> AudioPlan {
        let pcm = Arc::new(tone(440.0, 1.0, 44100, 0.5));
        AudioPlan {
            entries: vec![Entry {
                pcm,
                start: 0.0,
                end: 1.0,
                in_: 0.0,
                out: 1.0,
                loop_: false,
                fade_in: 0.0,
                fade_out: 0.0,
                pitch: 1.0,
                volume: vec![1.0],
                pan: vec![0.0],
            }],
            end: 1.0,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn fills_advance_and_loop() {
        let mut s = State { plan: Arc::new(plan()), playing: true, t: 0.5, loop_start: 0.25, loop_end: 0.75, ..Default::default() };
        let mut out = vec![0f32; 48000 * 2 / 10];
        let mut scratch = Vec::new();
        fill(&mut s, &mut out, 2, 48000, &mut scratch);
        assert!((s.t - 0.6).abs() < 1e-9, "advanced 0.1 s: {}", s.t);
        assert!(out.iter().any(|v| v.abs() > 0.1), "sound");
        for _ in 0..3 {
            fill(&mut s, &mut out, 2, 48000, &mut scratch);
        }
        assert!(s.t >= 0.25 && s.t < 0.75, "wrapped inside the loop: {}", s.t);
        s.playing = false;
        fill(&mut s, &mut out, 2, 48000, &mut scratch);
        assert!(out.iter().all(|v| *v == 0.0), "stopped: silent");
    }

    #[test]
    fn mono_devices_get_both_channels() {
        let mut s = State { plan: Arc::new(plan()), playing: true, ..Default::default() };
        let mut out = vec![0f32; 4410];
        fill(&mut s, &mut out, 1, 44100, &mut Vec::new());
        assert!(out.iter().any(|v| v.abs() > 0.1));
    }

    #[test]
    fn a_player_works_without_a_device() {
        let p = Player::new();
        eprintln!("audio: {} at {} Hz", p.device, p.rate);
        p.set_plan(plan());
        p.play(0.0, 0.0, 0.0);
        assert!(p.position().is_some());
        p.stop();
        assert!(p.position().is_none());
    }
}
