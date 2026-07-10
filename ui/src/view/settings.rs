use iced::widget::{self, column, container, pick_list, row, text};
use iced::{Alignment, Element, Length, Theme};

use crate::app::App;
use crate::types::Message;
use rocket_launcher_core::gamepad::Focus;

impl App {
    /// Full "Launcher Settings" section: header/status line, and (conditionally)
    /// the path inputs, theme picker, and action buttons.
    pub(super) fn settings_grid(&self) -> Element<'_, Message> {
        let all_paths_set = self.config.is_valid();

        let mut grid = column![Self::header_row(all_paths_set)].spacing(14);

        if self.state.show_advanced_settings || !all_paths_set {
            grid = grid
                .push(self.path_inputs())
                .push(self.theme_and_actions_row());
        }

        if all_paths_set {
            grid = grid.push(self.toggle_advanced_button());
        }

        grid.into()
    }

    fn header_row<'a>(all_paths_set: bool) -> Element<'a, Message> {
        let status_indicator = if all_paths_set {
            text("System configured correctly.").color(iced::Color::from_rgb(0.0, 0.7, 0.0))
        } else {
            text("Paths missing. Auto-detect or configure manually.")
                .color(iced::Color::from_rgb(0.8, 0.0, 0.0))
        };

        row![
            text("Launcher Settings").size(24),
            widget::space::horizontal(),
            status_indicator,
        ]
        .spacing(14)
        .into()
    }

    fn path_inputs(&self) -> Element<'_, Message> {
        column![
            self.labeled_input(
                "Rocket League Executable:",
                &self.config.rocket_league_path,
                Message::RocketLeaguePathChanged,
            ),
            self.labeled_input(
                "Proton Binary Path:",
                &self.config.proton_path,
                Message::ProtonPathChanged,
            ),
            self.labeled_input(
                "Compat Data Prefix:",
                &self.config.compat_data_path,
                Message::CompatDataPathChanged,
            ),
            self.labeled_input(
                "Steam Install Path:",
                &self.config.steam_install_path,
                Message::SteamInstallPathChanged,
            ),
        ]
        .spacing(14)
        .into()
    }

    fn theme_and_actions_row(&self) -> Element<'_, Message> {
        column![
            row![
                container(text("Theme:").size(15)).width(Length::Fixed(220.0)),
                pick_list(
                    Theme::ALL,
                    Some(self.config.get_theme()),
                    Message::ThemeSelected
                )
                .width(Length::Fill),
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
        .spacing(14)
        .into()
    }

    fn toggle_advanced_button(&self) -> Element<'_, Message> {
        let label = if self.state.show_advanced_settings {
            "Hide Advanced Settings"
        } else {
            "Show Advanced Settings"
        };

        self.focused_button(
            label,
            Focus::ToggleAdvancedSettings,
            Message::ToggleAdvancedSettings,
            true,
        )
        .into()
    }
}
