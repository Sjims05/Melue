//! Network folders: a music folder that's on another computer (Windows: a mapped drive like `Z:\`, or a
//! `\\server\share` path). It's a normal music folder, except when it's the share Melue syncs with (or inside it): its
//! songs are in the library already, as the synced folder's (folders.rs refuses it). And removing songs from it deletes
//! them for good: a network folder has no Recycle Bin.

use crate::share::setup::{ShareSettings, ShareSetup};

/// A path's network address, `\\server\share\...` (lowercase, `/` between parts, for comparing), or None when it's
/// on this computer.
pub fn address(path: &str) -> Option<String> {
    let path = path.replace('/', "\\");
    let unc = if path.starts_with("\\\\") { Some(path) } else { mapped_drive(&path) }?;
    Some(unc.trim_start_matches('\\').replace('\\', "/").trim_end_matches('/').to_lowercase())
}

/// The share's network addresses (`server/share/folder`, lowercase): an SMB share by its address and its home
/// address; a network drive by where it's mapped to.
pub fn share_addresses(settings: &ShareSettings) -> Vec<String> {
    match &settings.setup {
        ShareSetup::Smb { host, share, folder, home_host, .. } => [Some(host), home_host.as_ref()]
            .into_iter()
            .flatten()
            .filter(|h| !h.trim().is_empty())
            .map(|h| {
                let base = format!("{}/{}", h.trim(), share.trim_matches(['/', '\\']));
                let folder = folder.trim_matches(['/', '\\']).replace('\\', "/");
                if folder.is_empty() { base } else { format!("{base}/{folder}") }.to_lowercase()
            })
            .collect(),
        ShareSetup::Drive { path, .. } => address(path).into_iter().collect(),
    }
}

/// How `folder` (a network address) relates to the share's: "same", "inside" it, "contains" it, or None.
pub fn relation_to_share(folder: &str, share: &[String]) -> Option<&'static str> {
    share.iter().find_map(|s| {
        if folder == s {
            Some("same")
        } else if folder.starts_with(&format!("{s}/")) {
            Some("inside")
        } else if s.starts_with(&format!("{folder}/")) {
            Some("contains")
        } else {
            None
        }
    })
}

/// `Z:\Music` → `\\nas\music\Music` when Z: is a mapped network drive (Windows).
#[cfg(windows)]
fn mapped_drive(path: &str) -> Option<String> {
    use windows_sys::Win32::NetworkManagement::WNet::WNetGetConnectionW;
    let bytes = path.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        return None;
    }
    let drive: Vec<u16> = path[..2].encode_utf16().chain([0]).collect();
    let mut remote = vec![0u16; 1024];
    let mut len = remote.len() as u32;
    // 0: NO_ERROR (a local drive answers ERROR_NOT_CONNECTED).
    if unsafe { WNetGetConnectionW(drive.as_ptr(), remote.as_mut_ptr(), &mut len) } != 0 {
        return None;
    }
    let end = remote.iter().position(|&c| c == 0).unwrap_or(remote.len());
    let remote = String::from_utf16_lossy(&remote[..end]);
    Some(format!("{}{}", remote.trim_end_matches('\\'), &path[2..]))
}

#[cfg(not(windows))]
fn mapped_drive(_path: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unc_paths_are_network_addresses() {
        assert_eq!(address(r"\\NAS\Music\Rock\").as_deref(), Some("nas/music/rock"));
        assert_eq!(address("//nas/music").as_deref(), Some("nas/music"));
        assert_eq!(address(r"C:\Users\me\Music"), None, "a folder on this computer");
    }

    #[test]
    fn a_folder_is_the_share_inside_it_or_around_it() {
        let share = vec!["nas/music/melue".to_string(), "192.168.1.5/music/melue".to_string()];
        assert_eq!(relation_to_share("nas/music/melue", &share), Some("same"));
        assert_eq!(relation_to_share("192.168.1.5/music/melue/abba", &share), Some("inside"));
        assert_eq!(relation_to_share("nas/music", &share), Some("contains"));
        assert_eq!(relation_to_share("nas/music/melue2", &share), None, "a name that merely starts the same");
        assert_eq!(relation_to_share("nas/photos", &share), None);
    }
}
