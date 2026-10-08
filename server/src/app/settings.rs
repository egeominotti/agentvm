//! Current settings: loaded at start-up, validated, persisted and applied at runtime.

use std::path::PathBuf;
use std::sync::RwLock;

use crate::adapters::settings_file::{self, Saved};
use crate::domain::settings::{HostLimits, Settings, SettingsError};
use crate::domain::settings_recovery::recover;

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
    /// Saved settings win over `defaults`, field by field: one that no longer fits this Mac (or
    /// this version) falls back to its default alone. The file is copied aside first, if so.
    pub fn load(path: PathBuf, defaults: Settings, limits: HostLimits) -> Self {
        let (current, reset) = match settings_file::load(&path) {
            Saved::Missing => (defaults, vec![]),
            Saved::Corrupt => (defaults, vec!["(the whole file)".to_owned()]),
            Saved::Json(saved) => recover(&saved, &defaults, &limits),
        };
        if !reset.is_empty() {
            let copy = settings_file::keep_copy(&path)
                .map_or_else(|e| format!("not copied: {e}"), |p| p.display().to_string());
            eprintln!("settings: reset to default {} (the file as it was: {copy})", reset.join(", "));
        }
        SettingsService { path, limits, current: RwLock::new(current) }
    }

    pub fn get(&self) -> Settings {
        self.current.read().unwrap().clone()
    }

    pub fn limits(&self) -> HostLimits {
        self.limits
    }

    /// Everything but the S3 bucket, which only `set_s3` changes: the settings page sends the
    /// values it loaded, and a bucket configured since then must not be wiped by them.
    pub fn update(&self, mut new: Settings) -> Result<Settings, UpdateError> {
        let mut current = self.current.write().unwrap();
        new.s3 = current.s3.clone();
        new.validate(&self.limits)?;
        settings_file::save(&self.path, &new)?;
        *current = new.clone();
        Ok(new)
    }

    pub fn set_s3(&self, s3: Option<crate::domain::s3::S3Config>) -> Result<Settings, UpdateError> {
        let mut current = self.current.write().unwrap();
        let new = Settings { s3, ..current.clone() };
        settings_file::save(&self.path, &new)?;
        *current = new.clone();
        Ok(new)
    }
}
