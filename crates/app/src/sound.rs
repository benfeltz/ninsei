//! The speaker: one small mixer for everything the game plays. The modem handshake
//! (`ninsei_core::modem`) has its own channel, so hanging up can cut it. The music channel
//! holds one thing at a time: a cracktro's chiptune (`ninsei_core::intro`, played once, cut if
//! the intro is skipped) or the run's tune (`ninsei_core::music`, looped until the run stops
//! wanting it). The run's tune plays as a bed and a stem (its drop track) side by side, so the
//! trace can mute the stem without the loop losing its place; a tempo step renders the new
//! loop in the background and swaps it in at the same point in the music. Loops render a
//! slice a frame, never all at once. The speaker knows which kind of music is on, so a
//! cracktro closing as you jack in can't cut the run's tune. Sound cues (`ninsei_core::cue`)
//! are short clips that layer over everything and play out.
//! Native output goes through cpal; the browser uses Web Audio. If there's no audio device
//! (or the browser refuses), it stays quiet and the game carries on.

use std::collections::HashMap;
use std::sync::Arc;

use ninsei_core::cue::{Cue, SoundDef};
use ninsei_core::intro::{LoopRender, TuneDef};
use ninsei_core::modem::Handshake;
use ninsei_core::music::Wanted;

/// How loud cues play, under the modem.
const CUE_GAIN: f32 = 0.8;
/// How loud music plays (a cracktro's tune or the run's).
const MUSIC_GAIN: f32 = 0.7;
/// Most cue clips playing at once; more are dropped (a burst of cues is still one moment).
pub const MAX_VOICES: usize = 8;
/// Samples of a run loop rendered per frame (all tracks counted): a few milliseconds' work.
const LOOP_SLICE: usize = 250_000;

/// Samples shared between the speaker (which keeps what it plays, for the vis pane) and the
/// output.
pub type Clip = Arc<Vec<f32>>;

/// What the speaker plays through: cpal, Web Audio, or a test's stand-in.
pub trait Output: Sized {
    fn open() -> Option<Self>;
    fn rate(&self) -> u32;
    /// The modem channel; empty stops it.
    fn dial(&mut self, samples: Clip);
    /// Plays a tune once on the music channel, cutting what's there; empty stops it.
    fn music(&mut self, samples: Vec<f32>);
    /// Loops `bed` and `stem` (empty, or as long as the bed) together on the music channel,
    /// starting `from` (0-1) of the way through, the stem silent if `muted`. False if it
    /// couldn't start.
    fn music_loop(&mut self, bed: Clip, stem: Clip, from: f32, muted: bool) -> bool;
    /// How far through the loop it is, 0-1.
    fn music_place(&self) -> f32;
    /// Fades the loop's stem out or back in.
    fn mute_stem(&mut self, muted: bool);
    /// Plays a cue clip over everything; false if it was dropped (too many already playing).
    fn cue(&mut self, clip: Clip) -> bool;
    /// Silences everything at once (sound turned off).
    fn hush(&mut self);
    /// The master level, 0-1, over everything (eased, so a change never clicks).
    fn volume(&mut self, gain: f32);
}

/// What's on the music channel.
#[derive(Clone, Debug, PartialEq)]
enum Music {
    Quiet,
    Intro,
    Run {
        pack: String,
        tier: u32,
        tempo: u32,
        muted: bool,
    },
}

/// A run loop being rendered, a slice a frame.
struct Job {
    pack: String,
    tier: u32,
    tempo: u32,
    render: LoopRender,
}

pub struct Speaker<O: Output = backend::Out> {
    on: bool,
    out: Option<O>,
    /// Opening the device failed once; don't keep trying.
    dead: bool,
    /// Each cue rendered once per sound pack, at the device's rate.
    clips: HashMap<(String, Cue), Arc<Vec<f32>>>,
    music: Music,
    job: Option<Job>,
    /// The master level (`Save::volume`), 0-1.
    volume: f32,
    /// What's playing, kept for [`Speaker::listen`]: the run loop's bed and stem (and whether
    /// the stem is dropped), and clips (cues, the modem) with when they started.
    heard_loop: Option<(Clip, Clip, bool)>,
    heard: Vec<(Clip, f64)>,
    /// The modem's clip, apart from the cues so a hang-up can drop it.
    heard_dial: Option<(Clip, f64)>,
    /// The frame clock (seconds), from [`Speaker::tick`].
    clock: f64,
}

impl<O: Output> Speaker<O> {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            out: None,
            dead: false,
            clips: HashMap::new(),
            music: Music::Quiet,
            job: None,
            volume: 1.0,
            heard_loop: None,
            heard: Vec::new(),
            heard_dial: None,
            clock: 0.0,
        }
    }

    /// Moves the frame clock on to `now` (seconds) and forgets clips that have finished.
    pub fn tick(&mut self, now: f64) {
        self.clock = now;
        let Some(rate) = self.rate() else { return };
        let done = |(c, t0): &(Clip, f64)| (now - t0) * f64::from(rate) >= c.len() as f64;
        self.heard.retain(|h| !done(h));
        if self.heard_dial.as_ref().is_some_and(done) {
            self.heard_dial = None;
        }
    }

    /// The output's sample rate, once it's open.
    pub fn rate(&self) -> Option<u32> {
        self.out.as_ref().map(|o| o.rate())
    }

    /// The last `n` samples of what's playing (oldest first), as the speaker mixes them: the
    /// run loop where it's got to (its stem only while it isn't dropped), the cues and the
    /// modem by when they started, all at the master level. Silence when sound is off or
    /// nothing's playing. For drawing only: it reads, never plays.
    pub fn listen(&self, n: usize) -> Vec<f32> {
        let mut w = vec![0.0f32; n];
        let Some(out) = self.out.as_ref().filter(|_| self.on) else {
            return w;
        };
        if let Some((bed, stem, dropped)) = &self.heard_loop {
            let len = bed.len();
            if len > 0 {
                let end = (out.music_place().clamp(0.0, 1.0) * len as f32) as usize;
                for (i, o) in w.iter_mut().enumerate() {
                    let at = (end + len * (n / len + 1) - n + i) % len;
                    *o += bed[at];
                    if !dropped {
                        *o += stem.get(at).copied().unwrap_or(0.0);
                    }
                }
            }
        }
        let rate = f64::from(out.rate());
        for (clip, t0) in self.heard.iter().chain(&self.heard_dial) {
            let end = ((self.clock - t0) * rate) as i64;
            for (i, o) in w.iter_mut().enumerate() {
                let k = end - n as i64 + i as i64;
                if k >= 0 {
                    *o += clip.get(k as usize).copied().unwrap_or(0.0);
                }
            }
        }
        w.iter_mut()
            .for_each(|s| *s = (*s * self.volume).clamp(-1.0, 1.0));
        w
    }

    /// Sets the master level over the modem, the music and the cues alike (0-1).
    pub fn set_volume(&mut self, gain: f32) {
        self.volume = gain.clamp(0.0, 1.0);
        if let Some(out) = &mut self.out {
            out.volume(self.volume);
        }
    }

    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        if !on {
            self.stop();
            // Sound back on starts the run's tune again on the next frame.
            self.music = Music::Quiet;
            self.job = None;
            self.heard_loop = None;
            self.heard.clear();
            self.heard_dial = None;
            if let Some(out) = &mut self.out {
                out.hush();
            }
        }
    }

    fn out(&mut self) -> Option<&mut O> {
        if !self.on || self.dead {
            return None;
        }
        if self.out.is_none() {
            self.out = O::open();
            self.dead = self.out.is_none();
            // A new output starts at full; turn it down to the saved level.
            let gain = self.volume;
            if let Some(out) = self.out.as_mut().filter(|_| gain < 1.0) {
                out.volume(gain);
            }
        }
        self.out.as_mut()
    }

    /// Plays the call to `number` on the modem channel, cutting off any call still playing.
    pub fn dial(&mut self, number: &str) {
        if let Some(out) = self.out() {
            let samples = Arc::new(Handshake::new(number).render(out.rate()));
            out.dial(Arc::clone(&samples));
            self.heard_dial = Some((samples, self.clock));
        }
    }

    /// Plays a cracktro's tune on the music channel, cutting off whatever music is playing.
    pub fn tune(&mut self, tune: &TuneDef, secs: f32) {
        self.job = None;
        if let Some(out) = self.out() {
            let mut samples = tune.render(out.rate(), secs);
            samples.iter_mut().for_each(|s| *s *= MUSIC_GAIN);
            out.music(samples);
            self.music = Music::Intro;
            self.heard_loop = None;
        }
    }

    /// Cuts a cracktro's tune (the intro was skipped or closed). The run's tune plays on.
    pub fn stop_tune(&mut self) {
        if self.music == Music::Intro {
            self.music = Music::Quiet;
            if let Some(out) = &mut self.out {
                out.music(Vec::new());
            }
        }
    }

    /// Keeps the run's tune playing as `w` says; called every frame. A new tune renders a
    /// slice a frame and starts when it's ready; the same tune at a new tempo swaps in at the
    /// same place; a drop step only mutes or unmutes the stem. True while it's still rendering,
    /// so the caller keeps frames coming.
    pub fn run_music(&mut self, w: &Wanted) -> bool {
        let tempo = w.arrangement.tempo;
        let muted = w.arrangement.drop.is_some();
        if self.out().is_none() {
            return false;
        }
        let Some(out) = self.out.as_mut() else {
            return false;
        };
        if let Music::Run {
            pack,
            tier,
            tempo: playing,
            muted: was,
        } = &mut self.music
            && w.same_tune(pack, *tier)
        {
            // A drop step lands on the loop that's playing, even while a new tempo renders.
            if *was != muted {
                out.mute_stem(muted);
                *was = muted;
                if let Some(h) = &mut self.heard_loop {
                    h.2 = muted;
                }
            }
            if *playing == tempo {
                self.job = None;
                return false;
            }
        }
        let fits = |j: &Job| w.same_tune(&j.pack, j.tier) && j.tempo == tempo;
        if !self.job.as_ref().is_some_and(fits) {
            self.job = Some(Job {
                pack: w.pack.to_string(),
                tier: w.tier,
                tempo,
                render: w.tune.tune.loop_render(out.rate(), tempo, w.tune.drop),
            });
        }
        let Some(job) = self.job.take_if(|j| j.render.work(LOOP_SLICE)) else {
            return true;
        };
        let (mut bed, mut stem) = job.render.finish();
        bed.iter_mut()
            .chain(stem.iter_mut())
            .for_each(|s| *s *= MUSIC_GAIN);
        // The same tune at a new tempo picks up where the old one was.
        let from = match &self.music {
            Music::Run { pack, tier, .. } if w.same_tune(pack, *tier) => out.music_place(),
            _ => 0.0,
        };
        // If it couldn't start, the next frame tries again.
        let (bed, stem) = (Arc::new(bed), Arc::new(stem));
        if out.music_loop(Arc::clone(&bed), Arc::clone(&stem), from, muted) {
            self.heard_loop = Some((bed, stem, muted));
            self.music = Music::Run {
                pack: job.pack,
                tier: job.tier,
                tempo,
                muted,
            };
        }
        false
    }

    /// Stops the run's tune (the run ended or you went home). A cracktro's tune plays on.
    pub fn stop_music(&mut self) {
        self.job = None;
        if matches!(self.music, Music::Run { .. }) {
            self.music = Music::Quiet;
            self.heard_loop = None;
            if let Some(out) = &mut self.out {
                out.music(Vec::new());
            }
        }
    }

    /// Stops the modem (a BBS line dropped). Cues play out.
    pub fn stop(&mut self) {
        self.heard_dial = None;
        if let Some(out) = &mut self.out {
            out.dial(Arc::new(Vec::new()));
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
        // Only what the output took is heard: a dropped cue never played.
        if let Some(out) = &mut self.out
            && out.cue(Arc::clone(&clip))
        {
            self.heard.push((clip, self.clock));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::sync::{Arc, Mutex};

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SizedSample};

    /// What's playing: the modem channel, the music channel (a bed, maybe a stem beside it,
    /// looping or not), and cue clips with how far in each is.
    struct Mix {
        dial: (super::Clip, usize),
        music: super::Clip,
        stem: super::Clip,
        at: usize,
        looping: bool,
        /// The stem's loudness now and where it's heading, and how fast it gets there.
        stem_gain: f32,
        stem_to: f32,
        ramp: f32,
        /// The master level over everything, easing toward `master_to` like the stem.
        master: f32,
        master_to: f32,
        voices: Vec<(Arc<Vec<f32>>, usize)>,
    }

    impl Default for Mix {
        /// Nothing playing, at full level (the master multiplies everything: zero is silence).
        fn default() -> Mix {
            Mix {
                dial: (Arc::new(Vec::new()), 0),
                music: Arc::new(Vec::new()),
                stem: Arc::new(Vec::new()),
                at: 0,
                looping: false,
                stem_gain: 0.0,
                stem_to: 0.0,
                ramp: 0.0,
                master: 1.0,
                master_to: 1.0,
                voices: Vec::new(),
            }
        }
    }

    impl Mix {
        /// Nothing playing, at the same level.
        fn hushed(&self) -> Mix {
            Mix {
                ramp: self.ramp,
                master: self.master_to,
                master_to: self.master_to,
                ..Mix::default()
            }
        }

        fn next(&mut self) -> f32 {
            let (samples, pos) = &mut self.dial;
            let mut s = samples.get(*pos).copied().unwrap_or(0.0);
            *pos += 1;
            if self.looping && self.at >= self.music.len() {
                self.at = 0;
            }
            s += self.music.get(self.at).copied().unwrap_or(0.0);
            s += self.stem.get(self.at).copied().unwrap_or(0.0) * self.stem_gain;
            self.stem_gain += (self.stem_to - self.stem_gain) * self.ramp;
            self.at += 1;
            for (clip, at) in &mut self.voices {
                s += clip.get(*at).copied().unwrap_or(0.0);
                *at += 1;
            }
            self.voices.retain(|(clip, at)| *at < clip.len());
            s *= self.master;
            self.master += (self.master_to - self.master) * self.ramp;
            s.clamp(-1.0, 1.0)
        }
    }

    pub struct Out {
        mix: Arc<Mutex<Mix>>,
        rate: u32,
        _stream: cpal::Stream,
    }

    impl Out {
        fn with<R>(&self, f: impl FnOnce(&mut Mix) -> R) -> Option<R> {
            self.mix.lock().ok().map(|mut m| f(&mut m))
        }
    }

    impl super::Output for Out {
        fn open() -> Option<Self> {
            let device = cpal::default_host().default_output_device()?;
            let config = device.default_output_config().ok()?;
            let rate = config.sample_rate().0;
            let mix = Arc::new(Mutex::new(Mix {
                // About 10 ms to fade the stem, so muting it never clicks.
                ramp: 1.0 / (0.01 * rate as f32).max(1.0),
                ..Mix::default()
            }));
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

        fn rate(&self) -> u32 {
            self.rate
        }

        fn dial(&mut self, samples: super::Clip) {
            self.with(|m| m.dial = (samples, 0));
        }

        fn music(&mut self, samples: Vec<f32>) {
            self.with(|m| {
                (m.music, m.stem, m.at, m.looping) =
                    (Arc::new(samples), Arc::new(Vec::new()), 0, false);
            });
        }

        fn music_loop(
            &mut self,
            bed: super::Clip,
            stem: super::Clip,
            from: f32,
            muted: bool,
        ) -> bool {
            let at = (from.clamp(0.0, 1.0) * bed.len() as f32) as usize;
            let gain = if muted { 0.0 } else { 1.0 };
            self.with(|m| {
                (m.music, m.stem, m.at, m.looping) = (bed, stem, at, true);
                (m.stem_gain, m.stem_to) = (gain, gain);
            })
            .is_some()
        }

        fn music_place(&self) -> f32 {
            self.with(|m| m.at as f32 / m.music.len().max(1) as f32)
                .unwrap_or(0.0)
        }

        fn mute_stem(&mut self, muted: bool) {
            self.with(|m| m.stem_to = if muted { 0.0 } else { 1.0 });
        }

        fn cue(&mut self, clip: super::Clip) -> bool {
            self.with(|m| {
                let room = m.voices.len() < super::MAX_VOICES;
                if room {
                    m.voices.push((clip, 0));
                }
                room
            })
            .unwrap_or(false)
        }

        fn hush(&mut self) {
            self.with(|m| *m = m.hushed());
        }

        fn volume(&mut self, gain: f32) {
            self.with(|m| m.master_to = gain);
        }
    }

    #[cfg(test)]
    #[test]
    fn the_master_level_scales_everything_and_eases() {
        let mut m = Mix {
            dial: (Arc::new(vec![0.2; 400]), 0),
            music: Arc::new(vec![0.2; 400]),
            looping: true,
            voices: vec![(Arc::new(vec![0.2; 400]), 0)],
            ramp: 0.1,
            ..Mix::default()
        };
        assert!(
            (m.next() - 0.6).abs() < 1e-6,
            "full: modem, music and a cue"
        );
        m.master_to = 0.5;
        let first = m.next();
        assert!(first > 0.3, "it eases down, never jumps: {first}");
        let settled = (0..200).map(|_| m.next()).last().unwrap();
        assert!((settled - 0.3).abs() < 1e-3, "half: {settled}");
        // Hushing (sound off) silences everything but keeps the level.
        let h = m.hushed();
        assert_eq!((h.master, h.master_to, h.ramp), (0.5, 0.5, 0.1));
        assert!(h.music.is_empty() && h.voices.is_empty() && h.dial.0.is_empty());
        // A fresh mix is at full: whatever it's given is heard.
        let mut fresh = Mix {
            music: Arc::new(vec![0.2; 4]),
            ..Mix::default()
        };
        assert!((fresh.next() - 0.2).abs() < 1e-6);
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
    use web_sys::{AudioBufferSourceNode, AudioContext, GainNode};

    pub struct Out {
        ctx: AudioContext,
        /// Everything plays through this, at the master level.
        master: GainNode,
        /// The modem channel, so a hang-up can stop it.
        dialing: Option<AudioBufferSourceNode>,
        /// The music channel's sources (a tune, or a loop's bed and stem), so they can stop.
        playing: Vec<AudioBufferSourceNode>,
        /// The loop's stem goes through this, so it can be muted.
        stem_gain: Option<GainNode>,
        /// When (context time) the loop was at its start, and how long it lasts; 0 if none.
        loop_from: f64,
        loop_secs: f64,
        /// Cue clips still sounding (pruned as they end).
        voices: Vec<(AudioBufferSourceNode, f64)>,
    }

    impl Out {
        /// A source for `samples`, on repeat if `looping`, not yet connected or started.
        fn source(&self, samples: &[f32], looping: bool) -> Option<AudioBufferSourceNode> {
            let buf = self
                .ctx
                .create_buffer(1, samples.len() as u32, self.ctx.sample_rate())
                .ok()?;
            buf.copy_to_channel(samples, 0).ok()?;
            let src = self.ctx.create_buffer_source().ok()?;
            src.set_buffer(Some(&buf));
            src.set_loop(looping);
            Some(src)
        }

        /// Plays a clip once, straight out; returns its node.
        fn start(&self, samples: &[f32]) -> Option<AudioBufferSourceNode> {
            // Browsers start audio suspended until the page has been clicked; every sound here
            // follows a click or a key.
            let _ = self.ctx.resume();
            if samples.is_empty() {
                return None;
            }
            let src = self.source(samples, false)?;
            src.connect_with_audio_node(&self.master).ok()?;
            src.start().ok()?;
            Some(src)
        }

        fn stop_music(&mut self) {
            for src in self.playing.drain(..) {
                #[allow(deprecated)]
                let _ = src.stop();
            }
            self.stem_gain = None;
            self.loop_secs = 0.0;
        }

        /// Starts a loop's bed and stem together; their nodes, or `None` if any part failed.
        fn start_loop(
            &self,
            bed: &[f32],
            stem: &[f32],
            offset: f64,
            muted: bool,
        ) -> Option<(Vec<AudioBufferSourceNode>, Option<GainNode>)> {
            let _ = self.ctx.resume();
            let out = &self.master;
            let mut nodes = vec![self.source(bed, true)?];
            nodes[0].connect_with_audio_node(out).ok()?;
            let mut gain = None;
            if !stem.is_empty() {
                let g = self.ctx.create_gain().ok()?;
                g.gain().set_value(if muted { 0.0 } else { 1.0 });
                g.connect_with_audio_node(out).ok()?;
                let src = self.source(stem, true)?;
                src.connect_with_audio_node(&g).ok()?;
                nodes.push(src);
                gain = Some(g);
            }
            let now = self.ctx.current_time();
            for (i, n) in nodes.iter().enumerate() {
                if n.start_with_when_and_grain_offset(now, offset).is_err() {
                    // Don't leave a bed looping with nothing holding it.
                    for started in &nodes[..i] {
                        #[allow(deprecated)]
                        let _ = started.stop();
                    }
                    return None;
                }
            }
            Some((nodes, gain))
        }
    }

    impl super::Output for Out {
        fn open() -> Option<Self> {
            let ctx = AudioContext::new().ok()?;
            let master = ctx.create_gain().ok()?;
            master.connect_with_audio_node(&ctx.destination()).ok()?;
            Some(Self {
                ctx,
                master,
                dialing: None,
                playing: Vec::new(),
                stem_gain: None,
                loop_from: 0.0,
                loop_secs: 0.0,
                voices: Vec::new(),
            })
        }

        fn rate(&self) -> u32 {
            self.ctx.sample_rate() as u32
        }

        fn dial(&mut self, samples: super::Clip) {
            if let Some(src) = self.dialing.take() {
                #[allow(deprecated)]
                let _ = src.stop();
            }
            self.dialing = self.start(&samples);
        }

        fn music(&mut self, samples: Vec<f32>) {
            self.stop_music();
            self.playing.extend(self.start(&samples));
        }

        fn music_loop(
            &mut self,
            bed: super::Clip,
            stem: super::Clip,
            from: f32,
            muted: bool,
        ) -> bool {
            self.stop_music();
            if bed.is_empty() {
                return false;
            }
            let secs = bed.len() as f64 / f64::from(self.ctx.sample_rate());
            let offset = f64::from(from.clamp(0.0, 1.0)) * secs;
            let Some((nodes, gain)) = self.start_loop(&bed, &stem, offset, muted) else {
                return false;
            };
            self.playing = nodes;
            self.stem_gain = gain;
            self.loop_from = self.ctx.current_time() - offset;
            self.loop_secs = secs;
            true
        }

        fn music_place(&self) -> f32 {
            if self.loop_secs <= 0.0 {
                return 0.0;
            }
            let t = (self.ctx.current_time() - self.loop_from) / self.loop_secs;
            t.rem_euclid(1.0) as f32
        }

        fn mute_stem(&mut self, muted: bool) {
            if let Some(g) = &self.stem_gain {
                // About 10 ms to fade, so muting never clicks.
                let to = if muted { 0.0 } else { 1.0 };
                let _ = g
                    .gain()
                    .set_target_at_time(to, self.ctx.current_time(), 0.01);
            }
        }

        fn cue(&mut self, clip: super::Clip) -> bool {
            let now = self.ctx.current_time();
            self.voices.retain(|(_, ends)| *ends > now);
            if self.voices.len() >= super::MAX_VOICES {
                return false;
            }
            let secs = clip.len() as f64 / f64::from(self.ctx.sample_rate());
            let Some(src) = self.start(&clip) else {
                return false;
            };
            self.voices.push((src, now + secs));
            true
        }

        fn hush(&mut self) {
            self.dial(std::sync::Arc::new(Vec::new()));
            self.stop_music();
            for (src, _) in self.voices.drain(..) {
                #[allow(deprecated)]
                let _ = src.stop();
            }
        }

        fn volume(&mut self, gain: f32) {
            // About 10 ms to get there, so a change never clicks.
            let _ = self
                .master
                .gain()
                .set_target_at_time(gain, self.ctx.current_time(), 0.01);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ninsei_core::music::{Arrangement, RunTune};

    /// An output that writes down what it was asked to do.
    #[derive(Default)]
    struct Log {
        said: Vec<String>,
        place: f32,
        /// Loops refuse to start while this is above zero, counting down.
        refuse: u32,
        /// Every cue voice is busy: cues are dropped.
        full: bool,
    }

    impl Output for Log {
        fn open() -> Option<Self> {
            Some(Log::default())
        }
        fn rate(&self) -> u32 {
            8000
        }
        fn dial(&mut self, _: Clip) {}
        fn music(&mut self, s: Vec<f32>) {
            self.said
                .push(if s.is_empty() { "stop" } else { "tune" }.into());
        }
        fn music_loop(&mut self, bed: Clip, stem: Clip, from: f32, muted: bool) -> bool {
            assert_eq!(bed.len(), stem.len(), "the stem keeps step with the bed");
            if self.refuse > 0 {
                self.refuse -= 1;
                self.said.push("refused".into());
                return false;
            }
            self.said.push(format!("loop from {from} muted {muted}"));
            true
        }
        fn music_place(&self) -> f32 {
            self.place
        }
        fn mute_stem(&mut self, muted: bool) {
            self.said.push(format!("mute {muted}"));
        }
        fn cue(&mut self, _: Clip) -> bool {
            !self.full
        }
        fn hush(&mut self) {
            self.said.push("hush".into());
        }
        fn volume(&mut self, gain: f32) {
            self.said.push(format!("volume {gain}"));
        }
    }

    fn tune() -> RunTune {
        RunTune {
            tier: 1,
            drop: Some(1),
            tune: ron::from_str(
                "(bpm: 120, tracks: [(wave: Square, vol: 0.2, pattern: \"C4 E4 G4 .\"), \
                 (wave: Noise, vol: 0.1, pattern: \"x . x\")])",
            )
            .unwrap(),
        }
    }

    fn at<'a>(t: &'a RunTune, drop: bool, tempo: u32) -> Wanted<'a> {
        Wanted {
            pack: "ono",
            tier: 1,
            arrangement: Arrangement {
                drop: drop.then_some(1),
                tempo,
            },
            tune: t,
        }
    }

    /// Frames of `run_music` until the loop is ready; how many it took.
    fn settle(s: &mut Speaker<Log>, w: &Wanted) -> usize {
        (1..1000).find(|_| !s.run_music(w)).unwrap()
    }

    fn said(s: &mut Speaker<Log>) -> Vec<String> {
        std::mem::take(&mut s.out.as_mut().unwrap().said)
    }

    #[test]
    fn the_loop_starts_once_and_trace_steps_keep_its_place() {
        let t = tune();
        let mut s = Speaker::<Log>::new(true);
        settle(&mut s, &at(&t, false, 0));
        assert_eq!(said(&mut s), ["loop from 0 muted false"]);
        for _ in 0..5 {
            assert!(!s.run_music(&at(&t, false, 0)));
        }
        assert!(said(&mut s).is_empty(), "the same key never re-renders");
        // The drop step mutes the stem, back and forth, and never restarts the loop.
        s.run_music(&at(&t, true, 0));
        s.run_music(&at(&t, true, 0));
        s.run_music(&at(&t, false, 0));
        assert_eq!(said(&mut s), ["mute true", "mute false"]);
        // The tempo step swaps in a new loop where the old one had got to; the drop step that
        // came with it lands on the old loop first.
        s.out.as_mut().unwrap().place = 0.25;
        settle(&mut s, &at(&t, true, 12));
        assert_eq!(said(&mut s), ["mute true", "loop from 0.25 muted true"]);
    }

    #[test]
    fn a_drop_step_lands_while_the_new_tempo_renders() {
        let d = ninsei_core::Data::builtin().unwrap();
        let t = d.run_tune("ono", 2).unwrap();
        let mut s = Speaker::<Log>::new(true);
        settle(&mut s, &at(t, false, 0));
        said(&mut s);
        // Trace jumps past both steps at once: the stem mutes now, the faster loop follows
        // once it's rendered, and the mute isn't asked for twice.
        assert!(s.run_music(&at(t, true, 12)), "still rendering");
        assert!(s.run_music(&at(t, true, 12)), "still rendering");
        assert_eq!(said(&mut s), ["mute true"]);
        settle(&mut s, &at(t, true, 12));
        assert_eq!(said(&mut s), ["loop from 0 muted true"]);
    }

    #[test]
    fn a_long_loop_renders_over_several_frames() {
        let d = ninsei_core::Data::builtin().unwrap();
        let t = d.run_tune("ono", 2).unwrap();
        let mut s = Speaker::<Log>::new(true);
        let frames = settle(&mut s, &at(t, false, 0));
        assert!(frames > 3, "{frames} frames");
        assert_eq!(said(&mut s).len(), 1);
    }

    #[test]
    fn a_cracktro_and_the_run_tune_never_cut_each_other() {
        let t = tune();
        let mut s = Speaker::<Log>::new(true);
        s.tune(&t.tune, 1.0);
        s.stop_music();
        assert_eq!(
            said(&mut s),
            ["tune"],
            "the run's stop leaves a cracktro alone"
        );
        // Jacking in while the intro plays: the run takes the channel, then the intro's
        // window closing doesn't stop it.
        settle(&mut s, &at(&t, false, 0));
        s.stop_tune();
        assert!(!s.run_music(&at(&t, false, 0)));
        assert_eq!(said(&mut s), ["loop from 0 muted false"]);
        s.stop_music();
        s.stop_music();
        assert_eq!(said(&mut s), ["stop"]);
    }

    #[test]
    fn sound_off_and_on_brings_the_tune_back() {
        let t = tune();
        let mut s = Speaker::<Log>::new(true);
        settle(&mut s, &at(&t, false, 0));
        s.set_on(false);
        assert!(
            !s.run_music(&at(&t, false, 0)),
            "nothing renders while it's off"
        );
        s.set_on(true);
        settle(&mut s, &at(&t, false, 0));
        assert_eq!(
            said(&mut s),
            ["loop from 0 muted false", "hush", "loop from 0 muted false"]
        );
    }

    #[test]
    fn the_volume_reaches_the_output_whenever_it_opens() {
        let t = tune();
        // Set before anything played: the output opens at that level.
        let mut s = Speaker::<Log>::new(true);
        s.set_volume(0.4);
        assert!(s.out.is_none(), "nothing opened yet");
        settle(&mut s, &at(&t, false, 0));
        assert_eq!(said(&mut s), ["volume 0.4", "loop from 0 muted false"]);
        // Changed while playing: straight to the output, the loop untouched.
        s.set_volume(0.9);
        s.set_volume(7.0);
        assert_eq!(said(&mut s), ["volume 0.9", "volume 1"]);
        // Sound off and on again keeps the level.
        s.set_volume(0.5);
        s.set_on(false);
        s.set_on(true);
        settle(&mut s, &at(&t, false, 0));
        assert_eq!(
            said(&mut s),
            ["volume 0.5", "hush", "loop from 0 muted false"]
        );
        // At full, a new output is left as it opens.
        let mut s = Speaker::<Log>::new(true);
        settle(&mut s, &at(&t, false, 0));
        assert_eq!(said(&mut s), ["loop from 0 muted false"]);
    }

    #[test]
    fn it_hears_what_it_plays_and_nothing_when_it_s_off() {
        let t = tune();
        let mut s = Speaker::<Log>::new(true);
        assert!(s.listen(64).iter().all(|v| *v == 0.0), "nothing open yet");
        settle(&mut s, &at(&t, false, 0));
        s.out.as_mut().unwrap().place = 0.5;
        let loud = |w: &[f32]| w.iter().map(|v| v.abs()).sum::<f32>();
        let both = s.listen(256);
        assert_eq!(both.len(), 256);
        assert!(loud(&both) > 0.0, "the loop is heard");
        // The drop step takes the stem out of what's heard too.
        s.run_music(&at(&t, true, 0));
        let bed = s.listen(256);
        assert_ne!(bed, both);
        // The master level scales it.
        s.set_volume(0.5);
        let half = s.listen(256);
        assert!(
            bed.iter()
                .zip(&half)
                .all(|(a, b)| (a * 0.5 - b).abs() < 1e-6)
        );
        // A cue is heard from when it started until it's done.
        s.stop_music();
        assert!(s.listen(64).iter().all(|v| *v == 0.0), "the loop stopped");
        let d = ninsei_core::Data::builtin().unwrap();
        let pack = d.sound_pack("ono").unwrap();
        s.tick(10.0);
        s.cue(Cue::Hit, "ono", pack);
        s.tick(10.02);
        assert!(loud(&s.listen(64)) > 0.0, "the cue");
        s.tick(20.0);
        assert!(s.heard.is_empty(), "a finished cue is forgotten");
        // A cue the output dropped (every voice busy) never played, so it isn't heard.
        s.out.as_mut().unwrap().full = true;
        s.cue(Cue::Hit, "ono", pack);
        s.tick(20.02);
        assert!(s.heard.is_empty());
        assert!(s.listen(64).iter().all(|v| *v == 0.0));
        s.out.as_mut().unwrap().full = false;
        assert!(s.listen(64).iter().all(|v| *v == 0.0));
        // The modem is heard while it plays, from the very samples the output was given.
        s.tick(25.0);
        s.dial("555-0001");
        s.tick(25.5);
        assert!(loud(&s.listen(256)) > 0.0, "the handshake");
        s.stop();
        assert!(s.listen(256).iter().all(|v| *v == 0.0), "hung up");
        // Sound off: silence, whatever was playing.
        s.tick(30.0);
        s.cue(Cue::Hit, "ono", pack);
        s.tick(30.02);
        s.set_on(false);
        assert!(s.listen(64).iter().all(|v| *v == 0.0));
    }

    #[test]
    fn a_loop_that_fails_to_start_tries_again() {
        let t = tune();
        let mut s = Speaker::<Log>::new(true);
        s.out = Some(Log {
            refuse: 1,
            ..Log::default()
        });
        settle(&mut s, &at(&t, false, 0));
        settle(&mut s, &at(&t, false, 0));
        assert_eq!(said(&mut s), ["refused", "loop from 0 muted false"]);
    }
}
