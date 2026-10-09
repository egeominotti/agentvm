//! Current settings: loaded at start-up, validated, persisted and applied at runtime.

use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

use crate::adapters::settings_file::{self, Saved};
use crate::domain::settings::{HostLimits, Settings, SettingsError};
use crate::domain::settings_recovery::recover;

pub struct SettingsService {
    path: PathBuf,
    limits: HostLimits,
    current: RwLock<Settings>,
    /// One save at a time, with what follows it; reads only take `current`, never this.
    writer: Mutex<()>,
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
        SettingsService { path, limits, current: RwLock::new(current), writer: Mutex::new(()) }
    }

    pub fn get(&self) -> Settings {
        self.current.read().unwrap().clone()
    }

    pub fn limits(&self) -> HostLimits {
        self.limits
    }

    /// Everything but the S3 bucket, which only `set_s3` changes: the settings page sends the
    /// values it loaded, and a bucket configured since then must not be wiped by them.
    pub fn update(&self, new: Settings) -> Result<Settings, UpdateError> {
        self.update_then(new, |_| {})
    }

    /// `update`, then `then` with the saved settings, both before any other save: what follows a
    /// save (the scheduler's size) always matches the file. Readers never wait for the disk.
    pub fn update_then(&self, mut new: Settings, then: impl FnOnce(&Settings)) -> Result<Settings, UpdateError> {
        let _writer = self.writer.lock().unwrap();
        new.s3 = self.get().s3;
        new.validate(&self.limits)?;
        settings_file::save(&self.path, &new)?;
        *self.current.write().unwrap() = new.clone();
        then(&new);
        Ok(new)
    }

    pub fn set_s3(&self, s3: Option<crate::domain::s3::S3Config>) -> Result<Settings, UpdateError> {
        let _writer = self.writer.lock().unwrap();
        let new = Settings { s3, ..self.get() };
        settings_file::save(&self.path, &new)?;
        *self.current.write().unwrap() = new.clone();
        Ok(new)
    }
}
