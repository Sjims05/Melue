//! Where the share's password is kept, never in the library's database:
//! - [`PasswordStore::Secure`] (the default): the system's secure store: Windows Credential Manager, the macOS
//!   Keychain, Linux's Secret Service (the `keyring` crate); on Android the Android Keystore (Melue's Android
//!   plugin: encrypted with a key that never leaves the phone's secure hardware).
//! - [`PasswordStore::Ask`]: only in memory, asked for again after every start (for systems without a secure
//!   store, when the user prefers that).
//! - [`PasswordStore::File`]: a plain file in the app's data folder, when the user chooses it knowingly (no
//!   secure store, and asking each time is too much).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use crate::guard::Locked;

/// Where a password is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PasswordStore {
    Secure,
    Ask,
    File,
}

/// A place for secrets, by key.
pub trait Secrets: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    /// Not there: fine.
    fn delete(&self, key: &str) -> Result<(), String>;
}

/// In memory only: "ask each time", and the tests.
#[derive(Default)]
pub struct MemorySecrets(Mutex<HashMap<String, String>>);

impl Secrets for MemorySecrets {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.0.locked().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.0.locked().insert(key.into(), value.into());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        self.0.locked().remove(key);
        Ok(())
    }
}

/// A plain JSON file (readable by anyone with access to the app's data folder: only when chosen knowingly).
pub struct FileSecrets {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileSecrets {
    pub fn new(path: PathBuf) -> Self {
        Self { path, lock: Mutex::new(()) }
    }

    fn load(&self) -> Result<HashMap<String, String>, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, all: &HashMap<String, String>) -> Result<(), String> {
        if all.is_empty() {
            return match std::fs::remove_file(&self.path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&self.path, serde_json::to_string(all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
}

impl Secrets for FileSecrets {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        let _guard = self.lock.locked();
        Ok(self.load()?.get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let _guard = self.lock.locked();
        let mut all = self.load()?;
        all.insert(key.into(), value.into());
        self.save(&all)
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        let _guard = self.lock.locked();
        let mut all = self.load()?;
        if all.remove(key).is_some() {
            self.save(&all)?;
        }
        Ok(())
    }
}

/// The system's secure store.
pub struct SystemSecrets {
    #[cfg(target_os = "android")]
    app: tauri::AppHandle,
}

/// The name Melue's secrets are kept under in the system's store.
#[cfg(not(target_os = "android"))]
// Melue Dev (a development build) keeps its own: it never reads or changes the official app's.
pub(crate) const SERVICE: &str = if cfg!(debug_assertions) { "Melue Dev" } else { "Melue" };

impl SystemSecrets {
    pub fn new(app: &tauri::AppHandle) -> Self {
        #[cfg(target_os = "android")]
        {
            Self { app: app.clone() }
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            Self {}
        }
    }
}

impl Secrets for SystemSecrets {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        #[cfg(target_os = "android")]
        {
            use tauri_plugin_melue_android::MelueAndroidExt;
            self.app.melue_android().secret_get(key)
        }
        #[cfg(not(target_os = "android"))]
        {
            match keyring::Entry::new(SERVICE, key).and_then(|entry| entry.get_password()) {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(e.to_string()),
            }
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            use tauri_plugin_melue_android::MelueAndroidExt;
            self.app.melue_android().secret_set(key, value)
        }
        #[cfg(not(target_os = "android"))]
        {
            keyring::Entry::new(SERVICE, key).and_then(|entry| entry.set_password(value)).map_err(|e| e.to_string())
        }
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            use tauri_plugin_melue_android::MelueAndroidExt;
            self.app.melue_android().secret_delete(key)
        }
        #[cfg(not(target_os = "android"))]
        {
            match keyring::Entry::new(SERVICE, key).and_then(|entry| entry.delete_credential()) {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testkit::TestDir;

    fn round_trip(secrets: &dyn Secrets) {
        assert_eq!(secrets.get("share").unwrap(), None);
        secrets.set("share", "hunter2").unwrap();
        secrets.set("other", "x").unwrap();
        assert_eq!(secrets.get("share").unwrap().as_deref(), Some("hunter2"));
        secrets.set("share", "changed").unwrap();
        assert_eq!(secrets.get("share").unwrap().as_deref(), Some("changed"));
        secrets.delete("share").unwrap();
        secrets.delete("share").unwrap();
        assert_eq!(secrets.get("share").unwrap(), None);
        assert_eq!(secrets.get("other").unwrap().as_deref(), Some("x"));
    }

    #[test]
    fn memory_and_file_secrets_keep_and_forget() {
        round_trip(&MemorySecrets::default());
        let dir = TestDir::new("secrets");
        let path = dir.path.join("data/secrets.json");
        round_trip(&FileSecrets::new(path.clone()));
        // Kept in the file across restarts; the file goes when it's empty.
        assert_eq!(FileSecrets::new(path.clone()).get("other").unwrap().as_deref(), Some("x"));
        FileSecrets::new(path.clone()).delete("other").unwrap();
        assert!(!path.exists());
    }

    #[test]
    #[ignore = "uses this computer's real secure store (Credential Manager / Keychain): cargo test system_secrets -- --ignored"]
    fn system_secrets_keep_and_forget() {
        let key = format!("melue-test-{}", crate::library::testkit::unique());
        let secrets = SystemSecrets {};
        assert_eq!(secrets.get(&key).unwrap(), None);
        secrets.set(&key, "hunter2").unwrap();
        assert_eq!(secrets.get(&key).unwrap().as_deref(), Some("hunter2"));
        secrets.delete(&key).unwrap();
        assert_eq!(secrets.get(&key).unwrap(), None);
    }
}
