//! Randomness for the ids the use cases generate (tasks, snapshots, backups).

use std::io::Read;
use std::time::SystemTime;

/// Ten bytes from `/dev/urandom` (the 74 random bits of a UUIDv7), or from the clock when it
/// cannot be read.
pub fn random_bytes() -> [u8; 10] {
    let mut b = [0u8; 10];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_err() {
        let n = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        b.copy_from_slice(&n.to_le_bytes()[..10]);
    }
    b
}
