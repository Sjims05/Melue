//! When Melue syncs by itself: at start-up, when the user comes back to the app (Android: opening it again;
//! desktop: back into the window) and the last sync is a while ago, when the share says its music changed
//! (SMB change notifications, settled for a moment), and optionally every N minutes. The rules are here, as
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
}

/// How long the share has to be quiet after a change notification before syncing (a copy of an album sends
/// one per song).
pub const SETTLE_MS: i64 = 5_000;

/// Whether `trigger` starts a sync now, `last_ms` being when the last sync ended (None: never).
pub fn should_sync(trigger: Trigger, now_ms: i64, last_ms: Option<i64>, settings: &SyncSettings) -> bool {
    let since = |minutes: u32| last_ms.is_none_or(|last| now_ms - last >= i64::from(minutes) * 60_000);
    match trigger {
        Trigger::Button => true,
        Trigger::Start => settings.at_start,
        Trigger::Return => settings.on_return_after_minutes.is_some_and(since),
        Trigger::Changed => settings.on_change,
        Trigger::Periodic => settings.every_minutes.is_some_and(since),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;

    #[test]
    fn the_default_rules() {
        let s = SyncSettings::default();
        assert!(should_sync(Trigger::Start, 0, None, &s));
        assert!(should_sync(Trigger::Button, 0, Some(0), &s));
        assert!(should_sync(Trigger::Changed, 0, Some(0), &s));
        // Coming back: only when the last sync is 15 minutes ago or more.
        assert!(!should_sync(Trigger::Return, 14 * MINUTE, Some(0), &s));
        assert!(should_sync(Trigger::Return, 15 * MINUTE, Some(0), &s));
        assert!(should_sync(Trigger::Return, 0, None, &s), "never synced");
        // No periodic check by default.
        assert!(!should_sync(Trigger::Periodic, 1000 * MINUTE, Some(0), &s));
    }

    #[test]
    fn the_rules_follow_the_settings() {
        let s = SyncSettings { at_start: false, on_return_after_minutes: None, on_change: false, every_minutes: Some(30), ..Default::default() };
        assert!(!should_sync(Trigger::Start, 0, None, &s));
        assert!(!should_sync(Trigger::Return, 1000 * MINUTE, Some(0), &s));
        assert!(!should_sync(Trigger::Changed, 0, None, &s));
        assert!(!should_sync(Trigger::Periodic, 29 * MINUTE, Some(0), &s));
        assert!(should_sync(Trigger::Periodic, 30 * MINUTE, Some(0), &s));
        assert!(should_sync(Trigger::Button, 0, Some(0), &s), "the button always syncs");
    }
}
