//! The second pass of a scan: reading the tags, sound details and cover of the songs the first pass found
//! new or changed (scan.rs marks them with `tags_read_at = NULL`), a batch at a time. The tags are kept as
//! the file has them; the grouping rules (step 6) build albums and artists from them.
//!
//! What the tags don't say comes from the file name when it can ("01 Artist - Title.mp3", see
//! grouping.rs). A file whose tags can't be read is named after its file (and still plays); it's tried
//! again when it changes. A file that can't be opened at all (a card taken out mid-scan) is tried again
//! next scan.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use lofty::file::FileType;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::tag::Tag;
use rusqlite::params;

use super::grouping::{file_name_of, parse_file_name, sort_key};
use super::scan::{title_from_name, ScanSummary};
use super::{err, now_ms, FolderKind, Library, Result};

/// Songs read per batch (each batch is saved in one go).
const BATCH: usize = 50;
/// Bigger cover images than this are left alone.
const MAX_COVER_FILE: u64 = 30 * 1024 * 1024;

/// A song whose tags are to be read, and where its file is.
#[derive(Debug, Clone)]
pub struct TagJob {
    pub track_id: i64,
    pub kind: FolderKind,
    /// The folder's location.
    pub location: String,
    pub path: String,
    pub folder_cover: Option<String>,
}

/// Opens a song's file, or a folder cover (`path` is in the form the track's path is).
pub type Opener<'a> = dyn Fn(&TagJob, &str) -> Result<File> + 'a;

/// Opens files on a disk: relative to the folder, or absolute (all music on an Android phone).
pub fn open_local(job: &TagJob, path: &str) -> Result<File> {
    let full: PathBuf = match job.kind {
        FolderKind::Path => Path::new(&job.location).join(path),
        _ => PathBuf::from(path),
    };
    File::open(&full).map_err(|e| format!("can't open {}: {e}", full.display()))
}

/// What a file's tags and sound details say. Empty texts count as missing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileTags {
    pub title: Option<String>,
    /// The artist as tagged; several artist values are joined with "; ".
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub compilation: bool,
    pub title_sort: Option<String>,
    pub artist_sort: Option<String>,
    pub album_sort: Option<String>,
    pub album_artist_sort: Option<String>,
    pub track_no: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_no: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub duration_ms: Option<i64>,
    /// mp3, flac, m4a, ogg, opus, wav, ...
    pub format: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: Option<u32>,
    /// The front cover (or else the first picture) in the file.
    pub picture: Option<Vec<u8>>,
}

fn text(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty()).map(String::from)
}

fn texts<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let values: Vec<&str> = values.map(str::trim).filter(|v| !v.is_empty()).collect();
    (!values.is_empty()).then(|| values.join("; "))
}

fn format_name(file_type: FileType) -> String {
    match file_type {
        FileType::Mpeg => "mp3".into(),
        FileType::Mp4 => "m4a".into(),
        FileType::Vorbis => "ogg".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Reads a file's tags, sound details and picture.
pub fn read_file(file: &mut File) -> Result<FileTags> {
    let tagged = lofty::read_from(file).map_err(err)?;
    let properties = tagged.properties();
    let mut tags = FileTags {
        duration_ms: Some(properties.duration().as_millis() as i64).filter(|ms| *ms > 0),
        format: Some(format_name(tagged.file_type())),
        bitrate_kbps: properties.audio_bitrate().or(properties.overall_bitrate()).filter(|b| *b > 0),
        sample_rate: properties.sample_rate().filter(|r| *r > 0),
        ..Default::default()
    };
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        read_tag(tag, &mut tags);
    }
    // The picture can be in another of the file's tags (an MP3 with both ID3v2 and APE, say).
    let pictures = || tagged.tags().iter().flat_map(Tag::pictures);
    tags.picture = pictures()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures().next())
        .map(|p| p.data().to_vec());
    Ok(tags)
}

fn read_tag(tag: &Tag, tags: &mut FileTags) {
    tags.title = text(tag.title().as_deref());
    tags.artist = texts(tag.get_strings(ItemKey::TrackArtist));
    tags.album = text(tag.album().as_deref());
    tags.album_artist = texts(tag.get_strings(ItemKey::AlbumArtist));
    tags.compilation = tag
        .get_string(ItemKey::FlagCompilation)
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"));
    tags.title_sort = text(tag.get_string(ItemKey::TrackTitleSortOrder));
    tags.artist_sort = text(tag.get_string(ItemKey::TrackArtistSortOrder));
    tags.album_sort = text(tag.get_string(ItemKey::AlbumTitleSortOrder));
    tags.album_artist_sort = text(tag.get_string(ItemKey::AlbumArtistSortOrder));
    tags.track_no = tag.track().filter(|n| *n > 0);
    tags.track_total = tag.track_total().filter(|n| *n > 0);
    tags.disc_no = tag.disk().filter(|n| *n > 0);
    tags.disc_total = tag.disk_total().filter(|n| *n > 0);
    tags.year = tag.date().map(|d| u32::from(d.year)).filter(|y| *y > 0);
    tags.genre = texts(tag.get_strings(ItemKey::Genre));
}

fn read_cover_file(open: &Opener, job: &TagJob, path: &str) -> Option<Vec<u8>> {
    let file = open(job, path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_COVER_FILE + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_COVER_FILE).then_some(bytes)
}

impl Library {
    /// How many songs still need their tags read.
    pub fn tags_to_read(&self) -> Result<u32> {
        self.conn()
            .query_row(
                "SELECT count(*) FROM tracks t JOIN folders f ON f.id = t.folder_id
                 WHERE t.tags_read_at IS NULL AND t.missing_since IS NULL AND f.enabled",
                [],
                |r| r.get(0),
            )
            .map_err(err)
    }

    /// The next songs whose tags are to be read, after track `after` (in track order).
    fn tag_jobs(&self, after: i64, limit: usize) -> Result<Vec<TagJob>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, f.kind, f.location, t.path, t.folder_cover
                 FROM tracks t JOIN folders f ON f.id = t.folder_id
                 WHERE t.tags_read_at IS NULL AND t.missing_since IS NULL AND f.enabled AND t.id > ?1
                 ORDER BY t.id LIMIT ?2",
            )
            .map_err(err)?;
        let jobs = stmt
            .query_map(params![after, limit as i64], |r| {
                Ok(TagJob {
                    track_id: r.get(0)?,
                    kind: FolderKind::parse(&r.get::<_, String>(1)?).unwrap_or(FolderKind::Path),
                    location: r.get(2)?,
                    path: r.get(3)?,
                    folder_cover: r.get(4)?,
                })
            })
            .map_err(err)?;
        jobs.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Reads the tags of every song that needs it, calling `progress(done, total)` after each batch.
    /// Returns how many were read and how many couldn't be.
    pub fn read_tags(&self, open: &Opener, progress: &mut dyn FnMut(u32, u32)) -> Result<ScanSummary> {
        let total = self.tags_to_read()?;
        let mut summary = ScanSummary::default();
        let mut after = 0;
        loop {
            let (batch, last) = self.read_tag_batch(open, after, BATCH)?;
            let Some(last) = last else { break };
            after = last;
            summary.add(batch);
            progress(summary.tags_read + summary.unreadable, total);
        }
        Ok(summary)
    }

    /// Reads one batch of up to `limit` songs after track `after`; returns what it did and the last track
    /// it looked at (None: nothing left).
    pub fn read_tag_batch(&self, open: &Opener, after: i64, limit: usize) -> Result<(ScanSummary, Option<i64>)> {
        let jobs = self.tag_jobs(after, limit)?;
        let mut summary = ScanSummary::default();
        let mut results = Vec::with_capacity(jobs.len());
        for job in &jobs {
            // Couldn't even open it (gone since the first pass, a card taken out): next scan.
            let Ok(mut file) = open(job, &job.path) else {
                summary.unreadable += 1;
                continue;
            };
            match read_file(&mut file) {
                Ok(mut tags) => {
                    let picture = tags.picture.take().or_else(|| {
                        job.folder_cover.as_deref().and_then(|cover| read_cover_file(open, job, cover))
                    });
                    let cover_id = match picture.map(|bytes| self.cover_for(&bytes)) {
                        Some(Ok(id)) => id,
                        Some(Err(e)) => {
                            eprintln!("melue: can't store a cover: {e}");
                            None
                        }
                        None => None,
                    };
                    results.push((job, tags, cover_id, None));
                    summary.tags_read += 1;
                }
                Err(e) => {
                    results.push((job, FileTags::default(), None, Some(e)));
                    summary.unreadable += 1;
                }
            }
        }
        self.save_tags(&results)?;
        Ok((summary, jobs.last().map(|j| j.track_id)))
    }

    fn save_tags(&self, results: &[(&TagJob, FileTags, Option<i64>, Option<String>)]) -> Result<()> {
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        {
            let mut stmt = tx
                .prepare(
                    "UPDATE tracks SET tags_read_at = ?2, tags_error = ?3, title = ?4, sort_title = ?5,
                         tag_title = ?6, artist_credit = ?7, tag_album = ?8, tag_album_artist = ?9, tag_compilation = ?10,
                         tag_title_sort = ?11, tag_artist_sort = ?12, tag_album_sort = ?13, tag_album_artist_sort = ?14,
                         track_no = ?15, track_total = ?16, disc_no = ?17, disc_total = ?18, year = ?19, genre = ?20,
                         duration_ms = ?21, format = COALESCE(?22, format), bitrate_kbps = ?23, sample_rate = ?24,
                         cover_id = ?25
                     WHERE id = ?1",
                )
                .map_err(err)?;
            for (job, tags, cover_id, error) in results {
                // What the file name says, for what the tags don't.
                let name = parse_file_name(&title_from_name(&file_name_of(&job.path)));
                let title = tags.title.clone().unwrap_or(name.title);
                let artist = tags.artist.clone().or(if tags.title.is_none() { name.artist } else { None });
                let track_no = tags.track_no.or(name.track);
                let disc_no = tags.disc_no.or(if tags.track_no.is_none() { name.disc } else { None });
                let sort_title = sort_key(tags.title_sort.as_deref().unwrap_or(&title));
                stmt.execute(params![
                    job.track_id,
                    now,
                    error,
                    title,
                    sort_title,
                    tags.title,
                    artist,
                    tags.album,
                    tags.album_artist,
                    tags.compilation,
                    tags.title_sort,
                    tags.artist_sort,
                    tags.album_sort,
                    tags.album_artist_sort,
                    track_no,
                    tags.track_total,
                    disc_no,
                    tags.disc_total,
                    tags.year,
                    tags.genre,
                    tags.duration_ms,
                    tags.format,
                    tags.bitrate_kbps,
                    tags.sample_rate,
                    cover_id,
                ])
                .map_err(err)?;
            }
        }
        tx.commit().map_err(err)
    }
}
