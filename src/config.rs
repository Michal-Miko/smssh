use color_eyre::{eyre::Context, Result};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::cli::AliasKind;

static CONFIG_FILE_NAME: &str = "smssh.yaml";
static CONFIG_DIR_FALLBACK: &str = "~/.config";

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub key_aliases: HashMap<String, KeyAliasConfig>,
    pub hosts: HashMap<String, HostConfig>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum KeyAliasConfig {
    SecretsManager { secret_arn: String },
}

impl From<AliasKind> for KeyAliasConfig {
    fn from(kind: AliasKind) -> Self {
        match kind {
            AliasKind::SecretsManager { secret_arn, .. } => Self::SecretsManager { secret_arn },
        }
    }
}

impl Display for KeyAliasConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let yaml = serde_yaml::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", yaml)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HostConfig {
    pub key_alias: String,
    pub args: Vec<String>,
    pub destination: String,
}

impl Display for HostConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let yaml = serde_yaml::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", yaml)
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(CONFIG_DIR_FALLBACK))
            .join(CONFIG_FILE_NAME)
    }

    pub fn store(&self) -> Result<()> {
        let path = Self::config_path();
        let yaml = serde_yaml::to_string(&self)?;
        std::fs::write(path, yaml).wrap_err("Failed to write config file")?;
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let mut config = Self::new();

        let path = Self::config_path();
        if path.exists() {
            let yaml =
                std::fs::read_to_string(path).wrap_err("Failed to read config file at {path:?}")?;
            config =
                serde_yaml::from_str(&yaml).wrap_err("Failed to parse config from {path:?}")?;
        }

        Ok(config)
    }
}
