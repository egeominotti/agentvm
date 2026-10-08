//! Randomness for the ids the use cases generate (tasks, snapshots, backups).

use std::io::Read;
use std::time::SystemTime;

/// Two bytes from `/dev/urandom`, or from the clock when it cannot be read.
pub fn random_bytes() -> [u8; 2] {
    let mut b = [0u8; 2];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_err() {
        let n = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        b = [(n >> 8) as u8, n as u8];
    }
    b
}
