use egui::{Color32, Pos2, Rect, Sense, TextureHandle, Ui, Vec2};

pub struct ImageViewer {
    zoom:    f32,
    pan:     Vec2,
    dragging: bool,
    drag_start_pan: Vec2,
}

impl Default for ImageViewer {
    fn default() -> Self {
        Self { zoom: 1.0, pan: Vec2::ZERO, dragging: false, drag_start_pan: Vec2::ZERO }
    }
}

impl ImageViewer {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan  = Vec2::ZERO;
    }

    /// Affiche l'image avec zoom/pan. Retourne true si l'état a changé.
    pub fn show(&mut self, ui: &mut Ui, tex: &TextureHandle, img_w: u32, img_h: u32) {
        let available = ui.available_rect_before_wrap();
        ui.painter().rect_filled(available, 0.0, Color32::from_rgb(8, 8, 12));

        // Fit-to-screen de base
        let fit_scale = (available.width() / img_w as f32)
            .min(available.height() / img_h as f32)
            .min(1.0); // pas de zoom numérique par défaut

        let display_w = img_w as f32 * fit_scale * self.zoom;
        let display_h = img_h as f32 * fit_scale * self.zoom;

        let center = available.center();
        let img_rect = Rect::from_center_size(
            Pos2::new(center.x + self.pan.x, center.y + self.pan.y),
            Vec2::new(display_w, display_h),
        );

        ui.painter().image(
            tex.id(),
            img_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        // Interaction
        let resp = ui.allocate_rect(available, Sense::click_and_drag());

        // Scroll wheel → zoom
        let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let old_zoom = self.zoom;
            self.zoom = (self.zoom * (1.0 + scroll * 0.002)).clamp(0.1, 20.0);
            // Zoom vers le pointeur
            if let Some(ptr) = ui.ctx().input(|i| i.pointer.hover_pos()) {
                let d = ptr - (center + self.pan);
                self.pan += d * (1.0 - self.zoom / old_zoom);
            }
        }

        // Drag → pan
        if resp.drag_started() {
            self.drag_start_pan = self.pan;
        }
        if resp.dragged() {
            self.pan = self.drag_start_pan + resp.drag_delta()
                + ui.ctx().input(|i| i.pointer.press_origin())
                    .map(|o| o - o) // workaround: just use delta
                    .unwrap_or(Vec2::ZERO);
            self.pan += resp.drag_delta();
        }

        // Clavier
        ui.ctx().input(|i| {
            if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                self.zoom = (self.zoom * 1.2).min(20.0);
            }
            if i.key_pressed(egui::Key::Minus) {
                self.zoom = (self.zoom / 1.2).max(0.1);
            }
            if i.key_pressed(egui::Key::Num0) {
                self.zoom = 1.0;
                self.pan  = Vec2::ZERO;
            }
        });

        // Indicateur zoom
        if (self.zoom - 1.0).abs() > 0.01 {
            let zoom_text = format!("{:.0}%", self.zoom * fit_scale * 100.0);
            let pos = egui::pos2(available.right() - 60.0, available.bottom() - 24.0);
            ui.painter().text(
                pos, egui::Align2::CENTER_CENTER, zoom_text,
                egui::FontId::monospace(12.0), Color32::from_rgba_unmultiplied(255, 255, 255, 140),
            );
        }

        // Info dimensions
        let dim_text = format!("{} × {} px", img_w, img_h);
        ui.painter().text(
            egui::pos2(available.center().x, available.bottom() - 24.0),
            egui::Align2::CENTER_CENTER, dim_text,
            egui::FontId::proportional(11.0), Color32::from_rgba_unmultiplied(180, 180, 180, 120),
        );
    }
}
