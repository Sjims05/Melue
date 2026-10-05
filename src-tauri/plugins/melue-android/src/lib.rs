//! Melue's own Android code (see android/): the permissions to read the music (and, opt-in, photos) on
//! the phone, Android's music index, and secrets kept with Android Keystore encryption (the share's
//! password). Melue's Rust code uses it through [`MelueAndroidExt`]; there are no JavaScript commands.
//! Elsewhere than Android the plugin does nothing.

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("melue-android")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("com.simon.melue.android", "MelueAndroidPlugin")?;
                _app.manage(android::MelueAndroid(handle));
            }
            Ok(())
        })
        .build()
}

/// Whether Melue may read the music on the phone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionState {
    Granted,
    Denied,
    /// Not asked yet: requesting shows Android's dialog.
    Prompt,
    /// Denied once: requesting shows the dialog again (explain why first).
    PromptWithRationale,
}

/// A song (or playlist file) in Android's index.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFile {
    /// The file's path (readable with the music permission).
    pub path: String,
    pub name: String,
    pub size: i64,
    /// Milliseconds since 1970.
    pub modified_at: i64,
}

#[cfg(target_os = "android")]
pub use android::{MelueAndroid, MelueAndroidExt};

#[cfg(target_os = "android")]
mod android {
    use serde::{Deserialize, Serialize};
    use tauri::ipc::Channel;
    use tauri::plugin::PluginHandle;
    use tauri::{Manager, Runtime};

    use super::{MusicFile, PermissionState};

    pub struct MelueAndroid<R: Runtime>(pub(crate) PluginHandle<R>);

    #[derive(Deserialize)]
    struct State {
        state: PermissionState,
    }

    #[derive(Deserialize)]
    struct Count {
        count: u64,
    }

    #[derive(Deserialize)]
    struct Files {
        files: Vec<MusicFile>,
    }

    #[derive(Serialize)]
    struct Secret<'a> {
        key: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<&'a str>,
    }

    #[derive(Deserialize)]
    struct SecretValue {
        #[serde(default)]
        value: Option<String>,
    }

    #[derive(Serialize)]
    struct Move<'a> {
        document: &'a str,
        from: &'a str,
        to: &'a str,
    }

    #[derive(Deserialize)]
    struct Moved {
        uri: String,
    }

    #[derive(Deserialize)]
    struct DeviceName {
        name: String,
    }

    #[derive(Deserialize)]
    struct Metered {
        metered: bool,
    }

    #[derive(Serialize)]
    struct SyncNotification<'a> {
        done: u32,
        total: u32,
        text: &'a str,
    }

    #[derive(Serialize)]
    struct Listen {
        event: String,
        handler: Channel<serde_json::Value>,
    }

    impl<R: Runtime> MelueAndroid<R> {
        pub fn music_permission(&self) -> Result<PermissionState, String> {
            self.0.run_mobile_plugin::<State>("musicPermission", ()).map(|s| s.state).map_err(|e| e.to_string())
        }

        /// Asks for the permission when it isn't granted (waits for the user's answer).
        pub fn request_music_permission(&self) -> Result<PermissionState, String> {
            self.0.run_mobile_plugin::<State>("requestMusicPermission", ()).map(|s| s.state).map_err(|e| e.to_string())
        }

        /// Whether Melue may read the photos on the phone (cover images next to the songs).
        pub fn photos_permission(&self) -> Result<PermissionState, String> {
            self.0.run_mobile_plugin::<State>("photosPermission", ()).map(|s| s.state).map_err(|e| e.to_string())
        }

        /// Asks for the photos permission when it isn't granted (waits for the user's answer).
        pub fn request_photos_permission(&self) -> Result<PermissionState, String> {
            self.0.run_mobile_plugin::<State>("requestPhotosPermission", ()).map(|s| s.state).map_err(|e| e.to_string())
        }

        /// How many songs Android's music index has.
        pub fn count_music(&self) -> Result<u64, String> {
            self.0.run_mobile_plugin::<Count>("countMusic", ()).map(|c| c.count).map_err(|e| e.to_string())
        }

        /// Every song in Android's music index.
        pub fn list_music(&self) -> Result<Vec<MusicFile>, String> {
            self.0.run_mobile_plugin::<Files>("listMusic", ()).map(|f| f.files).map_err(|e| e.to_string())
        }

        /// The playlist files (.m3u, .m3u8) in Android's index (possibly none with only the music permission).
        pub fn list_playlists(&self) -> Result<Vec<MusicFile>, String> {
            self.0.run_mobile_plugin::<Files>("listPlaylists", ()).map(|f| f.files).map_err(|e| e.to_string())
        }

        /// A secret kept with Android Keystore encryption (the share's password), or None.
        pub fn secret_get(&self, key: &str) -> Result<Option<String>, String> {
            self.0
                .run_mobile_plugin::<SecretValue>("secretGet", Secret { key, value: None })
                .map(|s| s.value)
                .map_err(|e| e.to_string())
        }

        pub fn secret_set(&self, key: &str, value: &str) -> Result<(), String> {
            self.0
                .run_mobile_plugin::<serde_json::Value>("secretSet", Secret { key, value: Some(value) })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        pub fn secret_delete(&self, key: &str) -> Result<(), String> {
            self.0
                .run_mobile_plugin::<serde_json::Value>("secretDelete", Secret { key, value: None })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        /// Moves a document of a chosen folder (`document`, in folder `from`) into folder `to` of it, keeping the
        /// file and its time. Returns its new address.
        pub fn move_document(&self, document: &str, from: &str, to: &str) -> Result<String, String> {
            self.0
                .run_mobile_plugin::<Moved>("moveDocument", Move { document, from, to })
                .map(|m| m.uri)
                .map_err(|e| e.to_string())
        }

        /// The phone's name, as set in its settings, or its model.
        pub fn device_name(&self) -> Result<String, String> {
            self.0.run_mobile_plugin::<DeviceName>("deviceName", ()).map(|d| d.name).map_err(|e| e.to_string())
        }

        /// Whether the connection is metered (mobile data, a phone's hotspot).
        pub fn metered(&self) -> Result<bool, String> {
            self.0.run_mobile_plugin::<Metered>("networkMetered", ()).map(|m| m.metered).map_err(|e| e.to_string())
        }

        /// Shows or updates the "Syncing music" notification: `done` of `total` files, `text` the current one.
        pub fn sync_notification(&self, done: u32, total: u32, text: &str) -> Result<(), String> {
            self.0
                .run_mobile_plugin::<serde_json::Value>("syncNotification", SyncNotification { done, total, text })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        /// The sync is over: the notification goes.
        pub fn sync_notification_done(&self) -> Result<(), String> {
            self.0.run_mobile_plugin::<serde_json::Value>("syncNotificationDone", ()).map(|_| ()).map_err(|e| e.to_string())
        }

        /// Calls `handler` for each of the plugin's `event`s ("musicChanged", "resumed").
        pub fn listen(&self, event: &str, handler: impl Fn() + Send + Sync + 'static) -> Result<(), String> {
            let channel = Channel::new(move |_| {
                handler();
                Ok(())
            });
            self.0
                .run_mobile_plugin::<serde_json::Value>("registerListener", Listen { event: event.into(), handler: channel })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    }

    pub trait MelueAndroidExt<R: Runtime> {
        fn melue_android(&self) -> &MelueAndroid<R>;
    }

    impl<R: Runtime, T: Manager<R>> MelueAndroidExt<R> for T {
        fn melue_android(&self) -> &MelueAndroid<R> {
            self.state::<MelueAndroid<R>>().inner()
        }
    }
}
