//! Waiting for the VM to stop: stop requests, the backstop deadline, SIGTERM then SIGKILL.

use std::time::Duration;

use tokio::sync::watch;
use tokio::time::Instant;

use crate::adapters::vm::{VmEvent, VmProcess};
use crate::app::collect::VmEnd;
use crate::app::context::AppCtx;
use crate::domain::ids::TaskId;

const KILL_GRACE: Duration = Duration::from_secs(15);
const KILL_RETRY: Duration = Duration::from_secs(5);

/// How often the boot is followed until the VM is ready: a VM is used the moment it can be.
const BOOT_POLL: Duration = Duration::from_millis(50);

/// Which refresh is due: the full one, every second, or the boot's, every `BOOT_POLL` until the
/// VM is ready.
pub(super) enum Tick {
    Second,
    Boot,
}

/// Follows the VM from boot to shutdown. Stop and timeout also apply during boot;
/// if the helper ignores SIGTERM for `KILL_GRACE`, it gets SIGKILL. `on_tick(Tick::Second)` runs
/// every second, and the deadline is asked again after it (`None`: no deadline);
/// `on_tick(Tick::Boot)` runs every `BOOT_POLL` until it returns `true` (the VM is ready).
pub(super) async fn wait_for_vm(
    ctx: &AppCtx,
    id: &TaskId,
    vm: &mut VmProcess,
    mut deadline_of: impl FnMut() -> Option<std::time::Instant>,
    on_started: impl Fn(),
    mut on_tick: impl FnMut(Tick) -> bool,
) -> VmEnd {
    let (_never, fallback) = watch::channel(false);
    let mut stop = ctx.store.stop_signal(id).unwrap_or(fallback);
    let far = || Instant::now() + Duration::from_secs(100 * 365 * 86_400);
    let deadline = tokio::time::sleep_until(deadline_of().map_or_else(far, Into::into));
    let kill_timer = tokio::time::sleep(Duration::MAX / 4);
    let mut ticks = tokio::time::interval(Duration::from_secs(1));
    let mut boot_ticks = tokio::time::interval(BOOT_POLL);
    let mut booted = false;
    tokio::pin!(deadline, kill_timer);
    let (mut stop_requested, mut timed_out) = (false, false);
    loop {
        let terminating = stop_requested || timed_out;
        tokio::select! {
            event = vm.next_event() => match event {
                Some(VmEvent::Started) => on_started(),
                Some(_) => {}
                None => break,
            },
            _ = stop.wait_for(|s| *s), if !terminating => stop_requested = true,
            _ = &mut deadline, if !terminating => timed_out = true,
            _ = &mut kill_timer, if terminating => {
                vm.kill();
                // Once elapsed, a sleep stays ready: without a new deadline this loop would spin,
                // sending SIGKILL as fast as it can until the helper is gone.
                kill_timer.as_mut().reset(Instant::now() + KILL_RETRY);
            }
            _ = boot_ticks.tick(), if !booted => booted = on_tick(Tick::Boot),
            _ = ticks.tick() => {
                on_tick(Tick::Second);
                if !terminating {
                    deadline.as_mut().reset(deadline_of().map_or_else(far, Into::into));
                }
            }
        }
        if !terminating && (stop_requested || timed_out) {
            vm.terminate();
            kill_timer.as_mut().reset(Instant::now() + KILL_GRACE);
        }
    }
    VmEnd { exit: vm.wait().await, stop_requested, timed_out }
}
