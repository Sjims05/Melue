//! Auto playlists: lists Melue makes from the library, shown apart from the user's playlists (and at the top of
//! Android Auto's Playlists tab). In this order:
//! - "songs": All songs;
//! - "liked": Liked songs;
//! - "top:<period>": Top songs of the last month, half year, year (rolling: the last 30, 182, 365 days, from the
//!   plays per day) and all time (from track_stats), with the other devices' plays (sync/data.rs), each with its own size (25, 50, 100 or 200; default 50);
//! - "folder:<id>": one per music folder, by path (none when there's only one folder: All songs is the same).
//!
//! Any of them can be hidden, and shown again in the settings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::playback::DAY_MS;
use super::{err, now_ms, Library, Result};

const SETTING: &str = "auto_playlists";
pub const SIZES: [u32; 4] = [25, 50, 100, 200];
pub const DEFAULT_SIZE: u32 = 50;

/// The Top songs lists' periods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopPeriod {
    Month,
    HalfYear,
    Year,
    AllTime,
}

impl TopPeriod {
    pub const ALL: [TopPeriod; 4] = [Self::Month, Self::HalfYear, Self::Year, Self::AllTime];

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == value)
    }

    fn id(self) -> &'static str {
        match self {
            Self::Month => "month",
            Self::HalfYear => "half-year",
            Self::Year => "year",
            Self::AllTime => "all-time",
        }
    }

    pub fn source(self) -> String {
        format!("top:{}", self.id())
    }

    fn name(self) -> &'static str {
        match self {
            Self::Month => "Top songs: last month",
            Self::HalfYear => "Top songs: last 6 months",
            Self::Year => "Top songs: last year",
            Self::AllTime => "Top songs: all time",
        }
    }

    /// How many days back it counts (None: all time).
    pub fn days(self) -> Option<i64> {
        match self {
            Self::Month => Some(30),
            Self::HalfYear => Some(182),
            Self::Year => Some(365),
            Self::AllTime => None,
        }
    }
}

/// The auto playlists' settings: the Top songs lists' sizes, and the hidden lists.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoPlaylistSettings {
    /// By source ("top:month"); left out: [`DEFAULT_SIZE`].
    pub sizes: BTreeMap<String, u32>,
    /// Sources hidden.
    pub hidden: Vec<String>,
}

impl AutoPlaylistSettings {
    pub fn size(&self, period: TopPeriod) -> u32 {
        self.sizes.get(&period.source()).copied().unwrap_or(DEFAULT_SIZE)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoKind {
    Songs,
    Liked,
    Top,
    Folder,
}

/// An auto playlist, for the UI and Android Auto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoPlaylist {
    /// The list to play ("songs", "liked", "top:month", "folder:3").
    pub source: String,
    pub name: String,
    pub kind: AutoKind,
    /// Songs in it now.
    pub songs: i64,
    /// Top songs: how many it holds.
    pub size: Option<u32>,
    pub hidden: bool,
}

/// Today, in days since 1970 (UTC), like play_days.
fn today() -> i64 {
    now_ms().div_euclid(DAY_MS)
}

impl Library {
    pub fn auto_playlist_settings(&self) -> Result<AutoPlaylistSettings> {
        match self.setting(SETTING)? {
            Some(json) => serde_json::from_str(&json).map_err(err),
            None => Ok(AutoPlaylistSettings::default()),
        }
    }

    fn save_auto_playlist_settings(&self, settings: &AutoPlaylistSettings) -> Result<()> {
        self.set_setting(SETTING, &serde_json::to_string(settings).map_err(err)?)
    }

    /// Changes a Top songs list's size (25, 50, 100 or 200).
    pub fn set_auto_playlist_size(&self, source: &str, size: u32) -> Result<()> {
        if source.strip_prefix("top:").and_then(TopPeriod::parse).is_none() {
            return Err(format!("only the Top songs lists have a size, not {source}"));
        }
        if !SIZES.contains(&size) {
            return Err(format!("the size must be one of {SIZES:?}"));
        }
        let mut settings = self.auto_playlist_settings()?;
        settings.sizes.insert(source.to_string(), size);
        self.save_auto_playlist_settings(&settings)
    }

    /// Hides an auto playlist, or shows it again.
    pub fn set_auto_playlist_hidden(&self, source: &str, hidden: bool) -> Result<()> {
        let mut settings = self.auto_playlist_settings()?;
        settings.hidden.retain(|s| s != source);
        if hidden {
            settings.hidden.push(source.to_string());
        }
        self.save_auto_playlist_settings(&settings)
    }

    /// The most played songs that are there (ids, most first; the same play count: the latest played first).
    pub fn top_songs(&self, period: TopPeriod) -> Result<Vec<i64>> {
        let size = self.auto_playlist_settings()?.size(period);
        self.top_songs_on(period, size, today())
    }

    /// [`top_songs`](Self::top_songs) as of the day `today`.
    pub fn top_songs_on(&self, period: TopPeriod, size: u32, today: i64) -> Result<Vec<i64>> {
        let present = "JOIN tracks t ON t.id = s.track_id JOIN folders f ON f.id = t.folder_id
                       WHERE t.missing_since IS NULL AND f.enabled";
        // All time: the per-song counts. A period: the plays per day since its first day (?2).
        let sql = match period.days() {
            None => format!(
                "SELECT s.track_id FROM all_track_stats s {present} AND s.play_count > 0 AND ?2 IS NOT NULL
                 ORDER BY s.play_count DESC, s.last_played_at DESC, s.track_id LIMIT ?1"
            ),
            Some(_) => format!(
                "SELECT s.track_id FROM all_play_days s {present} AND s.day > ?2
                 GROUP BY s.track_id ORDER BY sum(s.plays) DESC, max(s.day) DESC, s.track_id LIMIT ?1"
            ),
        };
        let since = today - period.days().unwrap_or(0);
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let ids = stmt.query_map([i64::from(size), since], |r| r.get(0)).map_err(err)?;
        ids.collect::<rusqlite::Result<_>>().map_err(err)
    }

    /// Every auto playlist, in order, hidden ones too (marked).
    pub fn auto_playlists(&self) -> Result<Vec<AutoPlaylist>> {
        let settings = self.auto_playlist_settings()?;
        let list = |source: String, name: &str, kind, songs, size| AutoPlaylist {
            hidden: settings.hidden.contains(&source),
            source,
            name: name.to_string(),
            kind,
            songs,
            size,
        };
        let liked: i64 = self
            .conn()
            .query_row(
                "SELECT count(*) FROM likes l JOIN tracks t ON t.id = l.track_id JOIN folders f ON f.id = t.folder_id
                 WHERE t.missing_since IS NULL AND f.enabled",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let mut lists = vec![
            list("songs".into(), "All songs", AutoKind::Songs, self.status()?.tracks, None),
            list("liked".into(), "Liked songs", AutoKind::Liked, liked, None),
        ];
        for period in TopPeriod::ALL {
            let songs = self.top_songs(period)?.len() as i64;
            lists.push(list(period.source(), period.name(), AutoKind::Top, songs, Some(settings.size(period))));
        }
        let folders: Vec<_> = self.folders()?.into_iter().filter(|f| f.enabled).collect();
        if folders.len() > 1 {
            for folder in folders {
                lists.push(list(format!("folder:{}", folder.id), &folder.name, AutoKind::Folder, folder.tracks, None));
            }
        }
        Ok(lists)
    }
}
