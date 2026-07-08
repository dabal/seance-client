//! Persistent app settings: saved hosts and Agent Bridge preferences, as a
//! JSON file in the platform config dir. Secrets never live here — saved
//! host passwords go in the OS credential store (see `hosts_*` commands),
//! keyed by `host:port`, and this file only records that one exists.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone)]
pub struct SavedHost {
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    /// A password for this host exists in the OS credential store.
    #[serde(default)]
    pub has_password: bool,
    /// Optional user-named folder the host is filed under ("Lab A", "Home",
    /// …). `None` renders at the drawer's top level.
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Settings {
    #[serde(default)]
    pub hosts: Vec<SavedHost>,
    /// Agent Bridge consent policy: "ask" | "allowlist" | "autonomous".
    #[serde(default)]
    pub consent_mode: Option<String>,
    #[serde(default)]
    pub allow_list: Vec<String>,
}

fn path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("settings.json")
}

pub fn load(app: &AppHandle) -> Settings {
    std::fs::read(path(app))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, s: &Settings) {
    let p = path(app);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(s) {
        let _ = std::fs::write(p, bytes);
    }
}
