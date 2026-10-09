//! Launching a queued task: wait for a VM slot, prepare the job folder, boot the VM.

use std::sync::Arc;
use std::time::Instant;

use super::bundles;
use super::context::AppCtx;
use super::record::TaskRecord;
use super::supervise::supervise;
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::vm::{VmConfig, VmProcess};
use crate::domain::ids::TaskId;
use crate::domain::outcome::Final;
use crate::domain::spec::TaskSpec;
use crate::domain::task::{TaskEvent, TaskState};
use crate::secret::Secret;

/// Runs a queued task once a VM slot is free; a stop while it waits ends it there.
pub(super) async fn run(ctx: Arc<AppCtx>, id: TaskId) {
    let Some(mut stop) = ctx.store.stop_signal(&id) else { return };
    let Some(memory_mb) = ctx.store.get(&id).map(|r| r.memory_mb) else { return };
    // This Mac's memory minus what macOS keeps: VMs beyond it would push the Mac into swap.
    let budget_mb = ctx.settings.limits().max_memory_mb();
    if ctx.scheduler.reserved_mb() + memory_mb > budget_mb {
        ctx.store.push_boot(&id, "host: waiting for memory: other VMs are using it".into());
        tracing::info!(task = %id, memory_mb, reserved_mb = ctx.scheduler.reserved_mb(), budget_mb, "waiting for memory");
    }
    let _slot = tokio::select! {
        slot = ctx.scheduler.acquire_with(memory_mb, budget_mb, &crate::adapters::host::memory_free_mb) => slot,
        _ = stop.wait_for(|s| *s) => return, // stopped while queued
    };
    if ctx.store.get(&id).is_none_or(|r| r.state != TaskState::Queued) {
        return;
    }
    if let Err(reason) = execute(&ctx, &id).await {
        let _ = ctx.store.apply(&id, TaskEvent::Failure(reason));
    }
}

/// Errors before the VM shuts down → `Err(reason)`; the normal outcome goes through `Finished`.
async fn execute(ctx: &AppCtx, id: &TaskId) -> Result<(), String> {
    let apply = |e| ctx.store.apply(id, e).map(drop).map_err(|e| e.to_string());
    apply(TaskEvent::SlotAcquired)?;
    let record = ctx.store.get(id).ok_or("task disappeared")?;
    let (ws, token) = prepare(ctx, id, &record).await?;
    apply(TaskEvent::Prepared)?;

    let stop_requested_early = ctx.store.stop_signal(id).is_some_and(|s| *s.borrow());
    if stop_requested_early {
        apply(TaskEvent::VmExited)?;
        return apply(TaskEvent::Finished(Final::Stopped, id.branch()));
    }

    let mut vm = VmProcess::spawn(&ctx.config.vm_helper, &ws.config_path(), &vm_config(&record, &ws), &ws.events())
        .map_err(|e| e.to_string())?;
    // Without its pid on disk a restart could not find this VM again: it would run unsupervised.
    if let Err(e) = ws.write_pid(vm.pid()) {
        vm.kill();
        return Err(format!("could not record the VM's process id ({e}): it was stopped"));
    }
    tracing::info!(task = %id, pid = vm.pid(), cpus = record.cpus, memory_mb = record.memory_mb, "vm started");
    supervise(ctx, id, &record, ws, vm, token).await
}

/// Fills the job folder with everything the VM boots from: repository, task spec, token, disk.
/// The repository, the token and the disk do not depend on each other: they are prepared at once,
/// the first two on blocking threads while the disk is cloned.
async fn prepare(ctx: &AppCtx, id: &TaskId, record: &TaskRecord) -> Result<(JobWorkspace, Secret), String> {
    let timeout_s = ctx.settings.get().timeout_s;
    let ws = JobWorkspace::create(&ctx.config.jobs(), id).map_err(|e| format!("job directory: {e}"))?;
    let t = Instant::now();
    let repo = record.restore_from.is_none().then(|| pack_repo(ctx, record, &ws));
    let keychain = ctx.keychain.clone();
    let joins = record.tailscale;
    let secrets = tokio::task::spawn_blocking(move || {
        (keychain.read_token(), joins.then(|| keychain.read_tailscale_key()).flatten())
    });
    let disk = ws
        .write_spec(&task_spec(id, record, timeout_s))
        .map_err(|e| format!("task.json: {e}"))
        .and_then(|()| install_disk(ctx, record, &ws));
    // Every task is awaited before any error returns: none may still write into a folder that
    // is being removed.
    let repo = match repo {
        Some(task) => Some(task.await.map_err(|e| e.to_string()).and_then(|r| r)),
        None => None,
    };
    let (token, tailscale_key) = match secrets.await {
        Ok((token, key)) => (token.map_err(|e| e.to_string()), key),
        Err(e) => (Err(e.to_string()), None),
    };
    if let Some(repo) = repo {
        let (shared, took) = repo?;
        let how = if shared { "shared" } else { "packed" };
        ctx.store.push_boot(id, format!("host: repository {how} in {} ms", took.as_millis()));
    }
    disk?;
    let token = token?;
    ws.write_token(&token).map_err(|e| format!("token: {e}"))?;
    if record.tailscale {
        let spec = crate::domain::tailscale::spec_for(&super::proxy::vm_name(record), &ctx.settings.get().tailscale);
        // A key removed since the launch was asked: the VM boots, and says it could not join.
        JobWorkspace::offer_tailnet(&ws.share(), &spec, tailscale_key.as_ref())
            .map_err(|e| format!("Tailscale: {e}"))?;
    }
    ctx.store.push_boot(id, format!("host: disk ready in {} ms", t.elapsed().as_millis()));
    Ok((ws, token))
}

/// Puts the repository bundle in the job folder, on a blocking thread (packing a large repository
/// takes seconds): `true` when a shared bundle was reused, and how long it took.
fn pack_repo(
    ctx: &AppCtx,
    record: &TaskRecord,
    ws: &JobWorkspace,
) -> tokio::task::JoinHandle<Result<(bool, std::time::Duration), String>> {
    let (home, repo, dest) = (ctx.config.home.clone(), record.repo.as_path().to_path_buf(), ws.repo_bundle());
    let base = record.base_sha.clone();
    tokio::task::spawn_blocking(move || {
        let t = Instant::now();
        bundles::prepare(&home, &repo, &dest, &base).map(|shared| (shared, t.elapsed()))
    })
}

/// The VM's disk: a clone of the snapshot it is restored from, or of the golden image.
fn install_disk(ctx: &AppCtx, record: &TaskRecord, ws: &JobWorkspace) -> Result<(), String> {
    match &record.restore_from {
        Some(snap) => {
            ws.clone_disk(&ctx.snapshots.disk(snap)).map_err(|e| format!("snapshot disk clone: {e}"))?;
            ws.copy_efivars(&ctx.snapshots.efivars(snap)).map_err(|e| format!("snapshot EFI variables: {e}"))?;
        }
        None => ws.clone_disk(&ctx.config.golden()).map_err(|e| format!("disk clone: {e}"))?,
    }
    // Never boot through a link: the VM would write into the image it points to.
    ws.check_own_disk().map_err(|e| e.to_string())
}

/// What the guest job reads from `task.json`.
fn task_spec(id: &TaskId, record: &TaskRecord, timeout_s: u64) -> TaskSpec {
    TaskSpec {
        id: id.to_string(),
        prompt: record.prompt.as_ref().map(|p| p.as_str().to_owned()).unwrap_or_default(),
        branch: id.branch(),
        base_sha: record.base_sha.as_str().to_owned(),
        timeout_s,
        interactive: record.interactive,
        model: record.model.cli_name().map(str::to_owned),
        claude_version: record.claude_version.clone(),
        restore: record.restore_from.is_some(),
    }
}

fn vm_config(record: &TaskRecord, ws: &JobWorkspace) -> VmConfig {
    VmConfig {
        disk: ws.disk(),
        efivars: ws.efivars(),
        share: ws.share(),
        console: ws.console(),
        cpus: record.cpus,
        memory_mb: record.memory_mb,
        seed_iso: None,
        pty_socket: record.interactive.then(|| ws.pty_socket()),
        balloon: Some(ws.balloon()),
    }
}
