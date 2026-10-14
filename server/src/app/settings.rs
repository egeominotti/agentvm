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
    #[error("could not save the provider credential: {0}")]
    ProviderCredential(String),
    #[error("could not save the provider settings ({save}); provider rollback failed: {rollback}")]
    ProviderRollback {
        #[source]
        save: std::io::Error,
        rollback: String,
    },
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
        let current = self.get();
        new.s3 = current.s3.clone();
        new.providers = current.providers.clone();
        reset_inherited_model(&mut new, &current);
        new.validate(&self.limits)?;
        settings_file::save(&self.path, &new)?;
        *self.current.write().unwrap() = new.clone();
        then(&new);
        Ok(new)
    }

    /// The credential is changed only after validation, under the same writer lock as settings.
    pub fn set_provider_with<R>(
        &self,
        provider: crate::domain::providers::Provider,
        config: crate::domain::providers::ProviderConfig,
        credential: impl FnOnce() -> Result<R, String>,
        rollback: impl FnOnce(R) -> Result<(), String>,
    ) -> Result<Settings, UpdateError> {
        let _writer = self.writer.lock().unwrap();
        let old = self.get();
        let mut new = old.clone();
        new.providers.insert(provider, config);
        reset_inherited_model(&mut new, &old);
        new.validate(&self.limits)?;
        let previous_credential = credential().map_err(UpdateError::ProviderCredential)?;
        if let Err(save) = settings_file::save(&self.path, &new) {
            let mut failures = Vec::new();
            if let Err(error) = rollback(previous_credential) {
                failures.push(format!("credential: {error}"));
            }
            // The directory flush can fail after atomic_file has already renamed the new file.
            if settings_file::load(&self.path) == Saved::Json(serde_json::to_value(&new).expect("settings serialize"))
                && let Err(error) = settings_file::save(&self.path, &old)
            {
                failures.push(format!("configuration: {error}"));
            }
            if !failures.is_empty() {
                return Err(UpdateError::ProviderRollback { save, rollback: failures.join("; ") });
            }
            return Err(UpdateError::Save(save));
        }
        *self.current.write().unwrap() = new.clone();
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

/// A full settings form may carry the previous CLI's model without offering a model input.
fn reset_inherited_model(new: &mut Settings, old: &Settings) {
    let protocol = |settings: &Settings| {
        settings.agent.resolved_provider().map(|provider| {
            settings.providers.get(&provider).map_or_else(|| provider.config().protocol, |config| config.protocol)
        })
    };
    let changed = new.agent.kind != old.agent.kind
        || new.agent.resolved_provider() != old.agent.resolved_provider()
        || new.agent.resolved_account() != old.agent.resolved_account()
        || protocol(new) != protocol(old);
    if changed && new.model == old.model {
        new.model = crate::domain::settings::Model::default_choice();
    }
}
