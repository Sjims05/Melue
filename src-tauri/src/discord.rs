//! Discord (Settings: Connections; computers): "Listening to Melue" in your Discord status, through the Discord app on
//! the same computer (its local IPC: no sign-in). The song, its artist, a progress bar, and its cover: Discord only
//! shows pictures from the internet, so the album's is looked up in MusicBrainz's Cover Art Archive (else Melue's
//! icon). Hovering it can show a weekly line ("This week: 312 songs · most played ABBA"), its own setting. Paused or
//! stopped: the status goes. Playing on another device (Connect device): that device's song.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::library::Library;
use crate::guard::Locked;

#[cfg(desktop)]
pub use show::start;

/// The settings (a library setting, shared across devices).
const SETTINGS: &str = "discord_settings";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiscordSettings {
    /// Show what plays in your Discord status (default off).
    pub enabled: bool,
    /// Hovering the cover: this week's listening.
    pub weekly: bool,
}

pub fn settings(library: &Library) -> DiscordSettings {
    library.setting(SETTINGS).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command(async)]
pub fn discord_settings(app: AppHandle) -> DiscordSettings {
    settings(&app.state::<Library>())
}

#[tauri::command(async)]
pub fn discord_set_settings(app: AppHandle, settings: DiscordSettings) -> Result<DiscordSettings, String> {
    app.state::<Library>().set_setting(SETTINGS, &serde_json::to_string(&settings).map_err(|e| e.to_string())?)?;
    Ok(settings)
}

/// Whether the status shows now (Connections says so): Discord found and showing, or not.
static SHOWING: Mutex<Option<bool>> = Mutex::new(None);

/// Discord's state for Connections: "off" (the setting), "showing", or "noDiscord" (Discord isn't running).
#[tauri::command]
pub fn discord_status(app: AppHandle) -> &'static str {
    if !settings(&app.state::<Library>()).enabled {
        return "off";
    }
    match *SHOWING.locked() {
        Some(false) => "noDiscord",
        _ => "showing",
    }
}

impl Library {
    /// This week's listening (the last 7 days, all your devices): how many plays, and the artist played most. Counted
    /// as the Top songs lists count (plays per day: a song played, not every start or skip). Discord's: a computer's.
    #[cfg(desktop)]
    pub fn weekly_listening(&self) -> Result<(i64, Option<String>), String> {
        let since = crate::library::now_ms().div_euclid(86_400_000) - 7;
        let conn = self.conn();
        let plays: i64 = conn.query_row("SELECT coalesce(sum(plays), 0) FROM all_play_days WHERE day > ?1", [since], |r| r.get(0)).map_err(|e| e.to_string())?;
        let top: Option<String> = conn
            .query_row(
                "SELECT t.artist_credit FROM all_play_days d JOIN tracks t ON t.id = d.track_id
                 WHERE d.day > ?1 AND t.artist_credit IS NOT NULL GROUP BY t.artist_credit ORDER BY sum(d.plays) DESC LIMIT 1",
                [since],
                |r| r.get(0),
            )
            .ok();
        Ok((plays, top))
    }
}

/// Showing it in Discord (computers: Discord's app talks to apps on the same computer).
#[cfg(desktop)]
mod show {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use discord_rich_presence::activity::{Activity, ActivityType, Assets, StatusDisplayType, Timestamps};
    use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
    use serde::Deserialize;
    use tauri::{AppHandle, Manager};
    use tauri_plugin_native_audio::NativeAudioExt;

    use super::{settings, SHOWING};
    use crate::guard::Locked;
    use crate::library::{now_ms, Library};

    /// Melue's application on Discord (public: it names the app, "Listening to Melue").
    const APPLICATION_ID: &str = "1563520894733787176";
    /// Melue's icon, when there's no cover.
    const ICON: &str = "https://raw.githubusercontent.com/Sjims05/Melue/main/src-tauri/icons/128x128@2x.png";
    /// How often what plays is looked at.
    const LOOK_EVERY: Duration = Duration::from_secs(2);
    /// Discord not running (or it went): tried again this often.
    const RETRY_EVERY: Duration = Duration::from_secs(15);
    /// How often the weekly line is worked out again.
    const WEEKLY_EVERY: Duration = Duration::from_secs(5 * 60);

    /// What plays, as the status shows it.
    #[derive(Debug, Clone, PartialEq)]
    struct Playing {
        title: String,
        artist: Option<String>,
        album: Option<String>,
        /// Seconds into the song, and its length.
        position: f64,
        duration: f64,
    }

    /// What plays now (on another device, when playing there), or None (paused, stopped).
    /// The song last found in the queue: its id and what's shown of it (the queue is fetched again only for another song).
    type Known = Option<(Option<i64>, String, Option<String>, Option<String>)>;

    fn playing(app: &AppHandle, known: &mut Known) -> Option<Playing> {
        // A song's album, from the library (the player doesn't keep it): for its cover.
        let album_of = |id: Option<i64>| id.and_then(|id| app.state::<Library>().queue_items_for(&[id]).ok()?.into_iter().next()?.album);
        if let Some((_, state)) = crate::remote::link::remote_status(app.clone()).and_then(|(c, s)| Some((c, s?))) {
            if !state.playing {
                return None;
            }
            let song = usize::try_from(state.index).ok().and_then(|i| state.queue.get(i))?;
            // Its copy here (when it's here), for the album.
            let here = song.path.clone().and_then(|path| crate::remote::link::remote_song_ids(app.clone(), vec![Some(path)]).ok()?.into_iter().next().flatten());
            return Some(Playing { title: song.title.clone()?, artist: song.artist.clone(), album: album_of(here), position: state.position, duration: state.duration });
        }
        let player = app.native_audio();
        let state = player.get_state().ok()?;
        if !state.is_playing {
            return None;
        }
        if !known.as_ref().is_some_and(|(id, ..)| *id == state.current_id) {
            let queue = player.get_queue().ok()?;
            let item = usize::try_from(queue.current_index).ok().and_then(|i| queue.items.get(i))?;
            let album = item.album.clone().or_else(|| album_of(item.id));
            *known = Some((state.current_id, item.title.clone()?, item.artist.clone(), album));
        }
        let (_, title, artist, album) = known.clone()?;
        Some(Playing { title, artist, album, position: state.current_time, duration: state.duration })
    }

    /// The album's cover on the internet (MusicBrainz's Cover Art Archive), looked up once per album.
    fn cover(artist: &str, album: &str, known: &mut HashMap<(String, String), Option<String>>) -> Option<String> {
        let key = (artist.to_lowercase(), album.to_lowercase());
        if let Some(found) = known.get(&key) {
            return found.clone();
        }
        let found = look_up_cover(artist, album);
        known.insert(key, found.clone());
        found
    }

    fn look_up_cover(artist: &str, album: &str) -> Option<String> {
        // MusicBrainz asks that each app names itself.
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .max_redirects(0)
            .http_status_as_error(false)
            .user_agent(concat!("Melue/", env!("CARGO_PKG_VERSION"), " ( https://github.com/Sjims05/Melue )"))
            .build()
            .into();
        let quoted = |s: &str| s.replace('"', "");
        let query = format!("releasegroup:\"{}\" AND artist:\"{}\"", quoted(album), quoted(artist));
        #[derive(Deserialize)]
        struct Groups {
            #[serde(rename = "release-groups")]
            groups: Vec<Group>,
        }
        #[derive(Deserialize)]
        struct Group {
            id: String,
            score: Option<u32>,
        }
        let mut answer = agent.get("https://musicbrainz.org/ws/2/release-group/").query("query", &query).query("fmt", "json").query("limit", "1").call().ok()?;
        let group = answer.body_mut().read_json::<Groups>().ok()?.groups.into_iter().next().filter(|g| g.score.unwrap_or(0) >= 90)?;
        // The archive answers with where the picture is (a redirect), or 404 without one.
        let url = format!("https://coverartarchive.org/release-group/{}/front-250", group.id);
        let answer = agent.head(&url).call().ok()?;
        match answer.status().as_u16() {
            200 => Some(url),
            300..=399 => answer.headers().get("location").and_then(|l| l.to_str().ok()).map(str::to_string).or(Some(url)),
            _ => None,
        }
    }

    /// "This week: 312 songs · most played ABBA".
    fn weekly_line(library: &Library) -> Option<String> {
        let (plays, top) = library.weekly_listening().ok()?;
        let songs = if plays == 1 { "1 song".to_string() } else { format!("{plays} songs") };
        Some(match top {
            Some(artist) => format!("This week: {songs} · most played {artist}"),
            None => format!("This week: {songs}"),
        })
    }

    /// Starts following what plays (computers): the status follows the setting and the player.
    pub fn start(app: &AppHandle) {
        let app = app.clone();
        let _ = std::thread::Builder::new().name("melue-discord".into()).spawn(move || crate::guard::keep_running("Discord", move || {
            let mut client: Option<DiscordIpcClient> = None;
            let mut shown: Option<(Playing, Option<String>)> = None;
            let mut covers = HashMap::new();
            let mut retry_at: Option<Instant> = None;
            let mut known: Known = None;
            // The weekly line, worked out again every few minutes (it's a count over the week's plays).
            let mut weekly_at: Option<(Instant, Option<String>)> = None;
            loop {
                std::thread::sleep(LOOK_EVERY);
                let library = app.state::<Library>();
                let settings = settings(&library);
                let now = if settings.enabled { playing(&app, &mut known) } else { None };
                let Some(now) = now else {
                    // Nothing to show: the status goes (and, switched off, the connection to Discord too).
                    if let Some(c) = client.as_mut() {
                        if shown.take().is_some() {
                            let _ = c.clear_activity();
                        }
                        if !settings.enabled {
                            let _ = c.close();
                            client = None;
                            *SHOWING.locked() = None;
                        }
                    }
                    continue;
                };
                let weekly = if settings.weekly {
                    if !weekly_at.as_ref().is_some_and(|(at, _)| at.elapsed() < WEEKLY_EVERY) {
                        weekly_at = Some((Instant::now(), weekly_line(&library)));
                    }
                    weekly_at.as_ref().and_then(|(_, line)| line.clone())
                } else {
                    None
                };
                // The same song, going on as expected (the bar moves by itself): nothing to tell Discord.
                if let Some((was, was_weekly)) = &shown {
                    let same = was.title == now.title && was.artist == now.artist && (was.position - now.position).abs() < 3.0 + LOOK_EVERY.as_secs_f64() * 2.0;
                    if same && *was_weekly == weekly {
                        shown = Some((Playing { position: now.position, ..was.clone() }, weekly));
                        continue;
                    }
                }
                if client.is_none() {
                    if retry_at.is_some_and(|at| Instant::now() < at) {
                        continue;
                    }
                    let mut c = DiscordIpcClient::new(APPLICATION_ID);
                    if c.connect().is_err() {
                        retry_at = Some(Instant::now() + RETRY_EVERY);
                        *SHOWING.locked() = Some(false);
                        continue;
                    }
                    client = Some(c);
                    retry_at = None;
                }
                let picture = match (&now.artist, &now.album) {
                    (Some(artist), Some(album)) => cover(artist, album, &mut covers),
                    _ => None,
                };
                let start = now_ms() - (now.position * 1000.0) as i64;
                let mut assets = Assets::new().large_image(picture.as_deref().unwrap_or(ICON));
                let hover = weekly.clone().or_else(|| now.album.clone());
                if let Some(text) = hover.as_deref() {
                    assets = assets.large_text(text);
                }
                let mut activity = Activity::new()
                    .activity_type(ActivityType::Listening)
                    // The member list's line: the song's title, not "Melue".
                .status_display_type(StatusDisplayType::Details)
                    .details(now.title.as_str())
                    .assets(assets);
                if let Some(artist) = now.artist.as_deref() {
                    activity = activity.state(artist);
                }
                if now.duration > 0.0 {
                    activity = activity.timestamps(Timestamps::new().start(start).end(start + (now.duration * 1000.0) as i64));
                }
                match client.as_mut().map(|c| c.set_activity(activity)) {
                    Some(Ok(())) => {
                        shown = Some((now, weekly));
                        *SHOWING.locked() = Some(true);
                    }
                    // Discord went (closed, restarted): connect again later.
                    _ => {
                        client = None;
                        shown = None;
                        retry_at = Some(Instant::now() + RETRY_EVERY);
                        *SHOWING.locked() = Some(false);
                    }
                }
            }
        }));
    }
}
