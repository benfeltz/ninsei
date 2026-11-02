//! The speaker: plays the modem handshake from `ninsei_core::modem`.
//! Native output goes through cpal; the browser uses Web Audio. If there's no audio
//! device (or the browser refuses), it stays quiet and the game carries on.

use ninsei_core::modem::Handshake;

pub struct Speaker {
    on: bool,
    out: Option<backend::Out>,
    /// Opening the device failed once; don't keep trying.
    dead: bool,
}

impl Speaker {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            out: None,
            dead: false,
        }
    }

    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        if !on {
            self.stop();
        }
    }

    /// Plays the call to `number`, cutting off anything still playing.
    pub fn dial(&mut self, number: &str) {
        if !self.on || self.dead {
            return;
        }
        if self.out.is_none() {
            self.out = backend::Out::open();
            self.dead = self.out.is_none();
        }
        if let Some(out) = &mut self.out {
            let samples = Handshake::new(number).render(out.rate());
            out.play(samples);
        }
    }

    pub fn stop(&mut self) {
        if let Some(out) = &mut self.out {
            out.stop();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::sync::{Arc, Mutex};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SizedSample};

    /// The clip being played and how far in we are.
    type Clip = Arc<Mutex<(Vec<f32>, usize)>>;

    pub struct Out {
        clip: Clip,
        rate: u32,
        _stream: cpal::Stream,
    }

    impl Out {
        pub fn open() -> Option<Self> {
            let device = cpal::default_host().default_output_device()?;
            let config = device.default_output_config().ok()?;
            let rate = config.sample_rate().0;
            let clip: Clip = Arc::new(Mutex::new((Vec::new(), 0)));
            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => stream::<f32>(&device, &config.into(), clip.clone()),
                cpal::SampleFormat::I16 => stream::<i16>(&device, &config.into(), clip.clone()),
                cpal::SampleFormat::U16 => stream::<u16>(&device, &config.into(), clip.clone()),
                _ => None,
            }?;
            stream.play().ok()?;
            Some(Self {
                clip,
                rate,
                _stream: stream,
            })
        }

        pub fn rate(&self) -> u32 {
            self.rate
        }

        pub fn play(&mut self, samples: Vec<f32>) {
            if let Ok(mut c) = self.clip.lock() {
                *c = (samples, 0);
            }
        }

        pub fn stop(&mut self) {
            self.play(Vec::new());
        }
    }

    fn stream<T: SizedSample + FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        clip: Clip,
    ) -> Option<cpal::Stream> {
        let channels = config.channels as usize;
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _| {
                    let Ok(mut c) = clip.lock() else { return };
                    let (samples, pos) = &mut *c;
                    for frame in data.chunks_mut(channels) {
                        let s = samples.get(*pos).copied().unwrap_or(0.0);
                        *pos += 1;
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
    use web_sys::{AudioBufferSourceNode, AudioContext};

    pub struct Out {
        ctx: AudioContext,
        playing: Option<AudioBufferSourceNode>,
    }

    impl Out {
        pub fn open() -> Option<Self> {
            Some(Self {
                ctx: AudioContext::new().ok()?,
                playing: None,
            })
        }

        pub fn rate(&self) -> u32 {
            self.ctx.sample_rate() as u32
        }

        pub fn play(&mut self, samples: Vec<f32>) {
            self.stop();
            // Browsers start audio suspended until the page has been clicked; dialing is a click.
            let _ = self.ctx.resume();
            let Ok(buf) = self
                .ctx
                .create_buffer(1, samples.len() as u32, self.ctx.sample_rate())
            else {
                return;
            };
            if buf.copy_to_channel(&samples, 0).is_err() {
                return;
            }
            let Ok(src) = self.ctx.create_buffer_source() else {
                return;
            };
            src.set_buffer(Some(&buf));
            if src.connect_with_audio_node(&self.ctx.destination()).is_ok() && src.start().is_ok() {
                self.playing = Some(src);
            }
        }

        pub fn stop(&mut self) {
            if let Some(src) = self.playing.take() {
                #[allow(deprecated)]
                let _ = src.stop();
            }
        }
    }
}
