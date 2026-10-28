//! This device's small server for your other devices (they find its addresses and port in its presence file): every
//! request proves it comes from one of your devices (key.rs). On a computer, Windows asks once whether Melue may take
//! connections. Who's there (`/ping`), and playing on it (`/control`, control.rs). [`AppHost`] is this device as the
//! server sees it: its key, its player and its library.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::control::{self, Command, Host, Place, RemoteSong, RemoteState, SharedList, ASK_WAIT};
use tauri_plugin_native_audio::QueueItem;
use super::presence::{self, Device};
use super::{key, Allow};
use crate::library::{now_ms, Library};
use crate::sync::handoff::{self, Playback};
use tauri_plugin_native_audio::NativeAudioExt;
use crate::guard::Locked;

/// The port tried first (the same each time, so a firewall rule keeps working); another one when it's taken.
const PORT: u16 = 47821;
/// The header carrying a request's proof.
pub const PROOF: &str = "x-melue-proof";

/// The port the server listens on (0: not started).
static LISTENING: AtomicU16 = AtomicU16::new(0);

pub fn port() -> Option<u16> {
    Some(LISTENING.load(Ordering::SeqCst)).filter(|p| *p != 0)
}

/// What a device says about itself (`/ping`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub device_id: String,
    pub name: String,
    pub version: String,
    pub allow: Allow,
}

async fn ping(State(host): State<Arc<dyn Host>>, headers: HeaderMap) -> Result<Json<Hello>, StatusCode> {
    let key = host.key().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let proof = headers.get(PROOF).and_then(|v| v.to_str().ok()).ok_or(StatusCode::UNAUTHORIZED)?;
    key::check(&key, proof, "GET /ping", now_ms()).ok_or(StatusCode::UNAUTHORIZED)?;
    host.hello().map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// The server's routes, for `host`.
pub fn routes(host: Arc<dyn Host>) -> Router {
    Router::new().route("/ping", get(ping)).route("/control", get(control::control)).with_state(host)
}

/// Who plays on this device changed: the app gets them (their names).
pub const CONTROLLED_EVENT: &str = "remote-controlled";

/// The devices playing on this one now, by name.
fn controlled_by(app: &AppHandle) -> Vec<String> {
    let ids = control::controllers();
    if ids.is_empty() {
        return Vec::new();
    }
    let names = presence::all(app).unwrap_or_default();
    ids.iter().map(|id| names.iter().find(|d| d.id == *id).map_or_else(|| "another of your devices".into(), |d| d.name.clone())).collect()
}

/// The devices playing on this one now (Connect device shows them, to disconnect).
#[tauri::command(async)]
pub fn remote_controlled_by(app: AppHandle) -> Vec<String> {
    controlled_by(&app)
}

/// Disconnects the devices playing on this one: what plays here goes on, and they're told this device ended it.
#[tauri::command(async)]
pub fn remote_end_control(app: AppHandle) {
    let name = handoff::settings(&app).ok().and_then(|s| s.device_name).unwrap_or_else(|| "The other device".into());
    control::end_sessions(&format!("{name} ended the connection"));
}

/// A command from another device changed the player here: the app reads it again (its views follow).
pub const CHANGED_EVENT: &str = "remote-changed-here";

/// "Ask first": the question to the app (`remote-ask`, with its id and who asks), answered with `remote_answer`.
pub const ASK_EVENT: &str = "remote-ask";
static ASKS: Mutex<Option<HashMap<u64, std::sync::mpsc::Sender<bool>>>> = Mutex::new(None);
static NEXT_ASK: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asking {
    pub id: u64,
    /// The device that wants to play here (its name).
    pub from: String,
    /// How long the question stays (seconds).
    pub seconds: u64,
}

/// The answer to an "Ask first" question.
#[tauri::command]
pub fn remote_answer(id: u64, yes: bool) {
    if let Some(answer) = ASKS.locked().get_or_insert_with(HashMap::new).remove(&id) {
        let _ = answer.send(yes);
    }
}

/// This device, as its server sees it.
pub struct AppHost(pub AppHandle);

impl AppHost {
    /// The synced folder's songs: by id, their path on the share.
    fn paths(&self) -> Result<HashMap<i64, String>, String> {
        let library = self.0.state::<Library>();
        let folder = library.synced_folder()?.ok_or("no share is set up")?;
        Ok(library.synced_songs(&folder)?.into_iter().collect())
    }
}

impl Host for AppHost {
    fn key(&self) -> Result<Vec<u8>, String> {
        key::key(&self.0)
    }

    fn hello(&self) -> Result<Hello, String> {
        Ok(Hello {
            device_id: handoff::device_id(&self.0.state::<Library>())?,
            name: handoff::settings(&self.0)?.device_name.unwrap_or_default(),
            version: self.0.package_info().version.to_string(),
            allow: super::settings(&self.0)?.allow,
        })
    }

    fn state(&self) -> Result<RemoteState, String> {
        let player = self.0.native_audio();
        let state = player.get_state()?;
        let queue = player.get_queue()?;
        let paths = self.paths().unwrap_or_default();
        // A phone's media volume, in its steps; a computer's: Melue's own (smooth).
        let (volume, volume_levels) = self.phone_volume().unwrap_or((state.volume, 0));
        Ok(RemoteState {
            playing: state.is_playing,
            position: state.current_time,
            duration: state.duration,
            volume,
            volume_levels,
            shuffle: state.shuffle,
            repeat: state.repeat_mode,
            index: queue.current_index,
            play_order: queue.play_order,
            queue: queue
                .items
                .into_iter()
                .map(|item| RemoteSong { path: item.id.and_then(|id| paths.get(&id).cloned()), title: item.title, artist: item.artist })
                .collect(),
        })
    }

    fn connection(&self, from: &str, open: bool) {
        // Synced playback rests while another device plays here.
        if open {
            // One way at a time: this device stops playing on another.
            let name = presence::device(&self.0, from).ok().flatten().map_or_else(|| "Another of your devices".to_string(), |d| d.name);
            super::link::give_way(&self.0, &name);
            handoff::connection_began()
        } else {
            handoff::connection_ended()
        }
        // The app shows who plays here (Connect device: Disconnect it).
        let _ = self.0.emit(CONTROLLED_EVENT, controlled_by(&self.0));
    }

    fn ask(&self, from: &str) -> bool {
        let name = presence::device(&self.0, from).ok().flatten().map_or_else(|| "Another of your devices".to_string(), |d| d.name);
        let id = NEXT_ASK.fetch_add(1, Ordering::SeqCst);
        let (answer, answered) = std::sync::mpsc::channel();
        ASKS.locked().get_or_insert_with(HashMap::new).insert(id, answer);
        let _ = self.0.emit(ASK_EVENT, Asking { id, from: name, seconds: ASK_WAIT.as_secs() });
        let yes = answered.recv_timeout(ASK_WAIT).unwrap_or(false);
        ASKS.locked().get_or_insert_with(HashMap::new).remove(&id);
        yes
    }

    fn apply(&self, command: Command) -> Result<(), String> {
        // A like on the controller: the library's, not the player's (the app hears of it as of any like).
        if let Command::Like(like) = &command {
            return super::likes::apply(&self.0, like);
        }
        let applied = self.apply_here(command);
        // This device's own views follow (they hear of their own controls, not of these).
        let _ = self.0.emit(CHANGED_EVENT, ());
        applied
    }
}

impl AppHost {
    /// This device's copy of a list from the controller (as a play source, for the queue), or, when it hasn't got one,
    /// the controller's songs of it that are here.
    fn list_here(&self, list: &SharedList, songs: &[String]) -> Result<(Vec<QueueItem>, Option<String>), String> {
        let library = self.0.state::<Library>();
        let items = match library.shared_list_here(list)? {
            Some(source) => (library.queue_items(&source)?, Some(source)),
            None => (self.songs_here(songs)?, None),
        };
        if items.0.is_empty() {
            return Err("nothing in that list is on this device".into());
        }
        Ok(items)
    }

    /// The songs here for paths on the share (those not here left out).
    fn songs_here(&self, songs: &[String]) -> Result<Vec<QueueItem>, String> {
        let library = self.0.state::<Library>();
        let Some(folder) = library.synced_folder()? else { return Ok(Vec::new()) };
        let ids: HashMap<String, i64> = library.synced_songs(&folder)?.into_iter().map(|(id, p)| (handoff::key(&p), id)).collect();
        library.queue_items_for(&songs.iter().filter_map(|p| ids.get(&handoff::key(p)).copied()).collect::<Vec<_>>())
    }

    /// Whether the queue's songs at these places are the ones the controller means (it changed meanwhile: not).
    fn check_places(&self, places: &[Place]) -> Result<(), String> {
        let queue = self.0.native_audio().get_queue()?;
        let paths = self.paths()?;
        let same = places.iter().all(|place| {
            let here = queue.items.get(place.index).map(|item| item.id.and_then(|id| paths.get(&id)));
            match (here, &place.path) {
                (None, _) => false,
                (Some(here), Some(path)) => here.is_some_and(|h| handoff::key(h) == handoff::key(path)),
                (Some(_), None) => true,
            }
        });
        if same { Ok(()) } else { Err("Up next changed there meanwhile: have another look".into()) }
    }

    /// On a phone, the volume another device sees and sets is its media volume (what its buttons change; Melue's own
    /// volume there stays as it is): 0 to 1, and its levels. None elsewhere.
    fn phone_volume(&self) -> Option<(f64, u32)> {
        #[cfg(target_os = "android")]
        {
            use tauri_plugin_melue_android::MelueAndroidExt;
            let (step, max) = self.0.melue_android().media_volume().ok()?;
            Some((if max == 0 { 0.0 } else { f64::from(step) / f64::from(max) }, max))
        }
        #[cfg(not(target_os = "android"))]
        None
    }

    fn set_volume(&self, volume: f64) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            use tauri_plugin_melue_android::MelueAndroidExt;
            let android = self.0.melue_android();
            let (_, max) = android.media_volume()?;
            android.set_media_volume((volume * f64::from(max)).round() as u32)
        }
        #[cfg(not(target_os = "android"))]
        self.0.native_audio().set_volume(volume)
    }

    fn apply_here(&self, command: Command) -> Result<(), String> {
        let player = self.0.native_audio();
        match command {
            Command::Play => player.play(),
            Command::Pause => player.pause(),
            Command::Next => player.next(),
            Command::Previous => player.previous(),
            Command::Seek { position } => player.seek_to(position),
            Command::Volume { volume } => self.set_volume(volume.clamp(0.0, 1.0)),
            Command::Shuffle { on } => player.set_shuffle(on),
            Command::Repeat { mode } => player.set_repeat_mode(&mode),
            Command::SkipTo { index } => player.skip_to(index),
            Command::Like(_) => Ok(()),
            Command::PlayList { list, songs, start } => {
                let (items, source) = self.list_here(&list, &songs)?;
                let index = match start {
                    Some(start) => {
                        let paths = self.paths()?;
                        let wanted = handoff::key(&start);
                        items
                            .iter()
                            .position(|item| item.id.and_then(|id| paths.get(&id)).is_some_and(|p| handoff::key(p) == wanted))
                            .ok_or("that song isn’t on this device (not synced yet?)")?
                    }
                    // Shuffle on here: a random song first, the rest shuffled after it.
                    None if player.get_state()?.shuffle => (crate::library::now_ms() as usize) % items.len(),
                    None => 0,
                };
                player.set_queue(items, index, 0.0, source)?;
                player.play()
            }
            Command::Add { songs, list, next } => {
                let items = match list {
                    Some(list) => self.list_here(&list, &songs)?.0,
                    None => self.songs_here(&songs)?,
                };
                if items.is_empty() {
                    return Err(if songs.len() == 1 { "that song isn’t on this device (not synced yet?)" } else { "those songs aren’t on this device (not synced yet?)" }.into());
                }
                player.add_to_queue(items, next)
            }
            Command::Remove { mut places } => {
                self.check_places(&places)?;
                // From the end, so the places before stay right.
                places.sort_by(|a, b| b.index.cmp(&a.index));
                places.iter().try_for_each(|place| player.remove_from_queue(place.index))
            }
            Command::Jump { index, path } => {
                self.check_places(&[Place { index, path }])?;
                let queue = player.get_queue()?;
                let order: Vec<i64> = if queue.play_order.is_empty() { (0..queue.items.len() as i64).collect() } else { queue.play_order };
                let place = order.iter().position(|&i| i == index as i64).ok_or("that song isn't in the queue")?;
                let start = order.iter().position(|&i| i == queue.current_index).map_or(0, |p| p + 1);
                if place > start {
                    player.move_in_play_order(place, start)?;
                }
                player.next()?;
                player.play()
            }
            Command::Move { from, to, path } => {
                let queue = player.get_queue()?;
                let index = queue.play_order.get(from).map_or(from, |&i| i as usize);
                self.check_places(&[Place { index, path }])?;
                player.move_in_play_order(from, to)
            }
            Command::Load { queue, index, position, playing } => {
                // The songs here, by their paths on the share (as Continue does).
                let library = self.0.state::<Library>();
                let folder = library.synced_folder()?.ok_or("no share is set up here")?;
                let wanted = Playback { queue, index, position_s: position, playing, repeat: "off".into(), ..Default::default() };
                let (items, index) = handoff::queue_here(&library, &folder, &wanted)?.ok_or("that song isn’t on this device (not synced yet?)")?;
                player.set_queue(items, index, position, None)?;
                if playing {
                    player.play()?;
                }
                Ok(())
            }
        }
    }
}

/// Starts the server (setup), on all of this device's networks.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT))).await {
            Ok(listener) => listener,
            Err(_) => match tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await {
                Ok(listener) => listener,
                Err(e) => return eprintln!("melue: remote: can't take connections: {e}"),
            },
        };
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
        LISTENING.store(port, Ordering::SeqCst);
        eprintln!("melue: remote: taking connections from your devices on port {port}");
        // The others hear where to find it.
        presence::announce_soon();
        let routes = routes(Arc::new(AppHost(app)));
        if let Err(e) = axum::serve(listener, routes).await {
            eprintln!("melue: remote: stopped taking connections: {e}");
        }
    });
}

/// How a device was reached: at which address, how long it took, and what it said.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reached {
    pub address: String,
    pub ms: u64,
    pub hello: Hello,
}

/// Reaches one of your devices directly: all its addresses at once (home network, VPN), the first to answer wins (a
/// device that isn't running Melue doesn't answer).
pub fn reach(app: &AppHandle, device: &Device) -> Result<Reached, String> {
    let port = device.port.ok_or_else(|| format!("{} doesn’t take connections (an older Melue?)", device.name))?;
    let key = key::key(app)?;
    let me = handoff::device_id(&app.state::<Library>())?;
    if device.addresses.is_empty() {
        return Err(format!("{} has no address to reach it at", device.name));
    }
    let (answers, answered) = std::sync::mpsc::channel::<Result<Reached, String>>();
    for address in device.addresses.clone() {
        let (answers, key, me) = (answers.clone(), key.clone(), me.clone());
        std::thread::spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_millis(2500)))
                .http_status_as_error(false)
                .build()
                .into();
            let started = std::time::Instant::now();
            let answer = agent.get(&format!("http://{address}:{port}/ping")).header(PROOF, &key::proof(&key, &me, now_ms(), "GET /ping")).call();
            let _ = answers.send(match answer {
                Ok(mut answer) if answer.status() == 200 => answer
                    .body_mut()
                    .read_json::<Hello>()
                    .map(|hello| Reached { address: address.clone(), ms: started.elapsed().as_millis() as u64, hello })
                    .map_err(|e| format!("{address}: {e}")),
                Ok(answer) if answer.status() == 401 => Err(format!("{address}: not one of your devices (another share's key)")),
                Ok(answer) => Err(format!("{address}: answered {}", answer.status())),
                Err(e) => Err(format!("{address}: {e}")),
            });
        });
    }
    drop(answers);
    let mut why = Vec::new();
    for answer in answered {
        match answer {
            Ok(reached) => return Ok(reached),
            Err(e) => why.push(e),
        }
    }
    // The details for the log; the reason, in words, for the app.
    eprintln!("melue: can't reach {}: {}", device.name, why.join("; "));
    if let Some(key) = why.iter().find(|w| w.contains("another share")) {
        return Err(format!("{}: {}", device.name, key.split_once(": ").map_or(key.as_str(), |(_, w)| w)));
    }
    Err(if device.online {
        // Online (it says so on the share), yet nothing answers: something between blocks Melue's connections.
        format!("{} is online, but can’t be reached from here: a firewall may be blocking Melue there (on Windows: allow Melue in Windows Firewall)", device.name)
    } else {
        format!("{} isn’t running Melue right now", device.name)
    })
}

/// Reaches one of your devices (the dev page's test).
#[tauri::command(async)]
pub fn remote_reach(app: AppHandle, id: String) -> Result<Reached, String> {
    let device = presence::device(&app, &id)?.ok_or("no such device")?;
    reach(&app, &device)
}
