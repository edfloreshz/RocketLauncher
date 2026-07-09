// Persistent on-disk configuration for the launcher.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
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
    let config_dir = dirs::config_dir()
        .context("failed to locate config dir")?
        .join("rocket-launcher");

    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).context("failed to create config dir")?;
    }

    Ok(config_dir.join(CONFIG_FILE))
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
    if let Err(err) = fs::write(path, data) {
        bail!("failed to write config.json: {}", err)
    }
    Ok(())
}
