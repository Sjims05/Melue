//! Updating Melue on a phone (computers use Tauri's updater: src/lib/updates.ts): the newest published release on
//! GitHub (its public API: no sign-in; drafts and pre-releases aren't "latest") and its APK, downloaded into the app's
//! cache and handed to Android's installer, which installs it only when it's signed like the installed app (Melue's
//! Android plugin: installApk).

use std::io::{Read, Write};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

const LATEST: &str = "https://api.github.com/repos/Sjims05/Melue/releases/latest";
/// The download's progress, for the app: `{ done, total }` (bytes).
pub const PROGRESS_EVENT: &str = "update-download";

/// The newest release with an APK.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// "0.6.0" (its tag without the v).
    pub version: String,
    pub notes: String,
    /// The APK's address and size (bytes).
    pub url: String,
    pub size: u64,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(concat!("Melue/", env!("CARGO_PKG_VERSION")))
        .http_status_as_error(false)
        .build()
        .into()
}

/// The newest published release and its APK (None: none published yet, or none with an APK).
#[tauri::command(async)]
pub fn update_latest_release() -> Result<Option<Release>, String> {
    let mut answer = agent(Duration::from_secs(20))
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    match answer.status().as_u16() {
        200 => {}
        404 => return Ok(None),
        status => return Err(format!("GitHub answered {status}")),
    }
    let release: GitHubRelease = answer.body_mut().read_json().map_err(|e| e.to_string())?;
    let Some(apk) = release.assets.into_iter().find(|a| a.name.to_lowercase().ends_with(".apk")) else { return Ok(None) };
    Ok(Some(Release {
        version: release.tag_name.trim_start_matches('v').to_string(),
        notes: release.body.unwrap_or_default(),
        url: apk.browser_download_url,
        size: apk.size,
    }))
}

/// Downloads the APK at `url` into the app's cache (its progress as `update-download` events), then hands it to
/// Android's installer. Err("allow"): the user has to allow Melue to install apps first (Android's screen for it is
/// open); then this again.
#[tauri::command(async)]
pub fn update_install_apk(app: AppHandle, url: String, size: u64) -> Result<(), String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("melue-update.apk");
    let mut answer = agent(Duration::from_secs(15 * 60)).get(&url).call().map_err(|e| e.to_string())?;
    if answer.status() != 200 {
        return Err(format!("the download answered {}", answer.status()));
    }
    let mut body = answer.body_mut().as_reader();
    let mut out = std::fs::File::create(&file).map_err(|e| e.to_string())?;
    let (mut done, mut buf) = (0u64, vec![0u8; 64 * 1024]);
    let mut told = 0u64;
    loop {
        let n = body.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        // Every 1% or so.
        if done - told > size / 100 {
            told = done;
            let _ = app.emit(PROGRESS_EVENT, serde_json::json!({ "done": done, "total": size }));
        }
    }
    out.sync_all().map_err(|e| e.to_string())?;
    let _ = app.emit(PROGRESS_EVENT, serde_json::json!({ "done": done, "total": size }));
    install(&app, &file)
}

/// Hands the downloaded APK to Android's installer again (after allowing Melue to install apps).
#[tauri::command(async)]
pub fn update_install_downloaded(app: AppHandle) -> Result<(), String> {
    let file = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("update").join("melue-update.apk");
    install(&app, &file)
}

#[cfg(target_os = "android")]
fn install(app: &AppHandle, file: &std::path::Path) -> Result<(), String> {
    use tauri_plugin_melue_android::MelueAndroidExt;
    app.melue_android().install_apk(&file.to_string_lossy()).map_err(|e| if e.contains("allow") { "allow".to_string() } else { e })
}

#[cfg(not(target_os = "android"))]
fn install(_app: &AppHandle, _file: &std::path::Path) -> Result<(), String> {
    Err("installing an APK is for phones".into())
}
