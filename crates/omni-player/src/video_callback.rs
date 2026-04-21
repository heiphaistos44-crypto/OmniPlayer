use std::sync::Arc;
use parking_lot::Mutex;
use omni_core::decoder::DecodedVideoFrame;
use omni_renderer::VideoRenderer;

// Use eframe's re-exported wgpu and egui_wgpu to avoid duplicate crate instances
use eframe::{egui_wgpu, wgpu};

pub type SharedFrame = Arc<Mutex<Option<DecodedVideoFrame>>>;

/// Callback egui_wgpu : upload la dernière frame YUV vers le GPU et encode le rendu.
pub struct VideoPaintCallback {
    pub frame: SharedFrame,
}

impl egui_wgpu::CallbackTrait for VideoPaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _enc: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<VideoRenderer>() {
            if let Some(frame) = self.frame.lock().take() {
                renderer.upload_frame(device, queue, &frame);
            }
        }
        vec![]
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        rp: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<VideoRenderer>() {
            renderer.render(rp);
        }
    }
}
