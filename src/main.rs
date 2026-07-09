// egui/eframe GUI for the Rocket League / Epic Games launcher.
//
// Layout:
//  - Settings fields: RL exe path, Proton path, compat data path, Steam
//    install path, "skip EAC (offline only)" toggle.
//  - Login: shows a button to open the Epic login page, then a text field to
//    paste the authorization code, then a "Launch" button once a session
//    exists.
//  - Status line showing what's currently happening / any error.
//
// All network + process work happens inside background tasks using a dedicated
// tokio Runtime so the UI thread never blocks.

use eframe::egui;
use gilrs::{Axis, Button, Event, EventType, Gilrs};
use rocket_launcher::{
    Config, EPIC_LOGIN_URL, LaunchCredentials, discovery, exchange_code_for_refresh_token,
    gamepad::{Focus, GamepadAction},
    get_launch_credentials, launch_game, load_config, open_browser, save_config, updater,
};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, Sender},
};

// Represents events sent from background async/blocking tasks back to the UI thread.
enum AppMsg {
    LoginFinished(Result<String, String>),
    LaunchFinished(Result<(), String>),
    UpdateCheckFinished(Result<UpdateCheckResult, String>),
    UpdateLogLine(String),
    UpdateFinished(Result<(), String>),
    Gamepad(GamepadAction),
    GamepadConnected,
    GamepadDisconnected,
}

#[derive(Debug, Clone)]
struct UpdateCheckResult {
    installed_version: Option<String>,
    update_available: bool,
}

pub fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 700.0])
            .with_title("Rocket League Launcher"),
        ..Default::default()
    };

    eframe::run_native(
        "Rocket League Launcher",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc.egui_ctx.clone())))),
    )
}

struct App {
    cfg: Config,
    ctx: egui::Context,
    auth_code_input: String,
    status: String,
    busy: bool,
    logged_in: bool,
    checking_update: bool,
    update_available: bool,
    installed_version: Option<String>,
    updating: bool,
    update_log: Vec<String>,
    background: egui::TextureHandle,

    // Gamepad
    gamepad_connected: bool,
    focus: Option<Focus>,
    select_pressed: bool,

    // Threading / async interop
    rt: Arc<tokio::runtime::Runtime>,
    tx: Sender<AppMsg>,
    rx: Receiver<AppMsg>,
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        let cfg = load_config().unwrap_or_default();
        let logged_in = !cfg.epic_refresh_token.trim().is_empty();
        let (tx, rx) = mpsc::channel();
        let gilrs_tx = tx.clone();
        let gilrs_ctx = ctx.clone();

        // Spin up a dedicated tokio runtime for reqwest, isolated from winit's main thread.
        let rt = Arc::new(tokio::runtime::Runtime::new().expect("Failed to build tokio runtime"));

        std::thread::spawn(move || {
            // Initialize gilrs. If no gamepads are connected, it still runs safely.
            if let Ok(mut gilrs) = Gilrs::new() {
                let mut left_stick_y_neutral = true;
                let mut left_stick_x_neutral = true;

                loop {
                    while let Some(Event { event, .. }) = gilrs.next_event() {
                        match event {
                            EventType::Connected => {
                                let _ = gilrs_tx.send(AppMsg::GamepadConnected);
                            }
                            EventType::Disconnected => {
                                let _ = gilrs_tx.send(AppMsg::GamepadDisconnected);
                            }

                            // 1. Handle D-pad and Face Buttons
                            EventType::ButtonPressed(button, _) => {
                                let action = match button {
                                    Button::DPadUp => Some(GamepadAction::Up),
                                    Button::DPadDown => Some(GamepadAction::Down),
                                    Button::DPadLeft => Some(GamepadAction::Left),
                                    Button::DPadRight => Some(GamepadAction::Right),
                                    Button::South => Some(GamepadAction::Select), // A / Cross
                                    Button::Start => Some(GamepadAction::LaunchShortcut), // Start button
                                    _ => None,
                                };
                                if let Some(act) = action {
                                    let _ = gilrs_tx.send(AppMsg::Gamepad(act));
                                    gilrs_ctx.request_repaint();
                                }
                            }
                            // 2. Convert Left Analog Stick to D-pad steps (with a deadzone)
                            EventType::AxisChanged(axis, value, _) => {
                                const THRESHOLD: f32 = 0.5;
                                if axis == Axis::LeftStickY {
                                    if value.abs() < 0.2 {
                                        left_stick_y_neutral = true;
                                    } else if left_stick_y_neutral && value.abs() > THRESHOLD {
                                        left_stick_y_neutral = false;
                                        // Gilrs Y-axis is positive pointing up
                                        let act = if value > 0.0 {
                                            GamepadAction::Up
                                        } else {
                                            GamepadAction::Down
                                        };
                                        let _ = gilrs_tx.send(AppMsg::Gamepad(act));
                                        gilrs_ctx.request_repaint();
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
                                        let _ = gilrs_tx.send(AppMsg::Gamepad(act));
                                        gilrs_ctx.request_repaint();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    gilrs.inc();
                    std::thread::sleep(std::time::Duration::from_millis(16)); // ~60Hz polling
                }
            }
        });

        // NOTE: To load a real image file, add the `image` crate to your Cargo.toml and use:
        let bg_texture = ctx.load_texture(
            "bg-main",
            {
                let image_bytes = include_bytes!("../assets/background.jpg");
                let decoded = image::load_from_memory(image_bytes).unwrap().to_rgba8();
                egui::ColorImage::from_rgba_unmultiplied(
                    [decoded.width() as _, decoded.height() as _],
                    &decoded,
                )
            },
            Default::default(),
        );

        Self {
            cfg,
            ctx,
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
            background: bg_texture,

            gamepad_connected: false,
            focus: None,
            select_pressed: false,

            rt,
            tx,
            rx,
        }
    }

    /// Drains the channel for any messages completed by background threads.
    fn process_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMsg::GamepadConnected => {
                    self.gamepad_connected = true;

                    if self.focus.is_none() {
                        self.focus = Some(Focus::AutoDetect);
                    }
                }
                AppMsg::GamepadDisconnected => {
                    self.gamepad_connected = false;
                    self.focus = None;
                }
                AppMsg::Gamepad(action) => {
                    self.gamepad_connected = true;
                    if self.focus.is_none() {
                        self.focus = Some(Focus::AutoDetect);
                    }
                    match action {
                        GamepadAction::Up | GamepadAction::Left => {
                            if let Some(focus) = self.focus {
                                self.focus = Some(focus.previous(self.logged_in));
                            }
                        }
                        GamepadAction::Down | GamepadAction::Right => {
                            if let Some(focus) = self.focus {
                                self.focus = Some(focus.next(self.logged_in));
                            }
                        }
                        GamepadAction::Select => {
                            self.select_pressed = true;
                        }
                        GamepadAction::LaunchShortcut => {
                            // Global hotkey: Pressing 'Start' on the controller launches the game instantly
                            if self.logged_in
                                && !self.busy
                                && !self.cfg.rocket_league_path.trim().is_empty()
                            {
                                self.busy = true;
                                self.status =
                                    "Authenticating with Epic Games (Gamepad)...".to_string();
                                let cfg = self.cfg.clone();
                                let tx = self.tx.clone();
                                let ctx_clone = self.ctx.clone();
                                self.rt.spawn(async move {
                                    let res = do_launch(cfg).await;
                                    let _ = tx.send(AppMsg::LaunchFinished(res));
                                    ctx_clone.request_repaint();
                                });
                            }
                        }
                    }
                }

                AppMsg::LoginFinished(result) => {
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
                        Err(e) => {
                            self.status = format!("Login failed: {e}");
                        }
                    }
                }
                AppMsg::LaunchFinished(result) => {
                    self.busy = false;
                    match result {
                        Ok(()) => self.status = "Game launched.".to_string(),
                        Err(e) => self.status = format!("Launch failed: {e}"),
                    }
                }
                AppMsg::UpdateCheckFinished(result) => {
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
                        Err(e) => {
                            self.status = format!("Update check failed: {e}");
                        }
                    }
                }
                AppMsg::UpdateLogLine(line) => {
                    self.update_log.push(line);
                    if self.update_log.len() > 500 {
                        let excess = self.update_log.len() - 500;
                        self.update_log.drain(0..excess);
                    }
                }
                AppMsg::UpdateFinished(result) => {
                    self.updating = false;
                    match result {
                        Ok(()) => {
                            self.status = "Update complete.".to_string();
                            self.update_available = false;
                        }
                        Err(e) => self.status = format!("Update failed: {e}"),
                    }
                }
            }
        }
    }

    fn gamepad_button(
        &mut self,
        ui: &mut egui::Ui,
        enabled: bool,
        focus: Focus,
        button: egui::Button<'_>,
    ) -> bool {
        let mut button = button;

        if Some(focus) == self.focus {
            button = button.stroke(egui::Stroke::new(3.0, egui::Color32::YELLOW));
        }

        let response = ui.add_enabled(enabled, button);

        response.clicked()
            || (enabled && Some(focus) == self.focus && std::mem::take(&mut self.select_pressed))
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 1. Process any incoming messages from background threads
        self.process_messages();
        let ctx = ui.ctx().clone();

        // 2. FULL-SCREEN OPTIMIZATIONS
        // Globally scale everything up by 35% for readability in full screen
        ctx.set_pixels_per_point(1.35);

        // Increase spacing between elements and add chunkier padding to buttons
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 16.0);
        ui.spacing_mut().button_padding = egui::vec2(16.0, 10.0);

        // 3. BACKGROUND IMAGE PAINTING
        // Paint the background across the entire window before drawing any UI elements on top
        let screen_rect = ui.max_rect();

        // Get dimensions and find aspect ratios
        let screen_aspect = screen_rect.width() / screen_rect.height();
        let texture_size = self.background.size_vec2();
        let texture_aspect = texture_size.x / texture_size.y;

        // Calculate a cropped UV rectangle based on which dimension is "too wide"
        let uv_rect = if screen_aspect > texture_aspect {
            // The window is wider than the image -> crop the top and bottom of the image
            let uv_height = texture_aspect / screen_aspect;
            let y_min = 0.5 * (1.0 - uv_height);
            let y_max = 0.5 * (1.0 + uv_height);
            egui::Rect::from_min_max(egui::pos2(0.0, y_min), egui::pos2(1.0, y_max))
        } else {
            // The window is taller than the image -> crop the left and right sides
            let uv_width = screen_aspect / texture_aspect;
            let x_min = 0.5 * (1.0 - uv_width);
            let x_max = 0.5 * (1.0 + uv_width);
            egui::Rect::from_min_max(egui::pos2(x_min, 0.0), egui::pos2(x_max, 1.0))
        };

        ui.painter().image(
            self.background.id(),
            screen_rect,
            uv_rect,
            egui::Color32::WHITE.linear_multiply(1.),
        );

        // 4. CENTERED CONTAINER LAYOUT
        // Locks the UI width to a clean bounding box centered on screen
        let available_height = ui.available_height();

        ui.horizontal(|ui| {
            let total_width = ui.available_width();
            let container_width = total_width.min(1000.0);

            // Calculate the exact spacer needed on the left to keep us dead center
            let spacer_width = (total_width - container_width) / 2.0;
            ui.add_space(spacer_width);

            ui.allocate_ui_with_layout(
                egui::vec2(container_width, available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(200))
                    .outer_margin(24.)
                    .inner_margin(24.)
                    .corner_radius(12.)
                    .show(ui, |ui| {
                        // --- LAUNCH SECTION ---
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 0.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let response = ui.add(egui::Checkbox::new(
                                    &mut self.cfg.skip_eac,
                                    "Skip Easy Anti-Cheat (offline modes only)"
                                ));

                                if Some(Focus::SkipEasyAntiCheat) == self.focus {
                                    ui.painter().rect_stroke(
                                        response.rect.expand(2.0),
                                        4.0,
                                        egui::Stroke::new(2.0, egui::Color32::YELLOW),
                                        egui::StrokeKind::Outside,
                                    );
                                }

                                let gamepad_activated = Some(Focus::SkipEasyAntiCheat) == self.focus && std::mem::take(&mut self.select_pressed);
                                if gamepad_activated {
                                    self.cfg.skip_eac = !self.cfg.skip_eac;
                                }
                                if response.clicked() || gamepad_activated {
                                    if let Err(e) = save_config(&self.cfg) {
                                        self.status = format!("{} (failed to save: {e})", self.status);
                                    }
                                }

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let launch_label = if self.busy { "Working..." } else { "🚀 LAUNCH ROCKET LEAGUE" };
                                    let enabled = self.logged_in && !self.busy;
                                    let launch_btn = egui::Button::new(egui::RichText::new(launch_label).size(18.0).strong())
                                        .fill(egui::Color32::from_rgb(0, 102, 204));

                                    let clicked = self.gamepad_button(ui, enabled, Focus::Launch, launch_btn);

                                    if clicked {
                                        if self.cfg.rocket_league_path.trim().is_empty() {
                                            self.status = "Set the Rocket League executable path first.".to_string();
                                        } else {
                                            self.busy = true;
                                            self.status = "Authenticating with Epic Games...".to_string();

                                            let cfg = self.cfg.clone();
                                            let tx = self.tx.clone();
                                            let ctx = ctx.clone();

                                            self.rt.spawn(async move {
                                                let res = do_launch(cfg).await;
                                                let _ = tx.send(AppMsg::LaunchFinished(res));
                                                ctx.request_repaint();
                                            });
                                        }
                                    }
                                });
                            }
                        );

                        ui.separator();

                        ui.label(egui::RichText::new(&self.status).size(14.0).italics());
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            // --- SETTINGS SECTION ---
                            ui.heading(egui::RichText::new("Launcher Settings").size(24.0).strong());
                            ui.add_space(4.0);

                            egui::Grid::new("settings_grid")
                                .num_columns(2)
                                .spacing([20.0, 14.0])
                                .min_row_height(32.0)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Rocket League Executable:").size(15.0));
                                    ui.add(egui::TextEdit::singleline(&mut self.cfg.rocket_league_path).desired_width(f32::INFINITY));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Proton Binary Path:").size(15.0));
                                    ui.add(egui::TextEdit::singleline(&mut self.cfg.proton_path).desired_width(f32::INFINITY));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Compat Data Prefix:").size(15.0));
                                    ui.add(egui::TextEdit::singleline(&mut self.cfg.compat_data_path).desired_width(f32::INFINITY));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Steam Install Path:").size(15.0));
                                    ui.add(egui::TextEdit::singleline(&mut self.cfg.steam_install_path).desired_width(f32::INFINITY));
                                    ui.end_row();
                                });

                            ui.horizontal(|ui| {
                                let clicked = self.gamepad_button(ui, true, Focus::AutoDetect, egui::Button::new("🔄 Auto-detect paths"));
                                if clicked {
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
                                        format!("Auto-detected: {}. Review and Save settings.", notes.join(", "))
                                    };

                                    if let Err(e) = save_config(&self.cfg) {
                                        self.status = format!("{} (failed to save: {e})", self.status);
                                    }
                                }

                                let clicked = self.gamepad_button(ui, true, Focus::SaveSettings, egui::Button::new("💾 Save settings"));
                                if clicked {
                                    match save_config(&self.cfg) {
                                        Ok(()) => self.status = "Settings saved successfully.".to_string(),
                                        Err(e) => self.status = format!("Failed to save settings: {e}"),
                                    }
                                }
                            });

                            ui.separator();

                            // --- LOGIN SECTION ---
                            ui.heading(egui::RichText::new("Epic Games Account").size(20.0).strong());
                            if self.logged_in {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("✅ Authenticated").color(egui::Color32::LIGHT_GREEN));
                                    let clicked = self.gamepad_button(ui, true, Focus::SwitchAccount, egui::Button::new("Switch Account"));
                                    if clicked {
                                        open_browser(EPIC_LOGIN_URL);
                                        self.status = "Browser opened. Log in, then paste the authorization code below.".to_string();
                                        self.logged_in = false;
                                        self.focus = Some(Focus::CodeField);
                                    }
                                });
                            } else {
                                let clicked = self.gamepad_button(ui, true, Focus::OpenLogin, egui::Button::new("🔑 Open Epic Login Page"));
                                if clicked {
                                    open_browser(EPIC_LOGIN_URL);
                                    self.status = "Browser opened. Log in, then paste the authorization code below.".to_string();
                                }

                                ui.horizontal(|ui| {
                                    let text_edit = egui::TextEdit::singleline(&mut self.auth_code_input)
                                        .hint_text("Paste 32-character authorization code here")
                                        .desired_width(ui.available_width() - 120.0);

                                    let response = ui.add(text_edit);

                                    let focused = self.focus == Some(Focus::CodeField);

                                    if focused {
                                        ui.painter().rect_stroke(
                                            response.rect.expand(2.0),
                                            4.0,
                                            egui::Stroke::new(2.0, egui::Color32::YELLOW),
                                            egui::StrokeKind::Outside,
                                        );
                                    }

                                    let activated = self.gamepad_connected && response.clicked()
                                        || (focused && std::mem::take(&mut self.select_pressed));

                                    if activated {
                                        self.ctx
                                            .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                                    }

                                    if focused && !response.has_focus() {
                                        for event in ui.input(|i| i.events.clone()) {
                                            if let egui::Event::Paste(text) = event {
                                                self.auth_code_input = text.trim().to_owned();
                                            }
                                        }
                                    }

                                    let clicked = self.gamepad_button(ui, true, Focus::SubmitCode, egui::Button::new("Submit Code"));
                                    if clicked {
                                        let code = self.auth_code_input.trim().to_string();
                                        if code.len() != 32 {
                                            self.status = format!("Authorization code must be 32 characters (got {}).", code.len());
                                        } else {
                                            self.busy = true;
                                            self.status = "Exchanging token...".to_string();

                                            let tx = self.tx.clone();
                                            let ctx = ctx.clone();

                                            self.rt.spawn(async move {
                                                let client = reqwest::Client::new();
                                                let res = exchange_code_for_refresh_token(&client, &code)
                                                    .await
                                                    .map_err(|e| e.to_string());
                                                let _ = tx.send(AppMsg::LoginFinished(res));
                                                ctx.request_repaint();
                                            });
                                        }
                                    }
                                });
                            }

                            ui.separator();

                            // --- UPDATE SECTION ---
                            ui.heading(egui::RichText::new("Game Updates").size(20.0).strong());
                            ui.horizontal(|ui| {
                                let check_label = if self.checking_update { "Checking..." } else { "Check for Updates" };
                                let enabled = !self.checking_update && !self.updating;
                                let clicked = self.gamepad_button(ui, enabled, Focus::CheckUpdates, egui::Button::new(check_label));
                                if clicked {
                                    self.checking_update = true;
                                    self.status = "Checking for Rocket League updates via Legendary...".to_string();

                                    let tx = self.tx.clone();
                                    let ctx = ctx.clone();

                                    self.rt.spawn_blocking(move || {
                                        let res = updater::check_for_update()
                                            .map(|status| UpdateCheckResult {
                                                installed_version: status.installed_version,
                                                update_available: status.update_available,
                                            })
                                            .map_err(|e| e.to_string());

                                        let _ = tx.send(AppMsg::UpdateCheckFinished(res));
                                        ctx.request_repaint();
                                    });
                                }

                                let update_label = if self.updating { "Updating..." } else { "Update Now" };
                                let enabled = self.update_available && !self.updating;
                                let clicked = self.gamepad_button(ui, enabled, Focus::UpdateNow, egui::Button::new(update_label));
                                if clicked {
                                    self.updating = true;
                                    self.update_log.clear();
                                    self.status = "Updating Rocket League via Legendary...".to_string();

                                    let tx = self.tx.clone();
                                    let ctx = ctx.clone();

                                    std::thread::spawn(move || {
                                        let result = updater::update_rocket_league(|line| {
                                            let _ = tx.send(AppMsg::UpdateLogLine(line));
                                            ctx.request_repaint();
                                        });
                                        let _ = tx.send(AppMsg::UpdateFinished(result.map_err(|e| e.to_string())));
                                        ctx.request_repaint();
                                    });
                                }
                            });

                            if let Some(v) = &self.installed_version {
                                ui.label(format!("Installed version: {v}"));
                            }

                            if !self.update_log.is_empty() {
                                egui::ScrollArea::vertical().max_height(140.0).stick_to_bottom(true).show(ui, |ui| {
                                    let log_text = self.update_log.join("\n");
                                    ui.add(
                                        egui::TextEdit::multiline(&mut log_text.as_str())
                                            .desired_width(f32::INFINITY)
                                            .font(egui::TextStyle::Monospace)
                                            .interactive(false)
                                    );
                                });
                            }
                        });
                    });
                },
            );
        });
    }
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
