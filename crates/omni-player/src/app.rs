use eframe::{CreationContext, Frame};
use egui::{CentralPanel, Context, Id, Key, Order};
use std::sync::Arc;
use parking_lot::Mutex;

use crate::config::AppConfig;
use crate::player::{Player, PlayerState};
use crate::services::ServicesClient;
use crate::ui::{controls, file_browser, info_overlay, player_view, playlist, settings, url_dialog};
use crate::ui::image_viewer::ImageViewer;
use crate::video_callback::SharedFrame;
use omni_audio::AudioEngine;
use omni_renderer::VideoRenderer;

const ACCENT: egui::Color32 = egui::Color32::from_rgb(74, 158, 255);

struct Osd { text: String, expires_at: f64 }

pub struct OmniApp {
    player:            Player,
    audio:             Option<AudioEngine>,
    config:            AppConfig,
    show_settings:     bool,
    show_playlist:     bool,
    show_file_browser: bool,
    show_url_dialog:   bool,
    show_info:         bool,
    url_input:         String,
    is_fullscreen:     bool,
    playlist_items:    Vec<String>,
    playlist_idx:      Option<usize>,
    seek_request:      Option<f64>,
    video_frame:       SharedFrame,
    osd:               Option<Osd>,
    #[allow(dead_code)] services: Option<ServicesClient>,
    // Auto-hide controls
    last_mouse_move:   f64,
    // Image viewer
    image_viewer:      ImageViewer,
    image_texture:     Option<egui::TextureHandle>,
    image_path_loaded: String,
}

impl OmniApp {
    pub fn new(cc: &CreationContext, config: AppConfig) -> Self {
        Self::apply_theme(&cc.egui_ctx);

        if let Some(rs) = cc.wgpu_render_state.as_ref() {
            match VideoRenderer::new(&rs.device, rs.target_format) {
                Ok(r)  => { rs.renderer.write().callback_resources.insert(r); }
                Err(e) => log::error!("VideoRenderer init: {e}"),
            }
        }

        let audio = AudioEngine::new()
            .map_err(|e| log::error!("AudioEngine: {e}")).ok();

        let svc = ServicesClient::new(config.subtitle_service_port, config.media_indexer_port);
        let services = if svc.is_subtitle_service_up() { Some(svc) } else { None };

        Self {
            player: Player::new(), audio, config,
            show_settings: false, show_playlist: false,
            show_file_browser: false, show_url_dialog: false,
            show_info: false,
            url_input: String::new(), is_fullscreen: false,
            playlist_items: Vec::new(), playlist_idx: None, seek_request: None,
            video_frame: Arc::new(Mutex::new(None)),
            osd: None, services,
            last_mouse_move: 0.0,
            image_viewer: ImageViewer::default(),
            image_texture: None,
            image_path_loaded: String::new(),
        }
    }

    fn apply_theme(ctx: &Context) {
        let mut v = egui::Visuals::dark();
        v.window_corner_radius      = egui::CornerRadius::from(10.0_f32);
        v.panel_fill                = egui::Color32::from_rgb(10, 10, 16);
        v.window_fill               = egui::Color32::from_rgb(16, 16, 24);
        v.window_shadow             = egui::Shadow {
            offset: [0, 4].into(),
            blur: 20,
            spread: 0,
            color: egui::Color32::from_black_alpha(120),
        };
        v.widgets.inactive.bg_fill  = egui::Color32::from_rgb(26, 26, 38);
        v.widgets.inactive.corner_radius = egui::CornerRadius::from(5.0_f32);
        v.widgets.hovered.bg_fill   = egui::Color32::from_rgb(36, 46, 72);
        v.widgets.hovered.corner_radius  = egui::CornerRadius::from(5.0_f32);
        v.widgets.active.bg_fill    = egui::Color32::from_rgb(55, 100, 200);
        v.widgets.active.corner_radius   = egui::CornerRadius::from(5.0_f32);
        v.selection.bg_fill         = egui::Color32::from_rgb(40, 80, 160);
        v.hyperlink_color           = ACCENT;
        v.override_text_color       = Some(egui::Color32::from_gray(225));
        ctx.set_visuals(v);
    }

    fn controls_visible(&self, now: f64) -> bool {
        !matches!(self.player.state, PlayerState::Playing)
            || (now - self.last_mouse_move) < 3.0
    }

    fn open_file(&mut self, path: String) {
        log::info!("ouverture: {path}");
        self.try_load_adjacent_subtitle(&path);
        self.config.add_recent(&path);
        // Reset image viewer pour nouvelle image
        self.image_viewer.reset();
        self.image_texture = None;
        self.image_path_loaded.clear();
        if let Err(e) = self.player.open(&path) {
            log::error!("player.open: {e}");
            self.player.state = PlayerState::Error(e.to_string());
        }
    }

    fn try_load_adjacent_subtitle(&mut self, media_path: &str) {
        if omni_core::is_image_path(media_path) { return; }
        let base = std::path::Path::new(media_path).with_extension("");
        for ext in &["srt", "ass", "ssa"] {
            let sub = base.with_extension(ext);
            if sub.exists() {
                if self.player.load_subtitle(&sub.to_string_lossy()).is_ok() {
                    let name = sub.file_name().unwrap_or_default().to_string_lossy().to_string();
                    self.set_osd(format!("Sous-titre : {name}"));
                }
                break;
            }
        }
    }

    fn set_osd(&mut self, text: impl Into<String>) {
        self.osd = Some(Osd { text: text.into(), expires_at: 0.0 });
    }

    fn osd_text(&self, now: f64) -> Option<&str> {
        self.osd.as_ref()
            .filter(|o| o.expires_at == 0.0 || o.expires_at > now)
            .map(|o| o.text.as_str())
    }

    fn process_seek(&mut self) {
        if let Some(pos) = self.seek_request.take() { self.player.seek(pos); }
    }

    fn pump_audio(&mut self) {
        let Some(audio) = &self.audio else { return };
        while let Some(frame) = self.player.try_recv_audio_frame() {
            audio.push_frame(frame);
        }
        audio.set_paused(self.player.state == PlayerState::Paused);
        audio.set_volume(self.player.effective_volume());
    }

    fn pump_video(&mut self) {
        while let Some(frame) = self.player.try_recv_video_frame() {
            *self.video_frame.lock() = Some(frame);
        }
    }

    fn ensure_image_texture(&mut self, ctx: &Context) {
        if let Some(img) = &self.player.image_frame {
            if self.image_path_loaded != img.path {
                let color_img = egui::ColorImage::from_rgba_unmultiplied(
                    [img.width as usize, img.height as usize],
                    &img.pixels,
                );
                self.image_texture = Some(ctx.load_texture(
                    "image_viewer",
                    color_img,
                    egui::TextureOptions::LINEAR,
                ));
                self.image_path_loaded = img.path.clone();
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &Context, now: f64) {
        if ctx.wants_keyboard_input() { return; }

        let keys = ctx.input(|i| {
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::F),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::S),
                i.key_pressed(Key::A),
                i.key_pressed(Key::M),
                i.key_pressed(Key::N),
                i.key_pressed(Key::P),
                i.key_pressed(Key::I),
                i.modifiers.ctrl && i.key_pressed(Key::O),
                i.modifiers.ctrl && i.key_pressed(Key::L),
                i.modifiers.ctrl && i.key_pressed(Key::P),
                i.modifiers.ctrl && i.key_pressed(Key::Q),
            )
        });
        let (k_space, k_left, k_right, k_up, k_down,
             k_f, k_esc, k_s, k_a, k_m, k_n, k_p, k_i,
             k_ctrl_o, k_ctrl_l, k_ctrl_p, k_ctrl_q) = keys;

        if k_space  { self.player.play_pause(); }
        if k_left   { self.player.seek_relative(-10.0); self.set_osd("−10 s"); self.last_mouse_move = now; }
        if k_right  { self.player.seek_relative(10.0);  self.set_osd("+10 s");  self.last_mouse_move = now; }
        if k_up     {
            let v = (self.player.volume + 0.1).min(2.0);
            self.player.set_volume(v);
            self.set_osd(format!("Volume {:.0}%", v * 100.0));
        }
        if k_down   {
            let v = (self.player.volume - 0.1).max(0.0);
            self.player.set_volume(v);
            self.set_osd(format!("Volume {:.0}%", v * 100.0));
        }
        if k_m      {
            self.player.toggle_mute();
            self.set_osd(if self.player.muted { "Muet" } else { "Son actif" });
        }
        if k_s      { self.player.next_subtitle_track(); }
        if k_a      { self.player.next_audio_track(); }
        if k_n      { self.playlist_next(); }
        if k_p      { self.playlist_prev(); }
        if k_i      { self.show_info = !self.show_info; }
        if k_ctrl_o { self.show_file_browser = true; }
        if k_ctrl_l { self.show_url_dialog   = true; }
        if k_ctrl_p { self.show_playlist     = !self.show_playlist; }
        if k_ctrl_q { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }

        if k_f {
            self.is_fullscreen = !self.is_fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
        }
        if k_esc && self.is_fullscreen {
            self.is_fullscreen = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
    }

    fn handle_drop(&mut self, ctx: &Context) {
        let dropped: Vec<String> = ctx.input(|i|
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.as_ref().map(|p| p.to_string_lossy().to_string()))
                .collect()
        );
        for path in dropped {
            let is_sub = path.ends_with(".srt") || path.ends_with(".ass")
                      || path.ends_with(".ssa") || path.ends_with(".vtt");
            if is_sub {
                if self.player.load_subtitle(&path).is_ok() { self.set_osd("Sous-titre chargé"); }
            } else {
                if !self.playlist_items.contains(&path) { self.playlist_items.push(path.clone()); }
                if self.playlist_idx.is_none() {
                    let idx = self.playlist_items.len() - 1;
                    self.playlist_idx = Some(idx);
                    self.open_file(path);
                }
            }
        }
    }

    fn playlist_next(&mut self) {
        if let Some(idx) = self.playlist_idx {
            let next = idx + 1;
            if next < self.playlist_items.len() {
                let path = self.playlist_items[next].clone();
                self.playlist_idx = Some(next);
                self.open_file(path);
            }
        }
    }

    fn playlist_prev(&mut self) {
        if let Some(idx) = self.playlist_idx {
            if idx > 0 {
                let prev = idx - 1;
                let path = self.playlist_items[prev].clone();
                self.playlist_idx = Some(prev);
                self.open_file(path);
            }
        }
    }
}

impl eframe::App for OmniApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        let now = ctx.input(|i| i.time);

        // Tracking mouvement souris pour auto-hide
        if ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO || i.pointer.any_pressed()) {
            self.last_mouse_move = now;
        }

        // OSD expiration
        if let Some(osd) = &mut self.osd {
            if osd.expires_at == 0.0 { osd.expires_at = now + 2.5; }
        }

        // Pipeline
        self.player.poll_events();
        self.process_seek();
        self.pump_audio();
        self.pump_video();
        self.ensure_image_texture(ctx);

        if self.player.state == PlayerState::EndOfFile { self.playlist_next(); }

        // Entrées clavier
        self.handle_keyboard(ctx, now);
        self.handle_drop(ctx);

        // Drag-over highlight
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::new("drop_hint")))
                .rect_stroke(
                    ctx.screen_rect(), 0.0,
                    egui::Stroke::new(3.0, ACCENT),
                    egui::StrokeKind::Middle,
                );
        }

        let controls_vis = self.controls_visible(now);

        // ── Menu bar (masqué en plein écran quand contrôles cachés) ────────
        let show_menu = !self.is_fullscreen || controls_vis;
        if show_menu {
            egui::TopBottomPanel::top("top_bar")
                .frame(egui::Frame::new()
                    .fill(egui::Color32::from_rgb(12, 12, 18))
                    .inner_margin(egui::Margin::symmetric(4, 2)))
                .show(ctx, |ui| {
                    self.draw_menu_bar(ui, now);
                });
        }

        // ── Playlist ──────────────────────────────────────────────────────
        if self.show_playlist {
            let mut play_path: Option<String> = None;
            egui::SidePanel::right("playlist_panel")
                .resizable(true).default_width(270.0)
                .frame(egui::Frame::new()
                    .fill(egui::Color32::from_rgb(12, 12, 20))
                    .inner_margin(egui::Margin::same(8)))
                .show(ctx, |ui| {
                    playlist::show(ui, &mut self.playlist_items, &mut self.playlist_idx,
                        |path| play_path = Some(path));
                });
            if let Some(path) = play_path { self.open_file(path); }
        }

        // ── Zone centrale (vidéo) ─────────────────────────────────────────
        CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let osd = self.osd_text(now).map(|s| s.to_string());
                player_view::show(
                    ui, &self.player,
                    Arc::clone(&self.video_frame),
                    osd.as_deref(),
                    self.image_texture.as_ref(),
                    &mut self.image_viewer,
                );
            });

        // ── Contrôles overlay (auto-hide) ─────────────────────────────────
        if controls_vis {
            let screen = ctx.screen_rect();
            egui::Area::new(Id::new("controls_overlay"))
                .fixed_pos(egui::pos2(screen.left(), screen.bottom() - 92.0))
                .order(Order::Foreground)
                .show(ctx, |ui| {
                    ui.set_width(screen.width());
                    controls::show(ui, &mut self.player, &mut self.seek_request);
                });
        }

        // ── Info overlay ──────────────────────────────────────────────────
        if self.show_info {
            if let Some(info) = &self.player.media_info.clone() {
                let is_hdr = info.video.as_ref().map(|v| v.hdr).unwrap_or(false);
                info_overlay::show(ctx, info, is_hdr);
            }
        }

        // ── Modaux ────────────────────────────────────────────────────────
        if self.show_file_browser {
            let mut fb_open: Option<String> = None;
            file_browser::show(ctx, &mut self.show_file_browser, |path| {
                fb_open = Some(path);
            });
            if let Some(path) = fb_open {
                if !self.playlist_items.contains(&path) { self.playlist_items.push(path.clone()); }
                if let Some(idx) = self.playlist_items.iter().position(|x| x == &path) {
                    self.playlist_idx = Some(idx);
                }
                self.open_file(path);
            }
        }

        if self.show_url_dialog {
            if let Some(url) = url_dialog::show(ctx, &mut self.show_url_dialog, &mut self.url_input) {
                self.open_file(url);
                self.url_input.clear();
            }
        }

        if self.show_settings {
            settings::show(ctx, &mut self.show_settings, &mut self.config);
        }

        if self.player.is_active() { ctx.request_repaint(); }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.player.stop();
        self.config.save();
    }
}

impl OmniApp {
    fn draw_menu_bar(&mut self, ui: &mut egui::Ui, _now: f64) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("Fichier", |ui| {
                if ui.button("📂 Ouvrir…  Ctrl+O").clicked() {
                    self.show_file_browser = true; ui.close_menu();
                }
                if ui.button("🔗 Ouvrir URL…  Ctrl+L").clicked() {
                    self.show_url_dialog = true; ui.close_menu();
                }
                ui.separator();
                ui.menu_button("🕐 Récents", |ui| {
                    let recents = self.config.recent_files.clone();
                    for f in &recents {
                        let name = std::path::Path::new(f)
                            .file_name().map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| f.clone());
                        if ui.button(name).on_hover_text(f).clicked() {
                            let p = f.clone(); self.open_file(p); ui.close_menu();
                        }
                    }
                    if recents.is_empty() {
                        ui.label(egui::RichText::new("(vide)").color(egui::Color32::from_gray(110)));
                    }
                });
                ui.separator();
                if ui.button("💬 Charger sous-titre…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Sous-titres", &["srt", "ass", "ssa", "vtt"])
                        .pick_file()
                    {
                        let s = p.to_string_lossy().to_string();
                        if self.player.load_subtitle(&s).is_ok() { self.set_osd("Sous-titre chargé"); }
                    }
                    ui.close_menu();
                }
                if ui.button("✕ Effacer sous-titre").clicked() {
                    self.player.clear_subtitle(); ui.close_menu();
                }
                ui.separator();
                if ui.button("⏻ Quitter  Ctrl+Q").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Vue", |ui| {
                ui.checkbox(&mut self.show_playlist, "Playlist  Ctrl+P");
                if ui.checkbox(&mut self.show_info, "Infos média  I").changed() {}
                ui.separator();
                let fs_label = if self.is_fullscreen { "🗗 Quitter plein écran  F" } else { "⛶ Plein écran  F" };
                if ui.button(fs_label).clicked() {
                    self.is_fullscreen = !self.is_fullscreen;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
                    ui.close_menu();
                }
            });

            ui.menu_button("Lecture", |ui| {
                if ui.button("▶/⏸  Espace").clicked()          { self.player.play_pause(); ui.close_menu(); }
                if ui.button("⏮  Ch. précédent").clicked()      { self.player.chapter_prev(); ui.close_menu(); }
                if ui.button("⏭  Ch. suivant").clicked()        { self.player.chapter_next(); ui.close_menu(); }
                ui.separator();
                if ui.button("🎵  Piste audio  A").clicked()    { self.player.next_audio_track(); ui.close_menu(); }
                if ui.button("💬  Sous-titres  S").clicked()    { self.player.next_subtitle_track(); ui.close_menu(); }
                if ui.button("🔇  Muet  M").clicked()           { self.player.toggle_mute(); ui.close_menu(); }
            });

            ui.menu_button("Outils", |ui| {
                if ui.button("⚙ Paramètres").clicked() {
                    self.show_settings = true; ui.close_menu();
                }
            });

            // Droite : résolution + codec + HDR + titre
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("OmniPlayer").color(ACCENT).strong().size(13.0));
                if let Some(info) = &self.player.media_info {
                    if let Some(v) = &info.video {
                        ui.separator();
                        let res = omni_core::Resolution { width: v.width, height: v.height };
                        ui.label(egui::RichText::new(res.quality_label())
                            .color(egui::Color32::from_rgb(80, 210, 80)).small());
                        ui.label(egui::RichText::new(
                            format!("{}×{}", v.width, v.height))
                            .small().color(egui::Color32::from_gray(145)));
                        if v.fps > 0.0 {
                            ui.label(egui::RichText::new(format!("{:.0}fps", v.fps))
                                .small().color(egui::Color32::from_gray(130)));
                        }
                        if !v.codec_name.is_empty() {
                            ui.label(egui::RichText::new(v.codec_name.to_uppercase())
                                .small().color(egui::Color32::from_gray(145)));
                        }
                        if v.hdr {
                            ui.label(egui::RichText::new("HDR")
                                .color(egui::Color32::from_rgb(255, 160, 40)).small());
                        }
                    }
                }
            });
        });
    }
}
