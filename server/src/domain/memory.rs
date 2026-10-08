//! How much of its memory a VM keeps: idle machines give the rest back to the Mac (memory
//! balloon), busy ones always have everything they were given.

/// Seconds without work before a VM gives memory back.
pub const IDLE_AFTER_S: u64 = 60;
const HEADROOM_MB: u64 = 1024;
const STEP_MB: u64 = 256;
const FLOOR_MB: u64 = 1024;

pub fn memory_target(configured_mb: u64, used_mb: u64, busy: bool, idle_for_s: u64) -> u64 {
    if busy || idle_for_s <= IDLE_AFTER_S {
        return configured_mb;
    }
    let wanted = (used_mb + HEADROOM_MB).div_ceil(STEP_MB) * STEP_MB;
    wanted.max(FLOOR_MB).min(configured_mb)
}
