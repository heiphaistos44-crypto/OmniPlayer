use eframe::{CreationContext, Frame};
use egui::{CentralPanel, Context, TopBottomPanel};

use crate::config::AppConfig;
use crate::player::{Player, PlayerState};
use crate::ui::{controls, file_browser, playlist, player_view, settings};
use omni_audio::AudioEngine;

pub struct OmniApp {
    player:          Player,
    audio:           Option<AudioEngine>,
    config:          AppConfig,
    show_settings:   bool,
    show_playlist:   bool,
    show_file_browser: bool,
    playlist_items:  Vec<String>,
    playlist_idx:    Option<usize>,
    seek_request:    Option<f64>,
}

impl OmniApp {
    pub fn new(cc: &CreationContext, config: AppConfig) -> Self {
        // Style sombre cinéma
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding    = egui::Rounding::same(8.0);
        visuals.panel_fill         = egui::Color32::from_rgb(18, 18, 24);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 30, 40);
        cc.egui_ctx.set_visuals(visuals);

        let audio = AudioEngine::new()
            .map_err(|e| log::error!("audio engine: {e}"))
            .ok();

        Self {
            player:           Player::new(),
            audio,
            config,
            show_settings:    false,
            show_playlist:    false,
            show_file_browser: false,
            playlist_items:   Vec::new(),
            playlist_idx:     None,
            seek_request:     None,
        }
    }

    fn open_file(&mut self, path: String) {
        log::info!("ouverture: {path}");
        self.config.add_recent(&path);

        if let Err(e) = self.player.open(&path) {
            log::error!("player.open: {e}");
            self.player.state = PlayerState::Error(e.to_string());
        }
    }

    fn process_seek(&mut self) {
        if let Some(pos) = self.seek_request.take() {
            self.player.seek(pos);
        }
    }

    fn pump_audio(&mut self) {
        if let (Some(pipeline), Some(audio)) = (
            self.player.pipeline.as_ref(),
            self.audio.as_ref(),
        ) {
            while let Some(frame) = pipeline.try_recv_audio_frame() {
                audio.push_frame(frame);
            }
            audio.set_paused(self.player.state == PlayerState::Paused);
            audio.set_volume(self.player.volume);
        }
    }
}

impl eframe::App for OmniApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        // Tick du player (événements pipeline)
        self.player.poll_events();
        self.process_seek();
        self.pump_audio();

        // Auto-play suivant dans la playlist
        if self.player.state == PlayerState::EndOfFile {
            if let Some(idx) = self.playlist_idx {
                let next = idx + 1;
                if next < self.playlist_items.len() {
                    let path = self.playlist_items[next].clone();
                    self.playlist_idx = Some(next);
                    self.open_file(path);
                }
            }
        }

        // ── Barre supérieure (menus) ─────────────────────────────────────────
        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Fichier", |ui| {
                    if ui.button("Ouvrir fichier…").clicked() {
                        self.show_file_browser = true;
                        ui.close_menu();
                    }
                    if ui.button("Ouvrir URL…").clicked() {
                        // TODO: dialog URL
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Fichiers récents", |ui| {
                        let recents = self.config.recent_files.clone();
                        for f in &recents {
                            if ui.button(f).clicked() {
                                let path = f.clone();
                                self.open_file(path);
                                ui.close_menu();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("Quitter").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Vue", |ui| {
                    ui.checkbox(&mut self.show_playlist, "Playlist");
                    if ui.button("Plein écran").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                        ui.close_menu();
                    }
                });
                ui.menu_button("Outils", |ui| {
                    if ui.button("Paramètres").clicked() {
                        self.show_settings = true;
                        ui.close_menu();
                    }
                });

                // Indicateur de résolution à droite
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(info) = &self.player.media_info {
                        if let Some(v) = &info.video {
                            let res = omni_core::Resolution { width: v.width, height: v.height };
                            ui.label(
                                egui::RichText::new(res.quality_label())
                                    .color(egui::Color32::from_rgb(100, 200, 100))
                                    .small()
                            );
                            ui.label(
                                egui::RichText::new(format!("{}×{} • {}fps", v.width, v.height, v.fps as u32))
                                    .small()
                            );
                        }
                    }
                    ui.label(
                        egui::RichText::new("OmniPlayer")
                            .color(egui::Color32::from_rgb(80, 140, 255))
                            .strong()
                    );
                });
            });
        });

        // ── Panneau de contrôle bas ──────────────────────────────────────────
        TopBottomPanel::bottom("controls").show(ctx, |ui| {
            controls::show(ui, &mut self.player, &mut self.seek_request);
        });

        // ── Playlist latérale ────────────────────────────────────────────────
        if self.show_playlist {
            egui::SidePanel::right("playlist_panel")
                .resizable(true)
                .default_width(260.0)
                .show(ctx, |ui| {
                    playlist::show(
                        ui,
                        &mut self.playlist_items,
                        &mut self.playlist_idx,
                        |path| self.open_file(path),
                    );
                });
        }

        // ── Zone vidéo centrale ──────────────────────────────────────────────
        CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                player_view::show(ui, &self.player);
            });

        // ── Dialogues modaux ─────────────────────────────────────────────────
        if self.show_file_browser {
            file_browser::show(ctx, &mut self.show_file_browser, |path| {
                self.open_file(path);
            });
        }

        if self.show_settings {
            settings::show(ctx, &mut self.show_settings, &mut self.config);
        }

        // Refresh continu pendant la lecture
        if self.player.is_active() {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.player.stop();
        self.config.save();
    }
}
