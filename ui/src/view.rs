use iced::Alignment;
use iced::theme::Base;
use iced::widget::{
    self, button, checkbox, column, container, image, pick_list, row, scrollable, stack, text,
    text_input,
};
use iced::{Background, Border, Element, Font, Length, Theme};
use rocket_launcher_core::gamepad::Focus;

use crate::app::App;
use crate::types::Message;

impl App {
    pub fn view(&self) -> Element<'_, Message> {
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
                widget::space().width(Length::Fill),
                self.focused_button("Exit Launcher", Focus::Exit, Message::ExitPressed, true),
            ],
        ]
        .spacing(14);

        let items = if self.logged_in {
            column![top_row, status_line, scrollable(settings_grid)]
        } else {
            column![top_row, login_row, status_line, scrollable(settings_grid)]
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
