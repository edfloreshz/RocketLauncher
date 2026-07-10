// Launches Rocket League with Epic launch credentials, optionally through
// Proton on Linux.

use crate::auth::LaunchCredentials;
use crate::config::Config;
use anyhow::{Context, Result, bail};
use std::fs;
use std::process::Command;

fn game_args(creds: &LaunchCredentials, skip_eac: bool, extra_args: &[String]) -> Vec<String> {
    let mut args = vec![
        "-AUTH_LOGIN=unused".to_string(),
        format!("-AUTH_PASSWORD={}", creds.exchange_code),
        "-AUTH_TYPE=exchangecode".to_string(),
        "-epicapp=Sugar".to_string(),
        "-epicenv=Prod".to_string(),
        "-EpicPortal".to_string(),
        "-epicusername=\"\"".to_string(),
        format!("-epicuserid={}", creds.account_id),
    ];
    if skip_eac {
        args.push("-noeac".to_string());
    }
    args.extend_from_slice(extra_args);
    args
}

/// Given the configured RocketLeague.exe / RocketLeague_EAC.exe path and the
/// skip_eac flag, picks the matching executable in the same directory if the
/// filename looks swappable (mirrors Slipstream's behavior).
fn resolve_executable(cfg: &Config) -> String {
    let path = std::path::Path::new(&cfg.rocket_league_path);
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new(""));
    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_lowercase();

    if filename == "rocketleague.exe" || filename == "rocketleague_eac.exe" {
        let chosen = if cfg.skip_eac {
            "RocketLeague.exe"
        } else {
            "RocketLeague_EAC.exe"
        };
        dir.join(chosen).to_string_lossy().to_string()
    } else {
        cfg.rocket_league_path.clone()
    }
}

pub fn launch_game(cfg: &Config, creds: &LaunchCredentials, extra_args: &[String]) -> Result<()> {
    let exe_path = resolve_executable(cfg);
    let args = game_args(creds, cfg.skip_eac, extra_args);
    let use_proton = cfg!(target_os = "linux") && !cfg.proton_path.trim().is_empty();

    if use_proton {
        launch_via_proton(cfg, &exe_path, &args)
    } else {
        Command::new(&exe_path)
            .args(&args)
            .spawn()
            .with_context(|| format!("failed to start Rocket League at '{exe_path}'"))?;
        Ok(())
    }
}

/// Launches the game through Proton, the same way Steam would:
/// `proton run <exe> <args...>` with the compat env vars Proton expects.
fn launch_via_proton(cfg: &Config, exe_path: &str, args: &[String]) -> Result<()> {
    if cfg.compat_data_path.trim().is_empty() {
        bail!("Proton is configured but no compat data (prefix) path is set.");
    }
    if cfg.steam_install_path.trim().is_empty() {
        bail!("Proton is configured but no Steam install path is set.");
    }

    fs::create_dir_all(&cfg.compat_data_path).with_context(|| {
        format!(
            "failed to create compat data path '{}'",
            cfg.compat_data_path
        )
    })?;

    let mut cmd = Command::new(&cfg.proton_path);
    cmd.env("STEAM_COMPAT_DATA_PATH", &cfg.compat_data_path)
        .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &cfg.steam_install_path)
        .arg("run")
        .arg(exe_path)
        .args(args);

    cmd.spawn()
        .with_context(|| format!("failed to start Proton at '{}'", cfg.proton_path))?;
    Ok(())
}
