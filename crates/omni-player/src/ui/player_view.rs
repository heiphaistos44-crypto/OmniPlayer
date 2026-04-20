use egui::{Color32, Rect, Ui, Vec2};
use crate::player::{Player, PlayerState};

/// Zone vidéo centrale — affiche la frame courante via egui_wgpu callback.
/// En l'absence de frame (état Idle), affiche un écran d'accueil.
pub fn show(ui: &mut Ui, player: &Player) {
    let available = ui.available_rect_before_wrap();

    match &player.state {
        PlayerState::Idle => {
            draw_idle_screen(ui, available);
        }
        PlayerState::Loading => {
            draw_loading(ui, available);
        }
        PlayerState::Buffering(pct) => {
            // On dessine quand même la vidéo pendant le buffering
            draw_video_placeholder(ui, available);
            draw_buffering_overlay(ui, available, *pct);
        }
        PlayerState::Error(msg) => {
            draw_error(ui, available, msg);
        }
        PlayerState::Playing | PlayerState::Paused | PlayerState::EndOfFile => {
            draw_video_placeholder(ui, available);
        }
    }
}

/// Écran d'accueil : logo + raccourcis clavier.
fn draw_idle_screen(ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(10, 10, 16));

    let center = rect.center();
    let painter = ui.painter();

    // Logo
    painter.text(
        center - Vec2::new(0.0, 60.0),
        egui::Align2::CENTER_CENTER,
        "▶",
        egui::FontId::proportional(72.0),
        Color32::from_rgb(80, 140, 255),
    );
    painter.text(
        center - Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "OmniPlayer",
        egui::FontId::proportional(28.0),
        Color32::WHITE,
    );
    painter.text(
        center + Vec2::new(0.0, 28.0),
        egui::Align2::CENTER_CENTER,
        "Glissez un fichier ici ou utilisez Fichier → Ouvrir",
        egui::FontId::proportional(13.0),
        Color32::from_gray(140),
    );

    // Raccourcis
    let shortcuts = [
        ("Espace", "Play / Pause"),
        ("←  →",   "Reculer / Avancer 10s"),
        ("↑  ↓",   "Volume"),
        ("F",       "Plein écran"),
        ("S",       "Sous-titre suivant"),
        ("Ctrl+O",  "Ouvrir fichier"),
    ];
    let mut y = center.y + 70.0;
    for (key, action) in &shortcuts {
        painter.text(
            egui::pos2(center.x - 90.0, y),
            egui::Align2::RIGHT_CENTER,
            *key,
            egui::FontId::monospace(12.0),
            Color32::from_rgb(80, 140, 255),
        );
        painter.text(
            egui::pos2(center.x - 70.0, y),
            egui::Align2::LEFT_CENTER,
            *action,
            egui::FontId::proportional(12.0),
            Color32::from_gray(170),
        );
        y += 20.0;
    }
}

fn draw_loading(ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
    let t = ui.ctx().input(|i| i.time) as f32;
    let dots = ".".repeat(((t * 2.0) as usize % 4) + 1);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("Chargement{dots}"),
        egui::FontId::proportional(20.0),
        Color32::from_gray(180),
    );
    ui.ctx().request_repaint();
}

fn draw_video_placeholder(ui: &mut Ui, rect: Rect) {
    // Fond noir — le vrai rendu wgpu se fait via egui_wgpu::Callback
    // que l'on brancherait ici dans la version finale avec le VideoRenderer.
    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
}

fn draw_buffering_overlay(ui: &mut Ui, rect: Rect, pct: u8) {
    let bar_w  = rect.width() * 0.4;
    let bar_h  = 6.0;
    let bar_x  = rect.center().x - bar_w / 2.0;
    let bar_y  = rect.center().y + 40.0;
    let bar_rect = Rect::from_min_size(egui::pos2(bar_x, bar_y), Vec2::new(bar_w, bar_h));

    ui.painter().rect_filled(bar_rect, 3.0, Color32::from_gray(50));
    let filled = Rect::from_min_size(
        bar_rect.min,
        Vec2::new(bar_w * pct as f32 / 100.0, bar_h),
    );
    ui.painter().rect_filled(filled, 3.0, Color32::from_rgb(80, 140, 255));
    ui.painter().text(
        egui::pos2(rect.center().x, bar_y - 14.0),
        egui::Align2::CENTER_CENTER,
        format!("Mise en mémoire tampon… {pct}%"),
        egui::FontId::proportional(13.0),
        Color32::WHITE,
    );
}

fn draw_error(ui: &mut Ui, rect: Rect, msg: &str) {
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(24, 8, 8));
    let center = rect.center();
    ui.painter().text(
        center - Vec2::new(0.0, 16.0),
        egui::Align2::CENTER_CENTER,
        "⚠  Erreur de lecture",
        egui::FontId::proportional(20.0),
        Color32::from_rgb(255, 80, 80),
    );
    ui.painter().text(
        center + Vec2::new(0.0, 14.0),
        egui::Align2::CENTER_CENTER,
        msg,
        egui::FontId::proportional(12.0),
        Color32::from_gray(180),
    );
}
