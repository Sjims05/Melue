//! The library's test kit: temporary libraries, music folders filled with generated files carrying
//! the tags a test asks for, edits to those folders (as a user would make), and the real sample files
//! in `src-tauri/testdata/` (MP3, FLAC, M4A with tags and a cover, made with ffmpeg).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::items::Timestamp;
use lofty::tag::{Tag, TagType};

use super::Library;

/// A temporary folder, removed when dropped.
pub struct TestDir {
    pub path: PathBuf,
}

impl TestDir {
    pub fn new(name: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "melue-test-{name}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// A library in a temporary folder, with room for music folders next to it.
pub struct TestLibrary {
    pub library: Library,
    pub dir: TestDir,
}

impl TestLibrary {
    pub fn new(name: &str) -> Self {
        let dir = TestDir::new(name);
        let library = Library::open(&dir.path.join("library.db")).unwrap();
        Self { library, dir }
    }

    /// The same library opened again, as after an app restart.
    pub fn reopen(self) -> Self {
        let Self { library, dir } = self;
        drop(library);
        let library = Library::open(&dir.path.join("library.db")).unwrap();
        Self { library, dir }
    }

    /// A new, empty music folder next to the library.
    pub fn music_folder(&self, name: &str) -> MusicFolder {
        let root = self.dir.path.join(name);
        fs::create_dir_all(&root).unwrap();
        MusicFolder { root }
    }
}

/// Tags for a generated file. Unset fields aren't written.
#[derive(Debug, Clone, Default)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<u16>,
    pub genre: Option<String>,
    /// Embed the sample cover (testdata/cover.png).
    pub cover: bool,
}

impl Tags {
    /// The usual tags of an album track.
    pub fn song(title: &str, artist: &str, album: &str, track: u32) -> Self {
        Self {
            title: Some(title.into()),
            artist: Some(artist.into()),
            album: Some(album.into()),
            track: Some(track),
            ..Default::default()
        }
    }
}

/// A music folder of generated files: short silent WAVs with ID3 tags.
pub struct MusicFolder {
    pub root: PathBuf,
}

impl MusicFolder {
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// A WAV file at `relative` (folders are created) lasting `millis`, with `tags`.
    pub fn add(&self, relative: &str, tags: &Tags) -> PathBuf {
        let path = self.add_untagged(relative, 250);
        write_tags(&path, tags);
        path
    }

    /// A WAV file without any tags.
    pub fn add_untagged(&self, relative: &str, millis: u32) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, wav(millis)).unwrap();
        path
    }

    /// A copy of one of the real sample files (testdata/), e.g. "sample.flac".
    pub fn add_sample(&self, sample_name: &str, relative: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(sample(sample_name), &path).unwrap();
        path
    }

    /// A text file, e.g. an .m3u playlist.
    pub fn add_text(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    /// Changes a file's tags (and so its size), and makes sure its modified time moves on, as an
    /// edit in another program would.
    pub fn retag(&self, relative: &str, tags: &Tags) {
        let path = self.path(relative);
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        fs::write(&path, wav(300)).unwrap();
        write_tags(&path, tags);
        let file = fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(before + Duration::from_secs(2)).unwrap();
    }

    pub fn remove(&self, relative: &str) {
        fs::remove_file(self.path(relative)).unwrap();
    }

    pub fn rename(&self, from: &str, to: &str) {
        let to = self.path(to);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::rename(self.path(from), to).unwrap();
    }

    /// Sets a file's modified time, e.g. to make an older copy look newer.
    pub fn touch(&self, relative: &str, time: SystemTime) {
        let file = fs::File::options().write(true).open(self.path(relative)).unwrap();
        file.set_modified(time).unwrap();
    }
}

/// The path of a sample file in testdata/.
pub fn sample(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata").join(name)
}

/// A silent 8 kHz mono 16-bit WAV lasting `millis`.
fn wav(millis: u32) -> Vec<u8> {
    const RATE: u32 = 8_000;
    let frames = RATE * millis / 1000;
    let data_len = frames * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(44 + data_len as usize, 0);
    bytes
}

fn write_tags(path: &Path, tags: &Tags) {
    let mut tag = Tag::new(TagType::Id3v2);
    if let Some(v) = &tags.title {
        tag.set_title(v.clone());
    }
    if let Some(v) = &tags.artist {
        tag.set_artist(v.clone());
    }
    if let Some(v) = &tags.album {
        tag.set_album(v.clone());
    }
    if let Some(v) = &tags.album_artist {
        tag.insert_text(ItemKey::AlbumArtist, v.clone());
    }
    if let Some(v) = tags.track {
        tag.set_track(v);
    }
    if let Some(v) = tags.disc {
        tag.set_disk(v);
    }
    if let Some(year) = tags.year {
        tag.set_date(Timestamp { year, ..Default::default() });
    }
    if let Some(v) = &tags.genre {
        tag.set_genre(v.clone());
    }
    if tags.cover {
        let data = fs::read(sample("cover.png")).unwrap();
        tag.push_picture(Picture::unchecked(data).pic_type(PictureType::CoverFront).mime_type(MimeType::Png).build());
    }
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::migrations;

    // ---- the library itself

    #[test]
    fn a_new_library_is_at_the_newest_schema() {
        let t = TestLibrary::new("new");
        assert_eq!(t.library.schema_version().unwrap(), migrations::latest());
    }

    #[test]
    fn what_was_saved_is_there_after_reopening() {
        let t = TestLibrary::new("reopen");
        t.library.set_setting("theme", "dark").unwrap();
        t.library.set_setting("theme", "light").unwrap(); // replaces
        let t = t.reopen();
        assert_eq!(t.library.setting("theme").unwrap().as_deref(), Some("light"));
        assert_eq!(t.library.setting("missing").unwrap(), None);
    }

    #[test]
    fn a_library_from_a_newer_melue_is_refused() {
        let t = TestLibrary::new("newer");
        t.library.conn().pragma_update(None, "user_version", migrations::latest() + 1).unwrap();
        let path = t.dir.path.join("library.db");
        let TestLibrary { library, dir: _dir } = t;
        drop(library);
        let error = Library::open(&path).err().unwrap();
        assert!(error.contains("newer Melue"), "{error}");
    }

    // ---- the kit

    #[test]
    fn generated_files_carry_the_tags_asked_for() {
        let t = TestLibrary::new("kit-tags");
        let folder = t.music_folder("music");
        let tags = Tags {
            album_artist: Some("Various".into()),
            disc: Some(2),
            year: Some(1999),
            genre: Some("Pop".into()),
            cover: true,
            ..Tags::song("Song", "Artist", "Album", 7)
        };
        let path = folder.add("Artist/Album/07 Song.wav", &tags);
        let file = lofty::read_from_path(&path).unwrap();
        let tag = file.primary_tag().unwrap();
        assert_eq!(tag.title().as_deref(), Some("Song"));
        assert_eq!(tag.artist().as_deref(), Some("Artist"));
        assert_eq!(tag.album().as_deref(), Some("Album"));
        assert_eq!(tag.get_string(ItemKey::AlbumArtist), Some("Various"));
        assert_eq!((tag.track(), tag.disk()), (Some(7), Some(2)));
        assert_eq!(tag.date().map(|d| d.year), Some(1999));
        assert_eq!(tag.genre().as_deref(), Some("Pop"));
        assert_eq!(tag.pictures().len(), 1);
        assert!(file.properties().duration() >= Duration::from_millis(240));

        let bare = folder.add_untagged("loose.wav", 100);
        assert!(lofty::read_from_path(&bare).unwrap().primary_tag().is_none());
    }

    #[test]
    fn the_sample_files_have_their_tags() {
        for name in ["sample.mp3", "sample.flac", "sample.m4a"] {
            let file = lofty::read_from_path(sample(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            let tag = file.primary_tag().unwrap_or_else(|| panic!("{name}: no tag"));
            assert_eq!(tag.title().as_deref(), Some("Sample Song"), "{name}");
            assert_eq!(tag.artist().as_deref(), Some("Test Artist"), "{name}");
            assert_eq!(tag.album().as_deref(), Some("Test Album"), "{name}");
            assert_eq!(tag.get_string(ItemKey::AlbumArtist), Some("Test Album Artist"), "{name}");
            assert_eq!((tag.track(), tag.disk()), (Some(3), Some(1)), "{name}");
            assert_eq!(tag.pictures().len(), 1, "{name}: cover");
            assert!(file.properties().duration() >= Duration::from_millis(500), "{name}");
        }
    }

    #[test]
    fn folder_edits_change_what_a_scan_would_see() {
        let t = TestLibrary::new("kit-edits");
        let folder = t.music_folder("music");
        folder.add("a.wav", &Tags::song("A", "X", "Y", 1));
        let before = fs::metadata(folder.path("a.wav")).unwrap();
        folder.retag("a.wav", &Tags::song("A2", "X", "Y", 1));
        let after = fs::metadata(folder.path("a.wav")).unwrap();
        assert_ne!(before.len(), after.len());
        assert!(after.modified().unwrap() > before.modified().unwrap());
        assert_eq!(lofty::read_from_path(folder.path("a.wav")).unwrap().primary_tag().unwrap().title().as_deref(), Some("A2"));

        folder.rename("a.wav", "sub/b.wav");
        assert!(!folder.path("a.wav").exists() && folder.path("sub/b.wav").exists());
        folder.remove("sub/b.wav");
        assert!(!folder.path("sub/b.wav").exists());

        folder.add("c.wav", &Tags::default());
        let later = SystemTime::now() + Duration::from_secs(60);
        folder.touch("c.wav", later);
        assert_eq!(fs::metadata(folder.path("c.wav")).unwrap().modified().unwrap(), later);

        let playlist = folder.add_text("lists/mix.m3u", "#EXTM3U
c.wav
");
        assert_eq!(fs::read_to_string(playlist).unwrap(), "#EXTM3U
c.wav
");

        let copy = folder.add_sample("sample.flac", "s.flac");
        assert_eq!(fs::metadata(copy).unwrap().len(), fs::metadata(sample("sample.flac")).unwrap().len());
    }

    #[test]
    fn test_folders_are_cleaned_up() {
        let path = {
            let dir = TestDir::new("cleanup");
            fs::write(dir.path.join("x"), "x").unwrap();
            dir.path.clone()
        };
        assert!(!path.exists());
    }
}
