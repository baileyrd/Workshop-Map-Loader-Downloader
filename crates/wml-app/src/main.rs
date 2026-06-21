//! Standalone desktop entry point for the Workshop Map Loader & Downloader.

mod app;

use std::sync::Arc;

use anyhow::Context;

use wml_core::config::Config;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config_path = wml_core::paths::config_file().context("resolving config path")?;
    let config = Config::load(&config_path).context("loading config")?;

    // One multi-threaded Tokio runtime, shared with the UI for async tasks
    // (catalog search, downloads). egui itself runs on the main thread.
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("building tokio runtime")?,
    );

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Workshop Map Loader & Downloader",
        native_options,
        Box::new(move |cc| {
            // Enables `egui::Image::from_uri` to fetch+decode remote previews.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::WmlApp::new(rt, config, config_path)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}
