mod app;
mod types;
mod view;
mod worker;

use app::App;

pub fn main() -> iced::Result {
    let settings = iced::window::Settings {
        fullscreen: true,
        ..Default::default()
    };

    iced::application(App::new, App::update, App::view)
        .title("Rocket League Launcher")
        .subscription(App::subscription)
        .theme(|state: &App| state.config.get_theme().clone())
        .window(settings)
        .run()
}
