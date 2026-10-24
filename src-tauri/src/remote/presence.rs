//! Which of your devices are online: each running Melue writes `.melue/devices/<device id>.json` on the share every
//! 20 s (its name, a computer or a phone, its version, whether others may play on it, and where it can be reached:
//! its network addresses and its server's port), and once more when it closes
//! (offline). A device whose file the share wrote in the last few minutes (the share's clock, the same for all) is
//! online. Settings, Across devices, lists them.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::Allow;
use crate::library::{now_ms, Library};
use crate::share::{Share, ShareError};
use crate::sync::handoff;

/// The devices' files on the share.
const DIR: &str = ".melue/devices";
/// How often a running device says it's there.
const HEARTBEAT: Duration = Duration::from_secs(20);
/// Online: its file written this recently (two heartbeats missed, a slow share). A phone can't say it's leaving when
/// it's swiped away: it shows offline after this.
pub const ONLINE_MS: i64 = 60_000;

/// A device's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub device_id: String,
    pub name: String,
    /// "computer" or "phone".
    pub kind: String,
    pub version: String,
    /// When it wrote this (its own clock).
    pub seen_ms: i64,
    /// False: it closed (said so on its way out).
    pub online: bool,
    /// "Let other devices play here".
    pub allow: Allow,
    /// Where its server can be reached: its IPv4 addresses (home network, VPN) and port.
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

/// A device as Across devices shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub online: bool,
    /// When it was last there, on this device's clock.
    pub last_seen_ms: i64,
    pub allow: Allow,
    pub addresses: Vec<String>,
    pub port: Option<u16>,
    /// This device.
    pub this: bool,
}

/// The devices, from their files and when the share wrote each (`files`), as seen by device `me` at `now_ms` (its
/// clock): this one first, then the online ones, then the newest.
pub fn devices(files: &[(Presence, i64)], me: &str, now_ms: i64) -> Vec<Device> {
    // How far the share's clock is ahead of this device's: from this device's own file.
    let ahead = files.iter().find(|(p, _)| p.device_id == me).map_or(0, |(p, written)| written - p.seen_ms);
    let mut all: Vec<Device> = files
        .iter()
        .map(|(p, written)| {
            let last_seen_ms = written - ahead;
            Device {
                id: p.device_id.clone(),
                name: p.name.clone(),
                kind: p.kind.clone(),
                online: p.online && now_ms - last_seen_ms <= ONLINE_MS,
                last_seen_ms,
                allow: p.allow,
                addresses: p.addresses.clone(),
                port: p.port,
                this: p.device_id == me,
            }
        })
        .collect();
    all.sort_by(|a, b| b.this.cmp(&a.this).then(b.online.cmp(&a.online)).then(b.last_seen_ms.cmp(&a.last_seen_ms)));
    all
}

/// This device's file now.
fn this_device(app: &AppHandle, online: bool) -> Result<Presence, String> {
    let library = app.state::<Library>();
    Ok(Presence {
        device_id: handoff::device_id(&library)?,
        name: handoff::settings(app)?.device_name.unwrap_or_default(),
        kind: if cfg!(mobile) { "phone" } else { "computer" }.into(),
        version: app.package_info().version.to_string(),
        seen_ms: now_ms(),
        online,
        allow: super::settings(app)?.allow,
        addresses: addresses(),
        port: super::server::port(),
    })
}

/// This device's IPv4 addresses on its networks (not loopback): where the others can reach it.
fn addresses() -> Vec<String> {
    let mut all: Vec<String> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_link_local() => Some(ip.to_string()),
            _ => None,
        })
        .collect();
    all.sort();
    all.dedup();
    all
}

fn write(app: &AppHandle, share: &dyn Share, online: bool) -> Result<(), String> {
    let me = this_device(app, online)?;
    share.create_dir_all(DIR)?;
    share.write(&format!("{DIR}/{}.json", me.device_id), &serde_json::to_vec_pretty(&me).map_err(|e| e.to_string())?)?;
    Ok(())
}

/// Every device's file, with when the share wrote it.
fn read_all(share: &dyn Share) -> Result<Vec<(Presence, i64)>, String> {
    let list = |dir: &str| match share.list(dir) {
        Ok(entries) => Ok(entries.into_iter().filter(|e| e.name.ends_with(".json")).collect::<Vec<_>>()),
        Err(ShareError::NotFound { .. }) => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    };
    let mut all: Vec<(Presence, i64)> =
        list(DIR)?.into_iter().filter_map(|e| Some((serde_json::from_slice::<Presence>(&share.read(&e.path).ok()?).ok()?, e.modified_ms))).collect();
    // A device from before devices said they're online (an older Melue, one removed since): known from its library
    // data, so it can be seen, and forgotten.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Data {
        device_id: String,
        device_name: String,
        #[serde(default)]
        mobile: Option<serde_json::Value>,
    }
    for entry in list(crate::sync::data::DIR)? {
        if all.iter().any(|(p, _)| entry.name == format!("{}.json", p.device_id)) {
            continue;
        }
        let Some(data) = share.read(&entry.path).ok().and_then(|bytes| serde_json::from_slice::<Data>(&bytes).ok()) else { continue };
        let kind = if data.mobile.is_some() { "phone" } else { "computer" };
        let presence = Presence {
            device_id: data.device_id,
            name: data.device_name,
            kind: kind.into(),
            version: String::new(),
            seen_ms: entry.modified_ms,
            online: false,
            allow: Allow::On,
            addresses: Vec::new(),
            port: None,
        };
        all.push((presence, entry.modified_ms));
    }
    Ok(all)
}

/// Forgets one of your devices (an old one, a Melue removed): its files on the share go (it said it's online, its
/// library data, its playback), so it isn't counted or offered anywhere. One still in use comes back as it syncs.
#[tauri::command(async)]
pub fn remote_forget_device(app: AppHandle, id: String) -> Result<(), String> {
    if id == handoff::device_id(&app.state::<Library>())? {
        return Err("that’s this device".into());
    }
    crate::sync::shared::with_share(&app, |share| {
        for dir in [DIR, crate::sync::data::DIR, handoff::DIR] {
            let path = format!("{dir}/{id}.json");
            if share.stat(&path).map_err(|e| e.to_string())?.is_some() {
                share.delete(&path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })?;
    // The others' data and the lists that count devices follow.
    crate::sync::data::soon(&app);
    Ok(())
}

/// Whether Melue may use the share now for this (a share set up; not "Nothing" on mobile data).
fn may_use_share(app: &AppHandle) -> bool {
    let library = app.state::<Library>();
    library.sync_available().unwrap_or(false)
        && !library.sync_settings().is_ok_and(|s| crate::sync::mobile::waits_for_wifi(app, &s))
}

/// Says this device is there now (and, `online` false, that it's leaving).
fn announce(app: &AppHandle, online: bool) {
    if !may_use_share(app) {
        return;
    }
    if let Err(e) = crate::sync::shared::with_share(app, |share| write(app, share, online)) {
        eprintln!("melue: devices: can't say this device is {}: {e}", if online { "online" } else { "leaving" });
    }
}

/// The heartbeat's wake-up: a change to tell the others now.
static SOON: Mutex<Option<Sender<()>>> = Mutex::new(None);

/// Tells the others now (a setting changed), not at the next heartbeat.
pub fn announce_soon() {
    if let Some(soon) = SOON.lock().unwrap().as_ref() {
        let _ = soon.send(());
    }
}

/// Melue is closing: the others hear it's gone (not a few minutes later).
pub fn leave(app: &AppHandle) {
    announce(app, false);
}

/// The heartbeat (setup): this device's file now, then every minute.
pub fn start(app: &AppHandle) {
    let (sender, wake) = mpsc::channel::<()>();
    *SOON.lock().unwrap() = Some(sender);
    let app = app.clone();
    std::thread::Builder::new()
        .name("melue-presence".into())
        .spawn(move || loop {
            announce(&app, true);
            match wake.recv_timeout(HEARTBEAT) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        })
        .expect("the presence thread");
}

/// Your devices: this one first, then the online ones.
pub fn all(app: &AppHandle) -> Result<Vec<Device>, String> {
    if !app.state::<Library>().sync_available()? {
        return Ok(Vec::new());
    }
    let me = handoff::device_id(&app.state::<Library>())?;
    let files = crate::sync::shared::with_share(app, read_all)?;
    Ok(devices(&files, &me, now_ms()))
}

/// One of your devices, by its id.
pub fn device(app: &AppHandle, id: &str) -> Result<Option<Device>, String> {
    Ok(all(app)?.into_iter().find(|d| d.id == id))
}

/// Your devices (Across devices).
#[tauri::command(async)]
pub fn remote_devices(app: AppHandle) -> Result<Vec<Device>, String> {
    all(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: &str, seen_ms: i64, online: bool) -> Presence {
        Presence {
            device_id: id.into(),
            name: id.into(),
            kind: "computer".into(),
            version: "0.6.1".into(),
            seen_ms,
            online,
            allow: Allow::On,
            addresses: vec!["192.168.1.10".into()],
            port: Some(47821),
        }
    }

    #[test]
    fn online_on_the_shares_clock() {
        // The share's clock is 5 s ahead of this device's (its own file says so).
        let files = [
            (file("me", 1_000_000, true), 1_005_000),
            // Written by the share half a minute ago: online.
            (file("phone", 0, true), 1_005_000 - 30_000),
            // Ten minutes ago: offline, though its file says online (it stopped without saying).
            (file("laptop", 0, true), 1_005_000 - 600_000),
            // Just now, but it said it's leaving.
            (file("old pc", 0, false), 1_005_000),
        ];
        let seen = devices(&files, "me", 1_000_000);
        let line: Vec<(&str, bool, bool)> = seen.iter().map(|d| (d.id.as_str(), d.online, d.this)).collect();
        assert_eq!(line, [("me", true, true), ("phone", true, false), ("old pc", false, false), ("laptop", false, false)]);
        assert_eq!(seen[1].last_seen_ms, 1_000_000 - 30_000, "on this device's clock");
    }
}
