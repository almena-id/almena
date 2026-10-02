//! Profiles: named sets of settings in `config.toml`, in the configuration
//! directory (`ALMENA_CONFIG_DIR`, else the system's place for it).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::cli::{Locale, OutputFormat};

const FILE: &str = "config.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_url: Option<String>,
    /// The tenant's id (`almena tenant use`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
}

/// Where the configuration (and, without a keychain, the sign-in) is kept.
pub fn dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("ALMENA_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("id", "almena", "almena")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .context("no home directory to keep the configuration in (set ALMENA_CONFIG_DIR)")
}

pub fn load(dir: &Path) -> anyhow::Result<Config> {
    let path = dir.join(FILE);
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("reading {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn save(dir: &Path, config: &Config) -> anyhow::Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(FILE);
    fs::write(&path, toml::to_string_pretty(config)?)
        .with_context(|| format!("writing {}", path.display()))
}
