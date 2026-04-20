use anyhow::{Context as _, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, Receiver, Sender};
use omni_core::decoder::DecodedAudioFrame;
use parking_lot::Mutex;
use std::sync::Arc;

/// Tampon circulaire d'échantillons audio f32 stéréo.
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

/// Moteur audio — sortie CPAL avec ring buffer thread-safe.
pub struct AudioEngine {
    _stream: cpal::Stream,
    sender:  Sender<DecodedAudioFrame>,
    volume:  Arc<Mutex<f32>>,
    paused:  Arc<Mutex<bool>>,
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

        let sample_rate = config.sample_rate().0;
        let channels    = config.channels() as usize;

        let ring    = Arc::new(Mutex::new(RingBuffer::new(sample_rate as usize * channels * 2)));
        let volume  = Arc::new(Mutex::new(1.0f32));
        let paused  = Arc::new(Mutex::new(false));

        let ring_w  = ring.clone();
        let vol_w   = volume.clone();
        let paused_w = paused.clone();

        let (tx, rx) = bounded::<DecodedAudioFrame>(64);

        // Thread de remplissage du ring buffer
        let ring_fill = ring.clone();
        std::thread::Builder::new()
            .name("audio-fill".into())
            .spawn(move || fill_ring(rx, ring_fill))?;

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _| {
                        if *paused_w.lock() {
                            data.fill(0.0);
                            return;
                        }
                        ring_w.lock().pop_slice(data);
                        let vol = *vol_w.lock();
                        for s in data.iter_mut() { *s *= vol; }
                    },
                    |e| log::error!("cpal error: {e}"),
                    None,
                )?
            }
            fmt => anyhow::bail!("format audio non géré: {fmt:?}"),
        };

        stream.play().context("play stream audio")?;

        Ok(Self { _stream: stream, sender: tx, volume, paused })
    }

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

fn fill_ring(rx: Receiver<DecodedAudioFrame>, ring: Arc<Mutex<RingBuffer>>) {
    for frame in rx {
        ring.lock().push_slice(&frame.samples);
    }
}
