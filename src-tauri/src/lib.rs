use std::sync::Arc;
use tokio::sync::Mutex;
use smb2::SmbClient;
use serde::Serialize;

struct SmbState(Arc<Mutex<Option<SmbClient>>>);
#[derive(Serialize)]
struct SmbEntry {
    name: String,
    is_directory: bool,
    size: u64,
}

// (placeholder for now — full version next)
#[tauri::command]
async fn smb_connect(
    state: tauri::State<'_, SmbState>,
    host: String,
    user: String,
    pass: String,
) -> Result<(), String> {
    let client = smb2::connect(&host, &user, &pass)
        .await
        .map_err(|e| e.to_string())?;
    *state.0.lock().await = Some(client);
    Ok(())
}

#[tauri::command]
async fn smb_list_dir(
    state: tauri::State<'_, SmbState>,
    share: String,
    path: String,
) -> Result<Vec<SmbEntry>, String> {
    let mut guard = state.0.lock().await;
    let client = guard.as_mut().ok_or("Not connected. Call smb_connect first.")?;

    let mut tree = client
        .connect_share(&share)
        .await
        .map_err(|e| e.to_string())?;

    let entries = client
        .list_directory(&mut tree, &path)
        .await
        .map_err(|e| e.to_string())?;

    Ok(entries
        .into_iter()
        .map(|e| SmbEntry {
            name: e.name,
            is_directory: e.is_directory,
            size: e.size,
        })
        .collect())
}

use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[tauri::command]
async fn smb_copy_file(
    state: tauri::State<'_, SmbState>,
    share: String,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    let mut guard = state.0.lock().await;
    let client = guard.as_mut().ok_or("Not connected. Call smb_connect first.")?;

    let tree = client
        .connect_share(&share)
        .await
        .map_err(|e| e.to_string())?;

    let mut download = client
        .download(&tree, &remote_path)
        .await
        .map_err(|e| e.to_string())?;

    let expected_size = download.size();
    let tmp_path = format!("{local_path}.tmp");

    // The destination folder may not exist yet (on Windows the app data folder is only made when needed).
    if let Some(parent) = std::path::Path::new(&local_path).parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }

    let mut local_file = File::create(&tmp_path).await.map_err(|e| e.to_string())?;

    let mut total_written: u64 = 0;
    while let Some(chunk_result) = download.next_chunk().await {
        let chunk = chunk_result.map_err(|e| e.to_string())?;
        local_file
            .write_all(&chunk)
            .await
            .map_err(|e| e.to_string())?;
        total_written += chunk.len() as u64;
    }
    local_file.flush().await.map_err(|e| e.to_string())?;
    drop(local_file);

    if total_written != expected_size {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(format!(
            "Size mismatch: expected {expected_size}, got {total_written}"
        ));
    }

    tokio::fs::rename(&tmp_path, &local_path)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // Mobile only (see Cargo.toml); their permissions are in capabilities/mobile.json and android.json.
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_native_audio::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_android_fs::init());

    builder
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_opener::init())
        .manage(SmbState(Arc::new(Mutex::new(None))))
        .invoke_handler(tauri::generate_handler![smb_connect, smb_list_dir, smb_copy_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
