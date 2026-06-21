# Rust Standalone Conversion — Design & Analysis

This document reviews the existing **Workshop Map Loader & Downloader** BakkesMod
plugin and lays out a plan to rebuild it as its **own standalone desktop
application written in Rust**.

> **Status:** design / analysis only. No Rust code has been written yet.
> **Launch strategy decision:** _Bridge + graceful fallback_ (see §2 and §6).

---

## 1. What this project is today

It is a **BakkesMod plugin** for Rocket League — a Windows DLL
(`ConfigurationType: DynamicLibrary`, output to `plugins\`) that BakkesMod
injects into the running game. It is **not** a standalone program: there is no
`main()`, no window of its own, and no event loop. It renders entirely through
the game's shared ImGui overlay and runs inside the game process.

Core features:

| Feature | How it works today |
|---|---|
| **Browse / launch local maps** | Scans a user folder for subfolders, each containing a `.upk` map, a `.json` metadata sidecar, and a preview image. Renders them as tiles/list. Clicking issues `load_workshop "<path>"` to BakkesMod. |
| **Search & download maps** | Queries `celab.jetfox.ovh` (a GitLab API behind rocketleaguemaps.us) for maps + releases, downloads the release zip, extracts it, writes a `.json` sidecar. |
| **Download "workshop textures"** | Pulls a ~47 MB zip from a hardcoded Discord CDN link and extracts it into the game's `CookedPCConsole`. |
| **Add maps manually** | Copies a user-selected `.upk` + image into the maps folder and generates the `.json`. |

Plus: French/English localization, a hand-rolled ImGui file explorer, XInput
controller navigation, and a custom `.cfg` config file.

The author's own README is candid: *"This was my first ever c++ project... the
code is pretty bad."* It is functional but rough.

### Source layout

```
Pluginx64/
├── WorkshopMapLoader.{h,cpp}            # plugin lifecycle, logic, utils
├── WorkshopMapLoaderGUI.cpp             # ~2600 lines: all rendering + wiring
├── WorkshopMapLoaderSettingsGUI.cpp     # BakkesMod settings tab
├── Gamepad.{h,cpp}                      # XInput controller wrapper
├── pch.{h,cpp}                          # precompiled headers, lib pragmas
├── IMGUI/                               # vendored Dear ImGui (~25 files)
└── LibrariesUsed/                       # cpr, libcurl, jsoncpp, zlib (prebuilt)
data/WorkshopMapLoader/                  # logos, NoPreview.jpg, cfg, unzip.bat
```

---

## 2. The single most important finding

**The plugin never loads a map itself.** Every "launch" is
(`WorkshopMapLoaderGUI.cpp:1196`):

```cpp
cvarManager->executeCommand("load_workshop \"" + map.Folder.string() + "/" + map.UpkFile.filename().string() + "\"");
```

`load_workshop` is a **BakkesMod console command**. The plugin is only a UI that
tells BakkesMod (already inside the game) to do the work. This single fact
dictates the entire standalone design:

> A standalone external `.exe` **cannot** load a map into Rocket League on its
> own. It cannot render UI inside the game, and it cannot execute console
> commands in the game's address space. To actually launch a map, a standalone
> app must talk *back* to BakkesMod over its **RCON WebSocket** (default port
> `9876`, gated by `rcon_password`) and send `load_workshop`.

So the conversion splits cleanly:

- **Fully standalone (no game needed):** searching, downloading, unzipping,
  library management, metadata, textures, manual-add. ~80% of the code. Ports
  cleanly to Rust.
- **Requires a bridge:** the actual "play this map" action.

### Decision: bridge + graceful fallback

The app will **use RCON when BakkesMod is detected/running** (full feature
parity, including in-app launching), and otherwise **behave as a manager-only
app** — searching, downloading, organizing, and opening the maps folder so the
game/BakkesMod can load maps the normal way. The UI surfaces BakkesMod
connection state and disables the in-app "Play" action with a clear hint when no
bridge is available.

---

## 3. Dependency & coupling audit

What ties the code to BakkesMod / Windows, and the Rust replacement:

| Current dependency | Used for | Rust replacement |
|---|---|---|
| `BakkesModPlugin` / `PluginWindow` base classes | Plugin lifecycle + overlay window | Native window via `eframe`/`egui` (own event loop) |
| `cvarManager->executeCommand("load_workshop ...")` | **Launching maps** | RCON WebSocket client (`tokio-tungstenite`) — *requires BakkesMod*; fallback otherwise |
| `cvarManager->log` | Logging | `tracing` + a subscriber |
| `gameWrapper->GetBakkesModPath()` / `current_path()` | Locating game + bakkesmod dirs | Read Steam/Epic install paths (registry / `steamapps`); config-driven overrides |
| `ImageWrapper` | Loading PNG/JPG into ImGui textures | `image` crate + `egui` texture handles |
| `HttpWrapper::SendCurlRequest` + `cpr`/`libcurl` | HTTP GET / download w/ progress | `reqwest` (async + streaming progress) |
| `jsoncpp` | Parsing API + sidecar JSON | `serde` + `serde_json` |
| Dear ImGui (vendored) | Entire UI | `egui` (immediate-mode — near 1:1 port of existing UI logic) |
| `system("powershell ... Expand-Archive")` / `.bat` + VBScript | Extracting downloaded zips | `zip` crate — in-process, cross-platform, no shelling out |
| `XInput` + `Gamepad.cpp` | Controller navigation | `gilrs` (cross-platform) — or cut |
| `ShellExecute` | Open folders / URLs | `open` crate |
| `GetLogicalDrives` + custom file-explorer popup | Folder/file picking | `rfd` native file dialog — deletes ~250 lines |
| `MultiByteToWideChar` / `s2ws` | Path / wide-string juggling | Native UTF-8 `String` / `PathBuf` — most of this disappears |
| Hand-rolled `.cfg` read/write | Config | `serde` + `toml` (with migration from old `.cfg`) |
| Custom HTML stripper / substring finder | Cleaning API descriptions | `ammonia` / `scraper` or a small helper |

**Net effect:** roughly half the existing code is Windows/BakkesMod plumbing and
string-encoding workarounds that **vanish** in idiomatic Rust. The unzip-via-batch
hack, wide-string conversions, manual drive enumeration, and the custom file
explorer are each replaced by a single mature crate.

---

## 4. Code-quality notes (what NOT to carry over)

- **Unsafe concurrency.** Raw `std::thread(...).detach()` + `Sleep()` polling +
  shared mutable bools. `RLMAPS_DownloadWorkshop` spins
  `while (UserIsChoosingYESorNO) Sleep(100)` on a detached thread, then mutates
  `RLMAPS_MapResultList` from background threads with no synchronization — a data
  race and the likely cause of the documented "random crash when searching."
  **Rust fix:** `tokio` tasks + `mpsc` channels to hand results back to the UI
  thread, or `Arc<Mutex<...>>` for shared state.
- **Shelling out to PowerShell/cmd for unzip** is fragile (breaks on paths with
  spaces/accents — a documented bug) and a quoting/code-exec risk. The `zip`
  crate removes the whole class of problem.
- **Rot-prone endpoints.** The textures zip is a Discord CDN attachment URL
  (these expire); the search backend is `rocketleaguemaps.us` /
  `celab.jetfox.ovh`. **Both must be verified live before relying on them.**
- **Brittle parsing.** `GetMapSize` does substring math on raw HTTP headers;
  `convertToMB` has paths with no return value (UB). Replace with typed parsing.
- **Mixed concerns.** Layout, business logic, networking, and localization all
  live in one ~2,600-line render function, and the full EN/FR string table is
  rebuilt every frame. Separate `core` / `ui` / `i18n`.
- **Fixed `char[200]` buffers + `strncpy`** everywhere — gone in Rust.

---

## 5. Proposed Rust architecture

A Cargo **workspace** so reusable logic is testable without a GUI:

```
workshop-map-loader/
├── crates/
│   ├── wml-core/        # pure logic, no UI — the testable heart
│   │   ├── library.rs   # scan maps folder, parse .json sidecars, model `Map`
│   │   ├── catalog.rs   # rocketleaguemaps.us API client (reqwest + serde)
│   │   ├── download.rs  # streamed download w/ progress + zip extraction
│   │   ├── config.rs    # serde config (migrate old .cfg -> toml)
│   │   ├── paths.rs     # locate RL install / CookedPCConsole / data dir
│   │   └── bakkesmod.rs # RCON WebSocket client -> send `load_workshop`
│   └── wml-app/         # egui/eframe desktop binary (UI + wiring)
└── assets/              # logos, NoPreview.jpg (reuse existing data/)
```

**Crate picks:** `eframe`/`egui` (UI), `tokio` + `reqwest` (async + downloads),
`serde`/`serde_json`/`toml` (data), `zip` (extraction), `image` (previews),
`rfd` (file dialogs), `open` (links/folders), `tokio-tungstenite` (BakkesMod
RCON), optionally `gilrs` (controller), `tracing` (logging).

**Why egui:** the existing UI is immediate-mode ImGui, and egui *is*
immediate-mode — the API maps almost line-for-line (`ImGui::Button` →
`ui.button`, `BeginChild` → `ScrollArea`, etc.), so porting is mechanical rather
than a redesign. It also gives a real resizable native window (what "its own
application" implies) and is cross-platform — opening the door to Linux/Proton
RL players that a Windows-only DLL can never serve.

---

## 6. Phased migration plan

1. **`wml-core` first, headless.** Port the data model, the rocketleaguemaps.us
   client, download + unzip, config, and folder scanning. Unit/integration tests
   against fixtures. Proves the backends still work; could ship as a CLI.
2. **egui shell.** Recreate the two tabs (Library + Search), tile/list views,
   preview images, progress bars, manual-add. Replace the custom explorer with
   `rfd`.
3. **The launch bridge.** Implement the BakkesMod RCON client and the "Play"
   button. Detect BakkesMod; when present, send `load_workshop`. When absent,
   fall back to manager-only mode with a clear UI hint and an "open maps folder"
   affordance.
4. **Polish.** i18n via resource files (EN/FR), config migration from the old
   `.cfg`, optional `gilrs` controller support, structured logging, packaging
   (installer / zip instead of "drop a DLL in the plugins folder").

### Implementation status

- ✅ **Step 1** — `wml-core`: library scanning, catalog client, streamed
  download + zip extraction, install pipeline, TOML config, path discovery.
  Unit tests passing.
- ✅ **Step 2** — `wml-app` egui shell: Library + Search tabs, async search with
  preview thumbnails, download with progress + release picker, `rfd` folder
  picker.
- ✅ **Step 3** — launch bridge: `BakkesModBridge` RCON client (`connect` /
  `execute` / `load_workshop`) over a WebSocket, a TCP reachability `probe`, a
  "Play" button, and the manager-only fallback ("Open Folder" + status when
  BakkesMod is not connected). *The RCON handshake is compile-verified but not
  yet validated against a live BakkesMod — see §7.*
- ✅ **Step 4** — polish:
  - Legacy `.cfg` migration (`Config::load_or_migrate` / `migrate_legacy_cfg`,
    with a Windows candidate path), unit-tested.
  - EN/FR i18n for UI labels/buttons (`wml-app/src/i18n.rs`) with a language
    selector; transient status-line text stays English for now.
  - Gamepad support behind the `controller` cargo feature (gilrs; off by default
    because it links libudev on Linux). Compile-verified with the feature on
    (after installing libudev) and covered by a dedicated CI job.
  - Packaging: release profile (thin LTO, strip), CI workflow (fmt + clippy
    `-D warnings` + test + Linux/Windows release build), and `BUILDING.md`.

---

## 7. Risks / open questions

- **Backend liveness:** are `rocketleaguemaps.us` (`celab.jetfox.ovh`) and the
  Discord textures URL still alive? If not, search/download/textures need new
  sources or get cut.
- **`load_workshop` over RCON:** BakkesMod exposes an RCON WebSocket (port
  `9876`, `rcon_password`) and routes console commands through it, but the exact
  handshake and `load_workshop` behavior over RCON must be verified against a
  live install before committing to it as the launch path.
- **Distribution:** a standalone app needs an installer/updater story; it no
  longer rides BakkesMod's plugin distribution.

---

## 8. Bottom line

A strong candidate for a Rust standalone rewrite. ~80% of the project (search,
download, unzip, library, metadata, config) is game-independent and becomes
dramatically simpler and safer in Rust — the threading races, the
PowerShell-unzip hack, the wide-string conversions, and the hand-rolled file
explorer all collapse into mature crates. The one hard constraint is map
*launching*, addressed by the **bridge + graceful fallback** strategy: use
BakkesMod RCON when available, act as a pure manager/downloader otherwise. egui
makes the UI port nearly mechanical, and cross-platform support is a free
upgrade.
