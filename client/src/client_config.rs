use std::{fs, io::ErrorKind, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

const DEFAULT_HUB: &str = "ws://127.0.0.1:8080/ws/client";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClientConfig {
    #[serde(default = "default_hub")]
    pub(crate) hub: String,
    #[serde(default)]
    pub(crate) token: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            hub: default_hub(),
            token: None,
        }
    }
}

pub(crate) fn default_hub() -> String {
    DEFAULT_HUB.to_string()
}

pub(crate) fn load() -> Result<ClientConfig> {
    let path = config_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => {
            toml::from_str(&text).with_context(|| format!("invalid config {}", path.display()))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(ClientConfig::default()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub(crate) fn save(config: &ClientConfig) -> Result<PathBuf> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(config)?;
    write_secret_file(&path, text.as_bytes())?;
    Ok(path)
}

pub(crate) fn resolve_hub(cli_hub: Option<&str>) -> Result<String> {
    if let Some(hub) = cli_hub {
        return Ok(hub.to_string());
    }
    if let Ok(hub) = std::env::var("PUMPKINPI_HUB") {
        return Ok(hub);
    }
    Ok(load()?.hub)
}

pub(crate) fn resolve_token() -> Result<Option<String>> {
    if let Ok(token) = std::env::var("PUMPKINPI_TOKEN") {
        return Ok(Some(token));
    }
    Ok(load()?.token)
}

pub(crate) fn config_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("PUMPKINPI_CLIENT_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home).join("pumpkinpi/client.toml"));
    }
    let home = std::env::var("HOME").context("HOME is required to locate client config")?;
    Ok(PathBuf::from(home).join(".config/pumpkinpi/client.toml"))
}

#[cfg(unix)]
fn write_secret_file(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true).mode(0o600);
    std::io::Write::write_all(&mut options.open(path)?, bytes)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(not(unix))]
fn write_secret_file(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn login(hub: String, token: String) -> Result<PathBuf> {
    if token.trim().is_empty() {
        return Err(anyhow!("token cannot be empty"));
    }
    save(&ClientConfig {
        hub,
        token: Some(token),
    })
}

pub(crate) fn logout() -> Result<PathBuf> {
    let mut config = load()?;
    config.token = None;
    save(&config)
}
