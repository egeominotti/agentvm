//! How much of the Mac's disk must stay free: VM disks grow as their guests write, and a full
//! disk fails them halfway (saves, snapshots, the server's own state).

/// The disk is too full to start anything new.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("only {} GB free on this Mac's disk (agentvm keeps at least {} GB free): free some space", .free_mb >> 10, .min_free_mb >> 10)]
pub struct DiskFull {
    pub free_mb: u64,
    pub min_free_mb: u64,
}

/// `Ok` when `free_mb` leaves at least `min_free_mb` free.
pub fn check_free(free_mb: u64, min_free_mb: u64) -> Result<(), DiskFull> {
    if free_mb >= min_free_mb { Ok(()) } else { Err(DiskFull { free_mb, min_free_mb }) }
}
