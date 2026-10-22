//! A report's screenshot (only when "Keep a screenshot with reports" is on; never sent): the app's window as it was
//! when the report was opened, kept as `reports/<id>.png` (saved.rs takes it to the share). On a computer the web
//! view's own capture (WebView2's CapturePreview), on a phone the window's pixels (Melue's Android plugin).

use tauri::AppHandle;

/// Takes the screenshot for report `id`. False: this system can't.
#[tauri::command(async)]
pub fn report_screenshot(app: AppHandle, id: String) -> Result<bool, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("not a report's id".into());
    }
    let file = super::saved::dir(&app)?.join(format!("{id}.png"));
    capture(&app, &file)
}

#[cfg(windows)]
fn capture(app: &AppHandle, file: &std::path::Path) -> Result<bool, String> {
    use std::os::windows::ffi::OsStrExt;
    use std::time::Duration;
    use tauri::Manager;
    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
    use windows::Win32::System::Com::{STGM_CREATE, STGM_READWRITE};
    use windows::Win32::UI::Shell::SHCreateStreamOnFileEx;

    let window = app.get_webview_window("main").ok_or("no window")?;
    let path: Vec<u16> = file.as_os_str().encode_wide().chain([0]).collect();
    let (done, wait) = std::sync::mpsc::channel::<Result<(), String>>();
    window
        .with_webview(move |webview| {
            let started = (|| -> windows::core::Result<()> {
                // SAFETY: on the window's own thread (with_webview), with the web view Tauri keeps alive.
                unsafe {
                    let core = webview.controller().CoreWebView2()?;
                    let stream = SHCreateStreamOnFileEx(PCWSTR(path.as_ptr()), (STGM_CREATE | STGM_READWRITE).0, FILE_ATTRIBUTE_NORMAL.0, true, None)?;
                    let finished = done.clone();
                    // The stream is kept until the capture is written (then closed, flushing the file).
                    let kept = stream.clone();
                    let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
                        drop(kept);
                        let _ = finished.send(result.map_err(|e| e.to_string()));
                        Ok(())
                    }));
                    core.CapturePreview(COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, &stream, &handler)
                }
            })();
            if let Err(e) = started {
                let _ = done.send(Err(e.to_string()));
            }
        })
        .map_err(|e| e.to_string())?;
    wait.recv_timeout(Duration::from_secs(5)).map_err(|_| "the screenshot took too long".to_string())??;
    Ok(true)
}

#[cfg(target_os = "android")]
fn capture(app: &AppHandle, file: &std::path::Path) -> Result<bool, String> {
    use tauri_plugin_melue_android::MelueAndroidExt;
    app.melue_android().screenshot(&file.to_string_lossy())?;
    Ok(true)
}

#[cfg(not(any(windows, target_os = "android")))]
fn capture(_app: &AppHandle, _file: &std::path::Path) -> Result<bool, String> {
    Ok(false)
}
