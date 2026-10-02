//! The sign-in of each profile: kept in the system's keychain (macOS Keychain,
//! Windows Credential Manager, the Secret Service on Linux), or in
//! `credentials.toml` beside the configuration, readable by its owner only,
//! when there is no keychain or `ALMENA_CREDENTIAL_STORE=file` asks for it.
//!
//! `ALMENA_TOKEN`, when set, is used instead of whatever is kept (see
//! `context`): that is how scripts and CI sign in.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "almena";
const FILE: &str = "credentials.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stored {
    pub token: String,
    pub kind: Kind,
    /// RFC 3339, as the API gave it.
    pub expires_at: Option<String>,
    /// The API it is good for.
    pub api_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A sign-in: ends after hours, or sooner when unused.
    Session,
    /// An API token: ends on its date only.
    ApiToken,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    profiles: BTreeMap<String, Stored>,
}

fn file_only() -> bool {
    std::env::var("ALMENA_CREDENTIAL_STORE").is_ok_and(|store| store == "file")
}

fn entry(profile: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, profile)
}

pub fn load(dir: &Path, profile: &str) -> anyhow::Result<Option<Stored>> {
    if !file_only()
        && let Ok(secret) = entry(profile).and_then(|entry| entry.get_password())
    {
        return Ok(Some(
            serde_json::from_str(&secret).context("reading the keychain's sign-in")?,
        ));
    }
    Ok(read_file(dir)?.profiles.remove(profile))
}

/// Keeps the sign-in; says where when it is not the keychain.
pub fn save(dir: &Path, profile: &str, stored: &Stored) -> anyhow::Result<()> {
    if !file_only() {
        let secret = serde_json::to_string(stored)?;
        match entry(profile).and_then(|entry| entry.set_password(&secret)) {
            Ok(()) => {
                // A copy left in the file from before would shadow nothing,
                // but it should not linger either.
                return remove_from_file(dir, profile);
            }
            Err(error) => eprintln!(
                "No keychain to keep the sign-in in ({error}); keeping it in {}",
                dir.join(FILE).display()
            ),
        }
    }
    let mut file = read_file(dir)?;
    file.profiles.insert(profile.to_owned(), stored.clone());
    write_file(dir, &file)
}

pub fn delete(dir: &Path, profile: &str) -> anyhow::Result<()> {
    if !file_only() {
        // Nothing there, or no keychain at all: either way only the file is left.
        let _ = entry(profile).and_then(|entry| entry.delete_credential());
    }
    remove_from_file(dir, profile)
}

fn remove_from_file(dir: &Path, profile: &str) -> anyhow::Result<()> {
    let mut file = read_file(dir)?;
    if file.profiles.remove(profile).is_some() {
        write_file(dir, &file)?;
    }
    Ok(())
}

fn read_file(dir: &Path) -> anyhow::Result<File> {
    let path = dir.join(FILE);
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("reading {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(File::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

fn write_file(dir: &Path, file: &File) -> anyhow::Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(FILE);
    let text = toml::to_string_pretty(file)?;
    write_private(&path, &text).with_context(|| format!("writing {}", path.display()))
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // An older file may have been made with wider permissions.
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    // The user's profile directory is already private to them on Windows.
    fs::write(path, text)
}
