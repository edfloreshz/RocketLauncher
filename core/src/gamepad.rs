#[derive(Debug, Clone)]
pub enum GamepadAction {
    Up,
    Down,
    Left,
    Right,
    Select,
    LaunchShortcut,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
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
    ThemeSelector,
    Exit,
}

#[derive(Debug, Clone, Copy)]
pub struct FocusNode {
    pub focus: Focus,
    pub up: Option<Focus>,
    pub down: Option<Focus>,
    pub left: Option<Focus>,
    pub right: Option<Focus>,
    pub logged_in_only: bool,
    pub logged_out_only: bool,
}

impl Focus {
    const LOGGED_OUT_NODES: &'static [FocusNode] = &[
        FocusNode {
            focus: Focus::CheckUpdates,
            up: None,
            down: Some(Focus::AutoDetect),
            left: None,
            right: Some(Focus::UpdateNow),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::UpdateNow,
            up: None,
            down: Some(Focus::ThemeSelector),
            left: Some(Focus::CheckUpdates),
            right: Some(Focus::OpenLogin),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::OpenLogin,
            up: None,
            down: Some(Focus::CodeField),
            left: Some(Focus::UpdateNow),
            right: Some(Focus::SkipEasyAntiCheat),
            logged_in_only: false,
            logged_out_only: true,
        },
        FocusNode {
            focus: Focus::Launch,
            up: None,
            down: Some(Focus::SkipEasyAntiCheat),
            left: Some(Focus::OpenLogin),
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::SkipEasyAntiCheat,
            up: Some(Focus::Launch),
            down: Some(Focus::SubmitCode),
            left: Some(Focus::OpenLogin),
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::CodeField,
            up: Some(Focus::OpenLogin),
            down: Some(Focus::ThemeSelector),
            left: None,
            right: Some(Focus::SubmitCode),
            logged_in_only: false,
            logged_out_only: true,
        },
        FocusNode {
            focus: Focus::SubmitCode,
            up: Some(Focus::SkipEasyAntiCheat),
            down: Some(Focus::ThemeSelector),
            left: Some(Focus::CodeField),
            right: None,
            logged_in_only: false,
            logged_out_only: true,
        },
        FocusNode {
            focus: Focus::ThemeSelector,
            up: Some(Focus::SubmitCode),
            down: Some(Focus::SaveSettings),
            left: None,
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::AutoDetect,
            up: Some(Focus::CheckUpdates),
            down: None,
            left: None,
            right: Some(Focus::SaveSettings),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::SaveSettings,
            up: Some(Focus::ThemeSelector),
            down: None,
            left: Some(Focus::AutoDetect),
            right: Some(Focus::Exit),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::Exit,
            up: Some(Focus::ThemeSelector),
            down: None,
            left: Some(Focus::SaveSettings),
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
    ];

    const LOGGED_IN_NODES: &'static [FocusNode] = &[
        FocusNode {
            focus: Focus::CheckUpdates,
            up: None,
            down: Some(Focus::AutoDetect),
            left: None,
            right: Some(Focus::UpdateNow),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::UpdateNow,
            up: None,
            down: Some(Focus::ThemeSelector),
            left: Some(Focus::CheckUpdates),
            right: Some(Focus::SwitchAccount),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::SwitchAccount,
            up: None,
            down: Some(Focus::ThemeSelector),
            left: Some(Focus::UpdateNow),
            right: Some(Focus::SkipEasyAntiCheat),
            logged_in_only: true,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::Launch,
            up: None,
            down: Some(Focus::SkipEasyAntiCheat),
            left: Some(Focus::SwitchAccount),
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::SkipEasyAntiCheat,
            up: Some(Focus::Launch),
            down: Some(Focus::ThemeSelector),
            left: Some(Focus::SwitchAccount),
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::ThemeSelector,
            up: Some(Focus::SwitchAccount),
            down: Some(Focus::SaveSettings),
            left: None,
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::AutoDetect,
            up: Some(Focus::CheckUpdates),
            down: None,
            left: None,
            right: Some(Focus::SaveSettings),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::SaveSettings,
            up: Some(Focus::ThemeSelector),
            down: None,
            left: Some(Focus::AutoDetect),
            right: Some(Focus::Exit),
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::Exit,
            up: Some(Focus::ThemeSelector),
            down: None,
            left: Some(Focus::SaveSettings),
            right: None,
            logged_in_only: false,
            logged_out_only: false,
        },
    ];

    fn nodes(logged_in: bool) -> &'static [FocusNode] {
        if logged_in {
            Self::LOGGED_IN_NODES
        } else {
            Self::LOGGED_OUT_NODES
        }
    }

    fn node(focus: Focus, logged_in: bool) -> &'static FocusNode {
        Self::nodes(logged_in)
            .iter()
            .find(|n| n.focus == focus)
            .unwrap()
    }

    fn available(focus: Focus, logged_in: bool) -> bool {
        let node = Self::node(focus, logged_in);

        !(logged_in && node.logged_out_only || !logged_in && node.logged_in_only)
    }

    pub fn navigate(self, dir: Direction, logged_in: bool) -> Self {
        let mut next = match dir {
            Direction::Up => Self::node(self, logged_in).up,
            Direction::Down => Self::node(self, logged_in).down,
            Direction::Left => Self::node(self, logged_in).left,
            Direction::Right => Self::node(self, logged_in).right,
        };

        while let Some(focus) = next {
            if Self::available(focus, logged_in) {
                return focus;
            }

            next = match dir {
                Direction::Up => Self::node(focus, logged_in).up,
                Direction::Down => Self::node(focus, logged_in).down,
                Direction::Left => Self::node(focus, logged_in).left,
                Direction::Right => Self::node(focus, logged_in).right,
            };
        }

        self
    }
}
