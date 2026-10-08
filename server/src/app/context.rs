//! The application context: every service a use case needs, built once at startup.

use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use super::golden::GoldenService;
use super::guest_channel::GuestChannel;
use super::ports::PortForwards;
use super::scheduler::Scheduler;
use super::settings::{SettingsService, UpdateError};
use super::store::Store;
use crate::adapters::keychain::Keychain;
use crate::adapters::releases::Releases;
use crate::adapters::snapshots::SnapshotStore;
use crate::config::Config;
use crate::domain::settings::{Model, Settings};

/// Claude Code releases as served to the dashboard.
pub type ClaudeReleases = Releases;

/// How long the list of published Claude Code versions stays fresh.
const RELEASES_TTL: Duration = Duration::from_secs(600);

pub struct AppCtx {
    pub config: Config,
    pub store: Store,
    pub scheduler: Scheduler,
    pub keychain: Keychain,
    pub settings: SettingsService,
    pub golden: GoldenService,
    pub snapshots: SnapshotStore,
    pub forwards: PortForwards,
    /// Requests to running VMs (save, flush, close), one at a time per VM.
    pub guest: GuestChannel,
    releases: Mutex<Option<(Instant, Releases)>>,
}

impl AppCtx {
    /// Settings saved in `AGENTVM_HOME` win over the environment defaults in `config`.
    pub fn new(config: Config, keychain: Keychain) -> Self {
        let limits = crate::adapters::host::host_limits();
        let defaults = Settings {
            max_vms: config.concurrency,
            cpus: config.cpus.min(limits.cpus),
            memory_mb: config.memory_mb,
            timeout_s: config.timeout_s,
            model: Model::default_choice(),
            default_repo: None,
            claude_version: Default::default(),
            s3: None,
            auto_snapshots: Default::default(),
        };
        let settings = SettingsService::load(config.home.join("settings.json"), defaults, limits);
        AppCtx {
            scheduler: Scheduler::new(settings.get().max_vms),
            store: Store::persistent(config.jobs()),
            golden: GoldenService::new(config.home.clone(), config.scripts_dir.join("build-golden.sh")),
            keychain,
            settings,
            snapshots: SnapshotStore::new(config.home.join("snapshots")),
            forwards: Default::default(),
            guest: Default::default(),
            releases: Mutex::new(None),
            config,
        }
    }

    /// Published Claude Code versions, cached for 10 minutes.
    pub async fn claude_releases(&self) -> Result<Releases, String> {
        let mut cache = self.releases.lock().await;
        if let Some((at, r)) = cache.as_ref()
            && at.elapsed() < RELEASES_TTL
        {
            return Ok(r.clone());
        }
        let fresh = tokio::task::spawn_blocking(crate::adapters::releases::fetch)
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        *cache = Some((Instant::now(), fresh.clone()));
        Ok(fresh)
    }

    pub fn update_settings(&self, new: Settings) -> Result<Settings, UpdateError> {
        let saved = self.settings.update(new)?;
        self.scheduler.resize(saved.max_vms);
        Ok(saved)
    }
}
