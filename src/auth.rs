// Epic Games OAuth flow (Slipstream-style): authenticate via Epic's OAuth
// endpoints using Epic's *public* launcher client credentials, exchange the
// resulting access token for a one-time launch code.

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use std::process::Command;
use uuid::Uuid;

pub const EPIC_API_URL: &str = "https://account-public-service-prod.ak.epicgames.com/account/api";
// Epic's own public launcher client ID + secret (same one shipped in the real
// Epic Games Launcher binary and used by Legendary/Heroic/Slipstream). Not a
// private credential of ours.
pub const EPIC_LAUNCHER_AUTH: &str = "basic MzRhMDJjZjhmNDQxNGUyOWIxNTkyMTg3NmRhMzZmOWE6ZGFhZmJjY2M3Mzc3NDUwMzlkZmZlNTNkOTRmYzc2Y2Y=";
pub const EPIC_LOGIN_URL: &str = "https://www.epicgames.com/id/login?redirectUrl=https%3A//www.epicgames.com/id/api/redirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode";

#[derive(Debug, Deserialize)]
struct ApiResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    account_id: Option<String>,
    code: Option<String>,
    #[serde(rename = "errorCode")]
    error_code: Option<String>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
}

pub struct LaunchCredentials {
    pub exchange_code: String,
    pub account_id: String,
}

pub fn open_browser(url: &str) {
    let result = if cfg!(target_os = "linux") {
        Command::new("xdg-open").arg(url).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("cmd").args(["/C", "start", url]).spawn()
    };
    if let Err(e) = result {
        eprintln!("Could not open browser automatically: {e}");
    }
}

/// Generates a correlation ID in the same format Epic's own launcher sends.
fn correlation_id() -> String {
    format!("UE4-{}", Uuid::new_v4().to_string().to_uppercase())
}

async fn api_request(
    client: &reqwest::Client,
    method: reqwest::Method,
    path: &str,
    form: Option<&[(&str, &str)]>,
    auth_header: &str,
) -> Result<ApiResponse> {
    let url = format!("{EPIC_API_URL}{path}");
    let mut req = client
        .request(method, &url)
        .header("Authorization", auth_header)
        .header("X-Epic-Correlation-ID", correlation_id())
        .header(
            "User-Agent",
            "UELauncher/16.12.1-36115220+++Portal+Release-Live",
        );

    if let Some(form) = form {
        req = req.form(form);
    }

    let resp: ApiResponse = req
        .send()
        .await?
        .json()
        .await
        .context("failed to decode Epic API response")?;

    if let Some(err) = &resp.error_message {
        bail!(
            "Epic API error ({}): {}",
            resp.error_code.clone().unwrap_or_default(),
            err
        );
    }
    Ok(resp)
}

async fn exchange_auth_code(client: &reqwest::Client, code: &str) -> Result<ApiResponse> {
    api_request(
        client,
        reqwest::Method::POST,
        "/oauth/token",
        Some(&[("grant_type", "authorization_code"), ("code", code)]),
        EPIC_LAUNCHER_AUTH,
    )
    .await
}

async fn exchange_refresh_token(client: &reqwest::Client, token: &str) -> Result<ApiResponse> {
    api_request(
        client,
        reqwest::Method::POST,
        "/oauth/token",
        Some(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", token),
            ("token_type", "eg1"),
        ]),
        EPIC_LAUNCHER_AUTH,
    )
    .await
}

async fn get_exchange_code(client: &reqwest::Client, access_token: &str) -> Result<ApiResponse> {
    let auth = format!("bearer {access_token}");
    api_request(client, reqwest::Method::GET, "/oauth/exchange", None, &auth).await
}

/// Exchanges a pasted 32-character authorization code for a refresh token.
/// The GUI and CLI each handle *getting* that code differently (dialog vs.
/// stdin), so this just does the network part.
pub async fn exchange_code_for_refresh_token(
    client: &reqwest::Client,
    auth_code: &str,
) -> Result<String> {
    if auth_code.len() != 32 {
        bail!(
            "invalid authorization code: expected 32 characters, got {}",
            auth_code.len()
        );
    }
    let resp = exchange_auth_code(client, auth_code).await?;
    resp.refresh_token
        .ok_or_else(|| anyhow!("no refresh token returned from Epic"))
}

/// Runs the refresh-token half of the auth flow: given a stored refresh
/// token, gets fresh launch credentials. Returns an error if the token is
/// missing/expired — callers should fall back to their own login UI and then
/// call `exchange_code_for_refresh_token` followed by this function again.
pub async fn get_launch_credentials(
    client: &reqwest::Client,
    stored_refresh_token: &str,
) -> Result<(LaunchCredentials, String)> {
    let refresh_token = stored_refresh_token.trim();
    if refresh_token.is_empty() {
        bail!("no refresh token available; login required");
    }

    let token_resp = exchange_refresh_token(client, refresh_token).await?;

    let access_token = token_resp
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("no access token in response"))?;
    let account_id = token_resp
        .account_id
        .clone()
        .ok_or_else(|| anyhow!("no account_id in response"))?;
    let new_refresh_token = token_resp
        .refresh_token
        .clone()
        .unwrap_or_else(|| refresh_token.to_string());

    let exchange_resp = get_exchange_code(client, &access_token).await?;
    let exchange_code = exchange_resp
        .code
        .ok_or_else(|| anyhow!("no exchange code in response"))?;

    Ok((
        LaunchCredentials {
            exchange_code,
            account_id,
        },
        new_refresh_token,
    ))
}
