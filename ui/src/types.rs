use iced::Theme;
use rocket_launcher_core::gamepad::GamepadAction;

#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub installed_version: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone)]
pub enum UpdateEvent {
    Line(String),
    Done,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Settings field edits
    RocketLeaguePathChanged(String),
    ProtonPathChanged(String),
    CompatDataPathChanged(String),
    SteamInstallPathChanged(String),
    SkipEacToggled(bool),
    ThemeSelected(Theme),
    Window(iced::window::Id, iced::window::Event),

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
    ExitPressed,

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
