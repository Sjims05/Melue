//! Melue Dev's tools (the dev page): what testing needs that Melue doesn't do on demand. Refused in the official app.

use tauri::AppHandle;

/// A small song (a short tone, tagged "Sample Song"), and where it goes on the share.
const SAMPLE: &[u8] = include_bytes!("../testdata/sample.mp3");
const SAMPLE_DIR: &str = "Melue test";

/// Puts the sample song on the share (`put`), or takes it off wherever it is (the share's layout may have moved it):
/// a change on the share, for "Sync on share changes" and "Remove songs gone from the share". Each one put has a name
/// of its own (Test song 1.mp3, 2…): new to every device, even one that has the last one.
#[tauri::command(async)]
pub fn dev_share_sample(app: AppHandle, put: bool) -> Result<String, String> {
    if !crate::app_is_dev() {
        return Err("only in Melue Dev".into());
    }
    let mut done = String::new();
    crate::sync::shared::with_share(&app, |share| {
        if put {
            let taken: Vec<String> = share.walk("")?.into_iter().map(|f| f.name).collect();
            let name = (1..).map(|n| format!("Test song {n}.mp3")).find(|n| !taken.contains(n)).expect("a free name");
            done = format!("Put on the share: {SAMPLE_DIR}/{name}");
            share.write(&format!("{SAMPLE_DIR}/{name}"), SAMPLE)?;
        } else {
            // Recognised by its bytes: the layout may have moved or renamed it.
            for file in share.walk("")?.into_iter().filter(|f| f.size == SAMPLE.len() as u64) {
                if share.read(&file.path)? == SAMPLE {
                    share.delete(&file.path)?;
                }
            }
            share.delete_empty_dir(SAMPLE_DIR)?;
            done = "Taken off the share: the test songs".into();
        }
        Ok(())
    })?;
    Ok(done)
}
