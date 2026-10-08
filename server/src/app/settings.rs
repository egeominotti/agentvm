//! Current settings: loaded at start-up, validated, persisted and applied at runtime.

use std::path::PathBuf;
use std::sync::RwLock;

use crate::adapters::settings_file;
use crate::domain::settings::{HostLimits, Settings, SettingsError};

pub struct SettingsService {
    path: PathBuf,
    limits: HostLimits,
    current: RwLock<Settings>,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error(transparent)]
    Invalid(#[from] SettingsError),
    #[error("could not save the settings: {0}")]
    Save(#[from] std::io::Error),
}

impl SettingsService {
    /// Saved settings win over `defaults`, unless they no longer fit this Mac.
    pub fn load(path: PathBuf, defaults: Settings, limits: HostLimits) -> Self {
        let saved = settings_file::load(&path).filter(|s| s.validate(&limits).is_ok());
        SettingsService { path, limits, current: RwLock::new(saved.unwrap_or(defaults)) }
    }

    pub fn get(&self) -> Settings {
        self.current.read().unwrap().clone()
    }

    pub fn limits(&self) -> HostLimits {
        self.limits
    }

    pub fn update(&self, new: Settings) -> Result<Settings, UpdateError> {
        new.validate(&self.limits)?;
        settings_file::save(&self.path, &new)?;
        *self.current.write().unwrap() = new.clone();
        Ok(new)
    }
}
