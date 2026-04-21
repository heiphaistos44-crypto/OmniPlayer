use egui::{Color32, Response, Rounding, Sense, Stroke, Ui, Vec2};
use crate::player::{Player, PlayerState};

const ACCENT:  Color32 = Color32::from_rgb(80, 140, 255);
const SURFACE: Color32 = Color32::from_rgb(22, 22, 30);

/// Barre de contrôle complète : chapitres, seek, play, pistes, volume.
pub fn show(ui: &mut Ui, player: &mut Player, seek_out: &mut Option<f64>) {
    ui.style_mut().spacing.item_spacing = Vec2::new(6.0, 3.0);

    ui.vertical(|ui| {
        // ── Titre + chapitre actuel ────────────────────────────────────────────
        if let Some(title) = player.display_title() {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&title).size(11.5).color(Color32::from_gray(180)));
                if let Some(ch) = current_chapter(player) {
                    ui.label(egui::RichText::new(format!("— {ch}")).size(11.0).color(Color32::from_gray(110)));
                }
            });
        }

        // ── Seek bar ──────────────────────────────────────────────────────────
        let dur = player.duration.max(1.0);
        let mut pos = player.position;
        let bar_resp = seek_bar(ui, &mut pos, dur, &player.chapters);
        if bar_resp.changed() { *seek_out = Some(pos); }

        // ── Ligne de contrôles ────────────────────────────────────────────────
        ui.horizontal(|ui| {
            // Chapitre précédent
            if !player.chapters.is_empty() {
                if icon_btn(ui, "⏮", "Chapitre précédent").clicked() { player.chapter_prev(); }
            }

            if icon_btn(ui, "⏹", "Stop").clicked()        { player.stop(); }

            let play_icon = if matches!(player.state, PlayerState::Playing) { "⏸" } else { "▶" };
            if icon_btn(ui, play_icon, "Play/Pause  [Espace]").clicked() { player.play_pause(); }

            if !player.chapters.is_empty() {
                if icon_btn(ui, "⏭", "Chapitre suivant").clicked() { player.chapter_next(); }
            }

            if icon_btn(ui, "↩10", "−10 s  [←]").clicked() { player.seek_relative(-10.0); }
            if icon_btn(ui, "10↪", "+10 s  [→]").clicked()  { player.seek_relative(10.0); }

            // Timestamp
            ui.label(
                egui::RichText::new(format!("  {} / {}", fmt_time(player.position), fmt_time(player.duration)))
                    .monospace().size(12.0).color(Color32::from_gray(200)),
            );

            // État
            match &player.state.clone() {
                PlayerState::Buffering(b) => {
                    ui.label(egui::RichText::new(format!("⏳ {b}%")).color(Color32::YELLOW).small());
                }
                PlayerState::Error(e) => {
                    ui.label(egui::RichText::new(format!("⚠ {e}")).color(Color32::RED).small());
                }
                _ => {}
            }

            // ── Droite : pistes + volume ──────────────────────────────────────
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Volume
                let vol_icon = if player.muted { "🔇" } else if player.volume < 0.3 { "🔈" } else { "🔊" };
                if icon_btn(ui, vol_icon, "Muet  [M]").clicked() { player.toggle_mute(); }
                let mut vol = player.volume;
                if ui.add_sized([68.0, 20.0], egui::Slider::new(&mut vol, 0.0..=1.5).show_value(false)).changed() {
                    player.set_volume(vol);
                }

                ui.separator();

                // Piste audio
                if let Some(info) = &player.media_info {
                    if info.audio.len() > 1 {
                        let label = info.audio.get(player.audio_track_idx)
                            .map(|a| format!("🎵 {}", if a.language.is_empty() { "?" } else { a.language.as_str() }))
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
                            .map(|s| format!("💬 {}", if s.language.is_empty() { "?" } else { s.language.as_str() }))
                            .unwrap_or_else(|| format!("💬 #{i}"))
                    }
                };
                if ui.small_button(&sub_label).on_hover_text("Sous-titres  [S]").clicked() {
                    player.next_subtitle_track();
                }
            });
        });
    });
}

/// Seek bar avec marqueurs de chapitres et tooltip de position.
fn seek_bar(ui: &mut Ui, pos: &mut f64, duration: f64, chapters: &[omni_core::probe::Chapter]) -> Response {
    let desired = Vec2::new(ui.available_width(), 8.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());

    if resp.clicked() || resp.dragged() {
        if let Some(mp) = resp.interact_pointer_pos() {
            let t = ((mp.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *pos = t as f64 * duration;
        }
    }

    if ui.is_rect_visible(rect) {
        let p = ui.painter();

        p.rect_filled(rect, Rounding::from(4.0_f32), Color32::from_gray(38));

        let filled_w = (rect.width() * (*pos / duration) as f32).max(0.0);
        p.rect_filled(
            egui::Rect::from_min_size(rect.min, Vec2::new(filled_w, rect.height())),
            Rounding::from(4.0_f32), ACCENT,
        );

        // Marqueurs chapitres
        for ch in chapters {
            let x = rect.left() + rect.width() * (ch.start_secs / duration) as f32;
            p.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 200, 80, 160)),
            );
        }

        // Thumb
        let thumb_x = rect.left() + filled_w;
        let r = if resp.hovered() || resp.dragged() { 7.5 } else { 4.5 };
        p.circle_filled(egui::pos2(thumb_x, rect.center().y), r, Color32::WHITE);

        // Tooltip temps
        if resp.hovered() {
            if let Some(mp) = resp.hover_pos() {
                let t = ((mp.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                p.text(
                    egui::pos2(mp.x, rect.top() - 14.0), egui::Align2::CENTER_BOTTOM,
                    fmt_time(t as f64 * duration),
                    egui::FontId::monospace(10.5), Color32::WHITE,
                );
            }
        }
    }

    resp
}

fn icon_btn(ui: &mut Ui, icon: &str, tooltip: &str) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(icon).size(14.5))
            .min_size(Vec2::splat(26.0))
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, Color32::from_gray(42))),
    ).on_hover_text(tooltip)
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
