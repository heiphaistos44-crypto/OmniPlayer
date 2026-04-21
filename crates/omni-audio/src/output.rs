use anyhow::{Context as _, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, Receiver, Sender};
use omni_core::decoder::DecodedAudioFrame;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::resampler::AudioResampler;

/// Ring buffer f32 interleaved, thread-safe.
struct RingBuffer {
    buf:  Vec<f32>,
    head: usize,
    tail: usize,
    len:  usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self { buf: vec![0.0; capacity], head: 0, tail: 0, len: 0 }
    }

    fn push_slice(&mut self, data: &[f32]) {
        for &s in data {
            if self.len < self.buf.len() {
                self.buf[self.tail] = s;
                self.tail = (self.tail + 1) % self.buf.len();
                self.len += 1;
            }
        }
    }

    fn pop_slice(&mut self, out: &mut [f32]) {
        for o in out.iter_mut() {
            if self.len > 0 {
                *o = self.buf[self.head];
                self.head = (self.head + 1) % self.buf.len();
                self.len -= 1;
            } else {
                *o = 0.0;
            }
        }
    }
}

pub struct AudioEngine {
    _stream:     cpal::Stream,
    sender:      Sender<DecodedAudioFrame>,
    volume:      Arc<Mutex<f32>>,
    paused:      Arc<Mutex<bool>>,
    device_rate: u32,
    channels:    usize,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let host   = cpal::default_host();
        let device = host
            .default_output_device()
            .context("aucun périphérique audio")?;
        let config = device
            .default_output_config()
            .context("config audio par défaut")?;

        log::info!("audio: {:?} — {:?}", device.name(), config);

        let device_rate = config.sample_rate().0;
        let channels    = config.channels() as usize;

        // 4 secondes de buffer pour absorber les pics de latence décodeur
        let ring   = Arc::new(Mutex::new(RingBuffer::new(device_rate as usize * channels * 4)));
        let volume = Arc::new(Mutex::new(1.0f32));
        let paused = Arc::new(Mutex::new(false));

        let (tx, rx) = bounded::<DecodedAudioFrame>(128);

        // Thread de resampling + remplissage du ring
        let ring_fill = ring.clone();
        std::thread::Builder::new()
            .name("audio-fill".into())
            .spawn(move || fill_ring(rx, ring_fill, device_rate, channels))?;

        let ring_w   = ring.clone();
        let vol_w    = volume.clone();
        let paused_w = paused.clone();

        let fmt  = config.sample_format();
        let cfg: cpal::StreamConfig = config.into();

        let stream = build_stream(&device, &cfg, fmt, ring_w, vol_w, paused_w)?;
        stream.play().context("play stream audio")?;

        Ok(Self { _stream: stream, sender: tx, volume, paused, device_rate, channels })
    }

    /// Taux d'échantillonnage du périphérique — à passer au décodeur.
    pub fn sample_rate(&self) -> u32 { self.device_rate }

    /// Nombre de canaux du périphérique (toujours 2 après downmix).
    pub fn channels(&self) -> usize { self.channels }

    pub fn push_frame(&self, frame: DecodedAudioFrame) {
        let _ = self.sender.try_send(frame);
    }

    pub fn set_volume(&self, v: f32) {
        *self.volume.lock() = v.clamp(0.0, 2.0);
    }

    pub fn set_paused(&self, p: bool) {
        *self.paused.lock() = p;
    }
}

// ─── Construction du stream CPAL (F32 / I16 / U16) ─────────────────────────

fn build_stream(
    device:  &cpal::Device,
    cfg:     &cpal::StreamConfig,
    fmt:     cpal::SampleFormat,
    ring:    Arc<Mutex<RingBuffer>>,
    volume:  Arc<Mutex<f32>>,
    paused:  Arc<Mutex<bool>>,
) -> Result<cpal::Stream> {
    macro_rules! make_f32_callback {
        ($ring:expr, $vol:expr, $paused:expr) => {{
            let ring_w   = $ring.clone();
            let vol_w    = $vol.clone();
            let paused_w = $paused.clone();
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if *paused_w.lock() { data.fill(0.0); return; }
                ring_w.lock().pop_slice(data);
                let vol = *vol_w.lock();
                if vol != 1.0 { for s in data.iter_mut() { *s *= vol; } }
            }
        }};
    }

    let err_fn = |e: cpal::StreamError| log::error!("cpal error: {e}");

    let stream = match fmt {
        cpal::SampleFormat::F32 => {
            device.build_output_stream(cfg, make_f32_callback!(ring, volume, paused), err_fn, None)?
        }
        cpal::SampleFormat::I16 => {
            let ring_w   = ring.clone();
            let vol_w    = volume.clone();
            let paused_w = paused.clone();
            device.build_output_stream(
                cfg,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    if *paused_w.lock() { data.fill(0); return; }
                    let mut tmp = vec![0f32; data.len()];
                    ring_w.lock().pop_slice(&mut tmp);
                    let vol = *vol_w.lock();
                    for (o, s) in data.iter_mut().zip(tmp.iter()) {
                        let v = (*s * vol).clamp(-1.0, 1.0);
                        *o = (v * i16::MAX as f32) as i16;
                    }
                },
                err_fn, None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let ring_w   = ring.clone();
            let vol_w    = volume.clone();
            let paused_w = paused.clone();
            device.build_output_stream(
                cfg,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    if *paused_w.lock() { data.fill(32768); return; }
                    let mut tmp = vec![0f32; data.len()];
                    ring_w.lock().pop_slice(&mut tmp);
                    let vol = *vol_w.lock();
                    for (o, s) in data.iter_mut().zip(tmp.iter()) {
                        let v = (*s * vol).clamp(-1.0, 1.0);
                        *o = ((v + 1.0) * 0.5 * u16::MAX as f32) as u16;
                    }
                },
                err_fn, None,
            )?
        }
        fmt => anyhow::bail!("format CPAL non géré: {fmt:?}"),
    };

    Ok(stream)
}

// ─── Thread de remplissage : resampling dynamique ───────────────────────────

fn fill_ring(
    rx:          Receiver<DecodedAudioFrame>,
    ring:        Arc<Mutex<RingBuffer>>,
    device_rate: u32,
    dev_channels: usize,
) {
    let mut resampler: Option<AudioResampler> = None;

    for frame in rx {
        let in_rate = frame.sample_rate;
        let in_ch   = frame.channels as usize;

        // Downmix surround → stéréo si nécessaire
        let stereo = if in_ch == dev_channels {
            frame.samples.clone()
        } else {
            downmix_to_stereo(&frame.samples, in_ch)
        };

        // Resampling si le décodeur n'est pas au taux du périphérique
        let final_samples = if in_rate == device_rate {
            stereo
        } else {
            // Crée ou recrée le resampler si les paramètres changent
            let needs_new = resampler.as_ref()
                .map(|r| r.in_rate() != in_rate || r.out_rate() != device_rate)
                .unwrap_or(true);

            if needs_new {
                resampler = AudioResampler::new(in_rate, device_rate, dev_channels.min(2)).ok();
            }

            if let Some(r) = &mut resampler {
                r.process_interleaved(&stereo).unwrap_or(stereo)
            } else {
                stereo
            }
        };

        ring.lock().push_slice(&final_samples);
    }
}

/// Downmix N canaux → stéréo par simple moyenne des canaux gauche/droit.
/// Mapping standard : FL FR FC LFE BL BR SL SR
fn downmix_to_stereo(samples: &[f32], in_channels: usize) -> Vec<f32> {
    if in_channels == 0 { return Vec::new(); }
    let frames = samples.len() / in_channels;
    let mut out = Vec::with_capacity(frames * 2);

    for f in 0..frames {
        let base = f * in_channels;
        let (l, r) = match in_channels {
            1 => {
                let m = samples[base];
                (m, m)
            }
            2 => (samples[base], samples[base + 1]),
            // 5.1 : FL FR FC LFE BL BR
            6 => {
                let fl = samples[base];
                let fr = samples[base + 1];
                let fc = samples[base + 2];
                let _lfe = samples[base + 3];
                let bl = samples[base + 4];
                let br = samples[base + 5];
                let l = (fl + fc * 0.707 + bl * 0.707).clamp(-1.0, 1.0);
                let r = (fr + fc * 0.707 + br * 0.707).clamp(-1.0, 1.0);
                (l, r)
            }
            // 7.1 : FL FR FC LFE BL BR SL SR
            8 => {
                let fl = samples[base];
                let fr = samples[base + 1];
                let fc = samples[base + 2];
                let _lfe = samples[base + 3];
                let bl = samples[base + 4];
                let br = samples[base + 5];
                let sl = samples[base + 6];
                let sr = samples[base + 7];
                let l = (fl + fc * 0.707 + bl * 0.5 + sl * 0.707).clamp(-1.0, 1.0);
                let r = (fr + fc * 0.707 + br * 0.5 + sr * 0.707).clamp(-1.0, 1.0);
                (l, r)
            }
            // Cas générique : moyenne des canaux pairs/impairs
            n => {
                let mut lsum = 0f32;
                let mut rsum = 0f32;
                for ch in 0..n {
                    if ch % 2 == 0 { lsum += samples[base + ch]; }
                    else           { rsum += samples[base + ch]; }
                }
                let half = (n / 2).max(1) as f32;
                ((lsum / half).clamp(-1.0, 1.0), (rsum / half).clamp(-1.0, 1.0))
            }
        };
        out.push(l);
        out.push(r);
    }
    out
}
