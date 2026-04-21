use anyhow::{Context as _, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, Receiver, Sender};
use omni_core::decoder::DecodedAudioFrame;
use parking_lot::Mutex;
use ringbuf::{HeapConsumer, HeapProducer, HeapRb};
use std::sync::{
    atomic::{AtomicU64, AtomicBool, Ordering},
    Arc,
};

use crate::resampler::AudioResampler;

// Capacité du ring buffer lock-free : 4 secondes @ 48kHz stéréo
const RING_SECS: usize = 4;

pub struct AudioEngine {
    _stream:     cpal::Stream,
    sender:      Sender<DecodedAudioFrame>,
    volume:      Arc<Mutex<f32>>,
    paused:      Arc<AtomicBool>,
    device_rate: u32,
    channels:    usize,
    /// Nombre d'échantillons occupés dans le ring buffer (mis à jour par fill_ring).
    ring_level:  Arc<AtomicU64>,
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

        log::info!("audio device: {:?} — {:?}", device.name(), config);

        let device_rate = config.sample_rate().0;
        let channels    = config.channels() as usize;
        let capacity    = device_rate as usize * channels * RING_SECS;

        // Ring buffer lock-free SPSC
        let rb: HeapRb<f32> = HeapRb::new(capacity);
        let (producer, consumer) = rb.split();

        let volume     = Arc::new(Mutex::new(1.0f32));
        let paused     = Arc::new(AtomicBool::new(false));
        let ring_level = Arc::new(AtomicU64::new(0));

        // Canal d'envoi de frames décodées vers le thread fill
        let (tx, rx) = bounded::<DecodedAudioFrame>(256);

        // Thread de resampling + remplissage du ring
        {
            let ring_level2 = ring_level.clone();
            std::thread::Builder::new()
                .name("audio-fill".into())
                .spawn(move || fill_ring(rx, producer, device_rate, channels, ring_level2))?;
        }

        let vol_w    = volume.clone();
        let paused_w = paused.clone();
        let fmt      = config.sample_format();
        let cfg: cpal::StreamConfig = config.into();

        let stream = build_stream(&device, &cfg, fmt, consumer, vol_w, paused_w)?;
        stream.play().context("play stream audio")?;

        Ok(Self {
            _stream: stream,
            sender: tx,
            volume,
            paused,
            device_rate,
            channels,
            ring_level,
        })
    }

    pub fn sample_rate(&self) -> u32 { self.device_rate }

    /// Secondes d'audio actuellement dans le ring buffer.
    pub fn buffered_secs(&self) -> f64 {
        let samples = self.ring_level.load(Ordering::Relaxed) as f64;
        samples / (self.device_rate as f64 * self.channels as f64)
    }

    /// Pousse un frame décodé. Bloque jusqu'à 200 ms si le canal est plein.
    pub fn push_frame(&self, frame: DecodedAudioFrame) {
        let _ = self.sender.send_timeout(frame, std::time::Duration::from_millis(200));
    }

    pub fn set_volume(&self, v: f32) {
        *self.volume.lock() = v.clamp(0.0, 2.0);
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
    }
}

// ─── Construction du stream CPAL ────────────────────────────────────────────

fn build_stream(
    device:  &cpal::Device,
    cfg:     &cpal::StreamConfig,
    fmt:     cpal::SampleFormat,
    consumer: HeapConsumer<f32>,
    volume:  Arc<Mutex<f32>>,
    paused:  Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let err_fn = |e: cpal::StreamError| log::error!("cpal error: {e}");

    // Consumer partagé via Mutex léger (accès exclusif CPAL uniquement)
    let cons = Arc::new(parking_lot::Mutex::new(consumer));

    let stream = match fmt {
        cpal::SampleFormat::F32 => {
            let cons_w   = cons.clone();
            let vol_w    = volume.clone();
            let paused_w = paused.clone();
            device.build_output_stream(
                cfg,
                move |data: &mut [f32], _| {
                    if paused_w.load(Ordering::Relaxed) {
                        data.fill(0.0); return;
                    }
                    let n = cons_w.lock().pop_slice(data);
                    if n < data.len() { data[n..].fill(0.0); } // underrun → silence
                    let vol = *vol_w.lock();
                    if (vol - 1.0).abs() > 0.001 {
                        for s in data.iter_mut() { *s *= vol; }
                    }
                },
                err_fn, None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let cons_w   = cons.clone();
            let vol_w    = volume.clone();
            let paused_w = paused.clone();
            device.build_output_stream(
                cfg,
                move |data: &mut [i16], _| {
                    if paused_w.load(Ordering::Relaxed) { data.fill(0); return; }
                    let mut tmp = vec![0f32; data.len()];
                    let n = cons_w.lock().pop_slice(&mut tmp);
                    if n < tmp.len() { tmp[n..].fill(0.0); }
                    let vol = *vol_w.lock();
                    for (o, s) in data.iter_mut().zip(tmp.iter()) {
                        *o = ((s * vol).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    }
                },
                err_fn, None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let cons_w   = cons.clone();
            let vol_w    = volume.clone();
            let paused_w = paused.clone();
            device.build_output_stream(
                cfg,
                move |data: &mut [u16], _| {
                    if paused_w.load(Ordering::Relaxed) { data.fill(32768); return; }
                    let mut tmp = vec![0f32; data.len()];
                    let n = cons_w.lock().pop_slice(&mut tmp);
                    if n < tmp.len() { tmp[n..].fill(0.0); }
                    let vol = *vol_w.lock();
                    for (o, s) in data.iter_mut().zip(tmp.iter()) {
                        *o = (((s * vol).clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32) as u16;
                    }
                },
                err_fn, None,
            )?
        }
        fmt => anyhow::bail!("format CPAL non géré: {fmt:?}"),
    };

    Ok(stream)
}

// ─── Thread fill_ring : resampling + downmix + écriture lock-free ────────────

fn fill_ring(
    rx:           Receiver<DecodedAudioFrame>,
    mut producer: HeapProducer<f32>,
    device_rate:  u32,
    dev_ch:       usize,
    ring_level:   Arc<AtomicU64>,
) {
    let mut resampler: Option<AudioResampler> = None;
    // Cible : garder au plus 2 secondes dans le ring pour éviter la dérive
    let target_cap = device_rate as usize * dev_ch * 2;

    for frame in rx {
        // ── Backpressure : ne pas accumuler plus que la cible ──
        // Attend que le ring ait de la place avant de pusher
        loop {
            let occupied = producer.len();
            if occupied < target_cap { break; }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let in_ch   = frame.channels as usize;
        let in_rate = frame.sample_rate;

        // Downmix surround → stéréo
        let stereo = if in_ch == dev_ch {
            frame.samples.clone()
        } else {
            downmix_to_stereo(&frame.samples, in_ch)
        };

        // Resampling si taux différents
        let final_samples = if in_rate == device_rate {
            stereo
        } else {
            let out_ch   = dev_ch.min(2);
            let needs_new = resampler.as_ref()
                .map(|r| r.in_rate() != in_rate || r.out_rate() != device_rate)
                .unwrap_or(true);
            if needs_new {
                resampler = AudioResampler::new(in_rate, device_rate, out_ch).ok();
            }
            if let Some(r) = &mut resampler {
                r.process_interleaved(&stereo).unwrap_or(stereo)
            } else {
                stereo
            }
        };

        // Push dans le ring lock-free
        let pushed = producer.push_slice(&final_samples);
        let occupied = producer.len();
        ring_level.store(occupied as u64, Ordering::Relaxed);

        if pushed < final_samples.len() {
            log::debug!("audio ring overflow: dropped {} samples", final_samples.len() - pushed);
        }
    }
}

// ─── Downmix N canaux → stéréo ───────────────────────────────────────────────

fn downmix_to_stereo(samples: &[f32], in_ch: usize) -> Vec<f32> {
    if in_ch == 0 { return Vec::new(); }
    let frames = samples.len() / in_ch;
    let mut out = Vec::with_capacity(frames * 2);

    for f in 0..frames {
        let b = f * in_ch;
        let (l, r) = match in_ch {
            1 => { let m = samples[b]; (m, m) }
            2 => (samples[b], samples[b + 1]),
            6 => {
                let (fl, fr, fc, bl, br) = (samples[b], samples[b+1], samples[b+2], samples[b+4], samples[b+5]);
                ((fl + fc*0.707 + bl*0.707).clamp(-1.0, 1.0),
                 (fr + fc*0.707 + br*0.707).clamp(-1.0, 1.0))
            }
            8 => {
                let (fl, fr, fc, bl, br, sl, sr) =
                    (samples[b], samples[b+1], samples[b+2], samples[b+4], samples[b+5], samples[b+6], samples[b+7]);
                ((fl + fc*0.707 + bl*0.5 + sl*0.707).clamp(-1.0, 1.0),
                 (fr + fc*0.707 + br*0.5 + sr*0.707).clamp(-1.0, 1.0))
            }
            n => {
                let (mut ls, mut rs) = (0f32, 0f32);
                for ch in 0..n {
                    if ch % 2 == 0 { ls += samples[b + ch]; }
                    else           { rs += samples[b + ch]; }
                }
                let h = (n / 2).max(1) as f32;
                ((ls / h).clamp(-1.0, 1.0), (rs / h).clamp(-1.0, 1.0))
            }
        };
        out.push(l);
        out.push(r);
    }
    out
}
