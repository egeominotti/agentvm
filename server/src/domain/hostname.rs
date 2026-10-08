//! Names under `*.localhost` for the services of each VM: browsers resolve every `*.localhost`
//! to this Mac, so `http://3000.demo-web-4f94.localhost:7777` reaches port 3000 of that VM
//! through the server's proxy, and every VM can use the same ports.

use super::ids::TaskId;

const MAX_SLUG: usize = 25;

/// `demo-web-4f94`: a DNS label from the machine's title, made unique by the end of its id.
pub fn vm_name(title: &str, id: &TaskId) -> String {
    let mut slug = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(MAX_SLUG).collect();
    let slug = slug.trim_end_matches('-');
    let suffix = &id.as_str()[id.as_str().len() - 4..];
    if slug.is_empty() { format!("vm-{suffix}") } else { format!("{slug}-{suffix}") }
}

/// `3000.demo-web-4f94.localhost:<server_port>` → (3000, "demo-web-4f94").
pub fn parse_proxy_host(host: &str, server_port: u16) -> Option<(u16, String)> {
    let rest = host.strip_suffix(&format!(".localhost:{server_port}"))?;
    let (port, name) = rest.split_once('.')?;
    let port: u16 = port.parse().ok().filter(|p| *p > 0)?;
    let valid = !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    valid.then(|| (port, name.to_owned()))
}

pub fn proxy_url(port: u16, vm: &str, server_port: u16) -> String {
    format!("http://{port}.{vm}.localhost:{server_port}")
}
