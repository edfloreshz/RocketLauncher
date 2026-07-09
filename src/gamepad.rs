#[derive(Debug, Clone)]
pub enum GamepadAction {
    Up,
    Down,
    Left,
    Right,
    Select,
    LaunchShortcut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    SkipEasyAntiCheat,
    AutoDetect,
    SaveSettings,
    OpenLogin,
    CodeField,
    SubmitCode,
    SwitchAccount,
    CheckUpdates,
    UpdateNow,
    Launch,
}

impl Focus {
    const LOGGED_OUT: [Focus; 9] = [
        Focus::AutoDetect,
        Focus::SaveSettings,
        Focus::OpenLogin,
        Focus::CodeField,
        Focus::SubmitCode,
        Focus::CheckUpdates,
        Focus::UpdateNow,
        Focus::SkipEasyAntiCheat,
        Focus::Launch,
    ];

    const LOGGED_IN: [Focus; 7] = [
        Focus::AutoDetect,
        Focus::SaveSettings,
        Focus::SwitchAccount,
        Focus::CheckUpdates,
        Focus::UpdateNow,
        Focus::SkipEasyAntiCheat,
        Focus::Launch,
    ];

    fn items(logged_in: bool) -> Vec<Focus> {
        if logged_in {
            Self::LOGGED_IN.to_vec()
        } else {
            Self::LOGGED_OUT.to_vec()
        }
    }

    pub fn next(self, logged_in: bool) -> Self {
        let set = Self::items(logged_in);
        match set.iter().position(|f| *f == self) {
            Some(i) => set[(i + 1) % set.len()],
            None => set[0],
        }
    }

    pub fn previous(self, logged_in: bool) -> Self {
        let set = Self::items(logged_in);
        match set.iter().position(|f| *f == self) {
            Some(i) => set[(i + set.len() - 1) % set.len()],
            None => set[0],
        }
    }
}
