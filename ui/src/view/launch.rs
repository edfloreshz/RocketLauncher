use iced::widget::{self, column, row, scrollable, text};
use iced::{Element, Font, Length};

use crate::app::{App, UpdateState};
use crate::types::Message;
use rocket_launcher_core::gamepad::Focus;

impl App {
    /// "Rocket Launcher" title + update controls + optional update log.
    pub(super) fn launch_column(&self) -> Element<'_, Message> {
        let checking = self.updates.state == UpdateState::CheckingUpdate;
        let updating = self.updates.state == UpdateState::Updating;

        let base = column![
            text("Rocket Launcher").size(28),
            row![
                self.focused_button(
                    if checking {
                        "Checking..."
                    } else {
                        "Check for Updates"
                    },
                    Focus::CheckUpdates,
                    Message::CheckUpdatesPressed,
                    !checking && !updating,
                ),
                widget::space().width(10),
                self.focused_button(
                    if updating {
                        "Updating..."
                    } else {
                        "Update Now"
                    },
                    Focus::UpdateNow,
                    Message::UpdateNowPressed,
                    self.updates.state == UpdateState::UpdateAvailable && !updating,
                ),
            ],
            self.updates
                .installed_version
                .as_ref()
                .map(|v| text(format!("Installed version: {v}"))),
        ]
        .spacing(12);

        if self.updates.log.is_empty() {
            base.into()
        } else {
            let log_text = self.updates.log.join("\n");
            column![
                base,
                scrollable(text(log_text).font(Font::MONOSPACE))
                    .height(Length::Fixed(140.0))
                    .anchor_bottom(),
            ]
            .spacing(12)
            .into()
        }
    }

    /// Launch button + "Skip EAC" checkbox column.
    pub(super) fn launch_actions_column(&self) -> Element<'_, Message> {
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

        column![launch_button, self.eac_checkbox()]
            .spacing(12)
            .into()
    }

    /// Bottom status text + Exit button.
    pub(super) fn status_line(&self) -> Element<'_, Message> {
        row![
            text(&self.state.status).size(14),
            widget::space::horizontal(),
            self.focused_button("Exit", Focus::Exit, Message::ExitPressed, true),
        ]
        .into()
    }
}
