use egui::{Color32, Context, RichText, ScrollArea, Vec2, Window};
use std::path::{Path, PathBuf};
use omni_core::SUPPORTED_EXTENSIONS;

#[derive(Clone)]
pub struct FileBrowser {
    current_dir: PathBuf,
    entries:     Vec<DirEntry>,
    filter:      String,
}

#[derive(Clone)]
struct DirEntry {
    path:    PathBuf,
    name:    String,
    is_dir:  bool,
    is_media: bool,
}

impl FileBrowser {
    pub fn new() -> Self {
        let start = dirs::video_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("C:\\"));
        let mut fb = Self {
            current_dir: start,
            entries:     Vec::new(),
            filter:      String::new(),
        };
        fb.refresh();
        fb
    }

    fn refresh(&mut self) {
        self.entries.clear();

        // Entrée ".." sauf à la racine
        if self.current_dir.parent().is_some() {
            self.entries.push(DirEntry {
                path:     self.current_dir.parent().unwrap().to_path_buf(),
                name:     ".. (dossier parent)".into(),
                is_dir:   true,
                is_media: false,
            });
        }

        let Ok(read_dir) = std::fs::read_dir(&self.current_dir) else { return };

        let mut dirs_vec  = Vec::new();
        let mut files_vec = Vec::new();

        for entry in read_dir.flatten() {
            let path     = entry.path();
            let name     = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let is_dir   = path.is_dir();
            let is_media = !is_dir && is_media_file(&path);

            let entry = DirEntry { path, name, is_dir, is_media };
            if is_dir { dirs_vec.push(entry); } else { files_vec.push(entry); }
        }

        dirs_vec.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files_vec.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        self.entries.extend(dirs_vec);
        self.entries.extend(files_vec);
    }
}

/// Fenêtre de navigation fichiers.
/// Appelle `on_open(path)` quand l'utilisateur double-clique sur un fichier média.
pub fn show<F>(ctx: &Context, open: &mut bool, mut on_open: F)
where
    F: FnMut(String),
{
    // État persistant via egui Id
    let mut browser = ctx.memory_mut(|mem| {
        mem.data
            .get_temp_mut_or_insert_with(egui::Id::new("file_browser"), FileBrowser::new)
            .clone()
    });

    let mut do_open: Option<String> = None;
    let mut navigated = false;

    Window::new("Ouvrir un fichier")
        .open(open)
        .resizable(true)
        .default_size([700.0, 500.0])
        .show(ctx, |ui| {
            // Barre d'adresse
            ui.horizontal(|ui| {
                if ui.button("⬆ Parent").clicked() {
                    if let Some(p) = browser.current_dir.parent() {
                        browser.current_dir = p.to_path_buf();
                        navigated = true;
                    }
                }
                ui.label(
                    RichText::new(browser.current_dir.to_string_lossy().as_ref())
                        .monospace()
                        .size(11.0),
                );
            });

            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut browser.filter);
                if ui.small_button("✕").clicked() {
                    browser.filter.clear();
                }
            });

            ui.separator();

            // Liste d'entrées
            let filter_lc = browser.filter.to_lowercase();

            ScrollArea::vertical().show(ui, |ui| {
                for entry in &browser.entries {
                    // Filtre texte
                    if !filter_lc.is_empty()
                        && !entry.name.to_lowercase().contains(&filter_lc)
                        && !entry.is_dir
                    {
                        continue;
                    }

                    let icon  = if entry.is_dir { "📁" } else if entry.is_media { "🎬" } else { "📄" };
                    let color = if entry.is_dir {
                        Color32::from_rgb(200, 170, 80)
                    } else if entry.is_media {
                        Color32::from_rgb(130, 200, 130)
                    } else {
                        Color32::from_gray(140)
                    };

                    let label = format!("{icon}  {}", entry.name);
                    let resp = ui.add(
                        egui::Button::new(RichText::new(&label).color(color).size(13.0))
                            .min_size(Vec2::new(ui.available_width(), 24.0))
                            .fill(Color32::TRANSPARENT),
                    );

                    if resp.double_clicked() {
                        if entry.is_dir {
                            browser.current_dir = entry.path.clone();
                            navigated = true;
                        } else if entry.is_media {
                            do_open = Some(entry.path.to_string_lossy().to_string());
                        }
                    }
                }
            });

            ui.separator();
            // Lecteurs Windows
            #[cfg(windows)]
            ui.horizontal(|ui| {
                ui.label("Lecteurs :");
                for letter in b'A'..=b'Z' {
                    let drive = format!("{}:\\", letter as char);
                    if Path::new(&drive).exists() {
                        if ui.small_button(&drive).clicked() {
                            browser.current_dir = PathBuf::from(&drive);
                            navigated = true;
                        }
                    }
                }
            });
        });

    if navigated { browser.refresh(); }

    // Persist l'état
    ctx.memory_mut(|mem| {
        mem.data.insert_temp(egui::Id::new("file_browser"), browser);
    });

    if let Some(path) = do_open {
        *open = false;
        on_open(path);
    }
}

fn is_media_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}
