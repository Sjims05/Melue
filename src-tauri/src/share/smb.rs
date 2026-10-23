//! The share over SMB (the `smb2` crate): Melue logs in itself, so it works on every platform, the phone
//! included.
//!
//! `smb2` is async: each share has a small runtime of its own, and every call runs there while the (non-async)
//! caller waits for its answer, so it can be used from any thread: plain threads, and also threads of another
//! async runtime (Tauri's), where Tokio's own blocking waits refuse to run (they panic), hence the waits
//! here use a runtime-independent executor. Connecting gives up after
//! [`CONNECT_TIMEOUT`] (start-up never hangs away from home), each operation after [`OPERATION_TIMEOUT`]
//! without progress. A dropped connection (Wi-Fi switching, the NAS asleep) is reconnected once and the
//! operation tried again, as long as no data has moved yet.

use std::future::Future;
use std::io::{Read, Write};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use smb2::{ClientConfig, ErrorKind, SmbClient, Tree};
use tokio::sync::{mpsc, Mutex};

use super::{clean_path, is_hidden, join, part_path, Entry, Share, ShareError, ShareResult, CHUNK};

/// A VPN that has just (re)connected can need several seconds before the NAS answers.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Without any answer for this long, an operation gives up.
pub const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the home address gets to answer before the main one is used.
const HOME_TIMEOUT: Duration = Duration::from_secs(2);
/// How long looking up a home address given by name may take (away from home it may not answer at all).
const HOME_LOOKUP_TIMEOUT: Duration = Duration::from_secs(1);

/// Where the share is and how to log in.
#[derive(Clone, Default)]
pub struct SmbConfig {
    /// "nas.local", "192.168.1.10" or with a port ("nas:445").
    pub host: String,
    /// The share's address on the home network (an address or a name), tried first when this device is on
    /// that network: reliable at home even when the VPN isn't (see [`home_address`]).
    pub home_host: Option<String>,
    /// Set when the home address was tried at home and didn't answer (the main one was used).
    pub home_note: Arc<std::sync::Mutex<Option<String>>>,
    /// How long connecting may take (None: [`CONNECT_TIMEOUT`]).
    pub connect_timeout: Option<Duration>,
    /// The share's name ("music").
    pub share: String,
    /// The music folder inside the share ("" for the share itself, "Media/Music").
    pub folder: String,
    pub user: String,
    pub password: String,
}

impl std::fmt::Debug for SmbConfig {
    /// Never shows the password.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmbConfig")
            .field("host", &self.host)
            .field("home_host", &self.home_host)
            .field("share", &self.share)
            .field("folder", &self.folder)
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}

struct Conn {
    client: SmbClient,
    tree: Tree,
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct SmbShare {
    config: Arc<SmbConfig>,
    /// The music folder's path in the share, with '/' ("" or "Media/Music").
    base: String,
    conn: Arc<Mutex<Option<Conn>>>,
    /// Always there until dropped (see Drop).
    runtime: Option<tokio::runtime::Runtime>,
}

impl Drop for SmbShare {
    /// Stops the share's runtime without waiting (waiting could panic when dropped from async code).
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// What an `smb2` error means for the user.
fn classify(e: smb2::Error, path: &str) -> ShareError {
    let detail = e.to_string();
    match e.kind() {
        ErrorKind::AuthRequired | ErrorKind::SigningRequired | ErrorKind::SessionExpired => ShareError::Login { detail },
        ErrorKind::AccessDenied => ShareError::NoPermission { path: path.into() },
        ErrorKind::NotFound => ShareError::NotFound { path: path.into() },
        ErrorKind::DiskFull => ShareError::Full { detail },
        ErrorKind::ConnectionLost | ErrorKind::TimedOut => ShareError::Unreachable { detail },
        ErrorKind::Io => ShareError::Unreachable { detail },
        _ => ShareError::Other { detail },
    }
}

/// Errors after which reconnecting may help.
fn connection_gone(e: &smb2::Error) -> bool {
    matches!(e.kind(), ErrorKind::ConnectionLost | ErrorKind::TimedOut | ErrorKind::SessionExpired | ErrorKind::Io)
}

fn host_with_port(host: &str) -> String {
    let host = host.trim().trim_start_matches("smb://").trim_start_matches(['\\', '/']).trim_end_matches(['/', '\\']);
    if host.contains(':') && !host.starts_with('[') { host.to_string() } else { format!("{host}:445") }
}

/// Whether two addresses are on the same home network (the same /24 range, as home routers hand out).
pub fn same_network(a: std::net::Ipv4Addr, b: std::net::Ipv4Addr) -> bool {
    a.octets()[..3] == b.octets()[..3]
}

/// The home address to try first: when it's a home-network address (given, or looked up from a name) and this
/// device's own address on the way there is in the same range (it's at home). None: go straight to the main
/// address.
async fn home_address(home: &str) -> Option<String> {
    use std::net::{IpAddr, SocketAddr};
    let with_port = host_with_port(home);
    let ip = match with_port.parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(_) => tokio::time::timeout(HOME_LOOKUP_TIMEOUT, tokio::net::lookup_host(with_port.clone()))
            .await
            .ok()?
            .ok()?
            .find(|a| a.is_ipv4())?,
    };
    let IpAddr::V4(nas) = ip.ip() else { return None };
    if !nas.is_private() {
        return None;
    }
    // The address this device would send from (no packet is sent): on the home network, it's in the same range.
    let probe = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    probe.connect(SocketAddr::new(IpAddr::V4(nas), 445)).ok()?;
    let IpAddr::V4(own) = probe.local_addr().ok()?.ip() else { return None };
    same_network(own, nas).then(|| ip.to_string())
}

async fn connect(config: &SmbConfig) -> ShareResult<Conn> {
    if let Some(home) = config.home_host.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
        if let Some(addr) = home_address(home).await {
            match connect_to(config, &addr, HOME_TIMEOUT).await {
                Ok(conn) => {
                    *config.home_note.lock().unwrap() = None;
                    return Ok(conn);
                }
                Err(e) => {
                    let note = format!("the home address ({home}) didn't answer, so the main one was used: it may have changed ({e})");
                    *config.home_note.lock().unwrap() = Some(note);
                }
            }
        }
    }
    connect_to(config, &host_with_port(&config.host), config.connect_timeout.unwrap_or(CONNECT_TIMEOUT)).await
}

async fn connect_to(config: &SmbConfig, addr: &str, timeout: Duration) -> ShareResult<Conn> {
    let client_config = ClientConfig {
        addr: addr.to_string(),
        timeout,
        username: config.user.clone(),
        password: config.password.clone(),
        domain: String::new(),
        auto_reconnect: false,
        compression: true,
        dfs_enabled: true,
        dfs_target_overrides: Default::default(),
    };
    let attempt = async {
        let mut client = SmbClient::connect(client_config).await.map_err(|e| classify(e, &config.host))?;
        let tree = client.connect_share(&config.share).await.map_err(|e| classify(e, &config.share))?;
        Ok(Conn { client, tree })
    };
    match tokio::time::timeout(timeout * 2, attempt).await {
        Ok(result) => result,
        Err(_) => Err(ShareError::Unreachable { detail: format!("no answer from {} in time", config.host) }),
    }
}

fn modified_ms(time: smb2::pack::FileTime) -> i64 {
    time.to_system_time()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

impl SmbShare {
    /// Doesn't connect yet: the first operation does (see [`SmbShare::check`] to try the login now).
    pub fn new(config: SmbConfig) -> ShareResult<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("melue-smb")
            .enable_all()
            .build()
            .map_err(|e| ShareError::Other { detail: e.to_string() })?;
        let base = clean_path(&config.folder)?;
        Ok(Self { config: Arc::new(config), base, conn: Arc::new(Mutex::new(None)), runtime: Some(runtime) })
    }

    /// Why the home address wasn't used at home (None: it was, or this device isn't at home).
    pub fn home_note(&self) -> Option<String> {
        self.config.home_note.lock().unwrap().clone()
    }

    /// Connects and lists the music folder (the tests' check that a login works).
    #[cfg(test)]
    pub fn check(&self) -> ShareResult<()> {
        self.list("").map(|_| ())
    }

    /// A path in the share for a path in the music folder.
    fn in_share(&self, path: &str) -> ShareResult<String> {
        Ok(join(&self.base, &clean_path(path)?).trim_matches('/').to_string())
    }

    /// Runs `op` on the share's runtime with a connection (connecting first when there's none) and waits for
    /// its answer. A dropped connection is reconnected and `op` tried once more, unless `started` says data
    /// already moved.
    fn run<T, F>(&self, path: String, started: Arc<AtomicBool>, op: F) -> ShareResult<T>
    where
        T: Send + 'static,
        F: for<'a> Fn(&'a mut Conn) -> BoxFuture<'a, Result<T, smb2::Error>> + Send + Sync + 'static,
    {
        let (conn, config) = (self.conn.clone(), self.config.clone());
        let (answer, wait) = std::sync::mpsc::channel();
        let Some(runtime) = self.runtime.as_ref() else {
            return Err(ShareError::Other { detail: "the share is closed".into() });
        };
        runtime.spawn(async move {
            let result = async {
                for attempt in 0..2 {
                    let mut guard = conn.lock().await;
                    if guard.is_none() {
                        *guard = Some(connect(&config).await?);
                    }
                    let connection = guard.as_mut().expect("connected");
                    match tokio::time::timeout(OPERATION_TIMEOUT, op(connection)).await {
                        Ok(Ok(value)) => return Ok(value),
                        Ok(Err(e)) if attempt == 0 && connection_gone(&e) && !started.load(Ordering::SeqCst) => {
                            *guard = None;
                        }
                        Ok(Err(e)) => {
                            if connection_gone(&e) {
                                *guard = None;
                            }
                            return Err(classify(e, &path));
                        }
                        Err(_) => {
                            *guard = None;
                            if attempt == 1 || started.load(Ordering::SeqCst) {
                                return Err(ShareError::Unreachable { detail: format!("no answer from the share for {path}") });
                            }
                        }
                    }
                }
                unreachable!("the second attempt always returns")
            }
            .await;
            let _ = answer.send(result);
        });
        wait.recv().unwrap_or_else(|_| Err(ShareError::Other { detail: "the share's runtime stopped".into() }))
    }

    fn quick<T, F>(&self, path: String, op: F) -> ShareResult<T>
    where
        T: Send + 'static,
        F: for<'a> Fn(&'a mut Conn) -> BoxFuture<'a, Result<T, smb2::Error>> + Send + Sync + 'static,
    {
        self.run(path, Arc::new(AtomicBool::new(false)), op)
    }
}

impl Share for SmbShare {
    fn list(&self, dir: &str) -> ShareResult<Vec<Entry>> {
        let dir = clean_path(dir)?;
        let full = self.in_share(&dir)?;
        let found = self.quick(full.clone(), move |c| {
            let full = full.clone();
            Box::pin(async move { c.client.list_directory(&mut c.tree, &full).await })
        })?;
        let mut entries: Vec<Entry> = found
            .into_iter()
            .filter(|e| e.name != "." && e.name != ".." && !is_hidden(&e.name))
            .map(|e| Entry {
                path: join(&dir, &e.name),
                size: if e.is_directory { 0 } else { e.size },
                is_dir: e.is_directory,
                modified_ms: modified_ms(e.modified),
                name: e.name,
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    fn stat(&self, path: &str) -> ShareResult<Option<Entry>> {
        let path = clean_path(path)?;
        let full = self.in_share(&path)?;
        let found = self.quick(full.clone(), move |c| {
            let full = full.clone();
            Box::pin(async move { c.client.stat(&mut c.tree, &full).await })
        });
        match found {
            Ok(info) => {
                let name = path.rsplit('/').next().unwrap_or_default().to_string();
                let size = if info.is_directory { 0 } else { info.size };
                Ok(Some(Entry { path, name, is_dir: info.is_directory, size, modified_ms: modified_ms(info.modified) }))
            }
            Err(ShareError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn read_to(&self, path: &str, out: &mut dyn Write, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        let full = self.in_share(path)?;
        let started = Arc::new(AtomicBool::new(false));
        // The chunks come over a small channel: the download waits while this side writes them.
        let (chunks, mut received) = mpsc::channel::<Vec<u8>>(4);
        let flag = started.clone();
        let run_path = full.clone();
        let reading = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                self.run(run_path.clone(), started, move |c| {
                    let (full, chunks, flag) = (run_path.clone(), chunks.clone(), flag.clone());
                    Box::pin(async move {
                        let mut download = c.client.download(&c.tree, &full).await?;
                        let mut total = 0u64;
                        while let Some(chunk) = download.next_chunk().await {
                            let chunk = chunk?;
                            flag.store(true, Ordering::SeqCst);
                            total += chunk.len() as u64;
                            if chunks.send(chunk).await.is_err() {
                                return Err(smb2::Error::Cancelled);
                            }
                        }
                        Ok(total)
                    })
                })
            });
            let mut written = 0u64;
            let mut local_error = None;
            while let Some(chunk) = futures_executor::block_on(received.recv()) {
                if let Err(e) = out.write_all(&chunk) {
                    local_error = Some(e);
                    break;
                }
                written += chunk.len() as u64;
                progress(written);
            }
            drop(received);
            let result = worker.join().unwrap_or_else(|_| Err(ShareError::Other { detail: "reading panicked".into() }));
            match local_error {
                Some(e) if e.kind() == std::io::ErrorKind::StorageFull => Err(ShareError::Full { detail: e.to_string() }),
                Some(e) => Err(ShareError::Other { detail: format!("can't write the copy: {e}") }),
                None => result,
            }
        });
        reading
    }

    fn write_from(&self, path: &str, input: &mut dyn Read, progress: &mut dyn FnMut(u64)) -> ShareResult<u64> {
        let path = clean_path(path)?;
        let full = self.in_share(&path)?;
        let part = self.in_share(&part_path(&path))?;
        if let Some((dir, _)) = path.rsplit_once('/') {
            self.create_dir_all(dir)?;
        }
        let started = Arc::new(AtomicBool::new(false));
        // The data goes over a small channel: this side reads it while the upload sends it.
        let (chunks, received) = mpsc::channel::<Vec<u8>>(4);
        let received = Arc::new(Mutex::new(received));
        // Set when reading the input failed: the end of the data then means "stop", not "done".
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = started.clone();
        let (run_full, run_part, run_cancelled) = (full.clone(), part.clone(), cancelled.clone());
        let written = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                self.run(run_full.clone(), started, move |c| {
                    let (full, part, received, flag) = (run_full.clone(), run_part.clone(), received.clone(), flag.clone());
                    let cancelled = run_cancelled.clone();
                    Box::pin(async move {
                        let mut writer = c.client.create_file_writer(&c.tree, &part).await?;
                        let mut received = received.lock().await;
                        let mut total = 0u64;
                        while let Some(chunk) = received.recv().await {
                            flag.store(true, Ordering::SeqCst);
                            if let Err(e) = writer.write_chunk(&chunk).await {
                                let _ = writer.abort().await;
                                let _ = c.client.delete_file(&mut c.tree, &part).await;
                                return Err(e);
                            }
                            total += chunk.len() as u64;
                        }
                        if cancelled.load(Ordering::SeqCst) {
                            let _ = writer.abort().await;
                            let _ = c.client.delete_file(&mut c.tree, &part).await;
                            return Err(smb2::Error::Cancelled);
                        }
                        writer.finish().await?;
                        // Put in place: what was there goes first (a rename doesn't replace on every server).
                        if c.client.stat(&mut c.tree, &full).await.is_ok() {
                            c.client.delete_file(&mut c.tree, &full).await?;
                        }
                        c.client.rename(&mut c.tree, &part, &full).await?;
                        Ok(total)
                    })
                })
            });
            let mut buffer = vec![0; CHUNK];
            let mut sent = 0u64;
            let mut local_error = None;
            loop {
                match input.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if futures_executor::block_on(chunks.send(buffer[..n].to_vec())).is_err() {
                            break; // the upload stopped: its error is the answer
                        }
                        sent += n as u64;
                        progress(sent);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        local_error = Some(e);
                        break;
                    }
                }
            }
            if let Some(e) = local_error {
                // Ends the upload without finishing it (the end of the data alone would look complete).
                cancelled.store(true, Ordering::SeqCst);
                drop(chunks);
                let _ = worker.join();
                return Err(ShareError::Other { detail: format!("can't read what to upload: {e}") });
            }
            drop(chunks);
            worker.join().unwrap_or_else(|_| Err(ShareError::Other { detail: "writing panicked".into() }))
        });
        written
    }

    fn rename(&self, from: &str, to: &str) -> ShareResult<()> {
        let (from, to) = (self.in_share(from)?, clean_path(to)?);
        if let Some((dir, _)) = to.rsplit_once('/') {
            self.create_dir_all(dir)?;
        }
        let to = self.in_share(&to)?;
        self.quick(from.clone(), move |c| {
            let (from, to) = (from.clone(), to.clone());
            Box::pin(async move {
                if c.client.stat(&mut c.tree, &to).await.is_ok() {
                    c.client.delete_file(&mut c.tree, &to).await?;
                }
                c.client.rename(&mut c.tree, &from, &to).await
            })
        })
    }

    fn delete(&self, path: &str) -> ShareResult<()> {
        let full = self.in_share(path)?;
        match self.quick(full.clone(), move |c| {
            let full = full.clone();
            Box::pin(async move { c.client.delete_file(&mut c.tree, &full).await })
        }) {
            Err(ShareError::NotFound { .. }) => Ok(()),
            other => other,
        }
    }

    fn create_dir_all(&self, dir: &str) -> ShareResult<()> {
        let dir = clean_path(dir)?;
        let mut so_far = String::new();
        for part in dir.split('/').filter(|p| !p.is_empty()) {
            so_far = join(&so_far, part);
            let full = self.in_share(&so_far)?;
            let made = self.quick(full.clone(), move |c| {
                let full = full.clone();
                Box::pin(async move {
                    match c.client.create_directory(&mut c.tree, &full).await {
                        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(()),
                        other => other,
                    }
                })
            });
            made?;
        }
        Ok(())
    }

    fn delete_empty_dir(&self, dir: &str) -> ShareResult<()> {
        let full = self.in_share(dir)?;
        match self.quick(full.clone(), move |c| {
            let full = full.clone();
            Box::pin(async move { c.client.delete_directory(&mut c.tree, &full).await })
        }) {
            Err(ShareError::NotFound { .. }) => Ok(()),
            other => other,
        }
    }

    fn watch(&self, on_change: Arc<dyn Fn(&str) + Send + Sync>) -> ShareResult<Option<super::Watch>> {
        let Some(runtime) = self.runtime.as_ref() else { return Ok(None) };
        let (conn, config, base) = (self.conn.clone(), self.config.clone(), self.base.clone());
        let alive = Arc::new(AtomicBool::new(true));
        let still = alive.clone();
        let (ready, started) = std::sync::mpsc::channel();
        let task = runtime.spawn(async move {
            let watcher = async {
                let mut guard = conn.lock().await;
                if guard.is_none() {
                    *guard = Some(connect(&config).await?);
                }
                let c = guard.as_mut().expect("connected");
                c.client.watch(&c.tree, &base, true).await.map_err(|e| classify(e, &base))
            }
            .await;
            let mut watcher = match watcher {
                Ok(watcher) => {
                    let _ = ready.send(Ok(()));
                    watcher
                }
                Err(e) => {
                    still.store(false, Ordering::SeqCst);
                    let _ = ready.send(Err(e));
                    return;
                }
            };
            // The watcher has its own connection: syncing goes on next to it.
            let ended = loop {
                match watcher.next_events().await {
                    Ok(events) => {
                        for event in events {
                            on_change(&event.filename.replace('\\', "/"));
                        }
                    }
                    Err(e) => break e,
                }
            };
            eprintln!("melue: share: watching for changes stopped ({ended}); it's started again within a minute");
            still.store(false, Ordering::SeqCst);
        });
        started.recv().unwrap_or_else(|_| Err(ShareError::Other { detail: "the share's runtime stopped".into() }))?;
        Ok(Some(super::Watch::new(alive, move || task.abort())))
    }

    fn note(&self) -> Option<String> {
        self.home_note()
    }

    fn free_space(&self) -> ShareResult<Option<u64>> {
        let info = self.quick(self.config.share.clone(), |c| Box::pin(async move { c.client.fs_info(&mut c.tree).await }))?;
        Ok(Some(info.free_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smb_errors_are_sorted_for_the_user() {
        let sorted = |e: smb2::Error| classify(e, "x");
        assert!(matches!(sorted(smb2::Error::Timeout), ShareError::Unreachable { .. }));
        assert!(matches!(sorted(smb2::Error::Disconnected), ShareError::Unreachable { .. }));
        assert!(matches!(sorted(smb2::Error::Io(std::io::ErrorKind::ConnectionRefused.into())), ShareError::Unreachable { .. }));
        assert!(matches!(sorted(smb2::Error::Auth { message: "no".into() }), ShareError::Login { .. }));
        assert!(matches!(sorted(smb2::Error::Cancelled), ShareError::Other { .. }));
        assert!(connection_gone(&smb2::Error::Disconnected) && !connection_gone(&smb2::Error::Cancelled));
    }

    #[test]
    fn hosts_get_the_smb_port() {
        assert_eq!(host_with_port("nas.local"), "nas.local:445");
        assert_eq!(host_with_port(" 192.168.1.10 "), "192.168.1.10:445");
        assert_eq!(host_with_port("nas:1445"), "nas:1445");
        assert_eq!(host_with_port("smb://nas/"), "nas:445");
        assert_eq!(host_with_port(r"\\nas"), "nas:445");
    }

    #[test]
    fn the_password_never_shows() {
        let config = SmbConfig { host: "nas".into(), share: "music".into(), user: "user1".into(), password: "hunter2".into(), ..Default::default() };
        let shown = format!("{config:?}");
        assert!(shown.contains("user1") && !shown.contains("hunter2"), "{shown}");
    }
}

#[cfg(test)]
mod home_tests {
    use super::same_network;

    #[test]
    fn the_same_home_network() {
        let ip = |s: &str| s.parse().unwrap();
        assert!(same_network(ip("192.168.0.198"), ip("192.168.0.20")));
        assert!(!same_network(ip("192.168.1.5"), ip("192.168.0.20")), "another network");
        assert!(!same_network(ip("100.66.71.88"), ip("192.168.0.20")), "away: the VPN's or the phone network's address");
    }
}
