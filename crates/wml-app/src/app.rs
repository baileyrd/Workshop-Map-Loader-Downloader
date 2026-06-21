//! The egui application: top-level state and the two main tabs.
//!
//! This is an immediate-mode UI, mirroring the plugin's ImGui structure:
//! a "Library" tab (local maps) and a "Search" tab (catalog).
//!
//! Async work runs on a shared Tokio runtime and reports back to the UI thread
//! through an `mpsc` channel; the task calls [`egui::Context::request_repaint`]
//! so results appear without requiring further input. Each search is tagged with
//! a monotonically increasing sequence number so stale responses are ignored.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use wml_core::catalog::{CatalogClient, MapResult};
use wml_core::config::Config;
use wml_core::library::Map;

/// Which top-level tab is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Library,
    Search,
}

/// Messages sent from background search tasks back to the UI thread.
enum SearchMsg {
    Results {
        seq: u64,
        page: u32,
        items: Vec<MapResult>,
    },
    Error {
        seq: u64,
        message: String,
    },
}

/// Root application state.
pub struct WmlApp {
    /// Shared Tokio runtime handle for spawning async tasks.
    rt: Arc<tokio::runtime::Runtime>,
    catalog: CatalogClient,
    config: Config,
    config_path: PathBuf,
    tab: Tab,

    // Library tab
    maps: Vec<Map>,
    filter: String,

    // Search tab
    search_query: String,
    results: Vec<MapResult>,
    searching: bool,
    search_seq: u64,
    page: u32,
    search_tx: Sender<SearchMsg>,
    search_rx: Receiver<SearchMsg>,

    status: String,
}

impl WmlApp {
    pub fn new(rt: Arc<tokio::runtime::Runtime>, config: Config, config_path: PathBuf) -> Self {
        let maps = wml_core::library::scan_maps(&config.maps_folder).unwrap_or_default();
        let (search_tx, search_rx) = std::sync::mpsc::channel();
        Self {
            rt,
            catalog: CatalogClient::default(),
            config,
            config_path,
            tab: Tab::default(),
            maps,
            filter: String::new(),
            search_query: String::new(),
            results: Vec::new(),
            searching: false,
            search_seq: 0,
            page: 1,
            search_tx,
            search_rx,
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

    /// Spawn a catalog search for the current query at `page`.
    fn start_search(&mut self, ctx: &egui::Context, page: u32) {
        let query = self.search_query.trim().to_string();
        self.search_seq += 1;
        let seq = self.search_seq;
        self.searching = true;
        self.page = page;
        self.status = if query.is_empty() {
            format!("Browsing maps (page {page})…")
        } else {
            format!("Searching for \"{query}\" (page {page})…")
        };

        let catalog = self.catalog.clone();
        let tx = self.search_tx.clone();
        let ctx = ctx.clone();
        self.rt.spawn(async move {
            let msg = match catalog.search(&query, page).await {
                Ok(items) => SearchMsg::Results { seq, page, items },
                Err(e) => SearchMsg::Error {
                    seq,
                    message: e.to_string(),
                },
            };
            // Ignore send errors: the app may have shut down.
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    /// Drain any pending search results, discarding stale (superseded) ones.
    fn poll_search(&mut self) {
        while let Ok(msg) = self.search_rx.try_recv() {
            match msg {
                SearchMsg::Results { seq, page, items } if seq == self.search_seq => {
                    self.status = format!("Found {} map(s) on page {page}.", items.len());
                    self.results = items;
                    self.page = page;
                    self.searching = false;
                }
                SearchMsg::Error { seq, message } if seq == self.search_seq => {
                    self.status = format!("Search failed: {message}");
                    self.searching = false;
                }
                _ => { /* stale response from a superseded search */ }
            }
        }
    }
}

impl eframe::App for WmlApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_search();

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Library, "Map Loader");
                ui.selectable_value(&mut self.tab, Tab::Search, "Search Workshop");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.searching {
                    ui.spinner();
                }
                ui.label(&self.status);
            });
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
        let ctx = ui.ctx().clone();
        // Page to (re-)search this frame, decided by the controls below.
        let mut start: Option<u32> = None;

        ui.horizontal(|ui| {
            ui.label("Search a workshop:");
            let resp = ui.text_edit_singleline(&mut self.search_query);
            let submitted =
                resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui
                .add_enabled(!self.searching, egui::Button::new("Search"))
                .clicked()
                || submitted
            {
                start = Some(1);
            }
            if ui
                .add_enabled(!self.searching, egui::Button::new("Browse"))
                .clicked()
            {
                self.search_query.clear();
                start = Some(1);
            }
        });

        ui.horizontal(|ui| {
            if self.page > 1
                && ui
                    .add_enabled(!self.searching, egui::Button::new("◀ Prev"))
                    .clicked()
            {
                start = Some(self.page - 1);
            }
            ui.label(format!("Page {}", self.page));
            if ui
                .add_enabled(!self.searching, egui::Button::new("Next ▶"))
                .clicked()
            {
                start = Some(self.page + 1);
            }
        });

        ui.separator();

        // Index of a card whose "Download" button was clicked this frame.
        let mut download_idx: Option<usize> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, result) in self.results.iter().enumerate() {
                    render_result_card(ui, result, i, &mut download_idx);
                }
            });
        });

        // Apply deferred actions after the immutable borrow of `self.results` ends.
        if let Some(page) = start {
            self.start_search(&ctx, page);
        }
        if let Some(i) = download_idx {
            let name = self.results[i].name.clone();
            self.status = format!("Download for \"{name}\" is the next wiring step.");
        }
    }
}

/// Render one search result as a fixed-width card. Free function so it doesn't
/// borrow `self` while `self.results` is being iterated.
fn render_result_card(
    ui: &mut egui::Ui,
    result: &MapResult,
    index: usize,
    download_idx: &mut Option<usize>,
) {
    ui.group(|ui| {
        ui.set_width(190.0);
        ui.vertical(|ui| {
            if result.preview_url.is_empty() {
                ui.allocate_space(egui::vec2(180.0, 110.0));
            } else {
                ui.add(
                    egui::Image::from_uri(result.preview_url.clone())
                        .fit_to_exact_size(egui::vec2(180.0, 110.0)),
                );
            }

            let title = ui.label(egui::RichText::new(&result.name).strong());
            if !result.description.is_empty() {
                title.on_hover_text(&result.description);
            }
            if !result.author.is_empty() {
                ui.label(format!("By {}", result.author));
            }

            let has_release = !result.releases.is_empty();
            if ui
                .add_enabled(has_release, egui::Button::new("Download"))
                .clicked()
            {
                *download_idx = Some(index);
            }
        });
    });
}
