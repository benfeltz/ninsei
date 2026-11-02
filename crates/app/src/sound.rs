//! The speaker: one small mixer for everything the game plays. The modem handshake
//! (`ninsei_core::modem`) has its own channel, so hanging up can cut it; sound cues
//! (`ninsei_core::cue`) are short clips that layer over each other and play out.
//! Native output goes through cpal; the browser uses Web Audio. If there's no audio device
//! (or the browser refuses), it stays quiet and the game carries on.

use std::collections::HashMap;
use std::sync::Arc;

use ninsei_core::cue::{Cue, SoundDef};
use ninsei_core::modem::Handshake;

/// How loud cues play, under the modem.
const CUE_GAIN: f32 = 0.8;
/// Most cue clips playing at once; more are dropped (a burst of cues is still one moment).
pub const MAX_VOICES: usize = 8;

pub struct Speaker {
    on: bool,
    out: Option<backend::Out>,
    /// Opening the device failed once; don't keep trying.
    dead: bool,
    /// Each cue rendered once per sound pack, at the device's rate.
    clips: HashMap<(String, Cue), Arc<Vec<f32>>>,
}

impl Speaker {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            out: None,
            dead: false,
            clips: HashMap::new(),
        }
    }

    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        if !on {
            self.stop();
            if let Some(out) = &mut self.out {
                out.hush();
            }
        }
    }

    fn out(&mut self) -> Option<&mut backend::Out> {
        if !self.on || self.dead {
            return None;
        }
        if self.out.is_none() {
            self.out = backend::Out::open();
            self.dead = self.out.is_none();
        }
        self.out.as_mut()
    }

    /// Plays the call to `number` on the modem channel, cutting off any call still playing.
    pub fn dial(&mut self, number: &str) {
        if let Some(out) = self.out() {
            let samples = Handshake::new(number).render(out.rate());
            out.dial(samples);
        }
    }

    /// Stops the modem (a BBS line dropped). Cues play out.
    pub fn stop(&mut self) {
        if let Some(out) = &mut self.out {
            out.dial(Vec::new());
        }
    }

    /// Plays a cue from sound pack `pack` over whatever else is playing.
    pub fn cue(&mut self, cue: Cue, pack: &str, sounds: &[SoundDef]) {
        let Some(out) = self.out() else { return };
        let rate = out.rate();
        let key = (pack.to_string(), cue);
        let clip = match self.clips.get(&key) {
            Some(c) => Arc::clone(c),
            None => {
                let Some(def) = sounds.iter().find(|d| d.cue == cue) else {
                    return;
                };
                let samples: Vec<f32> = def.render(rate).iter().map(|s| s * CUE_GAIN).collect();
                let c = Arc::new(samples);
                self.clips.insert(key, Arc::clone(&c));
                c
            }
        };
        if let Some(out) = &mut self.out {
            out.cue(clip);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::sync::{Arc, Mutex};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SizedSample};

    /// What's playing: the modem channel, and cue clips with how far in each is.
    #[derive(Default)]
    struct Mix {
        dial: (Vec<f32>, usize),
        voices: Vec<(Arc<Vec<f32>>, usize)>,
    }

    impl Mix {
        fn next(&mut self) -> f32 {
            let (samples, pos) = &mut self.dial;
            let mut s = samples.get(*pos).copied().unwrap_or(0.0);
            *pos += 1;
            for (clip, at) in &mut self.voices {
                s += clip.get(*at).copied().unwrap_or(0.0);
                *at += 1;
            }
            self.voices.retain(|(clip, at)| *at < clip.len());
            s.clamp(-1.0, 1.0)
        }
    }

    pub struct Out {
        mix: Arc<Mutex<Mix>>,
        rate: u32,
        _stream: cpal::Stream,
    }

    impl Out {
        pub fn open() -> Option<Self> {
            let device = cpal::default_host().default_output_device()?;
            let config = device.default_output_config().ok()?;
            let rate = config.sample_rate().0;
            let mix = Arc::new(Mutex::new(Mix::default()));
            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => stream::<f32>(&device, &config.into(), mix.clone()),
                cpal::SampleFormat::I16 => stream::<i16>(&device, &config.into(), mix.clone()),
                cpal::SampleFormat::U16 => stream::<u16>(&device, &config.into(), mix.clone()),
                _ => None,
            }?;
            stream.play().ok()?;
            Some(Self {
                mix,
                rate,
                _stream: stream,
            })
        }

        pub fn rate(&self) -> u32 {
            self.rate
        }

        pub fn dial(&mut self, samples: Vec<f32>) {
            if let Ok(mut m) = self.mix.lock() {
                m.dial = (samples, 0);
            }
        }

        pub fn cue(&mut self, clip: Arc<Vec<f32>>) {
            if let Ok(mut m) = self.mix.lock()
                && m.voices.len() < super::MAX_VOICES
            {
                m.voices.push((clip, 0));
            }
        }

        /// Silences everything at once (sound turned off).
        pub fn hush(&mut self) {
            if let Ok(mut m) = self.mix.lock() {
                *m = Mix::default();
            }
        }
    }

    fn stream<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mix: Arc<Mutex<Mix>>,
    ) -> Option<cpal::Stream> {
        let channels = config.channels as usize;
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _| {
                    let Ok(mut m) = mix.lock() else { return };
                    for frame in data.chunks_mut(channels) {
                        let s = m.next();
                        for out in frame {
                            *out = T::from_sample(s);
                        }
                    }
                },
                |_| {},
                None,
            )
            .ok()
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use std::sync::Arc;

    use web_sys::{AudioBufferSourceNode, AudioContext};

    pub struct Out {
        ctx: AudioContext,
        /// The modem channel, so a hang-up can stop it.
        dialing: Option<AudioBufferSourceNode>,
        /// Cue clips still sounding (pruned as they end).
        voices: Vec<(AudioBufferSourceNode, f64)>,
    }

    impl Out {
        pub fn open() -> Option<Self> {
            Some(Self {
                ctx: AudioContext::new().ok()?,
                dialing: None,
                voices: Vec::new(),
            })
        }

        pub fn rate(&self) -> u32 {
            self.ctx.sample_rate() as u32
        }

        /// Starts a clip; returns its node.
        fn start(&self, samples: &[f32]) -> Option<AudioBufferSourceNode> {
            // Browsers start audio suspended until the page has been clicked; every sound here
            // follows a click or a key.
            let _ = self.ctx.resume();
            if samples.is_empty() {
                return None;
            }
            let buf = self
                .ctx
                .create_buffer(1, samples.len() as u32, self.ctx.sample_rate())
                .ok()?;
            buf.copy_to_channel(samples, 0).ok()?;
            let src = self.ctx.create_buffer_source().ok()?;
            src.set_buffer(Some(&buf));
            src.connect_with_audio_node(&self.ctx.destination()).ok()?;
            src.start().ok()?;
            Some(src)
        }

        pub fn dial(&mut self, samples: Vec<f32>) {
            if let Some(src) = self.dialing.take() {
                #[allow(deprecated)]
                let _ = src.stop();
            }
            self.dialing = self.start(&samples);
        }

        pub fn cue(&mut self, clip: Arc<Vec<f32>>) {
            let now = self.ctx.current_time();
            self.voices.retain(|(_, ends)| *ends > now);
            if self.voices.len() >= super::MAX_VOICES {
                return;
            }
            let secs = clip.len() as f64 / f64::from(self.ctx.sample_rate());
            if let Some(src) = self.start(&clip) {
                self.voices.push((src, now + secs));
            }
        }

        pub fn hush(&mut self) {
            self.dial(Vec::new());
            for (src, _) in self.voices.drain(..) {
                #[allow(deprecated)]
                let _ = src.stop();
            }
        }
    }
}
