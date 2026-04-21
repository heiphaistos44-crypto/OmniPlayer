use anyhow::{Context as _, Result};
use ffmpeg_next as ffmpeg;
use ffmpeg::software::resampling::context::Context as SwrContext;

/// Frame audio décodée — échantillons f32 packed interleaved, canaux natifs.
#[derive(Clone)]
pub struct DecodedAudioFrame {
    pub pts_secs:    f64,
    pub samples:     Vec<f32>,   // interleaved, `channels` canaux
    pub sample_rate: u32,
    pub channels:    u8,
}

/// Décodeur audio.
/// Sort en f32 packed, taux natif de la source, canaux natifs (1/2/6/8…).
/// Le moteur audio (omni-audio) fait le downmix + resampling vers le périphérique.
pub struct AudioDecoder {
    decoder:     ffmpeg::codec::decoder::Audio,
    resampler:   Option<SwrContext>,
    time_base:   f64,
    src_rate:    u32,
    src_layout:  ffmpeg::channel_layout::ChannelLayout,
    src_channels: u8,
}

impl AudioDecoder {
    pub fn new(decoder: ffmpeg::codec::decoder::Audio, time_base: f64) -> Result<Self> {
        let src_rate    = decoder.rate();
        let src_layout  = decoder.channel_layout();
        let src_channels = decoder.channels() as u8;
        Ok(Self { decoder, resampler: None, time_base, src_rate, src_layout, src_channels })
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

        let pts_secs = raw.pts()
            .map(|p| p as f64 * self.time_base)
            .unwrap_or(0.0);

        // Conversion vers f32 packed, même taux, même layout
        let resampler = match &mut self.resampler {
            Some(r) => r,
            None => {
                let r = SwrContext::get(
                    raw.format(),
                    self.src_layout,
                    self.src_rate,
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    self.src_layout,   // même layout — le downmix est dans AudioEngine
                    self.src_rate,
                )
                .context("création SwrContext")?;
                self.resampler = Some(r);
                self.resampler.as_mut().unwrap()
            }
        };

        let mut resampled = ffmpeg::util::frame::audio::Audio::empty();
        resampler.run(&raw, &mut resampled).context("resampling audio")?;

        let samples = audio_frame_to_f32(&resampled);

        Ok(Some(DecodedAudioFrame {
            pts_secs,
            samples,
            sample_rate: self.src_rate,
            channels:    self.src_channels,
        }))
    }

    pub fn sample_rate(&self) -> u32 { self.src_rate }
    pub fn channels(&self)    -> u8  { self.src_channels }
}

fn audio_frame_to_f32(frame: &ffmpeg::util::frame::audio::Audio) -> Vec<f32> {
    let data = frame.data(0);
    let n = data.len() / std::mem::size_of::<f32>();
    let mut out = vec![0f32; n];
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        out[i] = f32::from_le_bytes(chunk.try_into().unwrap());
    }
    out
}
