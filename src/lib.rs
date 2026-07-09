// Core Epic Games OAuth + launch logic (Slipstream-style), shared between the
// CLI binary and the Iced GUI binary. This does the same thing Epic's own
// launcher does: authenticate via Epic's OAuth endpoints using Epic's
// *public* launcher client credentials, exchange the resulting access token
// for a one-time launch code, and start the game executable (optionally
// through Proton on Linux) with that code as a command-line argument.
//
// Organized as one file per concern:
//   config     - on-disk settings (Config struct, load/save)
//   auth       - Epic OAuth flow
//   launch     - starting the game (with optional Proton)
//   discovery  - best-effort auto-detection of install paths
//   updater    - shells out to Legendary for update checks/downloads

pub mod auth;
pub mod config;
pub mod discovery;
pub mod gamepad;
pub mod launch;
pub mod updater;

// Re-export the most commonly used items at the crate root so existing
// `use rl_launcher::{...}` call sites in the CLI/GUI binaries don't need to
// change to `rl_launcher::config::...` etc. everywhere.
pub use auth::{
    EPIC_API_URL, EPIC_LAUNCHER_AUTH, EPIC_LOGIN_URL, LaunchCredentials,
    exchange_code_for_refresh_token, get_launch_credentials, open_browser,
};
pub use config::{Config, config_path, load_config, save_config};
pub use launch::launch_game;
