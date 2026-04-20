use anyhow::{Context as _, Result};
use rubato::{FftFixedIn, Resampler};

/// Convertisseur de fréquence d'échantillonnage basé sur rubato (FFT).
/// Utilisé quand CPAL exige un taux différent de celui du décodeur.
pub struct AudioResampler {
    resampler: FftFixedIn<f32>,
    in_rate:   u32,
    out_rate:  u32,
    channels:  usize,
    chunk_sz:  usize,
    /// Buffer d'entrée accumulé (par canal).
    in_bufs:   Vec<Vec<f32>>,
}

impl AudioResampler {
    /// Crée un resampler in_rate → out_rate pour `channels` canaux.
    pub fn new(in_rate: u32, out_rate: u32, channels: usize) -> Result<Self> {
        if in_rate == out_rate {
            // Pas de conversion nécessaire — crée quand même l'objet par cohérence.
        }

        let chunk_sz   = 1024usize;
        let resampler  = FftFixedIn::<f32>::new(
            in_rate  as usize,
            out_rate as usize,
            chunk_sz,
            2,        // sub chunks
            channels,
        )
        .context("création FftFixedIn")?;

        Ok(Self {
            resampler,
            in_rate,
            out_rate,
            channels,
            chunk_sz,
            in_bufs: vec![Vec::new(); channels],
        })
    }

    /// Entrée : échantillons interleaved (LRLR...).
    /// Sortie : échantillons interleaved convertis.
    pub fn process_interleaved(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        // Dé-interleave → planar
        for ch in 0..self.channels {
            for frame in input.chunks_exact(self.channels) {
                self.in_bufs[ch].push(frame[ch]);
            }
        }

        let mut out_interleaved = Vec::new();

        // Traite par chunks de chunk_sz frames
        while self.in_bufs[0].len() >= self.chunk_sz {
            let chunk: Vec<Vec<f32>> = self
                .in_bufs
                .iter_mut()
                .map(|b| b.drain(..self.chunk_sz).collect())
                .collect();

            let out_planes = self
                .resampler
                .process(&chunk.iter().map(|v| v.as_slice()).collect::<Vec<_>>(), None)
                .context("rubato process")?;

            // Re-interleave → LRLR...
            let n_frames = out_planes[0].len();
            for f in 0..n_frames {
                for ch in 0..self.channels {
                    out_interleaved.push(out_planes[ch][f]);
                }
            }
        }

        Ok(out_interleaved)
    }

    pub fn in_rate(&self)  -> u32 { self.in_rate  }
    pub fn out_rate(&self) -> u32 { self.out_rate  }
    pub fn passthrough(&self) -> bool { self.in_rate == self.out_rate }
}
