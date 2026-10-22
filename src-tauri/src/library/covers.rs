//! Covers: the pictures in song files (or next to them), kept once each in the app's data folder in two
//! sizes: small (300 px) for lists and grids, and large (1000 px, or the original's size when smaller)
//! for the player, the notification and Android Auto. The same picture in many songs (a whole album) is
//! recognised by its content and stored once.

use std::collections::HashSet;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::DynamicImage;
use rusqlite::OptionalExtension;

use super::{err, Library, Result};

pub const SMALL: u32 = 300;
pub const LARGE: u32 = 1000;
const QUALITY: u8 = 88;

/// A cover's file in the covers folder: `<file>-300.jpg` or `<file>-1000.jpg`.
pub fn file_name(file: &str, size: u32) -> String {
    format!("{file}-{size}.jpg")
}

/// Whether `name` could be one of the covers' files (only those are ever served or deleted).
pub fn is_cover_file(name: &str) -> bool {
    name.strip_suffix(&format!("-{SMALL}.jpg"))
        .or_else(|| name.strip_suffix(&format!("-{LARGE}.jpg")))
        .is_some_and(|base| base.len() == 32 && base.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn hash(bytes: &[u8]) -> String {
    format!("{:032x}", xxhash_rust::xxh3::xxh3_128(bytes))
}

struct Prepared {
    small: Vec<u8>,
    large: Vec<u8>,
    width: u32,
    height: u32,
}

/// Decodes a picture and makes its two sizes (the slow part, done without holding the library).
fn prepare(bytes: &[u8]) -> Result<Prepared> {
    let image = image::load_from_memory(bytes).map_err(err)?;
    let large = fit(&image, LARGE);
    let small = fit(&large, SMALL);
    Ok(Prepared { small: jpeg(&small)?, large: jpeg(&large)?, width: image.width(), height: image.height() })
}

fn fit(image: &DynamicImage, size: u32) -> DynamicImage {
    if image.width() <= size && image.height() <= size {
        image.clone()
    } else {
        image.resize(size, size, FilterType::Triangle)
    }
}

fn jpeg(image: &DynamicImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, QUALITY).encode_image(&image.to_rgb8()).map_err(err)?;
    Ok(out)
}

impl Library {
    pub fn covers_dir(&self) -> &Path {
        &self.covers_dir
    }

    /// The cover for a picture found in a song (or next to it), stored the first time it's seen. None when
    /// it isn't a picture Melue can read (JPEG, PNG and WebP).
    pub fn cover_for(&self, bytes: &[u8]) -> Result<Option<i64>> {
        let hash = hash(bytes);
        let find = |library: &Library| {
            library.conn().query_row("SELECT id FROM covers WHERE hash = ?1", [&hash], |r| r.get::<_, i64>(0)).optional().map_err(err)
        };
        if let Some(id) = find(self)? {
            return Ok(Some(id));
        }
        let Ok(prepared) = prepare(bytes) else { return Ok(None) };
        std::fs::create_dir_all(&self.covers_dir).map_err(err)?;
        std::fs::write(self.covers_dir.join(file_name(&hash, SMALL)), &prepared.small).map_err(err)?;
        std::fs::write(self.covers_dir.join(file_name(&hash, LARGE)), &prepared.large).map_err(err)?;
        self.conn()
            .execute(
                "INSERT INTO covers (hash, file, width, height) VALUES (?1, ?1, ?2, ?3) ON CONFLICT (hash) DO NOTHING",
                rusqlite::params![hash, prepared.width, prepared.height],
            )
            .map_err(err)?;
        find(self)
    }

    /// How much space the covers take (bytes).
    pub fn covers_size(&self) -> u64 {
        let Ok(entries) = std::fs::read_dir(&self.covers_dir) else { return 0 };
        entries.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
    }

    /// Deletes the covers no song, album or playlist uses any more (songs that are missing keep theirs), and files in
    /// the covers folder that aren't a stored cover's (left over from an interrupted scan). Returns how
    /// many files were deleted.
    pub fn clean_covers(&self) -> Result<u32> {
        let kept: HashSet<String> = {
            let conn = self.conn();
            conn.execute(
                "DELETE FROM covers WHERE id NOT IN (SELECT cover_id FROM tracks WHERE cover_id IS NOT NULL)
                                     AND id NOT IN (SELECT cover_id FROM albums WHERE cover_id IS NOT NULL)
                                     AND id NOT IN (SELECT cover_id FROM playlists WHERE cover_id IS NOT NULL)",
                [],
            )
            .map_err(err)?;
            let mut stmt = conn.prepare("SELECT file FROM covers").map_err(err)?;
            let files = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
            files
                .flatten()
                .flat_map(|file| [file_name(&file, SMALL), file_name(&file, LARGE)])
                .collect()
        };
        let Ok(entries) = std::fs::read_dir(&self.covers_dir) else { return Ok(0) };
        let mut deleted = 0;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_cover_file(&name) && !kept.contains(&name) && std::fs::remove_file(entry.path()).is_ok() {
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}

/// Answers the UI's requests for covers (the `cover:` protocol, see lib.rs): `/<file>-300.jpg` or
/// `/<file>-1000.jpg` from the covers folder; anything else isn't found.
pub fn serve(covers_dir: &Path, request_path: &str) -> tauri::http::Response<Vec<u8>> {
    let name = request_path.trim_start_matches('/');
    let bytes = is_cover_file(name).then(|| std::fs::read(covers_dir.join(name)).ok()).flatten();
    let response = tauri::http::Response::builder();
    match bytes {
        // Named after their content, so they never change.
        Some(bytes) => response
            .header("Content-Type", "image/jpeg")
            .header("Cache-Control", "max-age=31536000, immutable")
            // The page reads a cover's colours (Now playing's lyrics), which needs the page allowed to.
            .header("Access-Control-Allow-Origin", "*")
            .body(bytes),
        None => response.status(404).body(Vec::new()),
    }
    .expect("a cover response")
}
