//! The server's time limit for an automatic task. The guest enforces the limit itself and saves
//! the work; this is the backstop if it cannot. It counts from when Claude starts, as the guest
//! does: boot and `setup.sh` can take long. After a server restart it still counts from Claude's
//! real start (the guest's uptime then and now), never from the restart.

use std::time::{Duration, Instant};

use crate::adapters::jobdir::JobWorkspace;
use crate::app::context::AppCtx;
use crate::app::record::TaskRecord;
use crate::domain::outcome::{PREPARE_MAX, TIMEOUT_GRACE, claude_start_uptime};

pub(super) struct Backstop<'a> {
    ws: &'a JobWorkspace,
    limit: Duration,
    /// When this server started following the VM.
    spawned: Instant,
    /// When Claude started; `None` until the guest logs it.
    claude_started: Option<Instant>,
}

impl<'a> Backstop<'a> {
    /// `None` for interactive terminals: they have no time limit.
    pub(super) fn new(ctx: &AppCtx, record: &TaskRecord, ws: &'a JobWorkspace) -> Option<Self> {
        // The guest keeps the limit it received at launch: a settings update or server restart
        // must not shorten it through the server's backstop.
        // The spec includes the prompt, which can approach the API's 2 MiB request limit.
        let saved = crate::guestfs::read(&ws.share().join("task.json"), 4 << 20)
            .and_then(|bytes| serde_json::from_slice::<crate::domain::spec::TaskSpec>(&bytes).ok());
        let timeout_s = saved
            .map(|spec| spec.timeout_s)
            .filter(|timeout| (60..=86_400).contains(timeout))
            .unwrap_or_else(|| ctx.settings.get().timeout_s);
        let limit = Duration::from_secs(timeout_s);
        (!record.interactive).then_some(Backstop { ws, limit, spawned: Instant::now(), claude_started: None })
    }

    /// When the server stops the VM, as known now (it moves once Claude starts).
    pub(super) fn deadline(&mut self) -> Instant {
        if self.claude_started.is_none()
            && let Some(at) = self.ws.job_log().iter().find_map(|l| claude_start_uptime(l))
        {
            // How long ago, from the guest's uptime now; unknown (no metrics yet): just now.
            let ago = self
                .ws
                .read_metrics()
                .map(|m| m.uptime_s as f64 - at)
                .filter(|s| s.is_finite() && *s > 0.0)
                .map_or(Duration::ZERO, Duration::from_secs_f64);
            self.claude_started = Some(Instant::now().checked_sub(ago).unwrap_or(self.spawned));
        }
        match self.claude_started {
            Some(start) => start + self.limit + TIMEOUT_GRACE,
            None => self.spawned + PREPARE_MAX + self.limit + TIMEOUT_GRACE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::keychain::Keychain;
    use crate::config::Config;
    use crate::domain::ids::{CommitSha, RepoPath, TaskId};
    use crate::domain::spec::TaskSpec;

    fn timeout_for_prompt(prompt: String) -> Duration {
        let home = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("git").args(["init", "-q"]).arg(repo.path()).status().unwrap().success());
        let ctx = AppCtx::new(
            Config {
                home: home.path().into(),
                port: 7791,
                concurrency: 1,
                cpus: 1,
                memory_mb: 1024,
                timeout_s: 60,
                vm_helper: "agentvm-vm".into(),
                scripts_dir: "scripts".into(),
                min_free_mb: 0,
            },
            Keychain::new(Some(home.path().join("none.keychain-db"))),
        );
        let id = TaskId::generate(std::time::SystemTime::now(), &[1, 2]);
        let record = TaskRecord::new(
            id.clone(),
            RepoPath::new(repo.path().into()).unwrap(),
            None,
            CommitSha::parse(&"a".repeat(40)).unwrap(),
            false,
        );
        let ws = JobWorkspace::create(&ctx.config.jobs(), &id).unwrap();
        ws.write_spec(&TaskSpec {
            id: id.to_string(),
            prompt,
            branch: id.branch(),
            base_sha: record.base_sha.as_str().into(),
            timeout_s: 1800,
            interactive: false,
            model: None,
            claude_version: None,
            restore: false,
        })
        .unwrap();
        let backstop = Backstop::new(&ctx, &record, &ws).unwrap();
        backstop.limit
    }

    #[test]
    fn a_restarted_supervisor_keeps_the_guest_original_timeout() {
        assert_eq!(
            timeout_for_prompt("original task".into()),
            Duration::from_secs(1800),
            "new settings must not shorten an existing task",
        );
    }

    #[test]
    fn a_large_prompt_keeps_the_original_timeout_after_restart() {
        assert_eq!(
            timeout_for_prompt("x".repeat(1536 << 10)),
            Duration::from_secs(1800),
            "a prompt below the API body limit must not hide the saved timeout",
        );
    }
}
