use iced::widget::image;
use iced::{Subscription, Task, Theme, stream};

use futures::SinkExt;

use rocket_launcher_core::{
    Config, EPIC_LOGIN_URL, discovery, exchange_code_for_refresh_token,
    gamepad::{Direction, Focus, GamepadAction},
    load_config, open_browser, save_config, updater,
};

use crate::types::{Message, UpdateCheckResult, UpdateEvent};
use crate::worker::{gamepad_worker, launch};

#[derive(Debug, Clone)]
pub struct App {
    pub state: AppState,
    pub auth: AuthState,
    pub updates: Updates,
    pub gamepad: GamepadState,
    pub config: Config,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub busy: bool,
    pub status: String,
    pub background: image::Handle,
    pub window_id: iced::window::Id,
    pub window_focused: bool,
    pub show_advanced_settings: bool,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    pub logged_in: bool,
    pub auth_code_input: String,
}

#[derive(Debug, Clone)]
pub struct Updates {
    pub log: Vec<String>,
    pub installed_version: Option<String>,
    pub state: UpdateState,
}

#[derive(Debug, Clone)]
pub struct GamepadState {
    pub connected: bool,
    pub focus: Option<Focus>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    CheckingUpdate,
    UpdateAvailable,
    Updating,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let config = load_config().unwrap_or_default();
        let logged_in = !config.epic_refresh_token.trim().is_empty();
        let status = if logged_in {
            "Ready. Session loaded from config.json.".to_string()
        } else {
            "Not logged in yet.".to_string()
        };

        let mut app = Self {
            state: AppState {
                busy: false,
                status,
                background: image::Handle::from_bytes(
                    include_bytes!("../assets/background.jpg").as_slice(),
                ),
                window_id: iced::window::Id::unique(),
                window_focused: false,
                show_advanced_settings: false,
            },
            auth: AuthState {
                logged_in,
                auth_code_input: String::new(),
            },
            updates: Updates {
                log: Vec::new(),
                installed_version: None,
                state: UpdateState::Idle,
            },
            gamepad: GamepadState {
                connected: false,
                focus: None,
            },
            config,
        };

        (
            app.clone(),
            Task::batch(vec![app.update(Message::AutoDetectPaths)]),
        )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let window = iced::window::events().map(|(id, event)| Message::Window(id, event));
        let gamepad = Subscription::run(gamepad_worker);
        Subscription::batch(vec![gamepad, window])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ExitPressed => iced::window::close(self.state.window_id),
            Message::Window(id, iced::window::Event::Focused) => {
                self.state.window_id = id;
                self.state.window_focused = true;
                Task::none()
            }
            Message::Window(_, iced::window::Event::Unfocused) => {
                self.state.window_focused = false;
                Task::none()
            }
            Message::Window(_, _) => Task::none(),
            Message::ToggleAdvancedSettings => {
                self.state.show_advanced_settings = !self.state.show_advanced_settings;
                Task::none()
            }
            Message::RocketLeaguePathChanged(v) => {
                self.config.rocket_league_path = v;
                Task::none()
            }
            Message::ProtonPathChanged(v) => {
                self.config.proton_path = v;
                Task::none()
            }
            Message::CompatDataPathChanged(v) => {
                self.config.compat_data_path = v;
                Task::none()
            }
            Message::SteamInstallPathChanged(v) => {
                self.config.steam_install_path = v;
                Task::none()
            }
            Message::SkipEacToggled(v) => {
                self.config.skip_eac = v;
                if let Err(e) = save_config(&self.config) {
                    self.state.status = format!("{} (failed to save: {e})", self.state.status);
                }
                Task::none()
            }
            Message::ThemeSelected(theme) => {
                self.config.theme = theme.to_string();
                if let Err(e) = save_config(&self.config) {
                    self.state.status = format!("{} (failed to save: {e})", self.state.status);
                }
                Task::none()
            }
            Message::AutoDetectPaths => {
                if self.config == Config::default() {
                    let (_, notes) = Self::auto_detect_paths(&mut self.config);
                    self.state.status = if notes.is_empty() {
                        "Auto-detect found nothing — fill in paths manually.".to_string()
                    } else {
                        format!(
                            "Auto-detected: {}. Review and Save settings.",
                            notes.join(", ")
                        )
                    };
                }
                Task::none()
            }
            Message::AutoDetectPressed => {
                let (config, notes) = Self::auto_detect_paths(&mut self.config);

                self.state.status = if notes.is_empty() {
                    "Auto-detect found nothing — fill in paths manually.".to_string()
                } else {
                    format!(
                        "Auto-detected: {}. Review and Save settings.",
                        notes.join(", ")
                    )
                };

                if let Err(e) = save_config(&config) {
                    self.state.status = format!("{} (failed to save: {e})", self.state.status);
                }
                Task::none()
            }
            Message::SaveSettingsPressed => {
                match save_config(&self.config) {
                    Ok(()) => self.state.status = "Settings saved successfully.".to_string(),
                    Err(e) => self.state.status = format!("Failed to save settings: {e}"),
                }
                Task::none()
            }

            Message::OpenLoginPressed => {
                open_browser(EPIC_LOGIN_URL);
                self.state.status =
                    "Browser opened. Log in, then paste the authorization code below.".to_string();
                Task::none()
            }
            Message::SwitchAccountPressed => {
                open_browser(EPIC_LOGIN_URL);
                self.state.status =
                    "Browser opened. Log in, then paste the authorization code below.".to_string();
                self.auth.logged_in = false;
                self.gamepad.focus = Some(Focus::CodeField);
                Task::none()
            }
            Message::AuthCodeChanged(v) => {
                self.auth.auth_code_input = v;
                Task::none()
            }
            Message::SubmitCodePressed => {
                let code = self.auth.auth_code_input.trim().to_string();
                if code.len() != 32 {
                    self.state.status = format!(
                        "Authorization code must be 32 characters (got {}).",
                        code.len()
                    );
                    Task::none()
                } else {
                    self.state.busy = true;
                    self.state.status = "Exchanging token...".to_string();
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
                self.state.busy = false;
                match result {
                    Ok(refresh_token) => {
                        self.config.epic_refresh_token = refresh_token;
                        self.auth.logged_in = true;
                        self.auth.auth_code_input.clear();
                        self.gamepad.focus = Some(Focus::CheckUpdates);

                        if let Err(e) = save_config(&self.config) {
                            self.state.status =
                                format!("Logged in, but failed to save session: {e}");
                        } else {
                            self.state.status = "Logged in. You can launch now.".to_string();
                        }
                    }
                    Err(e) => self.state.status = format!("Login failed: {e}"),
                }
                Task::none()
            }

            Message::LaunchPressed => {
                if self.config.rocket_league_path.trim().is_empty() {
                    self.state.status = "Set the Rocket League executable path first.".to_string();
                    Task::none()
                } else if !self.auth.logged_in || self.state.busy {
                    Task::none()
                } else {
                    self.state.busy = true;
                    self.state.status = "Authenticating with Epic Games...".to_string();
                    let cfg = self.config.clone();
                    Task::perform(launch(cfg), Message::LaunchFinished)
                }
            }
            Message::LaunchFinished(result) => {
                self.state.busy = false;
                match result {
                    Ok(()) => self.state.status = "Game launched.".to_string(),
                    Err(e) => self.state.status = format!("Launch failed: {e}"),
                }
                Task::none()
            }

            Message::CheckUpdatesPressed => {
                if self.updates.state == UpdateState::CheckingUpdate
                    || self.updates.state == UpdateState::Updating
                {
                    return Task::none();
                }
                self.updates.state = UpdateState::CheckingUpdate;
                self.state.status =
                    "Checking for Rocket League updates via Legendary...".to_string();
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
                self.updates.state = UpdateState::Idle;
                match result {
                    Ok(r) => {
                        self.updates.installed_version = r.installed_version.clone();
                        if r.update_available {
                            self.updates.state = UpdateState::UpdateAvailable;
                        }
                        self.state.status = match (&r.installed_version, r.update_available) {
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
                    Err(e) => self.state.status = format!("Update check failed: {e}"),
                }
                Task::none()
            }

            Message::UpdateNowPressed => {
                if self.updates.state != UpdateState::UpdateAvailable
                    || self.updates.state == UpdateState::Updating
                {
                    return Task::none();
                }
                self.updates.state = UpdateState::Updating;
                self.updates.log.clear();
                self.state.status = "Updating Rocket League via Legendary...".to_string();

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
                let current_index = Theme::ALL
                    .iter()
                    .position(|t| t == &self.config.get_theme())
                    .unwrap_or(0);

                let next_index = (current_index + 1) % Theme::ALL.len();

                self.config.theme = Theme::ALL[next_index].to_string().clone();
                Task::none()
            }
            Message::UpdateLogLine(line) => {
                self.updates.log.push(line);
                if self.updates.log.len() > 500 {
                    let excess = self.updates.log.len() - 500;
                    self.updates.log.drain(0..excess);
                }
                Task::none()
            }
            Message::UpdateFinished(result) => {
                self.updates.state = UpdateState::Idle;
                match result {
                    Ok(()) => {
                        self.state.status = "Update complete.".to_string();
                    }
                    Err(e) => self.state.status = format!("Update failed: {e}"),
                }
                Task::none()
            }

            Message::GamepadConnected => {
                self.gamepad.connected = true;
                if self.gamepad.focus.is_none() {
                    if self.auth.logged_in {
                        self.gamepad.focus = Some(Focus::Launch);
                    } else {
                        self.gamepad.focus = Some(Focus::OpenLogin);
                    }
                }
                Task::none()
            }
            Message::GamepadDisconnected => {
                self.gamepad.connected = false;
                self.gamepad.focus = None;
                Task::none()
            }
            Message::Gamepad(action) => self.handle_gamepad_action(action),
        }
    }

    fn handle_gamepad_action(&mut self, action: GamepadAction) -> Task<Message> {
        if !self.state.window_focused {
            return Task::none();
        }

        self.gamepad.connected = true;
        if self.gamepad.focus.is_none() {
            self.gamepad.focus = Some(Focus::AutoDetect);
        }

        match action {
            GamepadAction::Up => {
                if let Some(focus) = self.gamepad.focus {
                    self.gamepad.focus = Some(focus.navigate(Direction::Up, self.auth.logged_in));
                }
                Task::none()
            }
            GamepadAction::Down => {
                if let Some(focus) = self.gamepad.focus {
                    self.gamepad.focus = Some(focus.navigate(Direction::Down, self.auth.logged_in));
                }
                Task::none()
            }
            GamepadAction::Left => {
                if let Some(focus) = self.gamepad.focus {
                    self.gamepad.focus = Some(focus.navigate(Direction::Left, self.auth.logged_in));
                }
                Task::none()
            }
            GamepadAction::Right => {
                if let Some(focus) = self.gamepad.focus {
                    self.gamepad.focus =
                        Some(focus.navigate(Direction::Right, self.auth.logged_in));
                }
                Task::none()
            }
            GamepadAction::Select => {
                match self.gamepad.focus {
                    Some(Focus::CheckUpdates) => self.update(Message::CheckUpdatesPressed),
                    Some(Focus::UpdateNow) => self.update(Message::UpdateNowPressed),
                    Some(Focus::SwitchAccount) => self.update(Message::SwitchAccountPressed),
                    Some(Focus::OpenLogin) => self.update(Message::OpenLoginPressed),
                    Some(Focus::Launch) => self.update(Message::LaunchPressed),
                    Some(Focus::SkipEasyAntiCheat) => {
                        let new_val = !self.config.skip_eac;
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
                if self.auth.logged_in
                    && !self.state.busy
                    && !self.config.rocket_league_path.trim().is_empty()
                {
                    self.update(Message::LaunchPressed)
                } else {
                    Task::none()
                }
            }
        }
    }

    fn auto_detect_paths(config: &mut Config) -> (&mut Config, Vec<&str>) {
        let found = discovery::discover_all();
        let mut notes = Vec::new();

        if let Some(p) = found.rocket_league_path {
            config.rocket_league_path = p.to_string_lossy().to_string();
            notes.push("RL exe");
        }
        if let Some(p) = found.steam_install_path {
            config.steam_install_path = p.to_string_lossy().to_string();
            notes.push("Steam install");
        }
        if let Some(p) = found.proton_path {
            config.proton_path = p.to_string_lossy().to_string();
            notes.push("Proton");
        }
        if let Some(p) = found.compat_data_path {
            config.compat_data_path = p.to_string_lossy().to_string();
            notes.push("Proton prefix");
        }

        (config, notes)
    }
}
