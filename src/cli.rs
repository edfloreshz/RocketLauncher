use anyhow::{Context, Result, bail};
use rl_launcher::{
    Config, EPIC_LOGIN_URL, config_path, discovery, exchange_code_for_refresh_token,
    get_launch_credentials, launch_game, load_config, open_browser, save_config, updater,
};
use std::io::Write;

fn prompt(label: &str) -> Result<String> {
    print!("{label}: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

async fn interactive_login(client: &reqwest::Client) -> Result<String> {
    println!("A browser window will open. Log in to your Epic Games account,");
    println!("then copy the 'authorizationCode' value from the JSON response.");
    println!();
    println!("If the browser didn't open, visit this URL manually:");
    println!("{EPIC_LOGIN_URL}");
    println!();
    open_browser(EPIC_LOGIN_URL);

    let code = prompt("Paste the 32-character authorization code")?;
    exchange_code_for_refresh_token(client, &code).await
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("config: {}", config_path()?.display());
    let mut cfg: Config = load_config().context("failed to load configuration")?;

    if cfg.rocket_league_path.trim().is_empty() {
        println!("Searching common install locations...");
        let found = discovery::discover_all();

        if let Some(p) = &found.rocket_league_path {
            println!("Found Rocket League executable: {}", p.display());
        }
        if let Some(p) = &found.steam_install_path {
            println!("Found Steam install: {}", p.display());
        }
        if let Some(p) = &found.proton_path {
            println!("Found Proton: {}", p.display());
        }
        if let Some(p) = &found.compat_data_path {
            println!("Proton prefix to use: {}", p.display());
        }
        println!();

        let default_rl = found
            .rocket_league_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let rl_input = prompt(&format!(
            "Rocket League executable path{}",
            if default_rl.is_empty() {
                String::new()
            } else {
                format!(" [{default_rl}]")
            }
        ))?;
        cfg.rocket_league_path = if rl_input.is_empty() {
            default_rl
        } else {
            rl_input
        };

        if cfg!(target_os = "linux") {
            println!();
            println!("Detected Linux. To run this .exe you need Proton.");
            println!(
                "Press Enter to accept the auto-detected value, or type your own. Leave both blank to skip Proton."
            );

            let default_proton = found
                .proton_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let proton_input = prompt(&format!(
                "Path to 'proton' binary{}",
                if default_proton.is_empty() {
                    String::new()
                } else {
                    format!(" [{default_proton}]")
                }
            ))?;
            cfg.proton_path = if proton_input.is_empty() {
                default_proton
            } else {
                proton_input
            };

            if !cfg.proton_path.trim().is_empty() {
                let default_compat = found
                    .compat_data_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let compat_input = prompt(&format!(
                    "Path to Proton prefix folder{}",
                    if default_compat.is_empty() {
                        String::new()
                    } else {
                        format!(" [{default_compat}]")
                    }
                ))?;
                cfg.compat_data_path = if compat_input.is_empty() {
                    default_compat
                } else {
                    compat_input
                };

                let default_steam = found
                    .steam_install_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let steam_input = prompt(&format!(
                    "Path to your Steam install root{}",
                    if default_steam.is_empty() {
                        String::new()
                    } else {
                        format!(" [{default_steam}]")
                    }
                ))?;
                cfg.steam_install_path = if steam_input.is_empty() {
                    default_steam
                } else {
                    steam_input
                };
            }
        }

        save_config(&cfg)?;
    }

    let client = reqwest::Client::new();

    if cfg.epic_refresh_token.trim().is_empty() {
        println!("No saved session found — starting first-time login.");
        cfg.epic_refresh_token = interactive_login(&client).await?;
        save_config(&cfg)?;
    }

    let (creds, new_refresh_token) =
        match get_launch_credentials(&client, &cfg.epic_refresh_token).await {
            Ok(result) => result,
            Err(e) => {
                println!("Saved session expired ({e}). Re-authenticating...");
                cfg.epic_refresh_token = interactive_login(&client).await?;
                save_config(&cfg)?;
                get_launch_credentials(&client, &cfg.epic_refresh_token).await?
            }
        };

    if new_refresh_token != cfg.epic_refresh_token {
        cfg.epic_refresh_token = new_refresh_token;
        save_config(&cfg).context("failed to save session token")?;
    }

    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let update_requested = raw_args.iter().any(|a| a == "--update");
    let update_only = raw_args.iter().any(|a| a == "--update-only");
    let extra_args: Vec<String> = raw_args
        .into_iter()
        .filter(|a| a != "--update" && a != "--update-only")
        .collect();

    if update_requested || update_only {
        println!("Checking for Rocket League updates via Legendary...");
        match updater::check_for_update() {
            Ok(status) => {
                match (&status.installed_version, status.update_available) {
                    (Some(v), true) => {
                        println!("Update available (installed: {v}). Downloading...")
                    }
                    (Some(v), false) => println!("Already up to date (installed: {v})."),
                    (None, _) => {
                        println!(
                            "Rocket League isn't installed via Legendary — skipping update check."
                        );
                    }
                }
                if status.installed_version.is_some() {
                    // `install` is idempotent: if already current it just
                    // verifies and exits quickly rather than re-downloading.
                    updater::update_rocket_league(|line| println!("{line}"))
                        .context("update failed")?;
                    println!("Update check complete.");
                }
            }
            Err(e) => {
                println!("Update check failed: {e}. Continuing without updating.");
            }
        }
        if update_only {
            return Ok(());
        }
    }

    match launch_game(&cfg, &creds, &extra_args) {
        Ok(()) => {
            println!("Game process started.");
            Ok(())
        }
        Err(e) => bail!("failed to launch game: {e}"),
    }
}
