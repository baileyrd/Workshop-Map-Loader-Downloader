//! `wml-core` — the game-independent heart of the Workshop Map Loader.
//!
//! Everything in this crate runs without Rocket League or BakkesMod present,
//! with one exception: [`bakkesmod`], the optional bridge used to launch a map
//! in a running game. The UI crate (`wml-app`) depends on this crate and adds
//! only rendering and wiring on top.
//!
//! Module map:
//! - [`library`]  — scan the maps folder, parse `.json` sidecars, model a `Map`.
//! - [`catalog`]  — search/download metadata from rocketleaguemaps.us.
//! - [`download`] — streamed downloads with progress + in-process zip extraction.
//! - [`install`]  — install a catalog map into the local library.
//! - [`config`]   — typed config, serialized as TOML (migrates the old `.cfg`).
//! - [`paths`]    — locate the game install, `CookedPCConsole`, and app dirs.
//! - [`bakkesmod`]— RCON bridge that sends `load_workshop` to a running game.

pub mod bakkesmod;
pub mod catalog;
pub mod config;
pub mod download;
pub mod error;
pub mod install;
pub mod library;
pub mod paths;

pub use error::{Result, WmlError};
