//! Setting up the share (Phase 2 step 2): where it is (saved in the library's settings, the `sync_share`
//! setting, which also makes sync available) and its password (in a [`PasswordStore`], never the database),
//! and the library folder the share syncs into.
//!
//! Two kinds: an SMB share Melue logs in to itself (every platform), or on desktop a network drive the
//! system has already connected (the system logs in, so there's no password). A drive can be copied from
//! like the SMB share, or played straight from: then the drive itself is the synced library folder.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::folder::FolderShare;
use super::secrets::{PasswordStore, Secrets};
use super::smb::{SmbConfig, SmbShare};
use super::{Share, ShareError};
use crate::library::sync::SHARE_SETTING;
use crate::library::{Folder, FolderKind, Library};

/// Where the share is (never its password).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ShareSetup {
    /// An SMB share Melue logs in to.
    #[serde(rename_all = "camelCase")]
    Smb { host: String, share: String, folder: String, user: String, password_store: PasswordStore },
    /// A network drive the system has connected (desktop: `Z:\`, `\\nas\music`). `stream`: played straight
    /// from the drive instead of copied.
    #[serde(rename_all = "camelCase")]
    Drive { path: String, stream: bool },
}

/// The share as set up: saved in the library's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareSettings {
    #[serde(flatten)]
    pub setup: ShareSetup,
    /// The library folder the share syncs into (for a streamed drive: the drive itself).
    pub sync_folder_id: i64,
}

/// Where the synced music goes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFolderChoice {
    pub kind: FolderKind,
    pub location: String,
}

/// Why setting up (or using) the share didn't work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SetupError {
    /// The share couldn't be used: can't reach it, wrong password, folder not found...
    Share { error: ShareError },
    /// This system has no secure store for the password: ask for it each time, or keep it in a plain file?
    NoSecureStore { detail: String },
    /// The password is asked each time and hasn't been given since Melue started.
    #[cfg_attr(not(test), allow(dead_code))] // from current(), which the sync uses (step 3 on)
    NeedPassword,
    /// No share is set up.
    NotSetUp,
    Other { detail: String },
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Share { error } => write!(f, "{error}"),
            Self::NoSecureStore { detail } => write!(f, "this system has no secure place for the password ({detail})"),
            Self::NeedPassword => write!(f, "the share's password is needed"),
            Self::NotSetUp => write!(f, "no share is set up"),
            Self::Other { detail } => write!(f, "{detail}"),
        }
    }
}

impl From<ShareError> for SetupError {
    fn from(error: ShareError) -> Self {
        Self::Share { error }
    }
}

impl From<String> for SetupError {
    fn from(detail: String) -> Self {
        Self::Other { detail }
    }
}

/// The key the share's password is kept under.
pub const PASSWORD_KEY: &str = "share-password";

/// The places a password can be kept.
pub struct PasswordStores<'a> {
    pub secure: &'a dyn Secrets,
    /// For "ask each time": forgotten when Melue closes.
    pub memory: &'a dyn Secrets,
    pub file: &'a dyn Secrets,
}

impl PasswordStores<'_> {
    fn of(&self, store: PasswordStore) -> &dyn Secrets {
        match store {
            PasswordStore::Secure => self.secure,
            PasswordStore::Ask => self.memory,
            PasswordStore::File => self.file,
        }
    }

    /// Keeps the password in `store` only (it's removed from the others).
    fn keep(&self, store: PasswordStore, password: &str) -> Result<(), SetupError> {
        self.of(store).set(PASSWORD_KEY, password).map_err(|detail| match store {
            PasswordStore::Secure => SetupError::NoSecureStore { detail },
            _ => SetupError::Other { detail: format!("can't keep the password: {detail}") },
        })?;
        for other in [PasswordStore::Secure, PasswordStore::Ask, PasswordStore::File] {
            if other != store {
                let _ = self.of(other).delete(PASSWORD_KEY);
            }
        }
        Ok(())
    }

    fn forget(&self) {
        for store in [PasswordStore::Secure, PasswordStore::Ask, PasswordStore::File] {
            let _ = self.of(store).delete(PASSWORD_KEY);
        }
    }
}

/// Opens a share (with `password` for an SMB share). Doesn't connect yet.
pub type Opener<'a> = dyn Fn(&ShareSetup, Option<&str>) -> Result<Box<dyn Share>, SetupError> + 'a;

/// The real shares: SMB, or a folder (a network drive).
pub fn open(setup: &ShareSetup, password: Option<&str>) -> Result<Box<dyn Share>, SetupError> {
    Ok(match setup {
        ShareSetup::Smb { host, share, folder, user, .. } => Box::new(SmbShare::new(SmbConfig {
            host: host.clone(),
            share: share.clone(),
            folder: folder.clone(),
            user: user.clone(),
            password: password.unwrap_or_default().to_string(),
        })?),
        ShareSetup::Drive { path, .. } => Box::new(FolderShare::new(path)),
    })
}

/// Tries a share: connects and lists its music folder (the "Test connection" button).
pub fn test(opener: &Opener, setup: &ShareSetup, password: Option<&str>) -> Result<(), SetupError> {
    opener(setup, password)?.list("")?;
    Ok(())
}

impl Library {
    /// The share as set up, or None (also for a setting that isn't a setup, e.g. the spike's old test value).
    pub fn share_settings(&self) -> Result<Option<ShareSettings>, String> {
        Ok(self.setting(SHARE_SETTING)?.and_then(|json| serde_json::from_str(&json).ok()))
    }

    fn save_share_settings(&self, settings: Option<&ShareSettings>) -> Result<(), String> {
        let json = match settings {
            Some(settings) => serde_json::to_string(settings).map_err(|e| e.to_string())?,
            None => String::new(),
        };
        self.set_setting(SHARE_SETTING, &json)
    }
}

/// Sets up the share: tries it, keeps the password where its setup says, makes the sync folder (for a
/// streamed drive: the drive) the synced library folder, and saves the setup. A previous synced folder that
/// isn't this one becomes a normal folder (its songs stay).
pub fn set_up(
    library: &Library,
    stores: &PasswordStores,
    opener: &Opener,
    setup: ShareSetup,
    password: Option<&str>,
    sync_folder: Option<SyncFolderChoice>,
) -> Result<ShareSettings, SetupError> {
    test(opener, &setup, password)?;
    match &setup {
        ShareSetup::Smb { password_store, .. } => {
            let password = password.ok_or_else(|| SetupError::from("the share's password is needed".to_string()))?;
            stores.keep(*password_store, password)?;
        }
        ShareSetup::Drive { .. } => stores.forget(),
    }
    let folder: Folder = match (&setup, sync_folder) {
        (ShareSetup::Drive { path, stream: true }, _) => library.add_synced_folder(FolderKind::Path, path, Some("Share"))?,
        (_, Some(choice)) => library.add_synced_folder(choice.kind, &choice.location, Some("Synced music"))?,
        (_, None) => return Err("where should the synced music go? Choose a folder".to_string().into()),
    };
    let previous = library.share_settings()?;
    if let Some(old) = &previous {
        if old.sync_folder_id != folder.id {
            let _ = library.set_folder_synced(old.sync_folder_id, false);
        }
    }
    // Another share or folder: what was synced from the old one says nothing about the new one.
    let same = previous.as_ref().is_some_and(|old| old.sync_folder_id == folder.id && same_place(&old.setup, &setup));
    if !same {
        library.forget_all_synced()?;
    }
    let settings = ShareSettings { setup, sync_folder_id: folder.id };
    library.save_share_settings(Some(&settings))?;
    Ok(settings)
}

/// A new password (changed, or asked for again after a start): tried first, then kept in `store` (or where
/// it was kept).
pub fn set_password(
    library: &Library,
    stores: &PasswordStores,
    opener: &Opener,
    password: &str,
    store: Option<PasswordStore>,
) -> Result<ShareSettings, SetupError> {
    let mut settings = library.share_settings()?.ok_or(SetupError::NotSetUp)?;
    let ShareSetup::Smb { password_store, .. } = &mut settings.setup else {
        return Err("a network drive has no password in Melue (the system logs in)".to_string().into());
    };
    let store = store.unwrap_or(*password_store);
    *password_store = store;
    test(opener, &settings.setup, Some(password))?;
    stores.keep(store, password)?;
    library.save_share_settings(Some(&settings))?;
    Ok(settings)
}

/// Removes the share: its setup and password. The synced folder stays in the library as a normal folder,
/// with its songs.
pub fn remove(library: &Library, stores: &PasswordStores) -> Result<(), SetupError> {
    if let Some(settings) = library.share_settings()? {
        let _ = library.set_folder_synced(settings.sync_folder_id, false);
    }
    stores.forget();
    library.forget_all_synced()?;
    library.save_share_settings(None)?;
    Ok(())
}

/// Whether two setups are the same place on the same share (a new password or store doesn't matter).
fn same_place(a: &ShareSetup, b: &ShareSetup) -> bool {
    match (a, b) {
        (
            ShareSetup::Smb { host: h1, share: s1, folder: f1, .. },
            ShareSetup::Smb { host: h2, share: s2, folder: f2, .. },
        ) => h1.eq_ignore_ascii_case(h2) && s1.eq_ignore_ascii_case(s2) && f1.trim_matches('/') == f2.trim_matches('/'),
        (ShareSetup::Drive { path: p1, .. }, ShareSetup::Drive { path: p2, .. }) => p1 == p2,
        _ => false,
    }
}

/// The share, ready to use (with its password from where it's kept), and its setup.
#[cfg_attr(not(test), allow(dead_code))] // the sync uses it (step 3 on)
pub fn current(library: &Library, stores: &PasswordStores, opener: &Opener) -> Result<(ShareSettings, Box<dyn Share>), SetupError> {
    let settings = library.share_settings()?.ok_or(SetupError::NotSetUp)?;
    let password = match &settings.setup {
        ShareSetup::Smb { password_store, .. } => {
            let kept = stores.of(*password_store).get(PASSWORD_KEY).map_err(|detail| match password_store {
                PasswordStore::Secure => SetupError::NoSecureStore { detail },
                _ => SetupError::Other { detail },
            })?;
            Some(kept.ok_or(SetupError::NeedPassword)?)
        }
        ShareSetup::Drive { .. } => None,
    };
    let share = opener(&settings.setup, password.as_deref())?;
    Ok((settings, share))
}

/// Whether the password has to be asked for before the share can be used.
pub fn needs_password(library: &Library, stores: &PasswordStores) -> Result<bool, String> {
    Ok(match library.share_settings()? {
        Some(ShareSettings { setup: ShareSetup::Smb { password_store, .. }, .. }) => {
            stores.of(password_store).get(PASSWORD_KEY).map(|p| p.is_none()).unwrap_or(true)
        }
        _ => false,
    })
}

/// The default sync folder on desktop: `Music/Melue` (created).
pub fn default_sync_folder(music: &Path) -> Result<String, String> {
    let folder = music.join("Melue");
    std::fs::create_dir_all(&folder).map_err(|e| format!("can't create {}: {e}", folder.display()))?;
    Ok(folder.to_string_lossy().into_owned())
}
