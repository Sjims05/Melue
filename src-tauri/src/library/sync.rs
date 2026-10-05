//! Syncing with a share (Phase 2). Whether it's available: a share is set up (share/setup.rs saves it in the
//! SHARE_SETTING setting). The UI shows sync buttons and options only then (the sync code is always there);
//! setting up a share, or removing it, flips it while the app runs (a `library-changed` with reason "sync").

use super::{Library, Result};

/// The share Melue syncs with, as set up (see share/setup.rs; empty or missing: none).
pub const SHARE_SETTING: &str = "sync_share";

impl Library {
    /// Whether sync is available: a share is set up.
    pub fn sync_available(&self) -> Result<bool> {
        Ok(self.share_settings()?.is_some())
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
        for not_a_setup in ["", "  ", "spike-test-share", "{\"host\":\"nas\"}"] {
            t.library.set_setting(SHARE_SETTING, not_a_setup).unwrap();
            assert!(!t.library.sync_available().unwrap(), "{not_a_setup}");
        }
        t.library.set_setting(SHARE_SETTING, r#"{"kind":"drive","path":"Z:\\","stream":true,"syncFolderId":1}"#).unwrap();
        assert!(t.library.sync_available().unwrap());
        assert!(t.reopen().library.sync_available().unwrap(), "kept");
    }
}
