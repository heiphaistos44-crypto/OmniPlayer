use egui::{Color32, Rect, Ui, Vec2};
use eframe::egui_wgpu;
use crate::player::{Player, PlayerState};
use crate::video_callback::SharedFrame;

const SUBTITLE_BG: Color32 = Color32::from_black_alpha(178);

/// Zone vidéo centrale : rendu wgpu réel + sous-titres + OSD.
pub fn show(ui: &mut Ui, player: &Player, video_frame: SharedFrame, osd: Option<&str>) {
    let available = ui.available_rect_before_wrap();

    match &player.state {
        PlayerState::Idle     => draw_idle_screen(ui, available),
        PlayerState::Loading  => draw_loading(ui, available),
        PlayerState::Error(e) => { let e = e.clone(); draw_error(ui, available, &e); }
        _ => {
            draw_video(ui, available, video_frame);

            if let PlayerState::Buffering(pct) = &player.state {
                draw_buffering_overlay(ui, available, *pct);
            }

            if let Some(text) = &player.current_subtitle {
                draw_subtitle(ui, available, text);
            }

            if let Some(text) = osd {
                draw_osd(ui, available, text);
            }
        }
    }
}

fn draw_video(ui: &mut Ui, rect: Rect, video_frame: SharedFrame) {
    // Fond noir (visible avant la 1re frame)
    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
    // Callback wgpu : upload YUV + rendu
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        crate::video_callback::VideoPaintCallback { frame: video_frame },
    ));
}

fn draw_subtitle(ui: &mut Ui, rect: Rect, text: &str) {
    let painter = ui.painter();
    let font    = egui::FontId::proportional(18.0);
    let lines: Vec<&str> = text.lines().collect();
    let line_h  = 24.0;
    let pad     = Vec2::new(12.0, 6.0);
    let max_chars = lines.iter().map(|l| l.len()).max().unwrap_or(1);
    let box_w   = (max_chars as f32 * 10.2 + pad.x * 2.0).max(100.0);
    let box_h   = lines.len() as f32 * line_h + pad.y * 2.0;
    let box_pos = egui::pos2(rect.center().x - box_w * 0.5, rect.bottom() - box_h - 40.0);
    let box_rect = Rect::from_min_size(box_pos, Vec2::new(box_w, box_h));

    painter.rect_filled(box_rect, 5.0, SUBTITLE_BG);
    for (i, line) in lines.iter().enumerate() {
        painter.text(
            egui::pos2(rect.center().x, box_pos.y + pad.y + i as f32 * line_h + line_h * 0.5),
            egui::Align2::CENTER_CENTER,
            *line, font.clone(), Color32::WHITE,
        );
    }
}

fn draw_osd(ui: &mut Ui, rect: Rect, text: &str) {
    let p   = ui.painter();
    let bg  = Color32::from_rgba_unmultiplied(0, 0, 0, 155);
    let pos = egui::pos2(rect.center().x, rect.top() + 46.0);
    let sz  = Vec2::new(text.len() as f32 * 9.0 + 18.0, 28.0);
    p.rect_filled(Rect::from_center_size(pos, sz), 6.0, bg);
    p.text(pos, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(15.0), Color32::WHITE);
}

fn draw_idle_screen(ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(10, 10, 16));
    let p = ui.painter();
    let c = rect.center();

    p.circle_stroke(c, 50.0, egui::Stroke::new(1.0, Color32::from_rgb(28, 38, 58)));
    p.text(c - Vec2::new(0.0, 60.0), egui::Align2::CENTER_CENTER,
        "▶", egui::FontId::proportional(66.0), Color32::from_rgb(80, 140, 255));
    p.text(c - Vec2::new(0.0, 12.0), egui::Align2::CENTER_CENTER,
        "OmniPlayer", egui::FontId::proportional(28.0), Color32::WHITE);
    p.text(c + Vec2::new(0.0, 22.0), egui::Align2::CENTER_CENTER,
        "Glissez un fichier ici ou utilisez Fichier → Ouvrir",
        egui::FontId::proportional(13.0), Color32::from_gray(125));

    let shortcuts = [
        ("Espace", "Play / Pause"),
        ("← / →",  "±10 secondes"),
        ("↑ / ↓",  "Volume"),
        ("M",       "Muet"),
        ("F",       "Plein écran"),
        ("S",       "Sous-titres"),
        ("A",       "Piste audio"),
        ("Ctrl+O",  "Ouvrir fichier"),
        ("Ctrl+L",  "Ouvrir URL"),
    ];
    let mut y = c.y + 56.0;
    for (key, action) in &shortcuts {
        p.text(egui::pos2(c.x - 86.0, y), egui::Align2::RIGHT_CENTER,
            *key, egui::FontId::monospace(11.0), Color32::from_rgb(80, 140, 255));
        p.text(egui::pos2(c.x - 70.0, y), egui::Align2::LEFT_CENTER,
            *action, egui::FontId::proportional(11.0), Color32::from_gray(155));
        y += 18.0;
    }
}

fn draw_loading(ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
    let t    = ui.ctx().input(|i| i.time) as f32;
    let dots = ".".repeat(((t * 2.0) as usize % 4) + 1);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        format!("Chargement{dots}"),
        egui::FontId::proportional(20.0), Color32::from_gray(175));
    ui.ctx().request_repaint();
}

fn draw_buffering_overlay(ui: &mut Ui, rect: Rect, pct: u8) {
    let bar_w = rect.width() * 0.35;
    let bar_y = rect.center().y + 44.0;
    let bg_r  = Rect::from_min_size(egui::pos2(rect.center().x - bar_w * 0.5, bar_y), Vec2::new(bar_w, 5.0));
    let fil_r = Rect::from_min_size(bg_r.min, Vec2::new(bar_w * pct as f32 / 100.0, 5.0));
    let p     = ui.painter();
    p.rect_filled(bg_r,  3.0, Color32::from_gray(40));
    p.rect_filled(fil_r, 3.0, Color32::from_rgb(80, 140, 255));
    p.text(egui::pos2(rect.center().x, bar_y - 14.0), egui::Align2::CENTER_CENTER,
        format!("Mise en mémoire tampon… {pct}%"),
        egui::FontId::proportional(13.0), Color32::WHITE);
}

fn draw_error(ui: &mut Ui, rect: Rect, msg: &str) {
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(18, 5, 5));
    let c = rect.center(); let p = ui.painter();
    p.text(c - Vec2::new(0.0, 18.0), egui::Align2::CENTER_CENTER,
        "⚠  Erreur de lecture", egui::FontId::proportional(20.0), Color32::from_rgb(255, 75, 75));
    p.text(c + Vec2::new(0.0, 14.0), egui::Align2::CENTER_CENTER,
        msg, egui::FontId::proportional(12.0), Color32::from_gray(175));
}
