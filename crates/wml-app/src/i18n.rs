//! UI string tables for English and French.
//!
//! Persistent labels and buttons are translated here; transient status-line
//! messages remain in English for now. Pick a table with [`strings`] each frame
//! based on the active [`Language`].

use wml_core::config::Language;

/// Translatable UI strings.
pub struct Strings {
    pub tab_library: &'static str,
    pub tab_search: &'static str,
    pub language: &'static str,
    pub controller: &'static str,

    // Library tab
    pub maps_folder: &'static str,
    pub browse: &'static str,
    pub refresh_maps: &'static str,
    pub bakkesmod: &'static str,
    pub connected: &'static str,
    pub not_reachable: &'static str,
    pub disabled: &'static str,
    pub recheck: &'static str,
    pub launch_unavailable: &'static str,
    pub not_connected: &'static str,
    pub filter: &'static str,
    pub by: &'static str,
    pub needs_extraction: &'static str,
    pub play: &'static str,
    pub open_folder: &'static str,

    // Search tab
    pub search_a_workshop: &'static str,
    pub search: &'static str,
    pub browse_maps: &'static str,
    pub prev: &'static str,
    pub next: &'static str,
    pub page: &'static str,
    pub download: &'static str,
    pub no_release: &'static str,
    pub releases: &'static str,
}

/// The string table for `lang`.
pub fn strings(lang: Language) -> &'static Strings {
    match lang {
        Language::English => &EN,
        Language::French => &FR,
    }
}

static EN: Strings = Strings {
    tab_library: "Map Loader",
    tab_search: "Search Workshop",
    language: "Language",
    controller: "Controller",

    maps_folder: "Maps folder:",
    browse: "Browse…",
    refresh_maps: "Refresh Maps",
    bakkesmod: "BakkesMod:",
    connected: "Connected",
    not_reachable: "Not reachable",
    disabled: "Disabled",
    recheck: "Recheck",
    launch_unavailable: "— in-game launch unavailable; use Open Folder and load in-game.",
    not_connected: "BakkesMod not connected",
    filter: "Search:",
    by: "By",
    needs_extraction: "Needs extraction",
    play: "▶ Play",
    open_folder: "Open Folder",

    search_a_workshop: "Search a workshop:",
    search: "Search",
    browse_maps: "Browse",
    prev: "◀ Prev",
    next: "Next ▶",
    page: "Page",
    download: "Download",
    no_release: "This map has no downloadable release.",
    releases: "Releases",
};

static FR: Strings = Strings {
    tab_library: "Charger Map",
    tab_search: "Rechercher Workshop",
    language: "Langue",
    controller: "Manette",

    maps_folder: "Dossier des maps :",
    browse: "Parcourir…",
    refresh_maps: "Rafraîchir",
    bakkesmod: "BakkesMod :",
    connected: "Connecté",
    not_reachable: "Injoignable",
    disabled: "Désactivé",
    recheck: "Revérifier",
    launch_unavailable:
        "— lancement en jeu indisponible ; utilisez Ouvrir le dossier et chargez en jeu.",
    not_connected: "BakkesMod non connecté",
    filter: "Rechercher :",
    by: "Par",
    needs_extraction: "À extraire",
    play: "▶ Jouer",
    open_folder: "Ouvrir le dossier",

    search_a_workshop: "Rechercher un workshop :",
    search: "Rechercher",
    browse_maps: "Parcourir",
    prev: "◀ Préc.",
    next: "Suiv. ▶",
    page: "Page",
    download: "Télécharger",
    no_release: "Cette map n'a aucune version téléchargeable.",
    releases: "Versions",
};
