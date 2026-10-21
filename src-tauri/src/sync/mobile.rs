//! Mobile data (phones): what syncs on a metered connection (mobile data, a phone's hotspot), and the song data
//! Melue copied on it this month. The Mobile data settings (SyncSettings):
//!
//! - Nothing (`sync_on_metered` off): syncs that start by themselves, the library data and synced playback wait for
//!   Wi-Fi. Sync now still syncs, without songs.
//! - Small things (`copy_on_metered` off): everything but songs; songs wait for Wi-Fi.
//! - Everything: songs too, within the month's limit (`mobile_limit_mb`, counted from the plan's billing day). At the
//!   limit songs wait, or (`mobile_ask_at_limit`) the sync card offers to copy anyway. A sync that starts by itself and
//!   would copy more than `mobile_ask_over_mb` copies nothing yet: the card offers it. Sync now never asks.
//!
//! The month's use is kept here, and goes to the share with the library data (data.rs), so a computer shows what the
//! phones used.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::SyncSettings;
use crate::library::{err, now_ms, Library, Result};

const USE_SETTING: &str = "mobile_use";
const PHONES_SETTING: &str = "mobile_use_phones";
const MB: u64 = 1_000_000;

/// Song data copied on metered connections in one month.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileUse {
    /// The month's first day ("2026-10-15").
    pub month: String,
    pub bytes: u64,
}

/// A device's use this month, for the settings ("Your phones, this month").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceUse {
    pub device: String,
    /// This device.
    pub here: bool,
    #[serde(flatten)]
    pub used: MobileUse,
}

/// Why a sync on mobile data didn't copy its songs yet, and how much they are: the sync card offers to copy them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileOffer {
    /// "big": more than "Ask before a sync over"; "limit": the month's limit reached.
    pub reason: &'static str,
    pub bytes: u64,
}

/// Whether the connection is metered (mobile data, a phone's hotspot). Desktop: no.
pub fn metered(app: &AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        app.melue_android().metered().unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        false
    }
}

/// On a metered connection with "Nothing": what syncs by itself waits for Wi-Fi.
pub fn waits_for_wifi(app: &AppHandle, settings: &SyncSettings) -> bool {
    !settings.sync_on_metered && metered(app)
}

/// How much a sync on a metered connection may copy now (`planned`: its songs' bytes; `asked`: the user said to copy
/// on mobile data; `by_hand`: Sync now), and what to offer when it copies less than planned.
pub fn copy_budget(library: &Library, settings: &SyncSettings, planned: u64, asked: bool, by_hand: bool) -> Result<(Option<u64>, Option<MobileOffer>)> {
    if !settings.sync_on_metered || !settings.copy_on_metered {
        return Ok((Some(0), None));
    }
    if asked || planned == 0 {
        return Ok((None, None));
    }
    let left = match settings.mobile_limit_mb {
        0 => None,
        limit => Some((u64::from(limit) * MB).saturating_sub(library.mobile_use(settings)?.bytes)),
    };
    if left == Some(0) {
        let offer = settings.mobile_ask_at_limit.then_some(MobileOffer { reason: "limit", bytes: planned });
        return Ok((Some(0), offer));
    }
    let ask_over = u64::from(settings.mobile_ask_over_mb) * MB;
    if !by_hand && ask_over > 0 && planned.min(left.unwrap_or(u64::MAX)) > ask_over {
        return Ok((Some(0), Some(MobileOffer { reason: "big", bytes: planned })));
    }
    Ok((left, None))
}

/// The first day of the month `today` is in, the month starting on `starts` (1 to 28): "2026-10-15".
pub fn month_of(today: (i64, u32, u32), starts: u32) -> String {
    let starts = starts.clamp(1, 28);
    let (mut year, mut month, day) = today;
    if day < starts {
        if month == 1 {
            year -= 1;
            month = 12;
        } else {
            month -= 1;
        }
    }
    format!("{year:04}-{month:02}-{starts:02}")
}

/// Today's date (UTC): (year, month, day).
fn today() -> (i64, u32, u32) {
    let days = now_ms().div_euclid(86_400_000);
    // Days since 1970 to a date (Howard Hinnant's civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

impl Library {
    /// This device's use this month (a new month starts at 0).
    pub fn mobile_use(&self, settings: &SyncSettings) -> Result<MobileUse> {
        let month = month_of(today(), settings.mobile_month_starts);
        let kept: Option<MobileUse> = self.setting(USE_SETTING)?.and_then(|json| serde_json::from_str(&json).ok());
        Ok(kept.filter(|u| u.month == month).unwrap_or(MobileUse { month, bytes: 0 }))
    }

    /// Song data copied on a metered connection.
    pub fn add_mobile_use(&self, settings: &SyncSettings, bytes: u64) -> Result<()> {
        if bytes == 0 {
            return Ok(());
        }
        let mut used = self.mobile_use(settings)?;
        used.bytes += bytes;
        self.set_setting(USE_SETTING, &serde_json::to_string(&used).map_err(err)?)
    }

    /// The other devices' use, as their library data said (data.rs).
    pub fn keep_phones_use(&self, phones: &[DeviceUse]) -> Result<()> {
        self.set_setting(PHONES_SETTING, &serde_json::to_string(phones).map_err(err)?)
    }

    /// Each device's use this month: this one's (a phone), and the other phones' (a month that's over: not shown).
    pub fn devices_mobile_use(&self, me: &str) -> Result<Vec<DeviceUse>> {
        let settings = self.sync_settings()?;
        let month = month_of(today(), settings.mobile_month_starts);
        let mut all: Vec<DeviceUse> = self.setting(PHONES_SETTING)?.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default();
        all.retain(|d| d.used.month == month);
        if cfg!(target_os = "android") {
            all.insert(0, DeviceUse { device: me.to_string(), here: true, used: self.mobile_use(&settings)? });
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_month_starts_on_the_billing_day() {
        assert_eq!(month_of((2026, 10, 22), 1), "2026-10-01");
        assert_eq!(month_of((2026, 10, 22), 15), "2026-10-15");
        assert_eq!(month_of((2026, 10, 14), 15), "2026-09-15", "before the day: last month's");
        assert_eq!(month_of((2026, 1, 3), 10), "2025-12-10", "across the new year");
        assert_eq!(month_of((2026, 3, 31), 31), "2026-03-28", "at most the 28th");
    }

    fn everything(limit_mb: u32, ask_over_mb: u32, ask_at_limit: bool) -> SyncSettings {
        SyncSettings {
            copy_on_metered: true,
            mobile_limit_mb: limit_mb,
            mobile_ask_over_mb: ask_over_mb,
            mobile_ask_at_limit: ask_at_limit,
            ..SyncSettings::default()
        }
    }

    #[test]
    fn what_a_sync_may_copy_on_mobile_data() {
        let t = crate::library::testkit::TestLibrary::new("mobile-budget");
        let l = &t.library;
        let small = SyncSettings::default();
        assert_eq!(copy_budget(l, &small, 5 * MB, false, true).unwrap(), (Some(0), None), "small things: songs wait");
        let nothing = SyncSettings { sync_on_metered: false, ..everything(0, 0, false) };
        assert_eq!(copy_budget(l, &nothing, 5 * MB, false, false).unwrap(), (Some(0), None));
        assert_eq!(copy_budget(l, &everything(0, 100, false), 50 * MB, false, false).unwrap(), (None, None), "no limit");
        // Big: a sync by itself asks; Sync now doesn't.
        let big = copy_budget(l, &everything(0, 100, false), 300 * MB, false, false).unwrap();
        assert_eq!(big, (Some(0), Some(MobileOffer { reason: "big", bytes: 300 * MB })));
        assert_eq!(copy_budget(l, &everything(0, 100, false), 300 * MB, false, true).unwrap(), (None, None));
        // The limit: what's left of it, then nothing (or the offer).
        let limited = everything(500, 0, false);
        l.add_mobile_use(&limited, 200 * MB).unwrap();
        assert_eq!(copy_budget(l, &limited, 400 * MB, false, false).unwrap(), (Some(300 * MB), None));
        l.add_mobile_use(&limited, 300 * MB).unwrap();
        assert_eq!(copy_budget(l, &limited, 400 * MB, false, true).unwrap(), (Some(0), None), "reached: songs wait");
        let asking = everything(500, 0, true);
        assert_eq!(copy_budget(l, &asking, 400 * MB, false, false).unwrap().1, Some(MobileOffer { reason: "limit", bytes: 400 * MB }));
        assert_eq!(copy_budget(l, &asking, 400 * MB, true, false).unwrap(), (None, None), "copied anyway when asked to");
    }
}
