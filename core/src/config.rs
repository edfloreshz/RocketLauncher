// Persistent on-disk configuration for the launcher.

use anyhow::{Context, Result, bail};
use iced::Theme;
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
    #[serde(default)]
    pub theme: String,
}

impl Config {
    pub fn is_valid(&self) -> bool {
        !self.rocket_league_path.is_empty()
            && !self.proton_path.is_empty()
            && !self.compat_data_path.is_empty()
    }

    pub fn get_theme(&self) -> Theme {
        match self.theme.as_str() {
            "Light" => Theme::Light,
            "Dark" => Theme::Dark,
            "Dracula" => Theme::Dracula,
            "Nord" => Theme::Nord,
            "Solarized Light" => Theme::SolarizedLight,
            "Solarized Dark" => Theme::SolarizedDark,
            "Gruvbox Light" => Theme::GruvboxLight,
            "Gruvbox Dark" => Theme::GruvboxDark,
            "Catppuccin Latte" => Theme::CatppuccinLatte,
            "Catppuccin Frappé" => Theme::CatppuccinFrappe,
            "Catppuccin Macchiato" => Theme::CatppuccinMacchiato,
            "Catppuccin Mocha" => Theme::CatppuccinMocha,
            "Tokyo Night" => Theme::TokyoNight,
            "Tokyo Night Storm" => Theme::TokyoNightStorm,
            "Tokyo Night Light" => Theme::TokyoNightLight,
            "Kanagawa Wave" => Theme::KanagawaWave,
            "Kanagawa Dragon" => Theme::KanagawaDragon,
            "Kanagawa Lotus" => Theme::KanagawaLotus,
            "Moonfly" => Theme::Moonfly,
            "Nightfly" => Theme::Nightfly,
            "Oxocarbon" => Theme::Oxocarbon,
            "Ferra" => Theme::Ferra,
            _ => Theme::Ferra,
        }
    }
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
