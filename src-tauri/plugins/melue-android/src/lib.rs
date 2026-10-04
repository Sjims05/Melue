//! Melue's own Android code (see android/): the permission to read the music on the phone, and Android's
//! music index. Melue's Rust code uses it through [`MelueAndroidExt`]; there are no JavaScript commands.
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

/// A song in Android's music index.
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

        /// How many songs Android's music index has.
        pub fn count_music(&self) -> Result<u64, String> {
            self.0.run_mobile_plugin::<Count>("countMusic", ()).map(|c| c.count).map_err(|e| e.to_string())
        }

        /// Every song in Android's music index.
        pub fn list_music(&self) -> Result<Vec<MusicFile>, String> {
            self.0.run_mobile_plugin::<Files>("listMusic", ()).map(|f| f.files).map_err(|e| e.to_string())
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
