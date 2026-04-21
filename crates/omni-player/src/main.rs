mod app;
mod config;
mod player;
mod services;
mod ui;
mod video_callback;

use anyhow::Result;
use eframe::{NativeOptions, egui::ViewportBuilder};
use std::sync::Arc;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("OmniPlayer v{}", env!("CARGO_PKG_VERSION"));

    // Charge la config utilisateur
    let cfg = config::AppConfig::load();

    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("OmniPlayer")
            .with_inner_size([cfg.window_width as f32, cfg.window_height as f32])
            .with_min_inner_size([640.0, 400.0])
            .with_icon(load_icon()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "OmniPlayer",
        options,
        Box::new(|cc| Ok(Box::new(app::OmniApp::new(cc, cfg)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

fn load_icon() -> Arc<egui::IconData> {
    Arc::new(egui::IconData {
        rgba:   vec![0u8; 32 * 32 * 4],
        width:  32,
        height: 32,
    })
}
