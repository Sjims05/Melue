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
    pub volume: f64,
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

/// The controllers connected here now: each one's device, and a way to end its connection (with why).
static SESSIONS: std::sync::Mutex<Vec<(u64, String, tokio::sync::mpsc::UnboundedSender<String>)>> = std::sync::Mutex::new(Vec::new());
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
        let _ = end.send(why(from));
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
        // Someone there says yes first (the controller waits for it).
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
    let (end, mut ended) = tokio::sync::mpsc::unbounded_channel::<String>();
    SESSIONS.lock().unwrap().push((id, from.clone(), end));
    host.connection(&from, true);
    let _open = Open(host.clone(), id, from);
    let (mut out, mut incoming) = socket.split();
    let mut last: Option<RemoteState> = None;
    let mut position_sent = Instant::now();
    let mut look = tokio::time::interval(LOOK_EVERY);
    let mut check = tokio::time::interval(CHECK_EVERY);
    let mut heard = tokio::time::Instant::now();
    loop {
        let send_now = tokio::select! {
            _ = look.tick() => false,
            // Ended from here (Disconnect on this device): the controller is told why.
            why = ended.recv() => {
                let close = axum::extract::ws::CloseFrame { code: axum::extract::ws::close_code::NORMAL, reason: why.unwrap_or_default().into() };
                let _ = out.send(Message::Close(Some(close))).await;
                break;
            }
            // The controller still there? (It answers a ping by itself; nothing heard for a while: it's gone.)
            _ = check.tick() => {
                if heard.elapsed() > SILENT_AT_MOST || out.send(Message::Ping(Vec::new().into())).await.is_err() {
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
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
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
                break;
            }
            position_sent = Instant::now();
            last = Some(state);
        }
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
    on_closed: impl FnOnce(Option<String>) + Send + 'static,
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
                        break Some("the other device stopped answering".into());
                    }
                }
                command = queued.recv() => match command {
                    Some(command) => {
                        let text = serde_json::to_string(&command).unwrap_or_default();
                        if let Err(e) = out.send(TMessage::Text(text.into())).await {
                            break Some(e.to_string());
                        }
                    }
                    // The link was dropped: disconnect.
                    None => {
                        let _ = out.send(TMessage::Close(None)).await;
                        break None;
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
                        break Some(why.unwrap_or_else(|| "the other device closed the connection".into()));
                    }
                    None => break Some("the other device closed the connection".into()),
                    Some(Err(e)) => break Some(e.to_string()),
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
            assert_eq!(from, "phone", "asked about the device connecting");
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
        let seen = Arc::new(Mutex::new(Vec::new()));
        let keep = seen.clone();
        let link = connect("127.0.0.1", port, key, "phone", move |m| keep.lock().unwrap().push(m), |_| {}).await?;
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
        assert_eq!(why.as_deref(), Some("the other device stopped answering"));
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
}
