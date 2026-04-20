pub mod decoder;
pub mod hw_accel;
pub mod pipeline;
pub mod probe;

pub use decoder::{DecodedAudioFrame, DecodedVideoFrame};
pub use pipeline::{MediaPipeline, PipelineCommand, PipelineEvent};

/// Formats vidéo supportés — liste non-exhaustive (FFmpeg gère tout le reste).
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    // Conteneurs
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ts", "m2ts",
    "mts", "mpg", "mpeg", "m4v", "3gp", "3g2", "ogv", "rm", "rmvb",
    "divx", "xvid", "vob", "ifo", "iso", "f4v", "asf", "mxf", "dv",
    // Formats audio vidéo
    "mp3", "aac", "flac", "ogg", "opus", "wav", "wma", "m4a", "ape",
    "mka", "mpa",
    // Streaming
    "m3u8", "mpd",
];

/// Codec hardware supporté détecté au runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HwAccelKind {
    None,
    Dxva2,
    D3D11Va,
    NvDec,
    AmfDec,
    QuickSync,
    Vulkan,
}

/// Résolution vidéo avec classification qualité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width:  u32,
    pub height: u32,
}

impl Resolution {
    pub fn quality_label(&self) -> &'static str {
        match (self.width, self.height) {
            (w, h) if w >= 7680 || h >= 4320 => "8K",
            (w, h) if w >= 3840 || h >= 2160 => "4K UHD",
            (w, h) if w >= 2560 || h >= 1440 => "1440p",
            (w, h) if w >= 1920 || h >= 1080 => "1080p",
            (w, h) if w >= 1280 || h >= 720  => "720p",
            (w, h) if w >= 854  || h >= 480  => "480p",
            _                                 => "SD",
        }
    }
}
