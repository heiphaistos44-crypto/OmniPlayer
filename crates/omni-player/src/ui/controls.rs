use egui::{Color32, Response, Sense, Slider, Ui, Vec2};
use crate::player::{Player, PlayerState};

const ACCENT: Color32 = Color32::from_rgb(80, 140, 255);

/// Barre de contrôle inférieure : seek, play/pause, volume, temps.
pub fn show(ui: &mut Ui, player: &mut Player, seek_out: &mut Option<f64>) {
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);

    // ── Seek bar ─────────────────────────────────────────────────────────────
    ui.vertical(|ui| {
        let dur  = player.duration.max(1.0);
        let mut pos = player.position;

        let bar_resp = seek_bar(ui, &mut pos, dur);
        if bar_resp.changed() {
            *seek_out = Some(pos);
        }

        // ── Ligne de contrôles ────────────────────────────────────────────────
        ui.horizontal(|ui| {
            // Bouton Stop
            if icon_btn(ui, "⏹", "Stop").clicked() {
                player.stop();
            }

            // Bouton Play/Pause
            let play_icon = match player.state {
                PlayerState::Playing => "⏸",
                _                    => "▶",
            };
            if icon_btn(ui, play_icon, "Play/Pause").clicked() {
                player.play_pause();
            }

            // Timestamp
            ui.label(
                egui::RichText::new(format!(
                    "{} / {}",
                    fmt_time(player.position),
                    fmt_time(player.duration)
                ))
                .monospace()
                .size(13.0),
            );

            // État
            match &player.state {
                PlayerState::Buffering(b) => {
                    ui.label(
                        egui::RichText::new(format!("⏳ {b}%"))
                            .color(Color32::YELLOW)
                            .small(),
                    );
                }
                PlayerState::Error(e) => {
                    ui.label(
                        egui::RichText::new(format!("⚠ {e}"))
                            .color(Color32::RED)
                            .small(),
                    );
                }
                _ => {}
            }

            // Volume (droite)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut vol = player.volume;
                ui.label("🔊");
                let vol_slider = Slider::new(&mut vol, 0.0..=1.5)
                    .show_value(false)
                    .desired_width(80.0);
                if ui.add(vol_slider).changed() {
                    player.set_volume(vol);
                }
            });
        });
    });
}

/// Seek bar custom avec progression colorée.
fn seek_bar(ui: &mut Ui, pos: &mut f64, duration: f64) -> Response {
    let desired = Vec2::new(ui.available_width(), 6.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());

    if resp.clicked() || resp.dragged() {
        if let Some(mp) = resp.interact_pointer_pos() {
            let t = ((mp.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *pos = t * duration;
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        // Track
        painter.rect_filled(rect, 3.0, Color32::from_gray(50));
        // Progress
        let filled_w = rect.width() * (*pos / duration) as f32;
        let filled_rect = egui::Rect::from_min_size(
            rect.min,
            Vec2::new(filled_w, rect.height()),
        );
        painter.rect_filled(filled_rect, 3.0, ACCENT);
        // Thumb
        painter.circle_filled(
            egui::pos2(rect.min.x + filled_w, rect.center().y),
            7.0,
            Color32::WHITE,
        );
    }

    resp
}

fn icon_btn(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(icon).size(18.0))
            .min_size(Vec2::splat(32.0)),
    )
    .on_hover_text(tooltip)
}

fn fmt_time(secs: f64) -> String {
    let s   = secs as u64;
    let h   = s / 3600;
    let m   = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}
