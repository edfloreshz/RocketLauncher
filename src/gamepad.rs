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
}

#[derive(Debug, Clone, Copy)]
struct FocusNode {
    focus: Focus,
    x: f32,
    y: f32,
    logged_in_only: bool,
    logged_out_only: bool,
}

impl Focus {
    const FOCUS_NODES: &[FocusNode] = &[
        // Top row
        FocusNode {
            focus: Focus::CheckUpdates,
            x: 0.0,
            y: 0.0,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::UpdateNow,
            x: 1.0,
            y: 0.0,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::OpenLogin,
            x: 3.0,
            y: 0.0,
            logged_in_only: false,
            logged_out_only: true,
        },
        FocusNode {
            focus: Focus::SwitchAccount,
            x: 3.0,
            y: 0.0,
            logged_in_only: true,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::Launch,
            x: 6.0,
            y: 0.0,
            logged_in_only: false,
            logged_out_only: false,
        },
        // checkbox
        FocusNode {
            focus: Focus::SkipEasyAntiCheat,
            x: 6.0,
            y: 1.0,
            logged_in_only: false,
            logged_out_only: false,
        },
        // login row
        FocusNode {
            focus: Focus::CodeField,
            x: 2.5,
            y: 2.0,
            logged_in_only: false,
            logged_out_only: true,
        },
        FocusNode {
            focus: Focus::SubmitCode,
            x: 7.0,
            y: 2.0,
            logged_in_only: false,
            logged_out_only: true,
        },
        // bottom buttons
        FocusNode {
            focus: Focus::AutoDetect,
            x: 0.0,
            y: 6.0,
            logged_in_only: false,
            logged_out_only: false,
        },
        FocusNode {
            focus: Focus::SaveSettings,
            x: 1.5,
            y: 6.0,
            logged_in_only: false,
            logged_out_only: false,
        },
    ];

    fn node(focus: Focus) -> &'static FocusNode {
        Self::FOCUS_NODES.iter().find(|n| n.focus == focus).unwrap()
    }

    pub fn navigate(self, dir: Direction, logged_in: bool) -> Self {
        let current = Self::node(self);

        let mut best = None;
        let mut best_score = f32::MAX;

        for candidate in Self::FOCUS_NODES {
            if candidate.focus == self {
                continue;
            }

            if logged_in && candidate.logged_out_only {
                continue;
            }

            if !logged_in && candidate.logged_in_only {
                continue;
            }

            let dx = candidate.x - current.x;
            let dy = candidate.y - current.y;

            let valid = match dir {
                Direction::Up => dy < 0.0,
                Direction::Down => dy > 0.0,
                Direction::Left => dx < 0.0,
                Direction::Right => dx > 0.0,
            };

            if !valid {
                continue;
            }

            // Penalize movement away from the requested direction.
            let score = match dir {
                Direction::Up | Direction::Down => dy.abs() + dx.abs() * 2.0,
                Direction::Left | Direction::Right => dx.abs() + dy.abs() * 2.0,
            };

            if score < best_score {
                best_score = score;
                best = Some(candidate.focus);
            }
        }

        best.unwrap_or(self)
    }
}
