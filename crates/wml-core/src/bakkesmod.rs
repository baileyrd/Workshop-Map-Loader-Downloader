//! The BakkesMod bridge: launching a map in a running game.
//!
//! A standalone app cannot load a map into Rocket League by itself — the plugin
//! did it by issuing the BakkesMod console command `load_workshop "<path>"`. To
//! reproduce that, this module connects to BakkesMod's **RCON WebSocket**
//! (default `127.0.0.1:9876`, gated by `rcon_password`) and forwards commands.
//!
//! Strategy: *bridge + graceful fallback*. When BakkesMod is reachable we send
//! the command; otherwise the app stays in manager-only mode and the UI offers
//! "open maps folder" instead of an in-app launch.
//!
//! Status: the wire protocol is scaffolded but not yet implemented. Wiring it up
//! means adding a WebSocket client (e.g. `tokio-tungstenite`), performing the
//! `rcon_password <password>` auth handshake, then sending console commands.

use std::path::Path;

use crate::config::BakkesModConfig;
use crate::error::{Result, WmlError};

/// Whether the in-app launch bridge is usable right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeStatus {
    /// Connected and authenticated; `load_workshop` will work.
    Connected,
    /// BakkesMod not reachable; operate in manager-only mode.
    Unavailable,
    /// The bridge is disabled in config.
    Disabled,
}

/// Connection to BakkesMod's RCON server.
pub struct BakkesModBridge {
    #[allow(dead_code)]
    config: BakkesModConfig,
}

impl BakkesModBridge {
    /// Attempt to connect and authenticate. Not yet implemented.
    pub async fn connect(config: BakkesModConfig) -> Result<Self> {
        if !config.enabled {
            return Err(WmlError::Config("bakkesmod bridge disabled".into()));
        }
        // TODO: open ws://{host}:{port}, send `rcon_password <password>`, await ok.
        Err(WmlError::NotImplemented("BakkesModBridge::connect"))
    }

    /// Send a raw console command over RCON. Not yet implemented.
    pub async fn execute(&mut self, _command: &str) -> Result<()> {
        Err(WmlError::NotImplemented("BakkesModBridge::execute"))
    }

    /// Tell the game to load a workshop map by its `.upk`/`.udk` path.
    pub async fn load_workshop(&mut self, upk_path: &Path) -> Result<()> {
        let command = format!("load_workshop \"{}\"", upk_path.display());
        self.execute(&command).await
    }
}

/// Quick reachability probe (e.g. TCP connect to the RCON port) without a full
/// handshake. Not yet implemented; returns [`BridgeStatus::Disabled`] when off.
pub async fn probe(config: &BakkesModConfig) -> BridgeStatus {
    if !config.enabled {
        return BridgeStatus::Disabled;
    }
    // TODO: attempt a short TCP/WS connect to decide Connected vs Unavailable.
    BridgeStatus::Unavailable
}
