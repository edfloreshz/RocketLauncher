// Auto-update support: shells out to Legendary (https://github.com/legendary-gl/legendary),
// the open-source Epic Games CLI client that Heroic also uses under the
// hood, to check for and download Rocket League updates.
//
// This deliberately does NOT reimplement Epic's manifest/chunk download
// protocol. That's a large, brittle surface (binary manifests, chunk
// hashing/decompression, delta patching) that Legendary's maintainers have
// spent years getting right against a moving target; shelling out to it is
// both safer and far less code than reinventing it.
//
// Requires `legendary` to be installed and logged in separately
// (`legendary auth`) — this launcher's own Epic session is independent and
// isn't shared with Legendary's.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Rocket League's Epic "app name" (as opposed to its display title),
/// the identifier Legendary's CLI expects everywhere.
pub const ROCKET_LEAGUE_APP_NAME: &str = "Sugar";

#[derive(Debug, Deserialize)]
struct InstalledGame {
    app_name: String,
    version: String,
    install_path: String,
}

#[derive(Debug, Clone)]
pub struct UpdateStatus {
    pub installed_version: Option<String>,
    pub install_path: Option<PathBuf>,
    pub update_available: bool,
}

/// Locates the `legendary` executable on PATH. Returns an error with a
/// helpful message (rather than a raw "not found") if it's missing.
fn find_legendary() -> Result<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec!["legendary.exe", "legendary"]
    } else {
        vec!["legendary"]
    };

    for candidate in candidates {
        let found = Command::new(if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        })
        .arg(candidate)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

        if matches!(found, Ok(status) if status.success()) {
            return Ok(candidate.to_string());
        }
    }

    bail!(
        "Legendary CLI not found on PATH. Install it from \
         https://github.com/legendary-gl/legendary (e.g. `pip install legendary-gl`) \
         and run `legendary auth` once to log in, then try again."
    )
}

/// Runs `legendary list-installed --json` and looks for Rocket League.
/// This performs a network call under the hood (Legendary checks the
/// latest catalog version), so it can take a few seconds.
pub fn check_for_update() -> Result<UpdateStatus> {
    let legendary = find_legendary()?;

    let output = Command::new(&legendary)
        .args(["list-installed", "--json"])
        .output()
        .context("failed to run `legendary list-installed`")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`legendary list-installed` failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let installed: Vec<InstalledGame> =
        serde_json::from_str(&stdout).context("failed to parse legendary's JSON output")?;

    let rl = installed
        .iter()
        .find(|g| g.app_name == ROCKET_LEAGUE_APP_NAME);

    let Some(rl) = rl else {
        // Not installed via Legendary at all (e.g. installed through the
        // real Epic Launcher, or through Heroic with a different
        // backend) — nothing for us to check here.
        return Ok(UpdateStatus {
            installed_version: None,
            install_path: None,
            update_available: false,
        });
    };

    // `legendary status` refreshes the asset catalog and reports whether
    // *any* installed game has an update; combined with our own version
    // string from list-installed, `install` will just no-op if already
    // current, so we treat "installed at all" as "checkable" and let
    // `install` be the source of truth for whether work is needed.
    let status_output = Command::new(&legendary)
        .args(["status", "--json"])
        .output()
        .context("failed to run `legendary status`")?;

    let update_available = if status_output.status.success() {
        let stdout = String::from_utf8_lossy(&status_output.stdout);
        // `legendary status --json` includes an "update_available" style
        // field in recent versions; fall back to "assume unknown, let
        // install sort it out" if parsing fails, since install-based
        // update is idempotent anyway.
        stdout.contains("\"update_available\": true")
            || stdout.contains("\"update_available\":true")
    } else {
        false
    };

    Ok(UpdateStatus {
        installed_version: Some(rl.version.clone()),
        install_path: Some(PathBuf::from(&rl.install_path)),
        update_available,
    })
}

/// Runs `legendary install Sugar -y`, which for an already-installed
/// game behaves as an update-in-place (Legendary diffs the local
/// manifest against the latest one and only downloads changed chunks).
/// `-y` answers any interactive prompts (e.g. optional-pack selection)
/// with defaults so this can run unattended from a GUI button.
///
/// `on_line` is called once per line of Legendary's combined
/// stdout/stderr output (stdout and stderr are drained concurrently on
/// background threads and interleaved through a channel, so neither can
/// block the other if only one fills its OS pipe buffer). It runs on
/// whatever thread calls this function, so callers running this inside
/// an async task should relay through a channel rather than touching UI
/// state directly from here.
pub fn update_rocket_league(mut on_line: impl FnMut(String)) -> Result<()> {
    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;

    let legendary = find_legendary()?;

    let mut child = Command::new(&legendary)
        .args(["-y", "install", ROCKET_LEAGUE_APP_NAME])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start `legendary install`")?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel::<String>();

    let stdout_tx = tx.clone();
    // Keep an extra clone alive here so `rx`'s iterator only ends once both
    // reader threads' senders (and this one) are dropped — otherwise, if
    // one of stdout/stderr isn't piped, `rx` could end prematurely.
    let extra_tx = tx.clone();
    let stdout_thread = stdout.map(|out| {
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().flatten() {
                let _ = stdout_tx.send(line);
            }
        })
    });

    let stderr_thread = stderr.map(|err| {
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().flatten() {
                let _ = tx.send(line);
            }
        })
    });

    // Drop the extra clone (and the original `tx`, already moved into the
    // stderr closure above) so `rx` iteration ends once both reader threads
    // finish (i.e. every sender clone has been dropped).
    drop(extra_tx);

    for line in rx {
        on_line(line);
    }

    if let Some(t) = stdout_thread {
        let _ = t.join();
    }
    if let Some(t) = stderr_thread {
        let _ = t.join();
    }

    let status = child
        .wait()
        .context("failed to wait for `legendary install`")?;
    if !status.success() {
        bail!("`legendary install` exited with a non-zero status ({status})");
    }
    Ok(())
}
