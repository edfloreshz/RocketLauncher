use crate::types::Message;
use futures::SinkExt;
use gilrs::{Axis, Button as PadButton, Event as GilEvent, EventType, Gilrs};
use iced::stream;
use rocket_launcher_core::{
    Config, LaunchCredentials, gamepad::GamepadAction, get_launch_credentials, launch_game,
    save_config,
};

pub async fn do_launch(mut cfg: Config) -> Result<(), String> {
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

pub fn gamepad_worker() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(
        100,
        |mut output: futures::channel::mpsc::Sender<Message>| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

            std::thread::spawn(move || {
                if let Ok(mut gilrs) = Gilrs::new() {
                    for (_id, _gamepad) in gilrs.gamepads() {
                        let _ = tx.send(Message::GamepadConnected);
                    }

                    let mut left_stick_y_neutral = true;
                    let mut left_stick_x_neutral = true;

                    loop {
                        while let Some(GilEvent { event, .. }) = gilrs.next_event() {
                            match event {
                                EventType::Connected => {
                                    let _ = tx.send(Message::GamepadConnected);
                                }
                                EventType::Disconnected => {
                                    let _ = tx.send(Message::GamepadDisconnected);
                                }
                                EventType::ButtonPressed(button, _) => {
                                    let action = match button {
                                        PadButton::DPadUp => Some(GamepadAction::Up),
                                        PadButton::DPadDown => Some(GamepadAction::Down),
                                        PadButton::DPadLeft => Some(GamepadAction::Left),
                                        PadButton::DPadRight => Some(GamepadAction::Right),
                                        PadButton::South => Some(GamepadAction::Select),
                                        PadButton::Start => Some(GamepadAction::LaunchShortcut),
                                        _ => None,
                                    };
                                    if let Some(act) = action {
                                        let _ = tx.send(Message::Gamepad(act));
                                    }
                                }
                                EventType::AxisChanged(axis, value, _) => {
                                    const THRESHOLD: f32 = 0.5;
                                    if axis == Axis::LeftStickY {
                                        if value.abs() < 0.2 {
                                            left_stick_y_neutral = true;
                                        } else if left_stick_y_neutral && value.abs() > THRESHOLD {
                                            left_stick_y_neutral = false;
                                            let act = if value > 0.0 {
                                                GamepadAction::Up
                                            } else {
                                                GamepadAction::Down
                                            };
                                            let _ = tx.send(Message::Gamepad(act));
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
                                            let _ = tx.send(Message::Gamepad(act));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        gilrs.inc();
                        std::thread::sleep(std::time::Duration::from_millis(16));
                    }
                }
            });

            while let Some(msg) = rx.recv().await {
                if output.send(msg).await.is_err() {
                    break;
                }
            }
        },
    )
}
