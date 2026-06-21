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
//! ## Protocol note
//!
//! Based on the commonly documented BakkesMod RCON handshake: open a WebSocket
//! to `ws://{host}:{port}`, send the password as the first text frame, and
//! expect a reply containing `authyes`. Subsequent text frames are executed as
//! console commands. This has **not** been verified against a live BakkesMod
//! install in this environment; if the handshake differs, only [`BakkesModBridge::connect`]
//! needs adjusting.

use std::path::Path;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

use crate::config::BakkesModConfig;
use crate::error::{Result, WmlError};

/// How long to wait for the auth reply before giving up.
const AUTH_TIMEOUT: Duration = Duration::from_secs(3);
/// How long to wait for a TCP connection during [`probe`].
const PROBE_TIMEOUT: Duration = Duration::from_millis(600);

/// Whether the in-app launch bridge is usable right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeStatus {
    /// The RCON port is reachable; `load_workshop` should work.
    Connected,
    /// BakkesMod not reachable; operate in manager-only mode.
    Unavailable,
    /// The bridge is disabled in config.
    Disabled,
}

/// An authenticated connection to BakkesMod's RCON server.
pub struct BakkesModBridge {
    ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
}

impl BakkesModBridge {
    /// Open and authenticate an RCON connection.
    pub async fn connect(config: BakkesModConfig) -> Result<Self> {
        if !config.enabled {
            return Err(WmlError::Bridge("bridge disabled in config".into()));
        }

        let url = format!("ws://{}:{}", config.host, config.port);
        let (mut ws, _resp) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| WmlError::Bridge(format!("connect failed: {e}")))?;

        // Authenticate: the password is the first frame.
        ws.send(Message::Text(config.password.clone()))
            .await
            .map_err(|e| WmlError::Bridge(format!("sending password failed: {e}")))?;

        // Expect a reply acknowledging the auth.
        let reply = tokio::time::timeout(AUTH_TIMEOUT, ws.next())
            .await
            .map_err(|_| WmlError::Bridge("timed out waiting for auth reply".into()))?;

        match reply {
            Some(Ok(Message::Text(t))) if t.contains("authyes") => Ok(Self { ws }),
            Some(Ok(Message::Text(t))) => {
                Err(WmlError::Bridge(format!("authentication rejected: {t}")))
            }
            Some(Ok(_)) => Err(WmlError::Bridge(
                "unexpected non-text reply during auth".into(),
            )),
            Some(Err(e)) => Err(WmlError::Bridge(e.to_string())),
            None => Err(WmlError::Bridge("connection closed during auth".into())),
        }
    }

    /// Send a raw console command over RCON.
    pub async fn execute(&mut self, command: &str) -> Result<()> {
        self.ws
            .send(Message::Text(command.to_string()))
            .await
            .map_err(|e| WmlError::Bridge(format!("sending command failed: {e}")))
    }

    /// Tell the game to load a workshop map by its `.upk`/`.udk` path.
    pub async fn load_workshop(&mut self, upk_path: &Path) -> Result<()> {
        let command = format!("load_workshop \"{}\"", upk_path.display());
        self.execute(&command).await
    }

    /// Close the connection cleanly.
    pub async fn close(mut self) {
        let _ = self.ws.close(None).await;
    }
}

/// Quick reachability probe: a short TCP connect to the RCON port. A successful
/// connect is reported as [`BridgeStatus::Connected`] (the full auth handshake
/// happens later, when a map is actually launched).
pub async fn probe(config: &BakkesModConfig) -> BridgeStatus {
    if !config.enabled {
        return BridgeStatus::Disabled;
    }
    let addr = format!("{}:{}", config.host, config.port);
    match tokio::time::timeout(PROBE_TIMEOUT, TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => BridgeStatus::Connected,
        _ => BridgeStatus::Unavailable,
    }
}
