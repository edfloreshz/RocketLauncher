// Rocket League launcher.

use iced::theme::Base;
use iced::widget::{
    self, button, checkbox, column, container, image, pick_list, row, scrollable, stack, text,
    text_input,
};
use iced::{
    Alignment, Background, Border, Element, Font, Length, Subscription, Task, Theme, stream,
};

use gilrs::{Axis, Button as PadButton, Event as GilEvent, EventType, Gilrs};

use futures::SinkExt;

use rocket_launcher::{
    Config, EPIC_LOGIN_URL, LaunchCredentials, discovery, exchange_code_for_refresh_token,
    gamepad::{Direction, Focus, GamepadAction},
    get_launch_credentials, launch_game, load_config, open_browser, save_config, updater,
};

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Rocket League Launcher")
        .subscription(App::subscription)
        .theme(|state: &App| state.cfg.get_theme().clone())
        .window(iced::window::Settings {
            fullscreen: true,
            ..Default::default()
        })
        .run()
}

#[derive(Debug, Clone)]
struct UpdateCheckResult {
    installed_version: Option<String>,
    update_available: bool,
}

#[derive(Debug, Clone)]
enum UpdateEvent {
    Line(String),
    Done,
}

#[derive(Debug, Clone)]
enum Message {
    // Settings field edits
    RocketLeaguePathChanged(String),
    ProtonPathChanged(String),
    CompatDataPathChanged(String),
    SteamInstallPathChanged(String),
    SkipEacToggled(bool),
    ThemeSelected(Theme),

    // Buttons
    AutoDetectPressed,
    SaveSettingsPressed,
    OpenLoginPressed,
    SwitchAccountPressed,
    AuthCodeChanged(String),
    SubmitCodePressed,
    LaunchPressed,
    CheckUpdatesPressed,
    UpdateNowPressed,
    ThemeSelectorPressed,

    // Async completions
    LoginFinished(Result<String, String>),
    LaunchFinished(Result<(), String>),
    UpdateCheckFinished(Result<UpdateCheckResult, String>),
    UpdateLogLine(String),
    UpdateFinished(Result<(), String>),

    // Gamepad subscription events
    Gamepad(GamepadAction),
    GamepadConnected,
    GamepadDisconnected,
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
    background: image::Handle,
    gamepad_connected: bool,
    focus: Option<Focus>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
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
            },
            Task::none(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::run(gamepad_worker)
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

    fn view(&self) -> Element<'_, Message> {
        let bg = image(self.background.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Cover);

        let panel = self.settings_panel();

        let centered = container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding(24);

        stack![bg, centered].into()
    }

    fn focused_button<'a>(
        &'a self,
        label: &'a str,
        focus_variant: Focus,
        message: Message,
        enabled: bool,
    ) -> Element<'a, Message> {
        let is_focused = self.focus == Some(focus_variant);

        let mut b = button(text(label))
            .padding([10, 16])
            .style(widget::button::subtle);
        b = if enabled { b.on_press(message) } else { b };

        if is_focused {
            b = b.style(widget::button::primary);
        }

        b.into()
    }

    fn settings_panel(&self) -> Element<'_, Message> {
        let launch_col = column![
            text("Rocket Launcher").size(28),
            row![
                self.focused_button(
                    if self.checking_update {
                        "Checking..."
                    } else {
                        "Check for Updates"
                    },
                    Focus::CheckUpdates,
                    Message::CheckUpdatesPressed,
                    !self.checking_update && !self.updating,
                ),
                widget::space().width(10),
                self.focused_button(
                    if self.updating {
                        "Updating..."
                    } else {
                        "Update Now"
                    },
                    Focus::UpdateNow,
                    Message::UpdateNowPressed,
                    self.update_available && !self.updating,
                ),
            ],
            if let Some(v) = &self.installed_version {
                text(format!("Installed version: {v}"))
            } else {
                text("")
            },
        ]
        .spacing(12);

        let launch_col: Element<Message> = if !self.update_log.is_empty() {
            let log_text = self.update_log.join("\n");
            column![
                launch_col,
                scrollable(text(log_text).font(Font::MONOSPACE))
                    .height(Length::Fixed(140.0))
                    .anchor_bottom(),
            ]
            .spacing(12)
            .into()
        } else {
            launch_col.into()
        };

        let account_col = if self.logged_in {
            column![
                text("Epic Games Account").size(28),
                row![
                    self.focused_button(
                        "Switch Account",
                        Focus::SwitchAccount,
                        Message::SwitchAccountPressed,
                        true,
                    ),
                    widget::space().width(10),
                    text("Authenticated").color(self.cfg.get_theme().palette().success),
                ]
                .align_y(Alignment::Center),
            ]
        } else {
            column![
                text("Epic Games Account").size(28),
                self.focused_button(
                    "Open Epic Login Page",
                    Focus::OpenLogin,
                    Message::OpenLoginPressed,
                    true,
                ),
            ]
        }
        .spacing(12);

        let launch_label = if self.busy {
            "Working..."
        } else {
            "LAUNCH ROCKET LEAGUE"
        };
        let launch_enabled = self.logged_in && !self.busy;
        let launch_button = self.focused_button(
            launch_label,
            Focus::Launch,
            Message::LaunchPressed,
            launch_enabled,
        );

        let skip_eac_focused = self.focus == Some(Focus::SkipEasyAntiCheat);
        let mut eac_checkbox = checkbox(self.cfg.skip_eac)
            .label("Skip Easy Anti-Cheat (offline modes only)")
            .on_toggle(Message::SkipEacToggled);
        if skip_eac_focused {
            eac_checkbox = eac_checkbox.style(move |theme: &Theme, status| {
                let mut style = checkbox::primary(theme, status);
                style.background = Background::Color(self.cfg.get_theme().palette().primary);
                style
            });
        }

        let launch_col2 = column![launch_button, eac_checkbox].spacing(12);

        let top_row = row![launch_col, account_col, launch_col2]
            .spacing(24)
            .width(Length::Fill);

        let login_row: Element<Message> = if !self.logged_in {
            let code_focused = self.focus == Some(Focus::CodeField);
            let mut code_input = text_input(
                "Paste 32-character authorization code here",
                &self.auth_code_input,
            )
            .on_input(Message::AuthCodeChanged)
            .on_submit(Message::SubmitCodePressed)
            .width(Length::Fill);

            if code_focused {
                code_input = code_input.style(move |theme: &Theme, status| {
                    let mut style = text_input::default(theme, status);
                    if self.focus == Some(Focus::ThemeSelector) {
                        style.border = Border {
                            color: self.cfg.get_theme().palette().primary,
                            width: 2.0,
                            ..Default::default()
                        }
                    }
                    style
                })
            }

            row![
                code_input,
                widget::space().width(10),
                self.focused_button(
                    "Submit Code",
                    Focus::SubmitCode,
                    Message::SubmitCodePressed,
                    true,
                ),
            ]
            .into()
        } else {
            widget::space().into()
        };

        let status_line = text(&self.status).size(14);

        let settings_grid = column![
            text("Launcher Settings").size(24),
            labeled_input(
                "Rocket League Executable:",
                &self.cfg.rocket_league_path,
                Message::RocketLeaguePathChanged,
            ),
            labeled_input(
                "Proton Binary Path:",
                &self.cfg.proton_path,
                Message::ProtonPathChanged,
            ),
            labeled_input(
                "Compat Data Prefix:",
                &self.cfg.compat_data_path,
                Message::CompatDataPathChanged,
            ),
            labeled_input(
                "Steam Install Path:",
                &self.cfg.steam_install_path,
                Message::SteamInstallPathChanged,
            ),
            row![
                container(text("Theme:").size(15)).width(Length::Fixed(220.0)),
                pick_list(Theme::ALL, Some(self.cfg.get_theme()), |t: Theme| {
                    Message::ThemeSelected(t)
                })
                .width(Length::Fill)
                .style(move |theme: &Theme, status| {
                    let mut style = pick_list::default(theme, status);
                    if self.focus == Some(Focus::ThemeSelector) {
                        style.border = Border {
                            color: self.cfg.get_theme().palette().primary,
                            width: 2.0,
                            ..Default::default()
                        }
                    }
                    style
                })
            ]
            .spacing(20)
            .align_y(Alignment::Center),
            row![
                self.focused_button(
                    "Auto-detect paths",
                    Focus::AutoDetect,
                    Message::AutoDetectPressed,
                    true,
                ),
                widget::space().width(10),
                self.focused_button(
                    "Save settings",
                    Focus::SaveSettings,
                    Message::SaveSettingsPressed,
                    true,
                ),
            ],
        ]
        .spacing(14);

        let items = if self.logged_in {
            column![top_row, status_line, scrollable(settings_grid),]
        } else {
            column![top_row, login_row, status_line, scrollable(settings_grid),]
        };

        let body = items.spacing(16).width(Length::Fixed(1000.0));

        container(body)
            .padding(24)
            .style(|_theme| container::Style {
                background: Some(Background::Color(
                    self.cfg.get_theme().base().background_color,
                )),
                border: Border {
                    radius: 12.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .into()
    }
}

fn labeled_input<'a>(
    label: &'a str,
    value: &'a str,
    on_change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        container(text(label).size(15)).width(Length::Fixed(220.0)),
        text_input("", value)
            .on_input(on_change)
            .width(Length::Fill),
    ]
    .spacing(20)
    .align_y(Alignment::Center)
    .into()
}

async fn do_launch(mut cfg: Config) -> Result<(), String> {
    let client = reqwest::Client::new();
    let (creds, new_refresh_token): (LaunchCredentials, String) =
        get_launch_credentials(&client, &cfg.epic_refresh_token)
            .await
            .map_err(|e| e.to_string())?;

    if new_refresh_token != cfg.epic_refresh_token {
        cfg.epic_refresh_token = new_refresh_token;
        save_config(&cfg).map_err(|e| e.to_string())?;
    }

    launch_game(&cfg, &creds, &[]).map_err(|e| e.to_string())
}

/// Long-lived subscription stream that owns the gilrs context on a blocking
/// thread and forwards pad events into Iced's message loop. This is the
/// Subscription-based replacement for the egui version's
/// `std::thread::spawn` + `mpsc::Sender<AppMsg>` + `ctx.request_repaint()`.
///
/// FIX 3 (E0282): the `output` closure parameter's element type can't be
/// inferred from usage alone, so it needs an explicit
/// `futures::channel::mpsc::Sender<Message>` annotation.
fn gamepad_worker() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(
        100,
        |mut output: futures::channel::mpsc::Sender<Message>| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

            std::thread::spawn(move || {
                if let Ok(mut gilrs) = Gilrs::new() {
                    for (_id, _gamepad) in gilrs.gamepads() {
                        let _ = tx.send(Message::GamepadConnected);
                    }

                    let mut left_stick_y_neutral = true;
                    let mut left_stick_x_neutral = true;

                    loop {
                        while let Some(GilEvent { event, .. }) = gilrs.next_event() {
                            match event {
                                EventType::Connected => {
                                    let _ = tx.send(Message::GamepadConnected);
                                }
                                EventType::Disconnected => {
                                    let _ = tx.send(Message::GamepadDisconnected);
                                }
                                EventType::ButtonPressed(button, _) => {
                                    let action = match button {
                                        PadButton::DPadUp => Some(GamepadAction::Up),
                                        PadButton::DPadDown => Some(GamepadAction::Down),
                                        PadButton::DPadLeft => Some(GamepadAction::Left),
                                        PadButton::DPadRight => Some(GamepadAction::Right),
                                        PadButton::South => Some(GamepadAction::Select),
                                        PadButton::Start => Some(GamepadAction::LaunchShortcut),
                                        _ => None,
                                    };
                                    if let Some(act) = action {
                                        let _ = tx.send(Message::Gamepad(act));
                                    }
                                }
                                EventType::AxisChanged(axis, value, _) => {
                                    const THRESHOLD: f32 = 0.5;
                                    if axis == Axis::LeftStickY {
                                        if value.abs() < 0.2 {
                                            left_stick_y_neutral = true;
                                        } else if left_stick_y_neutral && value.abs() > THRESHOLD {
                                            left_stick_y_neutral = false;
                                            let act = if value > 0.0 {
                                                GamepadAction::Up
                                            } else {
                                                GamepadAction::Down
                                            };
                                            let _ = tx.send(Message::Gamepad(act));
                                        }
                                    }
                                    if axis == Axis::LeftStickX {
                                        if value.abs() < 0.2 {
                                            left_stick_x_neutral = true;
                                        } else if left_stick_x_neutral && value.abs() > THRESHOLD {
                                            left_stick_x_neutral = false;
                                            let act = if value > 0.0 {
                                                GamepadAction::Right
                                            } else {
                                                GamepadAction::Left
                                            };
                                            let _ = tx.send(Message::Gamepad(act));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        gilrs.inc();
                        std::thread::sleep(std::time::Duration::from_millis(16));
                    }
                }
            });

            while let Some(msg) = rx.recv().await {
                if output.send(msg).await.is_err() {
                    break;
                }
            }
        },
    )
}
