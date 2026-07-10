mod app;
mod types;
mod view;
mod worker;

use app::App;

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
