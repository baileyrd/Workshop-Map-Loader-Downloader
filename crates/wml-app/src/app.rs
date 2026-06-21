//! The egui application: top-level state and the two main tabs.
//!
//! This is an immediate-mode UI, mirroring the plugin's ImGui structure:
//! a "Library" tab (local maps) and a "Search" tab (catalog).
//!
//! Async work runs on a shared Tokio runtime and reports back to the UI thread
//! through `mpsc` channels; tasks call [`egui::Context::request_repaint`] so
//! results/progress appear without requiring further input. Searches carry a
//! monotonic sequence number so stale responses are ignored.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use wml_core::catalog::{CatalogClient, MapResult, Release};
use wml_core::config::Config;
use wml_core::download::Progress;
use wml_core::install::InstallStage;
use wml_core::library::Map;

/// Which top-level tab is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Library,
    Search,
}

/// Messages from background search tasks back to the UI thread.
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

/// Messages from the background download/install task.
enum DownloadMsg {
    Progress(Progress),
    Extracting,
    Done(PathBuf),
    Error(String),
}

/// Root application state.
pub struct WmlApp {
    /// Shared Tokio runtime handle for spawning async tasks.
    rt: Arc<tokio::runtime::Runtime>,
    catalog: CatalogClient,
    http: reqwest::Client,
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
    /// Result index whose releases are being picked for download, if any.
    release_pick: Option<usize>,

    // Download
    downloading: bool,
    download_label: String,
    download_progress: Option<Progress>,
    download_tx: Sender<DownloadMsg>,
    download_rx: Receiver<DownloadMsg>,

    status: String,
}

impl WmlApp {
    pub fn new(rt: Arc<tokio::runtime::Runtime>, config: Config, config_path: PathBuf) -> Self {
        let maps = wml_core::library::scan_maps(&config.maps_folder).unwrap_or_default();
        let (search_tx, search_rx) = std::sync::mpsc::channel();
        let (download_tx, download_rx) = std::sync::mpsc::channel();
        Self {
            rt,
            catalog: CatalogClient::default(),
            http: reqwest::Client::new(),
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
            release_pick: None,
            downloading: false,
            download_label: String::new(),
            download_progress: None,
            download_tx,
            download_rx,
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
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }

    /// Drain pending search results, discarding stale (superseded) ones.
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

    /// Spawn an install of `release` of `map` into the configured maps folder.
    fn start_download(&mut self, ctx: &egui::Context, map: MapResult, release: Release) {
        if !self.config.maps_folder.is_dir() {
            self.status = "Set a valid maps folder (Map Loader tab) first.".into();
            return;
        }
        if self.downloading {
            self.status = "A download is already in progress.".into();
            return;
        }

        self.downloading = true;
        self.download_label = map.name.clone();
        self.download_progress = None;
        self.status = format!("Downloading \"{}\"…", map.name);

        let client = self.http.clone();
        let maps_folder = self.config.maps_folder.clone();
        // A dedicated clone for the progress callback so the closure owns a
        // `Send` sender (std `Sender` is not `Sync`, so we can't share &tx).
        let progress_tx = self.download_tx.clone();
        let progress_ctx = ctx.clone();
        let done_tx = self.download_tx.clone();
        let done_ctx = ctx.clone();

        self.rt.spawn(async move {
            let result = wml_core::install::install_map(
                &client,
                &maps_folder,
                &map,
                &release,
                |stage| {
                    let msg = match stage {
                        InstallStage::Downloading(p) => DownloadMsg::Progress(p),
                        InstallStage::Extracting => DownloadMsg::Extracting,
                        _ => return,
                    };
                    let _ = progress_tx.send(msg);
                    progress_ctx.request_repaint();
                },
            )
            .await;

            let final_msg = match result {
                Ok(path) => DownloadMsg::Done(path),
                Err(e) => DownloadMsg::Error(e.to_string()),
            };
            let _ = done_tx.send(final_msg);
            done_ctx.request_repaint();
        });
    }

    /// Drain pending download progress/completion messages.
    fn poll_download(&mut self) {
        while let Ok(msg) = self.download_rx.try_recv() {
            match msg {
                DownloadMsg::Progress(p) => self.download_progress = Some(p),
                DownloadMsg::Extracting => {
                    self.download_progress = None;
                    self.status = format!("Extracting \"{}\"…", self.download_label);
                }
                DownloadMsg::Done(path) => {
                    self.downloading = false;
                    self.download_progress = None;
                    self.status = format!("Installed \"{}\" to {}", self.download_label, path.display());
                    self.refresh_maps();
                }
                DownloadMsg::Error(e) => {
                    self.downloading = false;
                    self.download_progress = None;
                    self.status = format!("Download failed: {e}");
                }
            }
        }
    }
}

impl eframe::App for WmlApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_search();
        self.poll_download();

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Library, "Map Loader");
                ui.selectable_value(&mut self.tab, Tab::Search, "Search Workshop");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if self.downloading {
                let frac = self.download_progress.and_then(|p| p.fraction()).unwrap_or(0.0);
                let text = match self.download_progress {
                    Some(p) => match p.total {
                        Some(t) => format!("{} / {}", human_bytes(p.downloaded), human_bytes(t)),
                        None => human_bytes(p.downloaded),
                    },
                    None => "preparing…".to_string(),
                };
                ui.add(egui::ProgressBar::new(frac).text(text));
            }
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

        self.render_release_picker(ctx);
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
            let submitted = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
        let download_enabled = !self.downloading;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, result) in self.results.iter().enumerate() {
                    render_result_card(ui, result, i, download_enabled, &mut download_idx);
                }
            });
        });

        // Apply deferred actions after the immutable borrow of `self.results` ends.
        if let Some(page) = start {
            self.start_search(&ctx, page);
        }
        if let Some(i) = download_idx {
            match self.results[i].releases.len() {
                0 => self.status = "This map has no downloadable release.".into(),
                1 => {
                    let map = self.results[i].clone();
                    let release = map.releases[0].clone();
                    self.start_download(&ctx, map, release);
                }
                _ => self.release_pick = Some(i),
            }
        }
    }

    /// A small window to choose which release to install when a map has several.
    fn render_release_picker(&mut self, ctx: &egui::Context) {
        let Some(idx) = self.release_pick else {
            return;
        };
        if idx >= self.results.len() {
            self.release_pick = None;
            return;
        }

        let mut open = true;
        let mut chosen: Option<usize> = None;
        let busy = self.downloading;
        let results = &self.results;
        egui::Window::new(format!("Releases — {}", results[idx].name))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                for (ri, rel) in results[idx].releases.iter().enumerate() {
                    let label = if rel.tag_name.is_empty() {
                        rel.name.clone()
                    } else {
                        rel.tag_name.clone()
                    };
                    if ui
                        .add_enabled(!busy, egui::Button::new(label))
                        .clicked()
                    {
                        chosen = Some(ri);
                    }
                }
            });

        if let Some(ri) = chosen {
            let map = self.results[idx].clone();
            let release = map.releases[ri].clone();
            self.start_download(ctx, map, release);
            self.release_pick = None;
        } else if !open {
            self.release_pick = None;
        }
    }
}

/// Render one search result as a fixed-width card. Free function so it doesn't
/// borrow `self` while `self.results` is being iterated.
fn render_result_card(
    ui: &mut egui::Ui,
    result: &MapResult,
    index: usize,
    enabled: bool,
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
                .add_enabled(enabled && has_release, egui::Button::new("Download"))
                .clicked()
            {
                *download_idx = Some(index);
            }
        });
    });
}

/// Format a byte count compactly (e.g. `12.3 MB`).
fn human_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
