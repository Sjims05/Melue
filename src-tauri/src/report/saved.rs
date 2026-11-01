//! Reports kept until they're sent, and after (About lists them): `<app data>/reports/<id>.json`, with the screenshot
//! as `<id>.png` when one was kept. A report can be deleted (and goes from the share too); a sent one whose issue was
//! closed on GitHub goes by itself a week after Melue saw it closed. With a share, each goes to `.melue/reports/` there at the library sync
//! (sync/data.rs), and is sent there: by its own device when signed in to GitHub (after reading the share's word on it,
//! so a report another device sent isn't sent again), or, after a while, by any other device signed in; then marked
//! sent there, so every device knows. Without a share, this device sends its own when it can.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::share::Share;
use crate::guard::Locked;

/// The reports' folder on the share.
const SHARE_DIR: &str = ".melue/reports";
/// Waiting reports are tried this long after Melue starts (then at each library sync).
const FIRST_TRY: Duration = Duration::from_secs(45);
/// A sent report whose issue was closed on GitHub: deleted this long after Melue saw it closed.
const CLOSED_KEPT_MS: i64 = 7 * 24 * 3_600_000;
/// Reports deleted here, to go from the share at the next library sync (one id a line).
const DELETED_FILE: &str = "deleted.txt";
/// Another device's waiting report is sent from here only this long after it was made: its own device goes first (two
/// devices sending the same report at once would make two issues).
const OTHERS_AFTER_MS: i64 = 10 * 60_000;

/// A report as made (its text already without personal things), and whether it's been sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedReport {
    pub id: String,
    pub made_ms: i64,
    pub title: String,
    pub body: String,
    /// The issue, once sent.
    #[serde(default)]
    pub sent_url: Option<String>,
    #[serde(default)]
    pub sent_ms: Option<i64>,
    /// A screenshot was kept with it (`<id>.png`).
    #[serde(default)]
    pub screenshot: bool,
    /// Sent, and its issue closed on GitHub: when Melue saw that (it goes a week after).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_ms: Option<i64>,
}

/// Only one round of sending at a time (a sync and Send later may both start one).
static SENDING: Mutex<()> = Mutex::new(());

pub fn dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("reports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// This device's reports, newest first.
fn read_all(app: &AppHandle) -> Result<Vec<SavedReport>, String> {
    let mut all: Vec<SavedReport> = std::fs::read_dir(dir(app)?)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    all.sort_by(|a, b| b.made_ms.cmp(&a.made_ms));
    Ok(all)
}

fn write(app: &AppHandle, report: &SavedReport) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    std::fs::write(dir(app)?.join(format!("{}.json", report.id)), json).map_err(|e| e.to_string())
}

/// An id that's safe as a file name (the app makes them; anything else is refused).
fn valid_id(id: &str) -> Result<(), String> {
    if !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Ok(())
    } else {
        Err("not a report's id".into())
    }
}

/// Sends this device's waiting reports, when it's signed in (stops at the first that can't go: offline). With a share,
/// at the library sync instead, soon (it knows what another device has sent already).
pub fn send_waiting(app: &AppHandle) {
    if app.state::<crate::library::Library>().sync_available().unwrap_or(false) {
        return crate::sync::data::soon(app);
    }
    let Ok(_one) = SENDING.try_lock() else { return };
    if !crate::github::signed_in(app) {
        return;
    }
    for mut report in read_all(app).unwrap_or_default().into_iter().filter(|r| r.sent_url.is_none()) {
        match crate::github::create_issue(app, &report.title, &report.body) {
            Ok(url) => {
                report.sent_url = Some(url);
                report.sent_ms = Some(now_ms());
                if let Err(e) = write(app, &report) {
                    eprintln!("melue: report sent, but can't mark it: {e}");
                }
            }
            Err(e) => {
                eprintln!("melue: can't send a waiting report yet: {e}");
                return;
            }
        }
    }
}

/// The reports deleted here that still have to go from the share.
fn deleted(app: &AppHandle) -> Vec<String> {
    let Ok(dir) = dir(app) else { return Vec::new() };
    std::fs::read_to_string(dir.join(DELETED_FILE)).unwrap_or_default().lines().filter(|id| valid_id(id).is_ok()).map(String::from).collect()
}

/// Report `id` deleted here (its screenshot too); with a share, noted to go from there at the next library sync.
fn delete_here(app: &AppHandle, id: &str) -> Result<(), String> {
    let dir = dir(app)?;
    let _ = std::fs::remove_file(dir.join(format!("{id}.json")));
    let _ = std::fs::remove_file(dir.join(format!("{id}.png")));
    if app.state::<crate::library::Library>().sync_available().unwrap_or(false) {
        let mut gone = deleted(app);
        if !gone.iter().any(|g| g == id) {
            gone.push(id.to_string());
        }
        std::fs::write(dir.join(DELETED_FILE), gone.join("\n")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// At the library sync (sync/data.rs): this device's reports with the share (the share's word on them first, then
/// the waiting ones sent from here when signed in, then to the share), and other devices' reports still waiting after a
/// while sent from here.
pub fn sync(app: &AppHandle, share: &dyn Share) -> Result<(), String> {
    let _one = SENDING.locked();
    // Deleted here: gone from the share too (so no other device sends it).
    let gone = deleted(app);
    if !gone.is_empty() {
        for id in &gone {
            for path in [format!("{SHARE_DIR}/{id}.json"), format!("{SHARE_DIR}/{id}.png")] {
                if share.stat(&path)?.is_some() {
                    share.delete(&path)?;
                }
            }
        }
        let _ = std::fs::remove_file(dir(app)?.join(DELETED_FILE));
    }
    // Once one can't go (offline), the rest wait for the next sync.
    let mut can_send = crate::github::signed_in(app);
    for mut report in read_all(app)? {
        let path = format!("{SHARE_DIR}/{}.json", report.id);
        let there: Option<SavedReport> = match share.stat(&path)? {
            Some(_) => serde_json::from_slice(&share.read(&path)?).ok(),
            None => None,
        };
        // Sent by another device: so it says here too (and isn't sent again).
        if let Some(sent) = there.as_ref().filter(|t| t.sent_url.is_some() && report.sent_url.is_none()) {
            report.sent_url = sent.sent_url.clone();
            report.sent_ms = sent.sent_ms;
            write(app, &report)?;
        }
        if report.sent_url.is_none() && can_send {
            match crate::github::create_issue(app, &report.title, &report.body) {
                Ok(url) => {
                    report.sent_url = Some(url);
                    report.sent_ms = Some(now_ms());
                    write(app, &report)?;
                }
                Err(e) => {
                    eprintln!("melue: can't send a waiting report yet: {e}");
                    can_send = false;
                }
            }
        }
        if there.as_ref() != Some(&report) {
            share.create_dir_all(SHARE_DIR)?;
            share.write(&path, &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)?;
        }
        // The screenshot: kept on the share, then not on the phone (it's only for looking at later).
        let png = dir(app)?.join(format!("{}.png", report.id));
        if png.exists() {
            let to = format!("{SHARE_DIR}/{}.png", report.id);
            if share.stat(&to)?.is_none() {
                share.write(&to, &std::fs::read(&png).map_err(|e| e.to_string())?)?;
            }
            let _ = std::fs::remove_file(&png);
        }
    }
    // Other devices' reports still waiting after a while (their own device goes first): sent from here, when signed in.
    if !can_send {
        return Ok(());
    }
    let mine: Vec<String> = read_all(app)?.into_iter().map(|r| r.id).collect();
    for entry in share.list(SHARE_DIR).unwrap_or_default() {
        let Some(id) = entry.name.strip_suffix(".json") else { continue };
        if mine.iter().any(|m| m == id) {
            continue;
        }
        let Ok(mut report) = serde_json::from_slice::<SavedReport>(&share.read(&entry.path)?) else { continue };
        if report.sent_url.is_some() || now_ms() - report.made_ms < OTHERS_AFTER_MS {
            continue;
        }
        match crate::github::create_issue(app, &report.title, &report.body) {
            Ok(url) => {
                report.sent_url = Some(url);
                report.sent_ms = Some(now_ms());
                share.write(&entry.path, &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)?;
            }
            Err(e) => {
                eprintln!("melue: can't send a waiting report yet: {e}");
                break;
            }
        }
    }
    Ok(())
}

/// Tries the waiting reports a while after Melue starts (setup).
pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_TRY);
        send_waiting(&app);
    });
}

/// Keeps a report (sent already: `sentUrl` set; else to send when it can), then tries sending what's waiting.
#[tauri::command(async)]
pub fn report_save(app: AppHandle, mut report: SavedReport) -> Result<(), String> {
    valid_id(&report.id)?;
    report.screenshot = dir(&app)?.join(format!("{}.png", report.id)).exists();
    if report.sent_url.is_some() && report.sent_ms.is_none() {
        report.sent_ms = Some(now_ms());
    }
    write(&app, &report)?;
    let waiting = report.sent_url.is_none();
    std::thread::spawn(move || {
        if waiting {
            send_waiting(&app);
        }
        // To the share at the next library sync, soon.
        crate::sync::data::soon(&app);
    });
    Ok(())
}

/// A report given up before it was kept: its screenshot goes.
#[tauri::command(async)]
pub fn report_discard(app: AppHandle, id: String) -> Result<(), String> {
    valid_id(&id)?;
    let dir = dir(&app)?;
    if !dir.join(format!("{id}.json")).exists() {
        let _ = std::fs::remove_file(dir.join(format!("{id}.png")));
    }
    Ok(())
}

/// This device's reports (About): newest first.
#[tauri::command(async)]
pub fn reports_list(app: AppHandle) -> Result<Vec<SavedReport>, String> {
    Ok(read_all(&app)?.into_iter().map(|r| SavedReport { body: String::new(), ..r }).collect())
}

/// Deletes report `id` here, and from the share (at the library sync, started now): it isn't sent (if it was, its issue
/// on GitHub stays).
#[tauri::command(async)]
pub fn report_delete(app: AppHandle, id: String) -> Result<(), String> {
    valid_id(&id)?;
    delete_here(&app, &id)?;
    if app.state::<crate::library::Library>().sync_available().unwrap_or(false) {
        crate::sync::data::soon(&app);
    }
    Ok(())
}

/// The sent reports' issues looked at on GitHub (About, as it shows them): which are closed; one seen closed a week
/// ago goes. Then the list again. Offline: as they were.
#[tauri::command(async)]
pub fn reports_check(app: AppHandle) -> Result<Vec<SavedReport>, String> {
    let now = now_ms();
    for mut report in read_all(&app)? {
        let Some(number) = report.sent_url.as_deref().and_then(issue_number) else { continue };
        let closed = match crate::github::issue_closed(&app, number) {
            Ok(closed) => closed,
            Err(e) => {
                eprintln!("melue: can't look at a report's issue: {e}");
                break;
            }
        };
        let closed_ms = if closed { Some(report.closed_ms.unwrap_or(now)) } else { None };
        if closed_ms.is_some_and(|at| now - at >= CLOSED_KEPT_MS) {
            delete_here(&app, &report.id)?;
        } else if closed_ms != report.closed_ms {
            report.closed_ms = closed_ms;
            write(&app, &report)?;
        }
    }
    reports_list(app)
}

/// An issue's number, from its address (".../issues/12").
fn issue_number(url: &str) -> Option<u64> {
    url.trim_end_matches('/').rsplit('/').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_issues_number_from_its_address() {
        assert_eq!(issue_number("https://github.com/Sjims05/Melue/issues/12"), Some(12));
        assert_eq!(issue_number("https://github.com/Sjims05/Melue/issues/3/"), Some(3));
        assert_eq!(issue_number("https://github.com/Sjims05/Melue"), None);
    }

    #[test]
    fn ids_are_plain() {
        assert!(valid_id("k2v9-3f").is_ok());
        assert!(valid_id("../x").is_err());
        assert!(valid_id("").is_err());
    }
}
