//! Signing in to GitHub (Settings: Connections), so bug reports go to Sjims05/Melue as issues from the user's own
//! account. GitHub's device flow for Melue's GitHub App (Issues: read and write, installed on Melue only): Melue shows
//! a code, the user enters it at github.com/login/device and approves, and Melue gets a token for their account. The
//! token is kept in the system's secure store, and reaches only the issues of the repositories the app is installed
//! on. Signing in or out on one device does so on all of them (connections.rs). The client ID is public; there is no secret in the app.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::library::Library;
use crate::share::secrets::{Secrets, SystemSecrets};
use crate::guard::Locked;

/// Melue's GitHub App (public: it says which app is asking, not who).
const CLIENT_ID: &str = "Iv23linuFkI5J6T149A3";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USER_URL: &str = "https://api.github.com/user";
/// Where bug reports go.
const ISSUES_URL: &str = "https://api.github.com/repos/Sjims05/Melue/issues";
/// The token, in the system's secure store.
pub(crate) const TOKEN_KEY: &str = "github_token";
/// The account's name (this device's library setting), shown in Connections.
pub(crate) const LOGIN_SETTING: &str = "github_login";

/// What the user is shown while signing in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInCode {
    /// "363C-2D66", to enter at `url`.
    pub code: String,
    pub url: String,
    /// How long the code works (seconds).
    pub expires_in: u64,
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

/// GitHub's answer while waiting: a token, or why not yet ("authorization_pending", "slow_down", "expired_token",
/// "access_denied").
#[derive(Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    error: Option<String>,
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct User {
    login: String,
}

/// The sign-in under way: its device code, until when it works, how often to ask.
struct Pending {
    device_code: String,
    until: Instant,
    every: Duration,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
/// Counts sign-ins started or cancelled: a wait that sees it change stops.
static ATTEMPT: AtomicU64 = AtomicU64::new(0);

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("Melue/", env!("CARGO_PKG_VERSION")))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Starts signing in: the code for the user to enter on GitHub (then [`github_sign_in_wait`]).
#[tauri::command(async)]
pub fn github_sign_in_start() -> Result<SignInCode, String> {
    ATTEMPT.fetch_add(1, Ordering::SeqCst);
    let mut answer = agent()
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .send_form([("client_id", CLIENT_ID)])
        .map_err(|e| e.to_string())?;
    if answer.status() != 200 {
        return Err(format!("GitHub answered {}", answer.status()));
    }
    let code: DeviceCode = answer.body_mut().read_json().map_err(|e| e.to_string())?;
    *PENDING.locked() = Some(Pending {
        device_code: code.device_code,
        until: Instant::now() + Duration::from_secs(code.expires_in),
        every: Duration::from_secs(code.interval.max(5)),
    });
    Ok(SignInCode { code: code.user_code, url: code.verification_uri, expires_in: code.expires_in })
}

/// Waits for the user to approve on GitHub, then keeps the token and returns the account's name. Err: "expired"
/// (the code ran out), "denied" (the user said no), "cancelled" (another sign-in, or [`github_sign_in_cancel`]).
#[tauri::command(async)]
pub fn github_sign_in_wait(app: AppHandle) -> Result<String, String> {
    let attempt = ATTEMPT.load(Ordering::SeqCst);
    let Some(Pending { device_code, until, mut every }) = PENDING.locked().take() else {
        return Err("cancelled".into());
    };
    let token = loop {
        // Asked in short steps, so cancelling is noticed soon.
        let next = Instant::now() + every;
        while Instant::now() < next {
            if ATTEMPT.load(Ordering::SeqCst) != attempt {
                return Err("cancelled".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if Instant::now() > until {
            return Err("expired".into());
        }
        let answer: TokenAnswer = agent()
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .send_form([
                ("client_id", CLIENT_ID),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())?;
        if let Some(token) = answer.access_token {
            break token;
        }
        match answer.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => every = Duration::from_secs(answer.interval.unwrap_or(every.as_secs() + 5)),
            Some("expired_token") => return Err("expired".into()),
            Some("access_denied") => return Err("denied".into()),
            other => return Err(format!("GitHub answered {}", other.unwrap_or("nothing"))),
        }
    };
    let login = account(&token)?;
    SystemSecrets::new(&app).set(TOKEN_KEY, &token)?;
    app.state::<Library>().set_setting(LOGIN_SETTING, &login).map_err(|e| e.to_string())?;
    // Your other devices too.
    crate::connections::changed(&app, "github");
    Ok(login)
}

/// Stops a sign-in under way.
#[tauri::command]
pub fn github_sign_in_cancel() {
    ATTEMPT.fetch_add(1, Ordering::SeqCst);
    PENDING.locked().take();
}

/// The GitHub account this device is signed in to (None: not signed in).
#[tauri::command(async)]
pub fn github_account(app: AppHandle) -> Result<Option<String>, String> {
    let login = app.state::<Library>().setting(LOGIN_SETTING).map_err(|e| e.to_string())?;
    Ok(login.filter(|l| !l.is_empty()))
}

/// Signs out, on every device: the token is forgotten. (The app stays authorized on GitHub until the user revokes it
/// there: github.com/settings/apps/authorizations.)
#[tauri::command(async)]
pub fn github_sign_out(app: AppHandle) -> Result<(), String> {
    SystemSecrets::new(&app).delete(TOKEN_KEY)?;
    app.state::<Library>().set_setting(LOGIN_SETTING, "").map_err(|e| e.to_string())?;
    crate::connections::changed(&app, "github");
    Ok(())
}

#[derive(Deserialize)]
struct Issue {
    html_url: String,
}

/// Sends a bug report as an issue on Melue's GitHub, from the account signed in; its address. Err("signed out"): no
/// account here, or GitHub no longer takes its token (signed out: sign in again).
#[tauri::command(async)]
pub fn github_create_issue(app: AppHandle, title: String, body: String) -> Result<String, String> {
    create_issue(&app, &title, &body)
}

/// Whether this device can send reports (signed in).
pub fn signed_in(app: &AppHandle) -> bool {
    SystemSecrets::new(app).get(TOKEN_KEY).ok().flatten().is_some()
}

/// Sends an issue to Melue's GitHub from the account signed in here; its address (see [`github_create_issue`]).
pub fn create_issue(app: &AppHandle, title: &str, body: &str) -> Result<String, String> {
    let token = SystemSecrets::new(app).get(TOKEN_KEY)?.ok_or("signed out")?;
    let mut answer = agent()
        .post(ISSUES_URL)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(serde_json::json!({ "title": title, "body": body }))
        .map_err(|e| e.to_string())?;
    match answer.status().as_u16() {
        201 => Ok(answer.body_mut().read_json::<Issue>().map_err(|e| e.to_string())?.html_url),
        401 => {
            let _ = github_sign_out(app.clone());
            Err("signed out".into())
        }
        status => Err(format!("GitHub answered {status}: {}", answer.body_mut().read_to_string().unwrap_or_default())),
    }
}

/// The token's account name (GitHub's /user).
fn account(token: &str) -> Result<String, String> {
    let mut answer = agent()
        .get(USER_URL)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| e.to_string())?;
    if answer.status() != 200 {
        return Err(format!("GitHub answered {}", answer.status()));
    }
    Ok(answer.body_mut().read_json::<User>().map_err(|e| e.to_string())?.login)
}
