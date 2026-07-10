use iced::Alignment;
use iced::theme::Base;
use iced::widget::{
    self, Button, button, checkbox, column, container, image, pick_list, row, scrollable, stack,
    text, text_input,
};
use iced::{Background, Border, Element, Font, Length, Theme};
use rocket_launcher_core::gamepad::Focus;

use crate::app::{App, UpdateState};
use crate::types::Message;

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        let bg = image(self.state.background.clone())
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
    ) -> Button<'a, Message> {
        let is_focused = self.gamepad.focus == Some(focus_variant);

        let mut button = button(text(label))
            .padding([10, 16])
            .style(widget::button::subtle);
        button = if enabled {
            button.on_press(message)
        } else {
            button
        };

        if is_focused {
            button = button.style(widget::button::primary);
        }

        button
    }

    fn settings_panel(&self) -> Element<'_, Message> {
        let launch_col = column![
            text("Rocket Launcher").size(28),
            row![
                self.focused_button(
                    if self.updates.state == UpdateState::CheckingUpdate {
                        "Checking..."
                    } else {
                        "Check for Updates"
                    },
                    Focus::CheckUpdates,
                    Message::CheckUpdatesPressed,
                    self.updates.state != UpdateState::CheckingUpdate
                        && self.updates.state != UpdateState::Updating,
                ),
                widget::space().width(10),
                self.focused_button(
                    if self.updates.state == UpdateState::Updating {
                        "Updating..."
                    } else {
                        "Update Now"
                    },
                    Focus::UpdateNow,
                    Message::UpdateNowPressed,
                    self.updates.state != UpdateState::UpdateAvailable
                        && self.updates.state != UpdateState::Updating,
                ),
            ],
            self.updates
                .installed_version
                .as_ref()
                .map(|v| text(format!("Installed version: {v}"))),
        ]
        .spacing(12);

        let launch_col: Element<Message> = if !self.updates.log.is_empty() {
            let log_text = self.updates.log.join("\n");
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

        let account_col = if self.auth.logged_in {
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
                    text("Authenticated").color(self.config.get_theme().palette().success),
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

        let launch_label = if self.state.busy {
            "Working..."
        } else {
            "LAUNCH ROCKET LEAGUE"
        };
        let launch_enabled = self.auth.logged_in && !self.state.busy;
        let launch_button = self.focused_button(
            launch_label,
            Focus::Launch,
            Message::LaunchPressed,
            launch_enabled,
        );

        let skip_eac_focused = self.gamepad.focus == Some(Focus::SkipEasyAntiCheat);
        let mut eac_checkbox = checkbox(self.config.skip_eac)
            .label("Skip Easy Anti-Cheat (offline modes only)")
            .on_toggle(Message::SkipEacToggled);
        if skip_eac_focused {
            eac_checkbox = eac_checkbox.style(move |theme: &Theme, status| {
                let mut style = checkbox::primary(theme, status);
                style.background = Background::Color(self.config.get_theme().palette().primary);
                style
            });
        }

        let launch_col2 = column![launch_button, eac_checkbox].spacing(12);

        let top_row = row![launch_col, account_col, launch_col2]
            .spacing(24)
            .width(Length::Fill);

        let login_row: Option<Element<Message>> = (!self.auth.logged_in).then(|| {
            let code_focused = self.gamepad.focus == Some(Focus::CodeField);
            let mut code_input = text_input(
                "Paste 32-character authorization code here",
                &self.auth.auth_code_input,
            )
            .on_input(Message::AuthCodeChanged)
            .on_submit(Message::SubmitCodePressed)
            .width(Length::Fill);

            if code_focused {
                code_input = code_input.style(move |theme: &Theme, status| {
                    let mut style = text_input::default(theme, status);
                    if self.gamepad.focus == Some(Focus::ThemeSelector) {
                        style.border = Border {
                            color: self.config.get_theme().palette().primary,
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
        });

        let status_line = row![
            text(&self.state.status).size(14),
            widget::space::horizontal(),
            self.focused_button("Exit", Focus::Exit, Message::ExitPressed, true),
        ];

        let all_paths_set = self.config.is_valid();

        // Status Indicator
        let status_indicator = if all_paths_set {
            text("System configured correctly.").color(iced::Color::from_rgb(0.0, 0.7, 0.0))
        } else {
            text("Paths missing. Auto-detect or configure manually.")
                .color(iced::Color::from_rgb(0.8, 0.0, 0.0))
        };

        let mut settings_grid = column![
            row![
                text("Launcher Settings").size(24),
                widget::space::horizontal(),
                status_indicator
            ]
            .spacing(14)
        ];

        if self.state.show_advanced_settings || !all_paths_set {
            settings_grid = settings_grid
                .spacing(14)
                .push(
                    column![
                        labeled_input(
                            "Rocket League Executable:",
                            &self.config.rocket_league_path,
                            Message::RocketLeaguePathChanged
                        ),
                        labeled_input(
                            "Proton Binary Path:",
                            &self.config.proton_path,
                            Message::ProtonPathChanged
                        ),
                        labeled_input(
                            "Compat Data Prefix:",
                            &self.config.compat_data_path,
                            Message::CompatDataPathChanged
                        ),
                        labeled_input(
                            "Steam Install Path:",
                            &self.config.steam_install_path,
                            Message::SteamInstallPathChanged
                        ),
                    ]
                    .spacing(14),
                )
                .push(
                    column![
                        row![
                            container(text("Theme:").size(15)).width(Length::Fixed(220.0)),
                            pick_list(
                                Theme::ALL,
                                Some(self.config.get_theme()),
                                Message::ThemeSelected
                            )
                            .width(Length::Fill)
                        ]
                        .spacing(20)
                        .align_y(Alignment::Center),
                        row![
                            self.focused_button(
                                "Auto-detect paths",
                                Focus::AutoDetect,
                                Message::AutoDetectPressed,
                                true
                            ),
                            widget::space().width(10),
                            self.focused_button(
                                "Save settings",
                                Focus::SaveSettings,
                                Message::SaveSettingsPressed,
                                true
                            ),
                        ]
                    ]
                    .spacing(14),
                );
        }

        if all_paths_set {
            let show_advanced_settings_label = if self.state.show_advanced_settings {
                "Hide Advanced Settings"
            } else {
                "Show Advanced Settings"
            };
            settings_grid = settings_grid.spacing(14).push(
                self.focused_button(
                    show_advanced_settings_label,
                    Focus::ToggleAdvancedSettings,
                    Message::ToggleAdvancedSettings,
                    true,
                )
                .on_press(Message::ToggleAdvancedSettings),
            );
        }

        let body = column![top_row, login_row, scrollable(settings_grid), status_line]
            .spacing(16)
            .width(Length::Fixed(1000.0));

        container(body)
            .padding(24)
            .style(|_theme| container::Style {
                background: Some(Background::Color(
                    self.config.get_theme().base().background_color,
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
