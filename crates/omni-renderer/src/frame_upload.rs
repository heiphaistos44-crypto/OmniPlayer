use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};
use omni_core::decoder::{DecodedVideoFrame, PixelFormat};

/// Textures GPU pour un frame YUV420p.
pub struct YuvTextures {
    pub y:  Texture,
    pub u:  Texture,
    pub v:  Texture,
    pub width:  u32,
    pub height: u32,
}

impl YuvTextures {
    /// Alloue ou ré-alloue les textures si la résolution change.
    pub fn ensure(
        current: Option<Self>,
        device:  &Device,
        w: u32,
        h: u32,
    ) -> Self {
        if let Some(t) = current {
            if t.width == w && t.height == h { return t; }
        }
        let make = |lw: u32, lh: u32| {
            device.create_texture(&TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count:    1,
                dimension:       TextureDimension::D2,
                format:          TextureFormat::R8Unorm,
                usage:           TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats:    &[],
            })
        };
        Self { y: make(w, h), u: make(w / 2, h / 2), v: make(w / 2, h / 2), width: w, height: h }
    }

    /// Upload un frame décodé vers les textures GPU.
    pub fn upload(&self, queue: &Queue, frame: &DecodedVideoFrame) {
        let upload = |tex: &Texture, data: &[u8], stride: usize, w: u32, h: u32| {
            queue.write_texture(
                tex.as_image_copy(),
                data,
                wgpu::TexelCopyBufferLayout {
                    offset:         0,
                    bytes_per_row:  Some(stride as u32),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        };

        match frame.format {
            PixelFormat::Yuv420p => {
                upload(&self.y, &frame.planes[0], frame.strides[0], self.width, self.height);
                upload(&self.u, &frame.planes[1], frame.strides[1], self.width / 2, self.height / 2);
                upload(&self.v, &frame.planes[2], frame.strides[2], self.width / 2, self.height / 2);
            }
            _ => {
                log::warn!("format {:?} non géré dans upload YUV", frame.format);
            }
        }
    }
}
