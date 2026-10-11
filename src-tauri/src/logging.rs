//! Android: everything Melue prints (the sync, the car, playback, and Rust panics) goes to Android's log (tag
//! "Melue") and to a small rolling file on the phone, `Android/data/com.simon.melue/files/melue.log` (1 MB, then
//! `melue.log.1`), which can be read over adb days later (Android's own log only keeps a few hours). Desktop:
//! nothing changes (it prints to the console).

#[cfg(target_os = "android")]
mod android {
    use std::ffi::CString;
    use std::fs::OpenOptions;
    use std::io::{BufRead, BufReader, Write};
    use std::os::fd::FromRawFd;
    use std::path::Path;
    use std::sync::{Mutex, Once};

    /// The app's own folder on shared storage (its identifier is in tauri.conf.json).
    const DIR: &str = "/storage/emulated/0/Android/data/com.simon.melue/files";
    const FILE: &str = "melue.log";
    const MAX_BYTES: u64 = 1024 * 1024;

    static FILE_LOCK: Mutex<()> = Mutex::new(());

    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(priority: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
    }

    /// One line to Android's log and the file, with the time (UTC).
    pub fn write(line: &str) {
        let (tag, text) = (CString::new("Melue").unwrap(), CString::new(line.replace('\0', " ")).unwrap_or_default());
        unsafe { __android_log_write(4, tag.as_ptr(), text.as_ptr()) };
        let _guard = FILE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let path = Path::new(DIR).join(FILE);
        let _ = std::fs::create_dir_all(DIR);
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
            let _ = std::fs::rename(&path, Path::new(DIR).join(format!("{FILE}.1")));
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{} {line}", now());
        }
    }

    /// Sends this process's printed output (stdout, stderr) to [`write`]. Once per process.
    pub fn start() {
        static STARTED: Once = Once::new();
        STARTED.call_once(|| unsafe {
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
            write(&format!("---- Melue {} started", env!("CARGO_PKG_VERSION")));
        });
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

/// Starts sending printed output to Android's log and the log file (Android; nothing elsewhere).
pub fn start() {
    #[cfg(target_os = "android")]
    android::start();
}

/// One line to the log (Android: its log and the file; elsewhere: the console).
#[cfg_attr(not(target_os = "android"), allow(dead_code))] // only the car (Android) writes lines itself
pub fn write(line: &str) {
    #[cfg(target_os = "android")]
    android::write(line);
    #[cfg(not(target_os = "android"))]
    eprintln!("{line}");
}
