// Best-effort auto-discovery of install paths, so the user doesn't have to
// type them by hand. Everything here just checks common, well-known
// locations on disk — it doesn't read the registry, doesn't call out to any
// network service, and never guesses at paths outside the user's home
// directory or standard system locations.

use std::path::{Path, PathBuf};

/// Candidate locations for a Steam install, relative to $HOME on Linux,
/// or as absolute paths on Windows/macOS.
fn steam_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        // Native Linux Steam
        out.push(home.join(".steam/steam"));
        out.push(home.join(".steam/root"));
        out.push(home.join(".local/share/Steam"));
        // Flatpak Steam
        out.push(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
        // Snap Steam
        out.push(home.join("snap/steam/common/.steam/steam"));
    }
    if cfg!(target_os = "windows") {
        out.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
        out.push(PathBuf::from("C:\\Program Files\\Steam"));
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = dirs::home_dir() {
            out.push(home.join("Library/Application Support/Steam"));
        }
    }
    out
}

/// Finds the first Steam install path that actually exists on disk.
pub fn find_steam_install() -> Option<PathBuf> {
    steam_candidates().into_iter().find(|p| p.is_dir())
}

/// All `steamapps` library folders Steam knows about, parsed out of
/// `libraryfolders.vdf` if present, falling back to just the main
/// `steamapps` dir under the Steam install itself.
fn steam_library_folders(steam_path: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam_path.join("steamapps")];

    let vdf_path = steam_path.join("steamapps/libraryfolders.vdf");
    if let Ok(contents) = std::fs::read_to_string(&vdf_path) {
        // Minimal VDF parsing: look for lines like `"path"		"/some/path"`.
        for line in contents.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("\"path\"") {
                let value = rest.trim().trim_matches('"');
                // The regex above is too naive for quoted values with
                // internal escapes, so instead just split on quotes.
                let parts: Vec<&str> = line.split('"').collect();
                // parts looks like ["", "path", "\t\t", "/the/actual/path", ""]
                if let Some(path_str) = parts.get(3) {
                    let p = PathBuf::from(path_str).join("steamapps");
                    if !libs.contains(&p) {
                        libs.push(p);
                    }
                } else if !value.is_empty() {
                    let p = PathBuf::from(value).join("steamapps");
                    if !libs.contains(&p) {
                        libs.push(p);
                    }
                }
            }
        }
    }
    libs
}

/// Searches standard Steam library folders, custom compatibility tool folders,
/// and third-party launchers (ProtonPlus, Heroic) for a Proton build.
/// Prefers "Experimental" or the newest-looking version string name.
pub fn find_proton(steam_path: Option<&Path>) -> Option<PathBuf> {
    let steam_path = steam_path.map(PathBuf::from).or_else(find_steam_install)?;
    let mut found: Vec<PathBuf> = Vec::new();

    // 1. Scan official Steam library folders (e.g., "common/Proton 8.0")
    for lib in steam_library_folders(&steam_path) {
        let common = lib.join("common");
        if let Ok(entries) = std::fs::read_dir(&common) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                // Expanded to match custom builds that might place "Proton" anywhere in the name
                if name.contains("Proton") || name.contains("proton") {
                    let proton_bin = entry.path().join("proton");
                    if proton_bin.is_file() {
                        found.push(proton_bin);
                    }
                }
            }
        }
    }

    // 2. Build a list of external custom paths to search
    let mut custom_paths = vec![
        // Native Steam custom compatibility tools (ProtonPlus / ProtonUp-Qt target)
        steam_path.join("compatibilitytools.d"),
    ];

    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        custom_paths.extend(vec![
            // Flatpak Steam custom compatibility tools path
            home.join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d"),
            // Heroic Games Launcher (Native installation path)
            home.join(".config/heroic/tools/proton"),
            // Heroic Games Launcher (Flatpak installation path)
            home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton"),
        ]);
    }

    // 3. Scan the custom tool paths
    for dir in custom_paths {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let proton_bin = entry.path().join("proton");
                if proton_bin.is_file() {
                    found.push(proton_bin);
                }
            }
        }
    }

    if found.is_empty() {
        return None;
    }

    // Sort by the immediate parent folder name (the version string)
    // to keep sorting accurate across completely different path roots.
    found.sort_by_cached_key(|path| {
        path.parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });

    // Prefer "Experimental" if present, otherwise fallback to the lexicographically newest name
    found
        .iter()
        .find(|p| p.to_string_lossy().contains("Experimental"))
        .cloned()
        .or_else(|| found.last().cloned())
}

/// Looks for an existing compatdata prefix for Rocket League under any
/// Steam library, or otherwise suggests a fresh folder path to create.
pub fn find_or_suggest_compat_data(steam_path: Option<&Path>) -> Option<PathBuf> {
    let steam_path = steam_path.map(PathBuf::from).or_else(find_steam_install)?;

    // Rocket League's legacy Steam AppID, in case a prefix already
    // exists from when it was a Steam app.
    const RL_STEAM_APPID: &str = "252950";

    for lib in steam_library_folders(&steam_path) {
        let existing = lib.join("compatdata").join(RL_STEAM_APPID);
        if existing.is_dir() {
            return Some(existing);
        }
    }

    // Nothing existing found — suggest a fresh, clearly-named prefix
    // next to the main Steam install so the user can just accept it.
    Some(steam_path.join("steamapps/compatdata/252950"))
}

/// Candidate locations where an Epic Games install of Rocket League's
/// executable commonly lives, covering native Windows Epic installs and
/// the typical Heroic/Legendary default prefix layout on Linux.
fn rocket_league_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let rel = "rocketleague/Binaries/Win64/RocketLeague_EAC.exe";
    let rel_alt = "rocketleague/Binaries/Win64/RocketLeague.exe";

    if cfg!(target_os = "windows") {
        out.push(PathBuf::from("C:\\Program Files\\Epic Games").join(rel));
        out.push(PathBuf::from("C:\\Program Files\\Epic Games").join(rel_alt));
    }

    if let Some(home) = dirs::home_dir() {
        // Heroic's default install location on Linux.
        out.push(home.join("Games/Heroic/rocketleague/Binaries/Win64/RocketLeague_EAC.exe"));
        out.push(home.join("Games/Heroic/rocketleague/Binaries/Win64/RocketLeague.exe"));
        // Generic "Epic Games" folder some users create manually under Wine prefixes.
        out.push(
            home.join("Games/epic-games-store/drive_c/Program Files/Epic Games")
                .join(rel),
        );
        out.push(
            home.join("Games/epic-games-store/drive_c/Program Files/Epic Games")
                .join(rel_alt),
        );
    }

    out
}

/// Best-effort search for the Rocket League executable in common
/// locations. Returns the first match; the user can still override it.
pub fn find_rocket_league_exe() -> Option<PathBuf> {
    rocket_league_candidates().into_iter().find(|p| p.is_file())
}

/// Runs full auto-discovery and returns whatever it could find. Any
/// field left `None` means the user needs to fill it in manually.
pub struct Discovered {
    pub rocket_league_path: Option<PathBuf>,
    pub steam_install_path: Option<PathBuf>,
    pub proton_path: Option<PathBuf>,
    pub compat_data_path: Option<PathBuf>,
}

pub fn discover_all() -> Discovered {
    let steam_install_path = find_steam_install();
    let proton_path = find_proton(steam_install_path.as_deref());
    let compat_data_path = find_or_suggest_compat_data(steam_install_path.as_deref());
    let rocket_league_path = find_rocket_league_exe();

    Discovered {
        rocket_league_path,
        steam_install_path,
        proton_path,
        compat_data_path,
    }
}
