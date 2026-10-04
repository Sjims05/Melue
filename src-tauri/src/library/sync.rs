//! Syncing with a share (Phase 2). For now only whether it's available: a share is set up. The UI shows sync
//! buttons and options only then (the sync code is always there); setting up a share, or removing it, flips
//! it while the app runs (a `library-changed` with reason "sync").

use super::{Library, Result};

/// The share Melue syncs with (Phase 2's share setup fills it; empty or missing: none).
pub const SHARE_SETTING: &str = "sync_share";

impl Library {
    /// Whether sync is available: a share is set up.
    pub fn sync_available(&self) -> Result<bool> {
        Ok(self.setting(SHARE_SETTING)?.is_some_and(|share| !share.trim().is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testkit::TestLibrary;

    #[test]
    fn sync_is_available_once_a_share_is_set_up() {
        let t = TestLibrary::new("sync-available");
        assert!(!t.library.sync_available().unwrap(), "no share yet");
        t.library.set_setting(SHARE_SETTING, "  ").unwrap();
        assert!(!t.library.sync_available().unwrap(), "an empty one isn't one");
        t.library.set_setting(SHARE_SETTING, "{\"host\":\"nas\"}").unwrap();
        assert!(t.library.sync_available().unwrap());
        assert!(t.reopen().library.sync_available().unwrap(), "kept");
    }
}
