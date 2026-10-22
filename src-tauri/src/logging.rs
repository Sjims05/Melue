//! Melue's log: everything it prints (the sync, the car, playback, start-up, and Rust panics) also goes to a small
//! rolling file (1 MB, then `melue.log.1`), which can be read days later.
//!
//! Android: `Android/data/<the app's package>/files/melue.log` (over adb, or shared from About), and Android's own log
//! (tag "Melue", which only keeps a few hours). Windows: `%LOCALAPPDATA%\<the app's identity>\logs\melue.log` (About
//! opens its folder), and the console still shows it (`tauri dev`). The official app is com.simon.melue; a development
//! or test build com.simon.melue.dev ("Melue Dev": its own data, tauri.dev.conf.json).

use std::path::PathBuf;

/// The log file's folder (None where Melue keeps no log file).
pub fn dir() -> Option<PathBuf> {
    // The app's own folder on shared storage: its package (the official app's, or Melue Dev's), as the process is named.
    #[cfg(target_os = "android")]
    return Some(PathBuf::from(format!("/storage/emulated/0/Android/data/{}/files", android_package())));
    // Tauri's app_log_dir on Windows (it needs the app, which isn't there yet when the log starts): a development build
    // is Melue Dev (debug builds are; tauri.dev.conf.json).
    #[cfg(windows)]
    return std::env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join(crate::IDENTITY).join("logs"));
    #[allow(unreachable_code)]
    None
}

/// The Android app's package (com.simon.melue, or com.simon.melue.dev for a test build): the process's name.
#[cfg(target_os = "android")]
pub fn android_package() -> String {
    std::fs::read("/proc/self/cmdline")
        .ok()
        .and_then(|bytes| String::from_utf8(bytes.split(|&b| b == 0).next()?.to_vec()).ok())
        .map(|name| name.split(':').next().unwrap_or_default().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "com.simon.melue".to_string())
}

/// The log file (None where Melue keeps none).
pub fn file() -> Option<PathBuf> {
    dir().map(|dir| dir.join(file::NAME))
}

mod file {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::Mutex;

    pub const NAME: &str = "melue.log";
    const MAX_BYTES: u64 = 1024 * 1024;
    static LOCK: Mutex<()> = Mutex::new(());

    /// One line to the file, with the time (UTC).
    pub fn write(line: &str) {
        let Some(dir) = super::dir() else { return };
        let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let path = dir.join(NAME);
        let _ = std::fs::create_dir_all(&dir);
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
            let _ = std::fs::rename(&path, dir.join(format!("{NAME}.1")));
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{} {line}", now());
        }
    }

    /// "2026-10-11 18:34:04" (UTC), without a date library.
    fn now() -> String {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
        let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
        // Days since 1970 to a date (Howard Hinnant's civil_from_days).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}", rest / 3600, rest % 3600 / 60, rest % 60)
    }
}

#[cfg(target_os = "android")]
mod android {
    use std::ffi::CString;
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(priority: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
    }

    /// One line to Android's log and the file.
    pub fn write(line: &str) {
        let (tag, text) = (CString::new("Melue").unwrap(), CString::new(line.replace('\0', " ")).unwrap_or_default());
        unsafe { __android_log_write(4, tag.as_ptr(), text.as_ptr()) };
        super::file::write(line);
    }

    /// Sends this process's printed output (stdout, stderr) to [`write`].
    pub fn start() {
        unsafe {
            let mut fds = [0; 2];
            if libc::pipe(fds.as_mut_ptr()) != 0 {
                return;
            }
            libc::dup2(fds[1], 1);
            libc::dup2(fds[1], 2);
            let read = std::fs::File::from_raw_fd(fds[0]);
            let _ = std::thread::Builder::new().name("melue-log".into()).spawn(move || {
                for line in BufReader::new(read).lines().map_while(Result::ok) {
                    write(&line);
                }
            });
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::io::{FromRawHandle, IntoRawHandle};
    use windows_sys::Win32::System::Console::{GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};

    /// Sends this process's printed output (Rust's stdout and stderr) to the file, and on to the console it had.
    pub fn start() {
        let Ok((read, write)) = std::io::pipe() else { return };
        let console = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
        // The pipe's end stays open for the rest of the process: it's where printing goes now.
        let write = write.into_raw_handle();
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, write);
            SetStdHandle(STD_ERROR_HANDLE, write);
        }
        // A release build has no console (no handle): only the file then.
        let mut console = (!console.is_null()).then(|| unsafe { File::from_raw_handle(console) });
        let _ = std::thread::Builder::new().name("melue-log".into()).spawn(move || {
            for line in BufReader::new(read).lines().map_while(Result::ok) {
                if let Some(out) = console.as_mut() {
                    let _ = writeln!(out, "{line}");
                }
                super::file::write(&line);
            }
        });
    }
}

/// About's log row: the phone hands the log to Android's share sheet; the computer shows it in its folder.
#[tauri::command]
pub fn log_show(app: tauri::AppHandle) -> Result<(), String> {
    let file = file().ok_or("Melue keeps no log here")?;
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_melue_android::MelueAndroidExt;
        return app.melue_android().share_file(&file.to_string_lossy(), "Melue's log");
    }
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_opener::OpenerExt;
        if !file.exists() {
            return app.opener().open_path(dir().unwrap().to_string_lossy(), None::<&str>).map_err(|e| e.to_string());
        }
        app.opener().reveal_item_in_dir(&file).map_err(|e| e.to_string())
    }
}

/// Starts sending printed output to the log file (and Android's log). Once, first thing.
pub fn start() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        #[cfg(target_os = "android")]
        android::start();
        #[cfg(windows)]
        windows::start();
        eprintln!("---- Melue {} started", env!("CARGO_PKG_VERSION"));
    });
}

/// One line to the log (Android: its log and the file; elsewhere: printed, which goes to the file too).
#[cfg_attr(not(target_os = "android"), allow(dead_code))] // only the car (Android) writes lines itself
pub fn write(line: &str) {
    #[cfg(target_os = "android")]
    android::write(line);
    #[cfg(not(target_os = "android"))]
    eprintln!("{line}");
}
