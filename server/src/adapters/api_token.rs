//! Secrets that let the browser (and only this user) use the server: other users of the Mac can
//! reach 127.0.0.1 too. Stored 0600 in AGENTVM_HOME, created once.

use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// The token in `<home>/<name>`, created (64 hex characters) on first use.
pub fn load_or_create(home: &Path, name: &str) -> io::Result<String> {
    let path = home.join(name);
    if let Ok(t) = std::fs::read_to_string(&path)
        && t.trim().len() == 64
        && t.trim().chars().all(|c| c.is_ascii_hexdigit())
    {
        return Ok(t.trim().to_owned());
    }
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let _ = std::fs::remove_file(&path);
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
    f.write_all(token.as_bytes())?;
    Ok(token)
}
