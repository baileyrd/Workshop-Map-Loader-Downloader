//! The egui application: top-level state and the two main tabs.
//!
//! This is an immediate-mode UI, mirroring the plugin's ImGui structure:
//! a "Library" tab (local maps) and a "Search" tab (catalog). Async work
//! (catalog search, downloads) runs on a Tokio runtime and reports back to the
//! UI thread; that wiring is stubbed here and filled in next.

use std::path::PathBuf;
use std::sync::Arc;

use wml_core::config::Config;
use wml_core::library::Map;

/// Which top-level tab is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Library,
    Search,
}

/// Root application state.
pub struct WmlApp {
    /// Shared Tokio runtime handle for spawning async tasks.
    #[allow(dead_code)]
    rt: Arc<tokio::runtime::Runtime>,
    config: Config,
    config_path: PathBuf,
    tab: Tab,
    /// Currently loaded local maps.
    maps: Vec<Map>,
    /// Quick-search filter text (library tab).
    filter: String,
    /// Search box text (catalog tab).
    search_query: String,
    status: String,
}

impl WmlApp {
    pub fn new(rt: Arc<tokio::runtime::Runtime>, config: Config, config_path: PathBuf) -> Self {
        let maps = wml_core::library::scan_maps(&config.maps_folder).unwrap_or_default();
        Self {
            rt,
            config,
            config_path,
            tab: Tab::default(),
            maps,
            filter: String::new(),
            search_query: String::new(),
            status: String::new(),
        }
    }

    fn refresh_maps(&mut self) {
        match wml_core::library::scan_maps(&self.config.maps_folder) {
            Ok(maps) => {
                self.status = format!("Loaded {} map(s).", maps.len());
                self.maps = maps;
            }
            Err(e) => self.status = format!("Failed to scan maps: {e}"),
        }
    }

    fn save_config(&mut self) {
        if let Err(e) = self.config.save(&self.config_path) {
            self.status = format!("Failed to save config: {e}");
        }
    }
}

impl eframe::App for WmlApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Library, "Map Loader");
                ui.selectable_value(&mut self.tab, Tab::Search, "Search Workshop");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Library => self.library_tab(ui),
            Tab::Search => self.search_tab(ui),
        });
    }
}

impl WmlApp {
    fn library_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Maps folder:");
            let mut folder = self.config.maps_folder.display().to_string();
            if ui.text_edit_singleline(&mut folder).changed() {
                self.config.maps_folder = PathBuf::from(folder);
            }
            if ui.button("Browse…").clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.config.maps_folder = dir;
                    self.save_config();
                    self.refresh_maps();
                }
            }
            if ui.button("Refresh Maps").clicked() {
                self.refresh_maps();
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.filter);
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            let filtered = wml_core::library::filter_maps(&self.maps, &self.filter);
            for map in filtered {
                ui.group(|ui| {
                    ui.label(egui::RichText::new(&map.name).strong());
                    if !map.author.is_empty() {
                        ui.label(format!("By {}", map.author));
                    }
                    if map.needs_extraction() {
                        ui.colored_label(egui::Color32::YELLOW, "Needs extraction");
                    }
                    // TODO: preview image, "Play" (via BakkesMod bridge), context menu.
                });
            }
        });
    }

    fn search_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search a workshop:");
            ui.text_edit_singleline(&mut self.search_query);
            if ui.button("Search").clicked() {
                // TODO: spawn CatalogClient::search on `self.rt`, stream results back.
                self.status = format!("Searching for \"{}\"… (not wired yet)", self.search_query);
            }
        });
        ui.separator();
        ui.label("Results will appear here.");
    }
}
