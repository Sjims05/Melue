//! Playing on another device: the device in your hand (the controller) holds a WebSocket to the one playing (its
//! server's `/control`), sends it commands, and gets its state back after each command and whenever it changes. A
//! queue goes by the songs' paths on the share: each device plays its own copies (a song the other device doesn't have
//! is left out there).
//!
//! The server side talks to its player through [`Host`], so the whole exchange can be tested without one.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use super::key;
use super::server::{Hello, PROOF};
use super::Allow;
pub use crate::library::shared_lists::SharedList;
use crate::library::now_ms;

/// What the controller asks of the device playing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Command {
    Play,
    Pause,
    Next,
    Previous,
    /// Seconds into the song.
    Seek { position: f64 },
    /// 0 to 1.
    Volume { volume: f64 },
    Shuffle { on: bool },
    /// "off", "all" or "one".
    Repeat { mode: String },
    /// The queue's song at this place.
    SkipTo { index: usize },
    /// A queue to play there: the songs' paths on the share, the song to start on, seconds into it, playing or paused.
    Load { queue: Vec<String>, index: usize, position: f64, playing: bool },
    /// A song was liked or unliked on the controller: the same there at once (remote/likes.rs).
    Like(Like),
    /// A list to play there: its own copy of `list` (its songs only there too), or, when it hasn't got one (a
    /// playlist not synced there yet), `songs` (the controller's songs of it on the share). From song `start` (a
    /// path), or the first (a random one with shuffle on there).
    PlayList { list: SharedList, songs: Vec<String>, start: Option<String> },
    /// Songs (paths on the share), or a list (as for PlayList), into Up next there: right after the playing song
    /// (`next`) or at the end.
    Add { songs: Vec<String>, list: Option<SharedList>, next: bool },
    /// Up next there: these songs out of the queue.
    Remove { places: Vec<Place> },
    /// Up next there: its song at `index` (of the queue's list; `path`: which song) plays now, the queue staying as it
    /// was (moved to the front of Up next, then on to it, in one go: one state comes back).
    Jump { index: usize, path: Option<String> },
    /// Up next there: the song at place `from` of the play order to place `to` (`path`: which song that is).
    Move { from: usize, to: usize, path: Option<String> },
}

/// A song in the playing device's queue, as the controller sees it: its place in the queue's list, and which song
/// that is (its path on the share; None for a song only there), so a queue that changed meanwhile isn't edited
/// in the wrong place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub index: usize,
    pub path: Option<String>,
}

/// A song liked or unliked (`liked`) on one device, for the other: by its path on the share, and when (ms).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Like {
    pub path: String,
    pub liked: bool,
    pub at: i64,
}

/// A song in the playing device's queue: its path on the share (to find it on the controller too), and what it is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSong {
    pub path: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
}

/// The playing device's state.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteState {
    pub playing: bool,
    /// Seconds into the song, when this was sent (the controller counts on while it plays).
    pub position: f64,
    pub duration: f64,
    /// 0 to 1. With `volume_levels`, that many levels' worth (a phone's media volume).
    pub volume: f64,
    /// A phone: how many levels its media volume has (the controller's slider steps through them: "Volume steps on a
    /// phone"). 0: smooth (a computer).
    #[serde(default)]
    pub volume_levels: u32,
    pub shuffle: bool,
    pub repeat: String,
    /// The song in `queue` (-1: nothing loaded).
    pub index: i64,
    pub queue: Vec<RemoteSong>,
    /// The order the queue plays in (places in `queue`; shuffled, it differs).
    #[serde(default)]
    pub play_order: Vec<i64>,
}

/// What the playing device sends: its state, or why a command didn't work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outgoing {
    State(RemoteState),
    Error(String),
    /// A song was liked or unliked on the playing device (remote/likes.rs).
    Liked(Like),
}

/// The device playing, as its server sees it.
pub trait Host: Send + Sync + 'static {
    fn key(&self) -> Result<Vec<u8>, String>;
    fn hello(&self) -> Result<Hello, String>;
    fn state(&self) -> Result<RemoteState, String>;
    fn apply(&self, command: Command) -> Result<(), String>;
    /// "Ask first": whether someone here lets device `from` play here (waits for the answer, a while at most).
    fn ask(&self, from: &str) -> bool;
    /// Device `from` connected to control this one (`open`), or its connection ended.
    fn connection(&self, from: &str, open: bool);
}

/// How a controller's connection ended (its `on_closed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closed {
    /// Closed from the controller's side (the link dropped: Disconnect, This device, another device).
    Disconnected,
    /// The other device ended it (Disconnect there, another device playing there), saying why.
    Ended(String),
    /// Lost: the other device stopped answering, or the network failed (the controller tries again: link.rs).
    Lost(String),
}

/// A controller lost lately (by its device id): when, so it can come back without being asked again (Ask first).
static LOST: std::sync::Mutex<Vec<(String, Instant)>> = std::sync::Mutex::new(Vec::new());
/// How long a lost controller may come back without asking (the controller tries for a minute: link.rs).
const COME_BACK_WITHIN: Duration = Duration::from_secs(90);

/// Whether device `from` is coming back: connected here now (the old connection not noticed as gone yet), or lost
/// lately. Taken off the lost list.
fn coming_back(from: &str) -> bool {
    let mut lost = LOST.lock().unwrap();
    lost.retain(|(_, at)| at.elapsed() < COME_BACK_WITHIN);
    let was_lost = lost.iter().any(|(id, _)| id == from);
    lost.retain(|(id, _)| id != from);
    was_lost || SESSIONS.lock().unwrap().iter().any(|(_, id, _)| id == from)
}

/// For a controller's connection, from elsewhere on this device.
enum ToSession {
    /// End it, telling the controller why.
    End(String),
    /// Tell the controller (a like here).
    Tell(Outgoing),
}

/// The controllers connected here now: each one's device, and a way to reach its connection.
static SESSIONS: std::sync::Mutex<Vec<(u64, String, tokio::sync::mpsc::UnboundedSender<ToSession>)>> = std::sync::Mutex::new(Vec::new());
static NEXT_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The devices controlling this one now (their ids).
pub fn controllers() -> Vec<String> {
    SESSIONS.lock().unwrap().iter().map(|(_, from, _)| from.clone()).collect()
}

/// Ends every controller's connection from here (`why` goes to them); what plays here goes on.
pub fn end_sessions(why: &str) {
    end_sessions_saying(|_| why.to_string());
}

/// Ends every controller's connection from here, telling each one (by its device id) why.
pub fn end_sessions_saying(why: impl Fn(&str) -> String) {
    for (_, from, end) in SESSIONS.lock().unwrap().iter() {
        let _ = end.send(ToSession::End(why(from)));
    }
}

/// Tells every controller connected here `message` (a like here).
pub fn tell_controllers(message: Outgoing) {
    for (_, _, to) in SESSIONS.lock().unwrap().iter() {
        let _ = to.send(ToSession::Tell(message.clone()));
    }
}

/// Each end sends a check this often (a ping; the other answers by itself)...
const CHECK_EVERY: Duration = Duration::from_secs(5);
/// ...and takes the other as gone (crashed, off the network) after this long without hearing from it.
const SILENT_AT_MOST: Duration = Duration::from_secs(15);

/// How long a device that asks first waits for an answer.
pub const ASK_WAIT: Duration = Duration::from_secs(30);
/// While playing, the position goes out this often (the controller counts on in between).
const POSITION_EVERY: Duration = Duration::from_secs(5);
/// How often the state is looked at for changes.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// The state without its position: what changing means (the position moves by itself while playing).
fn without_position(state: &RemoteState) -> RemoteState {
    RemoteState { position: 0.0, ..state.clone() }
}

/// `/control`: a controller connects, when it proves it's one of your devices and this device lets others play.
pub async fn control(State(host): State<Arc<dyn Host>>, headers: HeaderMap, upgrade: WebSocketUpgrade) -> Response {
    let Ok(key) = host.key() else { return StatusCode::SERVICE_UNAVAILABLE.into_response() };
    let proof = headers.get(PROOF).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let Some(from) = key::check(&key, proof, "GET /control", now_ms()) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match host.hello().map(|h| h.allow) {
        Ok(Allow::On) => {}
        // Someone there says yes first (the controller waits for it), unless it's coming back after losing the connection.
        Ok(Allow::Ask) if coming_back(&from) => {}
        Ok(Allow::Ask) => {
            let (asking, who) = (host.clone(), from.clone());
            if !tokio::task::spawn_blocking(move || asking.ask(&who)).await.unwrap_or(false) {
                return (StatusCode::FORBIDDEN, "it said no, or nobody answered there").into_response();
            }
        }
        Ok(Allow::Off) => return (StatusCode::FORBIDDEN, "it doesn’t let other devices play on it").into_response(),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
    upgrade.on_upgrade(move |socket| session(host, socket, from))
}

/// Ends the connection on the host's side however the session ends.
struct Open(Arc<dyn Host>, u64, String);

impl Drop for Open {
    fn drop(&mut self) {
        SESSIONS.lock().unwrap().retain(|(id, _, _)| *id != self.1);
        self.0.connection(&self.2, false);
    }
}

async fn session(host: Arc<dyn Host>, socket: WebSocket, from: String) {
    let id = NEXT_SESSION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let (end, mut told) = tokio::sync::mpsc::unbounded_channel::<ToSession>();
    {
        let mut sessions = SESSIONS.lock().unwrap();
        // The same device again (it lost its connection, and this one hadn't noticed yet): the old one goes.
        for (_, _, old) in sessions.iter().filter(|(_, other, _)| *other == from) {
            let _ = old.send(ToSession::End("connected again".into()));
        }
        sessions.push((id, from.clone(), end));
    }
    host.connection(&from, true);
    let _open = Open(host.clone(), id, from.clone());
    // Lost (not closed by either side): it may come back without asking.
    let mut lost = false;
    let (mut out, mut incoming) = socket.split();
    let mut last: Option<RemoteState> = None;
    let mut position_sent = Instant::now();
    let mut look = tokio::time::interval(LOOK_EVERY);
    let mut check = tokio::time::interval(CHECK_EVERY);
    let mut heard = tokio::time::Instant::now();
    loop {
        let send_now = tokio::select! {
            _ = look.tick() => false,
            told = told.recv() => match told {
                // Ended from here (Disconnect on this device): the controller is told why.
                Some(ToSession::End(why)) => {
                    let close = axum::extract::ws::CloseFrame { code: axum::extract::ws::close_code::NORMAL, reason: why.into() };
                    let _ = out.send(Message::Close(Some(close))).await;
                    break;
                }
                Some(ToSession::Tell(message)) => {
                    if out.send(Message::Text(serde_json::to_string(&message).unwrap_or_default().into())).await.is_err() {
                        lost = true;
                        break;
                    }
                    false
                }
                None => false,
            },
            // The controller still there? (It answers a ping by itself; nothing heard for a while: it's gone.)
            _ = check.tick() => {
                if heard.elapsed() > SILENT_AT_MOST || out.send(Message::Ping(Vec::new().into())).await.is_err() {
                    lost = true;
                    break;
                }
                false
            }
            message = incoming.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    heard = tokio::time::Instant::now();
                    let applied = match serde_json::from_str::<Command>(&text) {
                        Ok(command) => {
                            let host = host.clone();
                            tokio::task::spawn_blocking(move || host.apply(command)).await.unwrap_or_else(|e| Err(e.to_string()))
                        }
                        Err(e) => Err(format!("not a command: {e}")),
                    };
                    if let Err(e) = applied {
                        let _ = out.send(Message::Text(serde_json::to_string(&Outgoing::Error(e)).unwrap_or_default().into())).await;
                    }
                    true
                }
                Some(Ok(Message::Close(_))) => break,
                None | Some(Err(_)) => {
                    lost = true;
                    break;
                }
                Some(Ok(_)) => {
                    heard = tokio::time::Instant::now();
                    false
                }
            },
        };
        let looked = {
            let host = host.clone();
            tokio::task::spawn_blocking(move || host.state()).await.unwrap_or_else(|e| Err(e.to_string()))
        };
        let Ok(state) = looked else { continue };
        let changed = last.as_ref().is_none_or(|l| without_position(l) != without_position(&state));
        let position_due = state.playing && position_sent.elapsed() >= POSITION_EVERY;
        if send_now || changed || position_due {
            let text = serde_json::to_string(&Outgoing::State(state.clone())).unwrap_or_default();
            if out.send(Message::Text(text.into())).await.is_err() {
                lost = true;
                break;
            }
            position_sent = Instant::now();
            last = Some(state);
        }
    }
    if lost {
        LOST.lock().unwrap().push((from, Instant::now()));
    }
}

/// The controller's end: commands go out on `send`; what comes back goes to `on_message`, then `on_closed` once.
pub struct Link {
    commands: tokio::sync::mpsc::UnboundedSender<Command>,
}

impl Link {
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.commands.send(command).map_err(|_| "the connection is closed".to_string())
    }
}

/// Connects to the device at `address:port` (as `me`, with `key`).
pub async fn connect(
    address: &str,
    port: u16,
    key: &[u8],
    me: &str,
    on_message: impl Fn(Outgoing) + Send + 'static,
    on_closed: impl FnOnce(Closed) + Send + 'static,
) -> Result<Link, String> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message as TMessage;
    let mut request = format!("ws://{address}:{port}/control").into_client_request().map_err(|e| e.to_string())?;
    let proof = key::proof(key, me, now_ms(), "GET /control");
    request.headers_mut().insert(PROOF, proof.parse().map_err(|_| "a bad proof".to_string())?);
    // Long enough for someone at a device that asks first to answer.
    let (socket, _) = tokio::time::timeout(ASK_WAIT + Duration::from_secs(5), tokio_tungstenite::connect_async(request))
        .await
        .map_err(|_| "it took too long to answer".to_string())?
        .map_err(|e| match e {
            tokio_tungstenite::tungstenite::Error::Http(answer) => match answer.status().as_u16() {
                401 => "it isn’t one of your devices (another share’s key)".to_string(),
                403 => String::from_utf8_lossy(answer.body().as_deref().unwrap_or_default()).into_owned(),
                status => format!("it answered {status}"),
            },
            e => e.to_string(),
        })?;
    let (mut out, mut incoming) = socket.split();
    let (commands, mut queued) = tokio::sync::mpsc::unbounded_channel::<Command>();
    tokio::spawn(async move {
        let mut check = tokio::time::interval(CHECK_EVERY);
        let mut heard = tokio::time::Instant::now();
        let why = loop {
            tokio::select! {
                // The other device still there? (It pings every few seconds; nothing heard for a while: it's gone.)
                _ = check.tick() => {
                    if heard.elapsed() > SILENT_AT_MOST {
                        break Closed::Lost("the other device stopped answering".into());
                    }
                }
                command = queued.recv() => match command {
                    Some(command) => {
                        let text = serde_json::to_string(&command).unwrap_or_default();
                        if let Err(e) = out.send(TMessage::Text(text.into())).await {
                            break Closed::Lost(e.to_string());
                        }
                    }
                    // The link was dropped: disconnect.
                    None => {
                        let _ = out.send(TMessage::Close(None)).await;
                        break Closed::Disconnected;
                    }
                },
                message = incoming.next() => match message {
                    Some(Ok(TMessage::Text(text))) => {
                        heard = tokio::time::Instant::now();
                        if let Ok(message) = serde_json::from_str::<Outgoing>(&text) {
                            on_message(message);
                        }
                    }
                    Some(Ok(TMessage::Close(frame))) => {
                        let why = frame.map(|f| f.reason.to_string()).filter(|r| !r.is_empty());
                        break Closed::Ended(why.unwrap_or_else(|| "the other device closed the connection".into()));
                    }
                    None => break Closed::Lost("the connection broke".into()),
                    Some(Err(e)) => break Closed::Lost(e.to_string()),
                    Some(Ok(_)) => heard = tokio::time::Instant::now(),
                },
            }
        };
        on_closed(why);
    });
    Ok(Link { commands })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// A device with a pretend player: its state, changed by the commands.
    struct TestHost {
        key: Vec<u8>,
        allow: Allow,
        state: Mutex<RemoteState>,
        /// What someone there answers when asked.
        says_yes: bool,
    }

    impl Host for TestHost {
        fn key(&self) -> Result<Vec<u8>, String> {
            Ok(self.key.clone())
        }
        fn hello(&self) -> Result<Hello, String> {
            Ok(Hello { device_id: "pc".into(), name: "PC".into(), version: "0.6.2".into(), allow: self.allow })
        }
        fn state(&self) -> Result<RemoteState, String> {
            Ok(self.state.lock().unwrap().clone())
        }
        fn ask(&self, from: &str) -> bool {
            assert!(from.starts_with("phone-"), "asked about the device connecting: {from}");
            self.says_yes
        }
        fn connection(&self, _from: &str, _open: bool) {}
        fn apply(&self, command: Command) -> Result<(), String> {
            let mut s = self.state.lock().unwrap();
            match command {
                Command::Load { queue, index, position, playing } => {
                    if queue.iter().any(|p| p.contains("missing")) {
                        return Err("that song isn't here".into());
                    }
                    s.queue = queue.into_iter().map(|p| RemoteSong { path: Some(p), ..Default::default() }).collect();
                    (s.index, s.position, s.playing) = (index as i64, position, playing);
                }
                Command::Play => s.playing = true,
                Command::Pause => s.playing = false,
                Command::Seek { position } => s.position = position,
                Command::Volume { volume } => s.volume = volume,
                Command::Next => s.index += 1,
                // A like: as the app does, it's the library's; here it's kept to look at.
                Command::Like(like) => s.queue.push(RemoteSong { path: Some(format!("liked:{}:{}", like.path, like.liked)), ..Default::default() }),
                _ => {}
            }
            Ok(())
        }
    }

    /// A device's server with `host`, on a free port here.
    async fn serve(host: TestHost) -> u16 {
        let routes = super::super::server::routes(Arc::new(host));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, routes).await.unwrap() });
        port
    }

    fn host(key: &[u8], allow: Allow) -> TestHost {
        TestHost { key: key.to_vec(), allow, state: Mutex::new(RemoteState { index: -1, repeat: "off".into(), ..Default::default() }), says_yes: false }
    }

    /// Connects as the phone; what comes back is collected.
    async fn phone(port: u16, key: &[u8]) -> Result<(Link, Arc<Mutex<Vec<Outgoing>>>), String> {
        // A device of its own each time (the sessions are the process's: the tests run side by side).
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let me = format!("phone-{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let keep = seen.clone();
        let link = connect("127.0.0.1", port, key, &me, move |m| keep.lock().unwrap().push(m), |_| {}).await?;
        Ok((link, seen))
    }

    async fn until(seen: &Arc<Mutex<Vec<Outgoing>>>, what: impl Fn(&Outgoing) -> bool) -> Outgoing {
        for _ in 0..60 {
            if let Some(found) = seen.lock().unwrap().iter().rev().find(|m| what(m)).cloned() {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("not received; got {:?}", seen.lock().unwrap());
    }

    #[tokio::test]
    async fn the_phone_plays_a_queue_on_the_pc_and_sees_it_play() {
        let key = [3u8; 32];
        let port = serve(host(&key, Allow::On)).await;
        let (link, seen) = phone(port, &key).await.unwrap();
        // The state at once on connecting.
        until(&seen, |m| matches!(m, Outgoing::State(s) if s.index == -1)).await;
        link.send(Command::Load { queue: vec!["A.mp3".into(), "B.mp3".into()], index: 1, position: 30.0, playing: true }).unwrap();
        let Outgoing::State(state) = until(&seen, |m| matches!(m, Outgoing::State(s) if s.index == 1)).await else { unreachable!() };
        assert_eq!((state.playing, state.position, state.queue.len()), (true, 30.0, 2));
        link.send(Command::Pause).unwrap();
        link.send(Command::Volume { volume: 0.4 }).unwrap();
        until(&seen, |m| matches!(m, Outgoing::State(s) if !s.playing && s.volume == 0.4)).await;
        // A command that doesn't work there: said so.
        link.send(Command::Load { queue: vec!["missing.mp3".into()], index: 0, position: 0.0, playing: true }).unwrap();
        until(&seen, |m| matches!(m, Outgoing::Error(e) if e.contains("isn't here"))).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_device_that_stops_answering_is_taken_as_gone() {
        // A "device" that accepts the connection, then says nothing at all (crashed, off the network).
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            std::future::pending::<()>().await;
        });
        let (closed, why) = tokio::sync::oneshot::channel();
        let _link = connect("127.0.0.1", port, &[3u8; 32], "phone", |_| {}, move |w| {
            let _ = closed.send(w);
        })
        .await
        .unwrap();
        let why = tokio::time::timeout(Duration::from_secs(30), why).await.expect("noticed").unwrap();
        assert_eq!(why, Closed::Lost("the other device stopped answering".into()));
    }

    #[tokio::test]
    async fn only_your_devices_and_only_when_it_lets_them() {
        let key = [3u8; 32];
        let open = serve(host(&key, Allow::On)).await;
        let err = phone(open, &[9u8; 32]).await.err().unwrap();
        assert!(err.contains("another share"), "{err}");
        let off = serve(host(&key, Allow::Off)).await;
        let err = phone(off, &key).await.err().unwrap();
        assert!(err.contains("doesn’t let"), "{err}");
        // Ask first: in when someone there says yes, not when they say no.
        let no = serve(host(&key, Allow::Ask)).await;
        let err = phone(no, &key).await.err().unwrap();
        assert!(err.contains("said no"), "{err}");
        let yes = serve(TestHost { says_yes: true, ..host(&key, Allow::Ask) }).await;
        assert!(phone(yes, &key).await.is_ok());
    }

    #[tokio::test]
    async fn a_like_goes_both_ways_over_the_connection() {
        let key = [3u8; 32];
        let port = serve(host(&key, Allow::On)).await;
        let (link, seen) = phone(port, &key).await.unwrap();
        until(&seen, |m| matches!(m, Outgoing::State(_))).await;
        // Liked on the controller: there at once.
        link.send(Command::Like(Like { path: "Song.mp3".into(), liked: true, at: 5 })).unwrap();
        until(&seen, |m| matches!(m, Outgoing::State(s) if s.queue.iter().any(|q| q.path.as_deref() == Some("liked:Song.mp3:true")))).await;
        // Liked on the playing device: the controllers hear of it.
        tell_controllers(Outgoing::Liked(Like { path: "Other.mp3".into(), liked: false, at: 6 }));
        until(&seen, |m| matches!(m, Outgoing::Liked(l) if l.path == "Other.mp3" && !l.liked)).await;
    }

    #[test]
    fn a_device_that_lost_its_connection_comes_back_without_asking_once() {
        assert!(!coming_back("lost-phone"));
        LOST.lock().unwrap().push(("lost-phone".into(), Instant::now()));
        assert!(coming_back("lost-phone"));
        // Taken off the list: the next time (a new connection, not coming back) asks again.
        assert!(!coming_back("lost-phone"));
    }
}
