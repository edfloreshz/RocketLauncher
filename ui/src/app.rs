use iced::widget::image;
use iced::{Subscription, Task, Theme, stream};

use futures::SinkExt;

use rocket_launcher_core::{
    Config, EPIC_LOGIN_URL, discovery, exchange_code_for_refresh_token,
    gamepad::{Direction, Focus, GamepadAction},
    load_config, open_browser, save_config, updater,
};

use crate::types::{Message, UpdateCheckResult, UpdateEvent};
use crate::worker::{do_launch, gamepad_worker};

pub struct App {
    pub cfg: Config,
    pub auth_code_input: String,
    pub status: String,
    pub busy: bool,
    pub logged_in: bool,
    pub checking_update: bool,
    pub update_available: bool,
    pub installed_version: Option<String>,
    pub updating: bool,
    pub update_log: Vec<String>,
    pub background: image::Handle,
    pub gamepad_connected: bool,
    pub focus: Option<Focus>,
    pub window_id: iced::window::Id,
    pub window_focused: bool,
    pub show_advanced_settings: bool,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let mut cfg = load_config().unwrap_or_default();
        let logged_in = !cfg.epic_refresh_token.trim().is_empty();

        let mut status = if logged_in {
            "Ready. Session loaded from config.json.".to_string()
        } else {
            "Not logged in yet.".to_string()
        };

        // Auto-detect on first run, same as the egui version.
        if cfg == Config::default() {
            let found = discovery::discover_all();
            let mut notes = Vec::new();

            if let Some(p) = found.rocket_league_path {
                cfg.rocket_league_path = p.to_string_lossy().to_string();
                notes.push("RL exe");
            }
            if let Some(p) = found.steam_install_path {
                cfg.steam_install_path = p.to_string_lossy().to_string();
                notes.push("Steam install");
            }
            if let Some(p) = found.proton_path {
                cfg.proton_path = p.to_string_lossy().to_string();
                notes.push("Proton");
            }
            if let Some(p) = found.compat_data_path {
                cfg.compat_data_path = p.to_string_lossy().to_string();
                notes.push("Proton prefix");
            }

            status = if notes.is_empty() {
                "Auto-detect found nothing — fill in paths manually.".to_string()
            } else {
                format!(
                    "Auto-detected: {}. Review and Save settings.",
                    notes.join(", ")
                )
            };
        }

        let background =
            image::Handle::from_bytes(include_bytes!("../assets/background.jpg").as_slice());

        (
            Self {
                cfg,
                auth_code_input: String::new(),
                status,
                busy: false,
                logged_in,
                checking_update: false,
                update_available: false,
                installed_version: None,
                updating: false,
                update_log: Vec::new(),
                background,
                gamepad_connected: false,
                focus: None,
                window_focused: false,
                window_id: iced::window::Id::unique(),
                show_advanced_settings: false,
            },
            Task::none(),
        )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let window = iced::window::events().map(|(id, event)| Message::Window(id, event));
        let gamepad = Subscription::run(gamepad_worker);
        Subscription::batch(vec![gamepad, window])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleAdvancedSettings => {
                self.show_advanced_settings = !self.show_advanced_settings;
                Task::none()
            }
            Message::ExitPressed => iced::window::close(self.window_id),
            Message::Window(id, iced::window::Event::Focused) => {
                self.window_id = id;
                self.window_focused = true;
                Task::none()
            }
            Message::Window(_, iced::window::Event::Unfocused) => {
                self.window_focused = false;
                Task::none()
            }
            Message::Window(_, _) => Task::none(),
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
                if let Err(e) = save_config(&self.cfg) {
                    self.status = format!("{} (failed to save: {e})", self.status);
                }
                Task::none()
            }
            Message::ThemeSelected(theme) => {
                self.cfg.theme = theme.to_string();
                if let Err(e) = save_config(&self.cfg) {
                    self.status = format!("{} (failed to save: {e})", self.status);
                }
                Task::none()
            }

            Message::AutoDetectPressed => {
                self.auto_detect_paths();
                Task::none()
            }
            Message::SaveSettingsPressed => {
                match save_config(&self.cfg) {
                    Ok(()) => self.status = "Settings saved successfully.".to_string(),
                    Err(e) => self.status = format!("Failed to save settings: {e}"),
                }
                Task::none()
            }

            Message::OpenLoginPressed => {
                open_browser(EPIC_LOGIN_URL);
                self.status =
                    "Browser opened. Log in, then paste the authorization code below.".to_string();
                Task::none()
            }
            Message::SwitchAccountPressed => {
                open_browser(EPIC_LOGIN_URL);
                self.status =
                    "Browser opened. Log in, then paste the authorization code below.".to_string();
                self.logged_in = false;
                self.focus = Some(Focus::CodeField);
                Task::none()
            }
            Message::AuthCodeChanged(v) => {
                self.auth_code_input = v;
                Task::none()
            }
            Message::SubmitCodePressed => {
                let code = self.auth_code_input.trim().to_string();
                if code.len() != 32 {
                    self.status = format!(
                        "Authorization code must be 32 characters (got {}).",
                        code.len()
                    );
                    Task::none()
                } else {
                    self.busy = true;
                    self.status = "Exchanging token...".to_string();
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
            }

            Message::LoginFinished(result) => {
                self.busy = false;
                match result {
                    Ok(refresh_token) => {
                        self.cfg.epic_refresh_token = refresh_token;
                        self.logged_in = true;
                        self.auth_code_input.clear();
                        self.focus = Some(Focus::CheckUpdates);

                        if let Err(e) = save_config(&self.cfg) {
                            self.status = format!("Logged in, but failed to save session: {e}");
                        } else {
                            self.status = "Logged in. You can launch now.".to_string();
                        }
                    }
                    Err(e) => self.status = format!("Login failed: {e}"),
                }
                Task::none()
            }

            Message::LaunchPressed => {
                if self.cfg.rocket_league_path.trim().is_empty() {
                    self.status = "Set the Rocket League executable path first.".to_string();
                    Task::none()
                } else if !self.logged_in || self.busy {
                    Task::none()
                } else {
                    self.busy = true;
                    self.status = "Authenticating with Epic Games...".to_string();
                    let cfg = self.cfg.clone();
                    Task::perform(do_launch(cfg), Message::LaunchFinished)
                }
            }
            Message::LaunchFinished(result) => {
                self.busy = false;
                match result {
                    Ok(()) => self.status = "Game launched.".to_string(),
                    Err(e) => self.status = format!("Launch failed: {e}"),
                }
                Task::none()
            }

            Message::CheckUpdatesPressed => {
                if self.checking_update || self.updating {
                    return Task::none();
                }
                self.checking_update = true;
                self.status = "Checking for Rocket League updates via Legendary...".to_string();
                Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            updater::check_for_update()
                                .map(|status| UpdateCheckResult {
                                    installed_version: status.installed_version,
                                    update_available: status.update_available,
                                })
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()))
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
                            (Some(v), true) => format!(
                                "Update available (installed: {v}). Click Update to download it."
                            ),
                            (Some(v), false) => format!("Up to date (installed: {v})."),
                            (None, _) => {
                                "Rocket League isn't installed via Legendary — nothing to check."
                                    .to_string()
                            }
                        };
                    }
                    Err(e) => self.status = format!("Update check failed: {e}"),
                }
                Task::none()
            }

            Message::UpdateNowPressed => {
                if !self.update_available || self.updating {
                    return Task::none();
                }
                self.updating = true;
                self.update_log.clear();
                self.status = "Updating Rocket League via Legendary...".to_string();

                // The updater streams log lines via a sync callback. We bridge
                // that into an async stream with `stream::try_channel` (new
                // in 0.14). IMPORTANT: in try_channel, the closure returning
                // Ok(()) produces NO item on the stream — only Err surfaces
                // as an item. So a clean completion must be pushed explicitly
                // as a sentinel value through the channel; it can't be
                // inferred from the stream simply ending.
                //
                // FIX 2 (E0282): the `output` closure parameter's element
                // type can't be inferred from usage alone here, so it needs
                // an explicit `futures::channel::mpsc::Sender<UpdateEvent>`
                // annotation.
                Task::run(
                    stream::try_channel(
                        100,
                        |mut output: futures::channel::mpsc::Sender<UpdateEvent>| async move {
                            let mut sender = output.clone();
                            let result: Result<(), String> =
                                tokio::task::spawn_blocking(move || {
                                    updater::update_rocket_league(move |line| {
                                        let _ = sender.try_send(UpdateEvent::Line(line));
                                    })
                                    .map_err(|e| e.to_string())
                                })
                                .await
                                .unwrap_or_else(|e| Err(e.to_string()));

                            if result.is_ok() {
                                let _ = output.send(UpdateEvent::Done).await;
                            }

                            result
                        },
                    ),
                    |event: Result<UpdateEvent, String>| match event {
                        Ok(UpdateEvent::Line(line)) => Message::UpdateLogLine(line),
                        Ok(UpdateEvent::Done) => Message::UpdateFinished(Ok(())),
                        Err(e) => Message::UpdateFinished(Err(e)),
                    },
                )
            }
            Message::ThemeSelectorPressed => {
                // Find the index of the currently selected theme
                let current_index = Theme::ALL
                    .iter()
                    .position(|t| t == &self.cfg.get_theme())
                    .unwrap_or(0);

                // Calculate the next index, wrapping around to 0 at the end
                let next_index = (current_index + 1) % Theme::ALL.len();

                // Update the theme
                self.cfg.theme = Theme::ALL[next_index].to_string().clone();
                Task::none()
            }
            Message::UpdateLogLine(line) => {
                self.update_log.push(line);
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

            Message::GamepadConnected => {
                self.gamepad_connected = true;
                if self.focus.is_none() {
                    if self.logged_in {
                        self.focus = Some(Focus::Launch);
                    } else {
                        self.focus = Some(Focus::OpenLogin);
                    }
                }
                Task::none()
            }
            Message::GamepadDisconnected => {
                self.gamepad_connected = false;
                self.focus = None;
                Task::none()
            }
            Message::Gamepad(action) => self.handle_gamepad_action(action),
        }
    }

    /// Mirrors the egui version's `AppMsg::Gamepad` handling, including the
    /// "Start button launches instantly" shortcut and per-focus activation.
    fn handle_gamepad_action(&mut self, action: GamepadAction) -> Task<Message> {
        if !self.window_focused {
            return Task::none();
        }

        self.gamepad_connected = true;
        if self.focus.is_none() {
            self.focus = Some(Focus::AutoDetect);
        }

        match action {
            GamepadAction::Up => {
                if let Some(focus) = self.focus {
                    self.focus = Some(focus.navigate(Direction::Up, self.logged_in));
                }
                Task::none()
            }
            GamepadAction::Down => {
                if let Some(focus) = self.focus {
                    self.focus = Some(focus.navigate(Direction::Down, self.logged_in));
                }
                Task::none()
            }
            GamepadAction::Left => {
                if let Some(focus) = self.focus {
                    self.focus = Some(focus.navigate(Direction::Left, self.logged_in));
                }
                Task::none()
            }
            GamepadAction::Right => {
                if let Some(focus) = self.focus {
                    self.focus = Some(focus.navigate(Direction::Right, self.logged_in));
                }
                Task::none()
            }
            GamepadAction::Select => {
                // Dispatch to whatever action the currently-focused widget
                // represents, since Iced has no direct "synthesize a click"
                // equivalent to egui's `response.clicked() || select_pressed`.
                match self.focus {
                    Some(Focus::CheckUpdates) => self.update(Message::CheckUpdatesPressed),
                    Some(Focus::UpdateNow) => self.update(Message::UpdateNowPressed),
                    Some(Focus::SwitchAccount) => self.update(Message::SwitchAccountPressed),
                    Some(Focus::OpenLogin) => self.update(Message::OpenLoginPressed),
                    Some(Focus::Launch) => self.update(Message::LaunchPressed),
                    Some(Focus::SkipEasyAntiCheat) => {
                        let new_val = !self.cfg.skip_eac;
                        self.update(Message::SkipEacToggled(new_val))
                    }
                    Some(Focus::CodeField) => Task::none(), // text field, nothing to "click"
                    Some(Focus::SubmitCode) => self.update(Message::SubmitCodePressed),
                    Some(Focus::AutoDetect) => self.update(Message::AutoDetectPressed),
                    Some(Focus::SaveSettings) => self.update(Message::SaveSettingsPressed),
                    Some(Focus::ThemeSelector) => self.update(Message::ThemeSelectorPressed),
                    Some(Focus::ToggleAdvancedSettings) => {
                        self.update(Message::ToggleAdvancedSettings)
                    }
                    Some(Focus::Exit) => self.update(Message::ExitPressed),
                    None => Task::none(),
                }
            }
            GamepadAction::LaunchShortcut => {
                if self.logged_in && !self.busy && !self.cfg.rocket_league_path.trim().is_empty() {
                    self.update(Message::LaunchPressed)
                } else {
                    Task::none()
                }
            }
        }
    }

    fn auto_detect_paths(&mut self) {
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
    }
}
