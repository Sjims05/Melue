//! When Melue syncs by itself: at start-up, when the user comes back to the app (Android: opening it again;
//! desktop: back into the window) after being away a while, when the share says its music changed
//! (SMB change notifications, settled for a moment), and optionally every N minutes. When the share can't be
//! reached, it tries again after 30 seconds, 1, 2 and 5 minutes, then every 5 minutes, and right away when
//! the network changes (Android: Wi-Fi to mobile data, the VPN coming back). The rules are here, as
//! plain functions; scheduler.rs runs them.

use serde::{Deserialize, Serialize};

use super::SyncSettings;

/// What could start a sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// Melue started.
    Start,
    /// The user came back to the app.
    Return,
    /// The share said its music changed.
    Changed,
    /// The periodic check's time.
    Periodic,
    /// The Sync now button.
    Button,
    /// Trying again: the share couldn't be reached.
    Retry,
    /// The network changed (Android): tried again now when the share couldn't be reached.
    Network,
}

/// How long to wait before trying again, after `attempts` tries that couldn't reach the share.
pub fn retry_after_ms(attempts: u32) -> i64 {
    match attempts {
        0 | 1 => 30_000,
        2 => 60_000,
        3 => 120_000,
        _ => 300_000,
    }
}

/// How long the network has to be quiet after a change before trying (a VPN reconnects a moment after Wi-Fi).
pub const NETWORK_SETTLE_MS: i64 = 3_000;

/// How long the share has to be quiet after a change notification before syncing (a copy of an album sends
/// one per song).
pub const SETTLE_MS: i64 = 5_000;

/// Whether `trigger` starts a sync now, `last_ms` being when the last sync ended (None: never), and `away_ms` how long
/// the user was away from the app before coming back (Return; None: not known, as at start-up).
pub fn should_sync(trigger: Trigger, now_ms: i64, last_ms: Option<i64>, away_ms: Option<i64>, settings: &SyncSettings) -> bool {
    let since = |minutes: u32| last_ms.is_none_or(|last| now_ms - last >= i64::from(minutes) * 60_000);
    match trigger {
        Trigger::Button => true,
        Trigger::Start => settings.at_start,
        // Away that long (the window in the background, the app closed on the phone), not the last sync that long ago.
        Trigger::Return => settings.on_return_after_minutes.is_some_and(|minutes| away_ms.is_some_and(|away| away >= i64::from(minutes) * 60_000)),
        Trigger::Changed => settings.on_change,
        Trigger::Periodic => settings.every_minutes.is_some_and(since),
        // Only sent while a retry is waiting (see scheduler.rs).
        Trigger::Retry | Trigger::Network => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;

    #[test]
    fn the_default_rules() {
        let s = SyncSettings::default();
        assert!(should_sync(Trigger::Start, 0, None, None, &s));
        assert!(should_sync(Trigger::Button, 0, Some(0), None, &s));
        assert!(should_sync(Trigger::Changed, 0, Some(0), None, &s));
        // Coming back: only after being away 15 minutes or more, however long ago the last sync is.
        assert!(!should_sync(Trigger::Return, 1000 * MINUTE, Some(0), Some(MINUTE), &s), "away a minute");
        assert!(!should_sync(Trigger::Return, 14 * MINUTE, Some(0), Some(14 * MINUTE), &s));
        assert!(should_sync(Trigger::Return, 15 * MINUTE, Some(14 * MINUTE), Some(15 * MINUTE), &s), "synced just before leaving");
        assert!(!should_sync(Trigger::Return, 1000 * MINUTE, None, None, &s), "not known to have been away");
        // No periodic check by default.
        assert!(!should_sync(Trigger::Periodic, 1000 * MINUTE, Some(0), None, &s));
    }

    #[test]
    fn trying_again_waits_longer_each_time() {
        let waits: Vec<i64> = (1..=6).map(retry_after_ms).collect();
        assert_eq!(waits, vec![30_000, MINUTE, 2 * MINUTE, 5 * MINUTE, 5 * MINUTE, 5 * MINUTE]);
    }

    #[test]
    fn the_rules_follow_the_settings() {
        let s = SyncSettings { at_start: false, on_return_after_minutes: None, on_change: false, every_minutes: Some(30), ..Default::default() };
        assert!(!should_sync(Trigger::Start, 0, None, None, &s));
        assert!(!should_sync(Trigger::Return, 1000 * MINUTE, Some(0), Some(1000 * MINUTE), &s));
        assert!(!should_sync(Trigger::Changed, 0, None, None, &s));
        assert!(!should_sync(Trigger::Periodic, 29 * MINUTE, Some(0), None, &s));
        assert!(should_sync(Trigger::Periodic, 30 * MINUTE, Some(0), None, &s));
        assert!(should_sync(Trigger::Button, 0, Some(0), None, &s), "the button always syncs");
    }
}
