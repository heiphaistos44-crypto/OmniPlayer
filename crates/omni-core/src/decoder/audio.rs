use anyhow::{Context as _, Result};
use ffmpeg_next as ffmpeg;
use ffmpeg::software::resampling::context::Context as SwrContext;

/// Frame audio décodée — échantillons interleaved f32 stéréo.
#[derive(Clone)]
pub struct DecodedAudioFrame {
    pub pts_secs:    f64,
    pub samples:     Vec<f32>,   // stéréo interleaved LRLRLR...
    pub sample_rate: u32,
    pub channels:    u8,
}

/// Décodeur audio avec conversion vers f32 stéréo 48 kHz.
pub struct AudioDecoder {
    decoder:     ffmpeg::codec::decoder::Audio,
    resampler:   Option<SwrContext>,
    time_base:   f64,
    out_rate:    u32,
    out_channels: ffmpeg::channel_layout::ChannelLayout,
}

impl AudioDecoder {
    pub fn new(decoder: ffmpeg::codec::decoder::Audio, time_base: f64) -> Result<Self> {
        Ok(Self {
            decoder,
            resampler: None,
            time_base,
            out_rate: 48_000,
            out_channels: ffmpeg::channel_layout::ChannelLayout::STEREO,
        })
    }

    pub fn send_packet(&mut self, packet: &ffmpeg::Packet) -> Result<()> {
        self.decoder.send_packet(packet).context("send_packet audio")
    }

    pub fn send_eof(&mut self) -> Result<()> {
        self.decoder.send_eof().context("send_eof audio")
    }

    pub fn receive_frame(&mut self) -> Result<Option<DecodedAudioFrame>> {
        let mut raw = ffmpeg::util::frame::audio::Audio::empty();
        match self.decoder.receive_frame(&mut raw) {
            Ok(()) => {}
            Err(ffmpeg::Error::Other { errno: ffmpeg::error::EAGAIN }) => return Ok(None),
            Err(e) => return Err(e).context("receive_frame audio"),
        }

        let pts_secs = raw
            .pts()
            .map(|p| p as f64 * self.time_base)
            .unwrap_or(0.0);

        // Initialise le resampler à la première frame
        let resampler = match &mut self.resampler {
            Some(r) => r,
            None => {
                let r = ffmpeg::software::resampling::context::Context::get(
                    raw.format(),
                    raw.channel_layout(),
                    raw.rate(),
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    self.out_channels,
                    self.out_rate,
                )
                .context("création SwrContext")?;
                self.resampler = Some(r);
                self.resampler.as_mut().unwrap()
            }
        };

        let mut resampled = ffmpeg::util::frame::audio::Audio::empty();
        resampler
            .run(&raw, &mut resampled)
            .context("resampling audio")?;

        let samples = audio_frame_to_f32(&resampled);

        Ok(Some(DecodedAudioFrame {
            pts_secs,
            samples,
            sample_rate: self.out_rate,
            channels: 2,
        }))
    }
}

/// Extrait les échantillons f32 packed (stéréo interleaved) d'une frame.
fn audio_frame_to_f32(frame: &ffmpeg::util::frame::audio::Audio) -> Vec<f32> {
    let data = frame.data(0);
    // data contient des f32 en little-endian packed
    let n = data.len() / std::mem::size_of::<f32>();
    let mut out = vec![0f32; n];
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        out[i] = f32::from_le_bytes(chunk.try_into().unwrap());
    }
    out
}
