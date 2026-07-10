use iced::widget::{self, Button, button, container, text, text_input};
use iced::{Background, Border, Element, Theme};

use crate::app::App;
use crate::types::Message;
use rocket_launcher_core::gamepad::Focus;

impl App {
    /// Builds a button that is styled as "primary" when it currently has
    /// gamepad focus, and is disabled (no `on_press`) when `enabled` is false.
    pub(super) fn focused_button<'a>(
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

        if enabled {
            button = button.on_press(message);
        }

        if is_focused {
            button = button.style(widget::button::primary);
        }

        button
    }

    /// A labeled text input row used throughout the settings grid.
    pub(super) fn labeled_input<'a>(
        &'a self,
        label: &'a str,
        value: &'a str,
        on_change: impl Fn(String) -> Message + 'a,
    ) -> Element<'a, Message> {
        iced::widget::row![
            container(text(label).size(15)).width(iced::Length::Fixed(220.0)),
            text_input("", value)
                .on_input(on_change)
                .width(iced::Length::Fill),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center)
        .into()
    }

    /// A text input row that additionally highlights its border when the
    /// gamepad focus is on the theme selector (mirrors prior behavior).
    pub(super) fn code_input<'a>(&'a self) -> Element<'a, Message> {
        let code_focused = self.gamepad.focus == Some(Focus::CodeField);

        let mut input = text_input(
            "Paste 32-character authorization code here",
            &self.auth.auth_code_input,
        )
        .on_input(Message::AuthCodeChanged)
        .on_submit(Message::SubmitCodePressed)
        .width(iced::Length::Fill);

        if code_focused {
            input = input.style(move |theme: &Theme, status| {
                let mut style = text_input::default(theme, status);
                if self.gamepad.focus == Some(Focus::ThemeSelector) {
                    style.border = Border {
                        color: self.config.get_theme().palette().primary,
                        width: 2.0,
                        ..Default::default()
                    };
                }
                style
            });
        }

        input.into()
    }

    pub(super) fn eac_checkbox<'a>(&'a self) -> Element<'a, Message> {
        let focused = self.gamepad.focus == Some(Focus::SkipEasyAntiCheat);

        let mut checkbox = widget::checkbox(self.config.skip_eac)
            .label("Skip Easy Anti-Cheat (offline modes only)")
            .on_toggle(Message::SkipEacToggled);

        if focused {
            checkbox = checkbox.style(move |theme: &Theme, status| {
                let mut style = widget::checkbox::primary(theme, status);
                style.background = Background::Color(self.config.get_theme().palette().primary);
                style
            });
        }

        checkbox.into()
    }

    pub(super) fn panel_container<'a>(
        &'a self,
        body: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        use iced::theme::Base;

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
