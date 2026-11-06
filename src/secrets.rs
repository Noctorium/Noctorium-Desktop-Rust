//! Sign-ins, kept where the system keeps secrets: Credential Manager on Windows, the Keychain on a Mac, the
//! Secret Service on Linux -- as Noctorium Stats keeps its own. Where that cannot be reached, or
//! `NOCTORIUM_RUST_HOME` points somewhere for testing, in a file of this player's own instead, readable only
//! by its owner where the system allows it.

use crate::store::Places;
use std::collections::BTreeMap;

const SERVICE: &str = "Noctorium Rust";

fn use_keyring() -> bool {
    std::env::var_os("NOCTORIUM_RUST_HOME").is_none()
}

fn entry(name: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(SERVICE, name).ok()
}

fn file_secrets(places: &Places) -> BTreeMap<String, String> {
    places
        .file("secrets.json")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_file_secrets(places: &Places, secrets: &BTreeMap<String, String>) {
    let Some(path) = places.file("secrets.json") else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(secrets) {
        let _ = std::fs::write(&path, text);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

pub fn load(places: &Places, name: &str) -> Option<String> {
    if use_keyring() {
        if let Some(found) = entry(name).and_then(|e| e.get_password().ok()).filter(|s| !s.is_empty()) {
            return Some(found);
        }
    }
    file_secrets(places).get(name).cloned().filter(|s| !s.is_empty())
}

pub fn save(places: &Places, name: &str, secret: &str) {
    if use_keyring() && entry(name).is_some_and(|e| e.set_password(secret).is_ok()) {
        // A copy left in the file from a time the keyring could not be reached would outlive this one.
        let mut secrets = file_secrets(places);
        if secrets.remove(name).is_some() {
            write_file_secrets(places, &secrets);
        }
        return;
    }
    let mut secrets = file_secrets(places);
    secrets.insert(name.to_string(), secret.to_string());
    write_file_secrets(places, &secrets);
}

pub fn forget(places: &Places, name: &str) {
    if use_keyring() {
        if let Some(e) = entry(name) {
            let _ = e.delete_credential();
        }
    }
    let mut secrets = file_secrets(places);
    if secrets.remove(name).is_some() {
        write_file_secrets(places, &secrets);
    }
}
