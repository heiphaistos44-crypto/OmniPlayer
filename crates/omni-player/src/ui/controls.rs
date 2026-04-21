use egui::{Color32, CornerRadius, Response, Sense, Stroke, Ui, Vec2};
use crate::player::{Player, PlayerState};

const ACCENT:    Color32 = Color32::from_rgb(74, 158, 255);
const ACCENT_DIM: Color32 = Color32::from_rgb(40, 90, 160);
const SURFACE:   Color32 = Color32::from_rgba_premultiplied(20, 20, 30, 200);
const SURFACE2:  Color32 = Color32::from_rgba_premultiplied(30, 32, 44, 200);
const DIM:       Color32 = Color32::from_gray(120);
const HDR_COLOR: Color32 = Color32::from_rgb(255, 160, 40);

/// Barre de contrôles flottante — overlay semi-transparent.
pub fn show(ui: &mut Ui, player: &mut Player, seek_out: &mut Option<f64>) {
    let available_w = ui.ctx().screen_rect().width();

    // Fond gradient simulé par deux rects
    let bg_rect = ui.available_rect_before_wrap();
    let p = ui.painter();
    p.rect_filled(bg_rect, CornerRadius::ZERO,
        Color32::from_rgba_premultiplied(6, 6, 14, 180));
    // Ligne de séparation haute
    p.line_segment(
        [egui::pos2(bg_rect.left(), bg_rect.top()), egui::pos2(bg_rect.right(), bg_rect.top())],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(74, 158, 255, 40)),
    );

    ui.style_mut().spacing.item_spacing = Vec2::new(5.0, 2.0);
    ui.style_mut().spacing.button_padding = Vec2::new(6.0, 4.0);

    ui.vertical(|ui| {
        ui.add_space(4.0);

        // ── Titre + info technique ────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            if let Some(title) = player.display_title() {
                ui.label(egui::RichText::new(&title)
                    .size(11.5).color(Color32::from_gray(200)));
                if let Some(ch) = current_chapter(player) {
                    ui.label(egui::RichText::new(format!("— {ch}"))
                        .size(11.0).color(DIM));
                }
            }
            // HDR badge
            if player.media_info.as_ref().and_then(|m| m.video.as_ref())
                .map(|v| v.hdr).unwrap_or(false)
            {
                ui.add_space(6.0);
                badge(ui, "HDR", HDR_COLOR);
            }
            // Résolution badge
            if let Some(v) = player.media_info.as_ref().and_then(|m| m.video.as_ref()) {
                let res = omni_core::Resolution { width: v.width, height: v.height };
                badge(ui, res.quality_label(), Color32::from_rgb(80, 200, 120));
            }
        });

        // ── Seek bar ──────────────────────────────────────────────────────
        ui.add_space(3.0);
        let dur  = player.duration.max(1.0);
        let mut pos = player.position;
        let bar_resp = seek_bar(ui, &mut pos, dur, &player.chapters, available_w);
        if bar_resp.changed() { *seek_out = Some(pos); }

        // ── Ligne de contrôles ────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            if !player.chapters.is_empty() {
                if ctrl_btn(ui, "⏮", "Chapitre précédent [⬅]").clicked() { player.chapter_prev(); }
            }
            if ctrl_btn(ui, "⏹", "Stop").clicked() { player.stop(); }

            let play_icon = if matches!(player.state, PlayerState::Playing) { "⏸" } else { "▶" };
            let play_resp = ctrl_btn(ui, play_icon, "Lecture/Pause  [Espace]");
            if play_resp.clicked() { player.play_pause(); }

            if !player.chapters.is_empty() {
                if ctrl_btn(ui, "⏭", "Chapitre suivant [➡]").clicked() { player.chapter_next(); }
            }

            ui.add_space(4.0);
            if ctrl_btn(ui, "↩10", "−10 s  [←]").clicked() { player.seek_relative(-10.0); }
            if ctrl_btn(ui, "10↪", "+10 s  [→]").clicked()  { player.seek_relative(10.0); }

            ui.add_space(8.0);
            // Timestamp
            ui.label(
                egui::RichText::new(format!("{} / {}",
                    fmt_time(player.position), fmt_time(player.duration)))
                    .monospace().size(12.5).color(Color32::from_gray(215)),
            );

            // État
            match &player.state.clone() {
                PlayerState::Buffering(b) => {
                    ui.label(egui::RichText::new(format!("⏳ {b}%")).color(Color32::YELLOW).small());
                }
                PlayerState::Error(e) => {
                    ui.label(egui::RichText::new(format!("⚠ {}", &e[..e.len().min(30)]))
                        .color(Color32::RED).small());
                }
                _ => {}
            }

            // ── Droite : pistes + volume ──────────────────────────────────
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Volume
                let vol_icon = if player.muted { "🔇" }
                    else if player.volume == 0.0 { "🔈" }
                    else if player.volume < 0.6  { "🔉" }
                    else                         { "🔊" };
                if ctrl_btn(ui, vol_icon, "Muet  [M]").clicked() { player.toggle_mute(); }

                let mut vol = player.volume;
                if ui.add_sized([72.0, 18.0],
                    egui::Slider::new(&mut vol, 0.0..=1.5).show_value(false)
                ).changed() {
                    player.set_volume(vol);
                }

                ui.separator();

                // Piste audio
                if let Some(info) = &player.media_info {
                    if info.audio.len() > 1 {
                        let label = info.audio.get(player.audio_track_idx)
                            .map(|a| {
                                let lang = if a.language.is_empty() || a.language == "und"
                                    { "?".to_string() } else { a.language.clone() };
                                format!("🎵 {lang}")
                            })
                            .unwrap_or_else(|| "🎵".into());
                        if ui.small_button(&label).on_hover_text("Piste audio  [A]").clicked() {
                            player.next_audio_track();
                        }
                    }
                }

                // Sous-titres
                let sub_label = match player.sub_track_idx {
                    None if player.subtitle_track.is_none() => "💬 Off".to_string(),
                    None    => "💬 Ext".to_string(),
                    Some(i) => {
                        player.media_info.as_ref()
                            .and_then(|mi| mi.subtitles.get(i))
                            .map(|s| {
                                let lang = if s.language.is_empty() { "?" } else { &s.language };
                                format!("💬 {lang}")
                            })
                            .unwrap_or_else(|| format!("💬 #{i}"))
                    }
                };
                if ui.small_button(&sub_label).on_hover_text("Sous-titres  [S]").clicked() {
                    player.next_subtitle_track();
                }
            });
        });

        ui.add_space(6.0);
    });
}

/// Seek bar stylisée avec marqueurs chapitres et tooltip.
fn seek_bar(
    ui: &mut Ui,
    pos: &mut f64,
    duration: f64,
    chapters: &[omni_core::probe::Chapter],
    _available_w: f32,
) -> Response {
    let h = 10.0;
    let desired = Vec2::new(ui.available_width() - 16.0, h + 8.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());

    // Zone cliquable légèrement plus grande que la barre visuelle
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 8.0, rect.center().y - h * 0.5),
        Vec2::new(rect.width() - 16.0, h),
    );

    if resp.clicked() || resp.dragged() {
        if let Some(mp) = resp.interact_pointer_pos() {
            let t = ((mp.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
            *pos = t as f64 * duration;
        }
    }

    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let cr = CornerRadius::from(h * 0.5);

        // Track de fond
        p.rect_filled(bar, cr, Color32::from_gray(45));

        // Portion jouée
        let t = (*pos / duration).clamp(0.0, 1.0) as f32;
        let filled_w = bar.width() * t;
        if filled_w > 0.0 {
            let filled_rect = egui::Rect::from_min_size(bar.min, Vec2::new(filled_w, bar.height()));
            p.rect_filled(filled_rect, cr, ACCENT);
        }

        // Marqueurs chapitres
        for ch in chapters {
            let x = bar.left() + bar.width() * (ch.start_secs / duration) as f32;
            p.rect_filled(
                egui::Rect::from_center_size(
                    egui::pos2(x, bar.center().y),
                    Vec2::new(2.0, bar.height() + 4.0),
                ),
                CornerRadius::ZERO,
                Color32::from_rgb(255, 200, 80),
            );
        }

        // Thumb
        let thumb_x = bar.left() + filled_w;
        let r = if resp.hovered() || resp.dragged() { 8.0 } else { 5.0 };
        p.circle_filled(egui::pos2(thumb_x, bar.center().y), r, Color32::WHITE);
        if resp.hovered() || resp.dragged() {
            p.circle_stroke(
                egui::pos2(thumb_x, bar.center().y), r + 2.0,
                Stroke::new(1.5, Color32::from_rgba_premultiplied(74, 158, 255, 80)),
            );
        }

        // Tooltip position
        if resp.hovered() {
            if let Some(mp) = resp.hover_pos() {
                let t = ((mp.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                let time_str = fmt_time(t as f64 * duration);
                let tp = egui::pos2(mp.x.clamp(bar.left() + 20.0, bar.right() - 20.0), bar.top() - 18.0);
                let bg = egui::Rect::from_center_size(tp, Vec2::new(time_str.len() as f32 * 7.5 + 12.0, 18.0));
                p.rect_filled(bg, CornerRadius::from(4.0_f32), Color32::from_black_alpha(200));
                p.text(tp, egui::Align2::CENTER_CENTER, &time_str,
                    egui::FontId::monospace(11.0), Color32::WHITE);
            }
        }
    }

    resp
}

fn ctrl_btn(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(icon).size(14.0))
            .min_size(Vec2::splat(28.0))
            .fill(SURFACE2)
            .stroke(Stroke::new(1.0, Color32::from_gray(50))),
    ).on_hover_text(tooltip)
}

fn badge(ui: &mut Ui, text: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(text.len() as f32 * 7.0 + 10.0, 16.0),
        Sense::hover(),
    );
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(
            rect,
            CornerRadius::from(3.0_f32),
            color.linear_multiply(0.25),
        );
        ui.painter().rect_stroke(
            rect,
            CornerRadius::from(3.0_f32),
            Stroke::new(1.0, color.linear_multiply(0.8)),
            egui::StrokeKind::Middle,
        );
        ui.painter().text(
            rect.center(), egui::Align2::CENTER_CENTER, text,
            egui::FontId::monospace(9.5), color,
        );
    }
}

fn current_chapter(player: &Player) -> Option<String> {
    if player.chapters.is_empty() { return None; }
    let pos = player.position;
    player.chapters.iter().rev()
        .find(|c| c.start_secs <= pos)
        .map(|c| c.title.clone())
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let h = s / 3600; let m = (s % 3600) / 60; let sec = s % 60;
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }
}
