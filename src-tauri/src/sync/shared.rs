//! The share as synced playback and library data use it: kept open between uses (opened again after a
//! problem), and left alone for a moment after it couldn't be reached (unless the network changes), so a
//! share that's away isn't knocked on every few seconds. The file sync opens its own.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::library::Result;
use crate::share::commands::with_stores;
use crate::share::setup;
use crate::share::Share;

/// After the share couldn't be used, it's left alone this long (unless the network changes).
const REST_AFTER_FAILURE: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct OpenShare {
    share: Mutex<Option<Box<dyn Share>>>,
    /// When the share last couldn't be used.
    failed_at: Mutex<Option<Instant>>,
}

/// Runs `work` with the share (opened when needed; a failure closes it, to be opened again next time).
pub fn with_share<T>(app: &AppHandle, work: impl FnOnce(&dyn Share) -> Result<T>) -> Result<T> {
    let open = app.state::<OpenShare>();
    if open.failed_at.lock().unwrap().is_some_and(|at| at.elapsed() < REST_AFTER_FAILURE) {
        return Err("the share couldn't be reached a moment ago: trying again soon".into());
    }
    let mut share = open.share.lock().unwrap();
    if share.is_none() {
        let opened = with_stores(app, |library, stores| setup::current(library, stores, &setup::open)).and_then(|r| r);
        match opened {
            Ok((_, opened)) => *share = Some(opened),
            Err(e) => {
                *open.failed_at.lock().unwrap() = Some(Instant::now());
                return Err(format!("{e:?}"));
            }
        }
    }
    let result = work(share.as_deref().expect("opened"));
    *open.failed_at.lock().unwrap() = if result.is_err() { Some(Instant::now()) } else { None };
    if result.is_err() {
        *share = None;
    }
    result
}

/// The network changed (or the share was set up anew): opened again next time, right away.
pub fn reset(app: &AppHandle) {
    let open = app.state::<OpenShare>();
    *open.share.lock().unwrap() = None;
    *open.failed_at.lock().unwrap() = None;
}
