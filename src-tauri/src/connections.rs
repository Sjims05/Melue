//! Connections across devices (Settings: Connections): a sign-in made on one device is every device's, and signing
//! out on one signs out everywhere. Each device keeps the sign-in in its secure store; the share has it too, in
//! `.melue/connections.json`, encrypted (AES-256-GCM) with a key from the share's password: every device knows the
//! password, someone who can only read the share's files can't read the sign-ins. A share without a password (a
//! folder) keeps them as they are. The newest change of each wins (as for settings), with the library data sync.

use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{now_ms, Library};
use crate::share::secrets::{Secrets, SystemSecrets};
use crate::share::Share;

/// The file on the share.
const FILE: &str = ".melue/connections.json";
/// When each connection last changed here (a library setting: name → ms).
const CHANGED: &str = "connections_changed";
/// A connection changed from another device: the app shows it again.
pub const EVENT: &str = "connections-changed";

/// A connection: the secure store's key for its token, and the library setting with the account's name.
pub struct Connection {
    pub name: &'static str,
    pub token_key: &'static str,
    pub login_setting: &'static str,
}

/// The connections shared across devices.
pub const CONNECTIONS: &[Connection] = &[Connection { name: "github", token_key: crate::github::TOKEN_KEY, login_setting: crate::github::LOGIN_SETTING }];

/// A connection on the share: signed in (the account, the token: encrypted, or plain on a share without a password)
/// or signed out (no token), and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Shared {
    login: Option<String>,
    /// Hex: the nonce and the sealed token (or the token itself, `plain`).
    token: Option<String>,
    #[serde(default)]
    plain: bool,
    changed_at: i64,
}

/// The key from the share's password.
fn key(password: &str) -> Aes256Gcm {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(password.as_bytes()).expect("any key length");
    mac.update(b"melue connections v1");
    Aes256Gcm::new_from_slice(&mac.finalize().into_bytes()).expect("32 bytes")
}

fn seal(password: Option<&str>, token: &str) -> Result<(String, bool), String> {
    let Some(password) = password else { return Ok((token.to_string(), true)) };
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    let sealed = key(password).encrypt(Nonce::from_slice(&nonce), token.as_bytes()).map_err(|_| "can't encrypt the sign-in".to_string())?;
    Ok((hex::encode([nonce.as_slice(), &sealed].concat()), false))
}

fn open(password: Option<&str>, shared: &Shared) -> Result<String, String> {
    let token = shared.token.as_deref().ok_or("signed out")?;
    if shared.plain {
        return Ok(token.to_string());
    }
    let password = password.ok_or("the share's password isn't known here")?;
    let bytes = hex::decode(token).map_err(|e| e.to_string())?;
    if bytes.len() < 12 {
        return Err("a damaged sign-in".into());
    }
    let (nonce, sealed) = bytes.split_at(12);
    let plain = key(password).decrypt(Nonce::from_slice(nonce), sealed).map_err(|_| "it was shared with another password".to_string())?;
    String::from_utf8(plain).map_err(|e| e.to_string())
}

fn changed_here(library: &Library) -> HashMap<String, i64> {
    library.setting(CHANGED).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Connection `name` was signed in or out here (now): the others follow with the next library data sync.
pub fn changed(app: &AppHandle, name: &str) {
    let library = app.state::<Library>();
    let mut changed = changed_here(&library);
    changed.insert(name.into(), now_ms());
    if let Ok(json) = serde_json::to_string(&changed) {
        let _ = library.set_setting(CHANGED, &json);
    }
    crate::sync::data::soon(app);
}

/// With the library data sync: each connection's newest change wins, here and on the share. Returns whether one
/// changed here (the app shows Connections again).
pub fn sync(app: &AppHandle, share: &dyn Share, password: Option<&str>) -> Result<bool, String> {
    let library = app.state::<Library>();
    let secrets = SystemSecrets::new(app);
    let mut on_share: HashMap<String, Shared> = match share.read(FILE) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(crate::share::ShareError::NotFound { .. }) => HashMap::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut changed = changed_here(&library);
    let (mut write, mut here_changed) = (false, false);
    for connection in CONNECTIONS {
        let token = secrets.get(connection.token_key)?;
        // Signed in before connections were shared: that counts as a change here, at the start of time.
        let mine = changed.get(connection.name).copied().unwrap_or(if token.is_some() { 1 } else { 0 });
        let theirs = on_share.get(connection.name).cloned();
        match theirs {
            Some(shared) if shared.changed_at > mine => {
                // Signed in or out on another device: the same here.
                match shared.token.is_some() {
                    true => match open(password, &shared) {
                        Ok(token) => {
                            secrets.set(connection.token_key, &token)?;
                            library.set_setting(connection.login_setting, shared.login.as_deref().unwrap_or_default())?;
                        }
                        Err(e) => {
                            eprintln!("melue: connections: can't take {}'s sign-in: {e}", connection.name);
                            continue;
                        }
                    },
                    false => {
                        let _ = secrets.delete(connection.token_key);
                        library.set_setting(connection.login_setting, "")?;
                    }
                }
                changed.insert(connection.name.into(), shared.changed_at);
                here_changed = true;
            }
            Some(shared) if shared.changed_at == mine => {}
            _ if mine == 0 => {}
            // Changed here last: the share gets it.
            _ => {
                let login = library.setting(connection.login_setting)?.filter(|l| !l.is_empty());
                let shared = match token {
                    Some(token) => {
                        let (token, plain) = seal(password, &token)?;
                        Shared { login, token: Some(token), plain, changed_at: mine }
                    }
                    None => Shared { login: None, token: None, plain: false, changed_at: mine },
                };
                on_share.insert(connection.name.into(), shared);
                write = true;
            }
        }
    }
    if write {
        share.create_dir_all(".melue").map_err(|e| e.to_string())?;
        share.write(FILE, &serde_json::to_vec_pretty(&on_share).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    if here_changed {
        library.set_setting(CHANGED, &serde_json::to_string(&changed).map_err(|e| e.to_string())?)?;
        let _ = app.emit(EVENT, ());
    }
    Ok(here_changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_in_is_sealed_with_the_shares_password_and_only_that_opens_it() {
        let (token, plain) = seal(Some("hunter2"), "gho_secret").unwrap();
        assert!(!plain && !token.contains("gho_secret"));
        let shared = Shared { login: Some("me".into()), token: Some(token), plain, changed_at: 5 };
        assert_eq!(open(Some("hunter2"), &shared).unwrap(), "gho_secret");
        assert!(open(Some("another"), &shared).is_err());
        assert!(open(None, &shared).is_err());
        // A share without a password: as it is.
        let (token, plain) = seal(None, "gho_secret").unwrap();
        assert!(plain);
        assert_eq!(open(None, &Shared { login: None, token: Some(token), plain, changed_at: 1 }).unwrap(), "gho_secret");
    }
}
