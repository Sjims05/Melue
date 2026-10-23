//! This device's small server for your other devices (they find its addresses and port in its presence file): every
//! request proves it comes from one of your devices (key.rs). On a computer, Windows asks once whether Melue may take
//! connections. For now: who's there (`/ping`); playing on it comes next.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::presence::{self, Device};
use super::{key, Allow};
use crate::library::{now_ms, Library};
use crate::sync::handoff;

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

/// The device a request comes from, when its proof is good.
fn from_your_device(app: &AppHandle, headers: &HeaderMap, request: &str) -> Result<String, StatusCode> {
    let key = key::key(app).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let proof = headers.get(PROOF).and_then(|v| v.to_str().ok()).ok_or(StatusCode::UNAUTHORIZED)?;
    key::check(&key, proof, request, now_ms()).ok_or(StatusCode::UNAUTHORIZED)
}

async fn ping(State(app): State<AppHandle>, headers: HeaderMap) -> Result<Json<Hello>, StatusCode> {
    from_your_device(&app, &headers, "GET /ping")?;
    let library = app.state::<Library>();
    Ok(Json(Hello {
        device_id: handoff::device_id(&library).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        name: handoff::settings(&app).ok().and_then(|s| s.device_name).unwrap_or_default(),
        version: app.package_info().version.to_string(),
        allow: super::settings(&app).map(|s| s.allow).unwrap_or_default(),
    }))
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
        let routes = Router::new().route("/ping", get(ping)).with_state(app);
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
    Err(format!("{} isn’t running Melue right now, or can’t be reached ({})", device.name, why.join("; ")))
}

/// Reaches one of your devices (the dev page's test).
#[tauri::command(async)]
pub fn remote_reach(app: AppHandle, id: String) -> Result<Reached, String> {
    let device = presence::device(&app, &id)?.ok_or("no such device")?;
    reach(&app, &device)
}
