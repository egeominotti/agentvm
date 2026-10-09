//! Launching a queued task: wait for a VM slot, prepare the job folder, boot the VM.

use std::sync::Arc;
use std::time::Instant;

use super::bundles;
use super::context::AppCtx;
use super::record::TaskRecord;
use super::supervise::supervise;
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::kernel::{GoldenKernel, disk_identity};
use crate::adapters::vm::{DirectBoot, VmConfig, VmProcess};
use crate::domain::boot::{boots_directly, kernel_cmdline};
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
    let (ws, token, direct) = prepare(ctx, id, &record).await?;
    apply(TaskEvent::Prepared)?;

    let stop_requested_early = ctx.store.stop_signal(id).is_some_and(|s| *s.borrow());
    if stop_requested_early {
        apply(TaskEvent::VmExited)?;
        return apply(TaskEvent::Finished(Final::Stopped, id.branch()));
    }

    let config = vm_config(&record, &ws, direct);
    let mut vm =
        VmProcess::spawn(&ctx.config.vm_helper, &ws.config_path(), &config, &ws.events()).map_err(|e| e.to_string())?;
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
async fn prepare(
    ctx: &AppCtx,
    id: &TaskId,
    record: &TaskRecord,
) -> Result<(JobWorkspace, Secret, Option<DirectBoot>), String> {
    let timeout_s = ctx.settings.get().timeout_s;
    let ws = JobWorkspace::create(&ctx.config.jobs(), id).map_err(|e| format!("job directory: {e}"))?;
    let t = Instant::now();
    let repo = record.restore_from.is_none().then(|| pack_repo(ctx, record, &ws));
    let keychain = ctx.keychain.clone();
    let joins = record.tailscale;
    let secrets = tokio::task::spawn_blocking(move || {
        (keychain.read_token(), joins.then(|| keychain.read_tailscale_key()).flatten())
    });
    let disk = match ws.write_spec(&task_spec(id, record, timeout_s)).map_err(|e| format!("task.json: {e}")) {
        Ok(()) => install_disk(ctx, record, &ws).await,
        Err(e) => Err(e),
    };
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
    let direct = disk?;
    let token = token?;
    ws.write_token(&token).map_err(|e| format!("token: {e}"))?;
    if record.tailscale {
        let spec = crate::domain::tailscale::spec_for(&record.id, &ctx.settings.get().tailscale);
        // A key removed since the launch was asked: the VM boots, and says it could not join.
        JobWorkspace::offer_tailnet(&ws.share(), &spec, tailscale_key.as_ref())
            .map_err(|e| format!("Tailscale: {e}"))?;
    }
    let how = if direct.is_some() { ", booting straight into its kernel" } else { "" };
    ctx.store.push_boot(id, format!("host: disk ready in {} ms{how}", t.elapsed().as_millis()));
    Ok((ws, token, direct))
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

/// The VM's disk: the snapshot's it is restored from (written back from its chunks, booted
/// through EFI with the kernel on it), or a clone of the golden image, booted straight into the
/// kernel kept beside the image when that kernel was built with this very disk.
async fn install_disk(ctx: &AppCtx, record: &TaskRecord, ws: &JobWorkspace) -> Result<Option<DirectBoot>, String> {
    let direct = match &record.restore_from {
        Some(snap) => {
            super::snapshot_disks::restore_disk(ctx, snap, ws.disk())
                .await
                .map_err(|e| format!("snapshot disk: {e}"))?;
            ws.copy_efivars(&ctx.snapshots.efivars(snap)).map_err(|e| format!("snapshot EFI variables: {e}"))?;
            None
        }
        None => {
            let golden = ctx.config.golden();
            let before = disk_identity(&golden).unwrap_or_default();
            ws.clone_disk(&golden).map_err(|e| format!("disk clone: {e}"))?;
            direct_boot(ctx, ws, &before)
        }
    };
    // Never boot through a link: the VM would write into the image it points to.
    ws.check_own_disk().map_err(|e| e.to_string())?;
    Ok(direct)
}

/// The golden image's kernel, copied beside the clone, if it was built with the disk just cloned
/// (`before` names the image as it was just before the clone). Anything amiss and the VM boots
/// through EFI and GRUB, as it did before kernels were kept beside the image.
fn direct_boot(ctx: &AppCtx, ws: &JobWorkspace, before: &str) -> Option<DirectBoot> {
    let golden = GoldenKernel::new(ctx.config.golden_kernel());
    let after = disk_identity(&ctx.config.golden()).ok()?;
    if !boots_directly(golden.stamp().as_deref(), before, &after) {
        return None;
    }
    let cmdline = kernel_cmdline(&golden.cmdline()?)?;
    let files =
        golden.copy_into(ws.dir()).inspect_err(|e| tracing::warn!(error = %e, "kernel copy failed: EFI boot")).ok()?;
    Some(DirectBoot { kernel: files.kernel, initrd: files.initrd, cmdline })
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

fn vm_config(record: &TaskRecord, ws: &JobWorkspace, direct: Option<DirectBoot>) -> VmConfig {
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
        direct,
    }
}
