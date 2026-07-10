use iced::widget::{self, column, row, text};
use iced::{Alignment, Element, Length};

use crate::app::App;
use crate::types::Message;
use rocket_launcher_core::gamepad::Focus;

impl App {
    /// "Epic Games Account" column: shows either the logged-in state or a
    /// button to open the login page.
    pub(super) fn account_column(&self) -> Element<'_, Message> {
        if self.auth.logged_in {
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
        .spacing(12)
        .into()
    }

    /// Auth-code entry row, only shown while logged out.
    pub(super) fn login_row(&self) -> Option<Element<'_, Message>> {
        (!self.auth.logged_in).then(|| {
            row![
                self.code_input(),
                widget::space().width(10),
                self.focused_button(
                    "Submit Code",
                    Focus::SubmitCode,
                    Message::SubmitCodePressed,
                    true,
                ),
            ]
            .into()
        })
    }

    pub(super) fn top_row(&self) -> Element<'_, Message> {
        row![
            self.launch_column(),
            self.account_column(),
            self.launch_actions_column(),
        ]
        .spacing(24)
        .width(Length::Fill)
        .into()
    }
}
