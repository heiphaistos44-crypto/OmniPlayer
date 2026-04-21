use egui::{Color32, CornerRadius, Rect, TextureHandle, Ui, Vec2};
use eframe::egui_wgpu;
use crate::player::{Player, PlayerState};
use crate::video_callback::SharedFrame;
use crate::ui::image_viewer::ImageViewer;

const SUBTITLE_BG: Color32 = Color32::from_black_alpha(185);

/// Zone vidéo / image centrale avec overlays.
pub fn show(
    ui:          &mut Ui,
    player:      &Player,
    video_frame: SharedFrame,
    osd:         Option<&str>,
    image_tex:   Option<&TextureHandle>,
    img_viewer:  &mut ImageViewer,
) {
    let available = ui.available_rect_before_wrap();

    if player.is_image_mode() {
        if let (Some(tex), Some(img)) = (image_tex, &player.image_frame) {
            img_viewer.show(ui, tex, img.width, img.height);
        } else {
            draw_idle_screen(ui, available);
        }
        return;
    }

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
    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        crate::video_callback::VideoPaintCallback { frame: video_frame },
    ));
}

fn draw_subtitle(ui: &mut Ui, rect: Rect, text: &str) {
    let painter = ui.painter();
    let font    = egui::FontId::proportional(18.0);
    let lines: Vec<&str> = text.lines().collect();
    let line_h  = 26.0;
    let pad     = Vec2::new(16.0, 8.0);
    let max_chars = lines.iter().map(|l| l.len()).max().unwrap_or(1);
    let box_w   = (max_chars as f32 * 10.5 + pad.x * 2.0).max(100.0).min(rect.width() * 0.9);
    let box_h   = lines.len() as f32 * line_h + pad.y * 2.0;
    let box_pos = egui::pos2(
        rect.center().x - box_w * 0.5,
        rect.bottom() - box_h - 105.0, // au-dessus des contrôles
    );
    let box_rect = Rect::from_min_size(box_pos, Vec2::new(box_w, box_h));

    painter.rect_filled(box_rect, CornerRadius::from(6.0_f32), SUBTITLE_BG);
    // Bordure légère
    painter.rect_stroke(box_rect, CornerRadius::from(6.0_f32),
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 20)),
        egui::StrokeKind::Middle);

    for (i, line) in lines.iter().enumerate() {
        // Ombre du texte
        painter.text(
            egui::pos2(rect.center().x + 1.0, box_pos.y + pad.y + i as f32 * line_h + line_h * 0.5 + 1.0),
            egui::Align2::CENTER_CENTER, *line, font.clone(),
            Color32::from_black_alpha(200),
        );
        painter.text(
            egui::pos2(rect.center().x, box_pos.y + pad.y + i as f32 * line_h + line_h * 0.5),
            egui::Align2::CENTER_CENTER, *line, font.clone(), Color32::WHITE,
        );
    }
}

fn draw_osd(ui: &mut Ui, rect: Rect, text: &str) {
    let p   = ui.painter();
    let pos = egui::pos2(rect.center().x, rect.top() + 52.0);
    let sz  = Vec2::new(text.len() as f32 * 9.5 + 22.0, 32.0);
    let bg  = Rect::from_center_size(pos, sz);
    p.rect_filled(bg, CornerRadius::from(6.0_f32),
        Color32::from_rgba_premultiplied(10, 10, 20, 210));
    p.rect_stroke(bg, CornerRadius::from(6.0_f32),
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(74, 158, 255, 80)),
        egui::StrokeKind::Middle);
    p.text(pos, egui::Align2::CENTER_CENTER, text,
        egui::FontId::proportional(15.0), Color32::WHITE);
}

fn draw_idle_screen(ui: &mut Ui, rect: Rect) {
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 14));

    // Centre the content in the area above the controls bar (~92px at bottom)
    let c = egui::pos2(rect.center().x, rect.top() + (rect.height() - 92.0) * 0.5);

    // Anneau décoratif (stroke seulement, pas de fill)
    p.circle_stroke(c, 54.0,
        egui::Stroke::new(1.5, Color32::from_rgba_unmultiplied(74, 158, 255, 55)));
    p.circle_stroke(c, 56.0,
        egui::Stroke::new(0.5, Color32::from_rgba_unmultiplied(74, 158, 255, 22)));

    // Icône play centrée dans l'anneau
    p.text(c, egui::Align2::CENTER_CENTER,
        "▶", egui::FontId::proportional(48.0), Color32::from_rgb(74, 158, 255));

    p.text(egui::pos2(c.x, c.y + 74.0), egui::Align2::CENTER_CENTER,
        "OmniPlayer", egui::FontId::proportional(24.0), Color32::from_gray(235));

    p.text(egui::pos2(c.x, c.y + 97.0), egui::Align2::CENTER_CENTER,
        "Glissez un fichier ici  ·  Ctrl+O  ·  Ctrl+L",
        egui::FontId::proportional(11.5), Color32::from_gray(110));

    // Séparateur fin
    p.line_segment(
        [egui::pos2(c.x - 100.0, c.y + 114.0), egui::pos2(c.x + 100.0, c.y + 114.0)],
        egui::Stroke::new(0.5, Color32::from_rgba_unmultiplied(74, 158, 255, 40)),
    );

    let shortcuts = [
        ("Espace",  "Lecture / Pause"),
        ("← / →",   "±10 secondes"),
        ("↑ / ↓",   "Volume ±10 %"),
        ("F",        "Plein écran"),
        ("M",        "Muet"),
        ("S",        "Sous-titres"),
        ("A",        "Piste audio"),
        ("I",        "Infos média"),
        ("Ctrl+O",   "Ouvrir fichier"),
        ("Ctrl+L",   "Ouvrir URL"),
    ];
    let mut y = c.y + 130.0;
    for (key, action) in &shortcuts {
        p.text(egui::pos2(c.x - 88.0, y), egui::Align2::RIGHT_CENTER,
            *key, egui::FontId::monospace(10.0), Color32::from_rgb(74, 158, 255));
        p.text(egui::pos2(c.x - 72.0, y), egui::Align2::LEFT_CENTER,
            *action, egui::FontId::proportional(10.5), Color32::from_gray(145));
        y += 17.5;
    }
}

fn draw_loading(ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 14));
    let t    = ui.ctx().input(|i| i.time) as f32;
    let dots = ".".repeat(((t * 2.0) as usize % 4) + 1);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
        format!("Chargement{dots}"),
        egui::FontId::proportional(18.0), Color32::from_gray(165));
    ui.ctx().request_repaint();
}

fn draw_buffering_overlay(ui: &mut Ui, rect: Rect, pct: u8) {
    let bar_w = rect.width().min(400.0) * 0.6;
    let bar_y = rect.bottom() - 130.0;
    let bg_r  = Rect::from_min_size(
        egui::pos2(rect.center().x - bar_w * 0.5, bar_y),
        Vec2::new(bar_w, 4.0),
    );
    let fil_r = Rect::from_min_size(bg_r.min,
        Vec2::new(bar_w * pct as f32 / 100.0, 4.0));
    let p     = ui.painter();
    p.rect_filled(bg_r,  CornerRadius::from(2.0_f32), Color32::from_gray(40));
    p.rect_filled(fil_r, CornerRadius::from(2.0_f32), Color32::from_rgb(74, 158, 255));
    p.text(egui::pos2(rect.center().x, bar_y - 16.0), egui::Align2::CENTER_CENTER,
        format!("Chargement… {pct}%"),
        egui::FontId::proportional(12.5), Color32::from_gray(185));
}

fn draw_error(ui: &mut Ui, rect: Rect, msg: &str) {
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(16, 5, 5));
    let c = rect.center(); let p = ui.painter();
    p.text(egui::pos2(c.x, c.y - 20.0), egui::Align2::CENTER_CENTER,
        "⚠  Erreur de lecture",
        egui::FontId::proportional(20.0), Color32::from_rgb(255, 75, 75));
    p.text(egui::pos2(c.x, c.y + 16.0), egui::Align2::CENTER_CENTER,
        msg, egui::FontId::proportional(12.0), Color32::from_gray(165));
    p.text(egui::pos2(c.x, c.y + 40.0), egui::Align2::CENTER_CENTER,
        "Appuyez sur Ctrl+O pour ouvrir un autre fichier",
        egui::FontId::proportional(11.0), Color32::from_gray(110));
}
