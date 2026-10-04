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

#[cfg(target_os = "android")]
pub use android::{MelueAndroid, MelueAndroidExt};

#[cfg(target_os = "android")]
mod android {
    use serde::Deserialize;
    use tauri::plugin::PluginHandle;
    use tauri::{Manager, Runtime};

    use super::PermissionState;

    pub struct MelueAndroid<R: Runtime>(pub(crate) PluginHandle<R>);

    #[derive(Deserialize)]
    struct State {
        state: PermissionState,
    }

    #[derive(Deserialize)]
    struct Count {
        count: u64,
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
