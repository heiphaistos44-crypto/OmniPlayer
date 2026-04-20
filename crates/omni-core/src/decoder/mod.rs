pub mod audio;
pub mod context;
pub mod subtitle;
pub mod video;

pub use audio::DecodedAudioFrame;
pub use video::DecodedVideoFrame;

/// Format pixel brut livré au renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Yuv420p,   // planar YUV 4:2:0 — le plus courant
    Yuv422p,   // planar YUV 4:2:2
    Yuv444p,   // planar YUV 4:4:4
    Nv12,      // semi-planar NV12 (HW accel output)
    P010Le,    // 10-bit HDR (NV12 10-bit)
    Rgba,      // fallback RGBA
}
