use iced::Alignment;
use iced::widget::{column, container, image, scrollable, stack};
use iced::{Element, Length};

use crate::app::App;
use crate::types::Message;

mod account;
mod launch;
mod settings;
mod widgets;

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        let bg = image(self.state.background.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Cover);

        let centered = container(self.settings_panel())
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding(24);

        stack![bg, centered].into()
    }

    fn settings_panel(&self) -> Element<'_, Message> {
        let top_row = self.top_row();
        let login_row = self.login_row();
        let status_line = self.status_line();
        let settings_grid = scrollable(self.settings_grid());

        self.panel_container(
            column![top_row, login_row, settings_grid, status_line,]
                .spacing(16)
                .width(Length::Fixed(1000.0)),
        )
    }
}
