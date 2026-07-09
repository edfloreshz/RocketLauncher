// Iced 0.14 GUI for the Rocket League / Epic Games launcher.
//
// Layout:
//  - Settings fields: RL exe path, Proton path, compat data path, Steam
//    install path, "skip EAC (offline only)" toggle.
//  - Login: shows a button to open the Epic login page, then a text field to
//    paste the authorization code, then a "Launch" button once a session
//    exists.
//  - Status line showing what's currently happening / any error.
//
// All network + process work happens inside `Task::perform` using the async
// reqwest client, run on iced's tokio executor so the UI thread never blocks.

use iced::futures::sink::SinkExt;
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, Task, Theme};
use rocket_launcher::{
    Config, EPIC_LOGIN_URL, LaunchCredentials, discovery, exchange_code_for_refresh_token,
    get_launch_credentials, launch_game, load_config, open_browser, save_config, updater,
};

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    // Settings field edits
    RocketLeaguePathChanged(String),
    ProtonPathChanged(String),
    CompatDataPathChanged(String),
    SteamInstallPathChanged(String),
    SkipEacToggled(bool),
    SaveSettings,
    AutoDetect,

    // Login flow
    OpenLoginPage,
    AuthCodeChanged(String),
    SubmitAuthCode,
    LoginFinished(Result<String, String>),

    // Launch flow
    Launch,
    LaunchFinished(Result<(), String>),

    // Update flow
    CheckForUpdate,
    UpdateCheckFinished(Result<UpdateCheckResult, String>),
    RunUpdate,
    UpdateLogLine(String),
    UpdateFinished(Result<(), String>),
}

#[derive(Debug, Clone)]
struct UpdateCheckResult {
    installed_version: Option<String>,
    update_available: bool,
}

struct App {
    cfg: Config,
    auth_code_input: String,
    status: String,
    busy: bool,
    logged_in: bool,

    checking_update: bool,
    update_available: bool,
    installed_version: Option<String>,
    updating: bool,
    update_log: Vec<String>,
}

impl App {
    fn new() -> Self {
        let cfg = load_config().unwrap_or_default();
        let logged_in = !cfg.epic_refresh_token.trim().is_empty();
        Self {
            cfg,
            auth_code_input: String::new(),
            status: if logged_in {
                "Ready. Session loaded from config.json.".to_string()
            } else {
                "Not logged in yet.".to_string()
            },
            busy: false,
            logged_in,

            checking_update: false,
            update_available: false,
            installed_version: None,
            updating: false,
            update_log: Vec::new(),
        }
    }

    fn title(&self) -> String {
        "Rocket League Launcher".to_string()
    }

    fn theme(&self) -> Theme {
        Theme::TokyoNight
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RocketLeaguePathChanged(v) => {
                self.cfg.rocket_league_path = v;
                Task::none()
            }
            Message::ProtonPathChanged(v) => {
                self.cfg.proton_path = v;
                Task::none()
            }
            Message::CompatDataPathChanged(v) => {
                self.cfg.compat_data_path = v;
                Task::none()
            }
            Message::SteamInstallPathChanged(v) => {
                self.cfg.steam_install_path = v;
                Task::none()
            }
            Message::SkipEacToggled(v) => {
                self.cfg.skip_eac = v;
                Task::none()
            }
            Message::SaveSettings => {
                match save_config(&self.cfg) {
                    Ok(()) => self.status = "Settings saved.".to_string(),
                    Err(e) => self.status = format!("Failed to save settings: {e}"),
                }
                Task::none()
            }
            Message::AutoDetect => {
                let found = discovery::discover_all();
                let mut notes = Vec::new();

                if let Some(p) = found.rocket_league_path {
                    self.cfg.rocket_league_path = p.to_string_lossy().to_string();
                    notes.push("RL exe");
                }
                if let Some(p) = found.steam_install_path {
                    self.cfg.steam_install_path = p.to_string_lossy().to_string();
                    notes.push("Steam install");
                }
                if let Some(p) = found.proton_path {
                    self.cfg.proton_path = p.to_string_lossy().to_string();
                    notes.push("Proton");
                }
                if let Some(p) = found.compat_data_path {
                    self.cfg.compat_data_path = p.to_string_lossy().to_string();
                    notes.push("Proton prefix");
                }

                self.status = if notes.is_empty() {
                    "Auto-detect found nothing — fill in paths manually.".to_string()
                } else {
                    format!(
                        "Auto-detected: {}. Review and Save settings.",
                        notes.join(", ")
                    )
                };
                if let Err(e) = save_config(&self.cfg) {
                    self.status = format!("{} (failed to save: {e})", self.status);
                }
                Task::none()
            }

            Message::OpenLoginPage => {
                open_browser(EPIC_LOGIN_URL);
                self.status =
                    "Browser opened. Log in, then paste the authorization code below.".to_string();
                Task::none()
            }
            Message::AuthCodeChanged(v) => {
                self.auth_code_input = v;
                Task::none()
            }
            Message::SubmitAuthCode => {
                let code = self.auth_code_input.trim().to_string();
                if code.len() != 32 {
                    self.status = format!(
                        "Authorization code should be 32 characters, got {}.",
                        code.len()
                    );
                    return Task::none();
                }
                self.busy = true;
                self.status = "Exchanging authorization code...".to_string();
                Task::perform(
                    async move {
                        let client = reqwest::Client::new();
                        exchange_code_for_refresh_token(&client, &code)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::LoginFinished,
                )
            }
            Message::LoginFinished(result) => {
                self.busy = false;
                match result {
                    Ok(refresh_token) => {
                        self.cfg.epic_refresh_token = refresh_token;
                        self.logged_in = true;
                        self.auth_code_input.clear();
                        if let Err(e) = save_config(&self.cfg) {
                            self.status = format!("Logged in, but failed to save session: {e}");
                        } else {
                            self.status = "Logged in. You can launch now.".to_string();
                        }
                    }
                    Err(e) => {
                        self.status = format!("Login failed: {e}");
                    }
                }
                Task::none()
            }

            Message::Launch => {
                if self.cfg.rocket_league_path.trim().is_empty() {
                    self.status = "Set the Rocket League executable path first.".to_string();
                    return Task::none();
                }
                self.busy = true;
                self.status = "Authenticating...".to_string();
                let cfg = self.cfg.clone();
                Task::perform(async move { do_launch(cfg).await }, Message::LaunchFinished)
            }
            Message::LaunchFinished(result) => {
                self.busy = false;
                match result {
                    Ok(()) => self.status = "Game launched.".to_string(),
                    Err(e) => self.status = format!("Launch failed: {e}"),
                }
                Task::none()
            }

            Message::CheckForUpdate => {
                self.checking_update = true;
                self.status = "Checking for Rocket League updates via Legendary...".to_string();
                Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            updater::check_for_update().map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("update check task panicked: {e}")))
                        .map(|status| UpdateCheckResult {
                            installed_version: status.installed_version,
                            update_available: status.update_available,
                        })
                    },
                    Message::UpdateCheckFinished,
                )
            }
            Message::UpdateCheckFinished(result) => {
                self.checking_update = false;
                match result {
                    Ok(r) => {
                        self.installed_version = r.installed_version.clone();
                        self.update_available = r.update_available;
                        self.status = match (&r.installed_version, r.update_available) {
                            (Some(v), true) => {
                                format!(
                                    "Update available (installed: {v}). Click Update to download it."
                                )
                            }
                            (Some(v), false) => format!("Up to date (installed: {v})."),
                            (None, _) => {
                                "Rocket League isn't installed via Legendary — nothing to check."
                                    .to_string()
                            }
                        };
                    }
                    Err(e) => {
                        self.status = format!("Update check failed: {e}");
                    }
                }
                Task::none()
            }

            Message::RunUpdate => {
                self.updating = true;
                self.update_log.clear();
                self.status = "Updating Rocket League via Legendary...".to_string();
                Task::stream(update_stream())
            }
            Message::UpdateLogLine(line) => {
                self.update_log.push(line);
                // Keep the log from growing unbounded during long installs.
                if self.update_log.len() > 500 {
                    let excess = self.update_log.len() - 500;
                    self.update_log.drain(0..excess);
                }
                Task::none()
            }
            Message::UpdateFinished(result) => {
                self.updating = false;
                match result {
                    Ok(()) => {
                        self.status = "Update complete.".to_string();
                        self.update_available = false;
                    }
                    Err(e) => self.status = format!("Update failed: {e}"),
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let path_field = |label: &str, value: &str, on_change: fn(String) -> Message| {
            let label = label.to_string();
            let value = value.to_string();
            column![
                text(label).size(14),
                text_input("", &value).on_input(on_change).padding(6),
            ]
            .spacing(4)
        };

        let settings = column![
            text("Settings").size(20),
            path_field(
                "Rocket League executable path",
                &self.cfg.rocket_league_path,
                Message::RocketLeaguePathChanged,
            ),
            path_field(
                "Proton binary path (leave blank to run natively)",
                &self.cfg.proton_path,
                Message::ProtonPathChanged,
            ),
            path_field(
                "Proton prefix / compat data path",
                &self.cfg.compat_data_path,
                Message::CompatDataPathChanged,
            ),
            path_field(
                "Steam install path",
                &self.cfg.steam_install_path,
                Message::SteamInstallPathChanged,
            ),
            checkbox(self.cfg.skip_eac)
                .label("Skip Easy Anti-Cheat (offline modes only)")
                .on_toggle(Message::SkipEacToggled),
            row![
                button("Auto-detect paths").on_press(Message::AutoDetect),
                button("Save settings").on_press(Message::SaveSettings),
            ]
            .spacing(10),
        ]
        .spacing(10);

        let login_section: Element<'_, Message> = if self.logged_in {
            column![
                text("Logged in.").size(14),
                button("Log in with a different account").on_press(Message::OpenLoginPage),
            ]
            .spacing(8)
            .into()
        } else {
            column![
                text("Login").size(20),
                button("Open Epic login page").on_press(Message::OpenLoginPage),
                text_input("Paste authorization code here", &self.auth_code_input)
                    .on_input(Message::AuthCodeChanged)
                    .padding(6),
                button("Submit code").on_press(Message::SubmitAuthCode),
            ]
            .spacing(8)
            .into()
        };

        let launch_button = if self.logged_in {
            button(text(if self.busy { "Working..." } else { "Launch" }))
                .on_press_maybe((!self.busy).then_some(Message::Launch))
        } else {
            button("Launch").on_press_maybe(None)
        };

        let update_check_label = if self.checking_update {
            "Checking..."
        } else {
            "Check for updates"
        };
        let update_section = {
            let mut col = column![
                text("Updates (via Legendary)").size(20),
                row![
                    button(update_check_label).on_press_maybe(
                        (!self.checking_update && !self.updating)
                            .then_some(Message::CheckForUpdate)
                    ),
                    button(if self.updating {
                        "Updating..."
                    } else {
                        "Update now"
                    })
                    .on_press_maybe(
                        (self.update_available && !self.updating).then_some(Message::RunUpdate)
                    ),
                ]
                .spacing(10),
            ]
            .spacing(8);

            if let Some(v) = &self.installed_version {
                col = col.push(text(format!("Installed version: {v}")).size(14));
            }

            if !self.update_log.is_empty() {
                let log_text = self.update_log.join("\n");
                col = col.push(
                    scrollable(text(log_text).size(12))
                        .height(Length::Fixed(160.0))
                        .width(Length::Fill),
                );
            }

            col
        };

        let content = column![
            settings,
            login_section,
            update_section,
            row![launch_button].spacing(10),
            text(&self.status).size(14),
        ]
        .spacing(20)
        .padding(20)
        .max_width(560);

        container(content)
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }
}

async fn do_launch(cfg: Config) -> Result<(), String> {
    let client = reqwest::Client::new();
    let (creds, new_refresh_token): (LaunchCredentials, String) =
        get_launch_credentials(&client, &cfg.epic_refresh_token)
            .await
            .map_err(|e| e.to_string())?;

    let mut cfg = cfg;
    if new_refresh_token != cfg.epic_refresh_token {
        cfg.epic_refresh_token = new_refresh_token;
        save_config(&cfg).map_err(|e| e.to_string())?;
    }

    launch_game(&cfg, &creds, &[]).map_err(|e| e.to_string())
}

/// Bridges `updater::update_rocket_league` (a blocking function that streams
/// progress via a plain callback on a background OS thread) into an async
/// `Stream` of `Message`s that iced's `Task::stream` can drive.
///
/// A dedicated OS thread runs the blocking Legendary process and pushes each
/// output line into a standard `std::sync::mpsc` channel. A second, cheap
/// blocking-recv loop (spawned once via `spawn_blocking`, not per-line) drains
/// that channel and forwards each item into a `tokio::sync::mpsc` channel,
/// which *is* safely awaitable from the async stream body below. This keeps
/// the blocking work fully off iced's executor while still delivering
/// progress incrementally instead of all at once at the end.
fn update_stream() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(
        100,
        |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            let (std_tx, std_rx) = std::sync::mpsc::channel::<UpdateStreamEvent>();
            let (async_tx, mut async_rx) =
                tokio::sync::mpsc::unbounded_channel::<UpdateStreamEvent>();

            // Runs Legendary itself; blocking, so it gets its own OS thread.
            std::thread::spawn(move || {
                let result = updater::update_rocket_league(|line| {
                    let _ = std_tx.send(UpdateStreamEvent::Line(line));
                });
                let _ = std_tx.send(UpdateStreamEvent::Done(result.map_err(|e| e.to_string())));
            });

            // Bridges the blocking std channel into the async tokio channel.
            // `recv()` here blocks a threadpool worker (fine — spawn_blocking is
            // for exactly this), not iced's own executor thread.
            tokio::task::spawn_blocking(move || {
                while let Ok(event) = std_rx.recv() {
                    let is_done = matches!(event, UpdateStreamEvent::Done(_));
                    if async_tx.send(event).is_err() || is_done {
                        break;
                    }
                }
            });

            while let Some(event) = async_rx.recv().await {
                match event {
                    UpdateStreamEvent::Line(line) => {
                        let _ = output.send(Message::UpdateLogLine(line)).await;
                    }
                    UpdateStreamEvent::Done(result) => {
                        let _ = output.send(Message::UpdateFinished(result)).await;
                        break;
                    }
                }
            }
        },
    )
}

enum UpdateStreamEvent {
    Line(String),
    Done(Result<(), String>),
}
