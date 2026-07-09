// Persistent on-disk configuration for the launcher.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub rocket_league_path: String,
    #[serde(default)]
    pub epic_refresh_token: String,
    /// Path to `proton` inside a Proton build directory, e.g.
    /// "/home/you/.steam/steam/steamapps/common/Proton 9.0/proton".
    /// Leave empty on Windows or to run the .exe directly.
    #[serde(default)]
    pub proton_path: String,
    /// Path to the Steam compatdata prefix folder for this game.
    #[serde(default)]
    pub compat_data_path: String,
    /// Path to your Steam install root, e.g. "/home/you/.steam/steam".
    #[serde(default)]
    pub steam_install_path: String,
    /// Skip Easy Anti-Cheat (offline modes only: Free Play, Replays, Custom
    /// Training). Mirrors Epic's own "Launch without EAC" option. Do not use
    /// this to attempt online play — EAC is required for matchmaking and
    /// bypassing it there risks a ban.
    #[serde(default)]
    pub skip_eac: bool,
}

pub fn config_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("failed to locate current executable")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("executable has no parent dir"))?;
    Ok(dir.join(CONFIG_FILE))
}

pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    if path.exists() {
        let data = fs::read_to_string(&path).context("failed to read config.json")?;
        Ok(serde_json::from_str(&data).unwrap_or_default())
    } else {
        Ok(Config::default())
    }
}

pub fn save_config(cfg: &Config) -> Result<()> {
    let path = config_path()?;
    let data = serde_json::to_string_pretty(cfg)?;
    fs::write(path, data).context("failed to write config.json")?;
    Ok(())
}
