//! Embeds the dashboards into `$OUT_DIR/web_assets.rs` as `(path, bytes)` tables sorted by path,
//! which `http::assets` serves. A new dashboard file needs no Rust change.
//! - `FILES`: every file under `src/http/web/` but `index.html` (served at `/`).
//! - `NEXT`: the React dashboard built by `bun run build` in `web/`, served under `/next/`. Empty
//!   when it was not built, so the server still compiles and tests without Bun.
//!
//! Hidden files like `.DS_Store` are left out of both.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const WEB: &str = "src/http/web";
const NEXT: &str = "../web/dist";

fn main() {
    // A directory: Cargo reruns this script when any file under it is added, removed or changed.
    println!("cargo:rerun-if-changed={WEB}");
    println!("cargo:rerun-if-changed={NEXT}");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));

    let mut files = Vec::new();
    collect(&manifest.join(WEB), &manifest.join(WEB), &mut files);
    files.retain(|(path, _)| path != "index.html");

    let mut next = Vec::new();
    let dist = manifest.join(NEXT);
    if dist.join("index.html").is_file() {
        collect(&dist, &dist, &mut next);
    } else {
        println!("cargo:warning=web/dist is missing: /next/ is empty (cd web && bun install && bun run build)");
    }

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("set by Cargo")).join("web_assets.rs");
    let source = format!("{}{}", table("FILES", files), table("NEXT", next));
    std::fs::write(&out, source).unwrap_or_else(|e| panic!("cannot write {}: {e}", out.display()));
}

/// `pub static <name>` holding the files, sorted by path for a binary search.
fn table(name: &str, mut files: Vec<(String, String)>) -> String {
    files.sort();
    let mut out = format!("pub static {name}: &[(&str, &[u8])] = &[\n");
    for (path, source) in &files {
        writeln!(out, "    ({path:?}, include_bytes!({source:?})),").expect("writing to a String");
    }
    out.push_str("];\n");
    out
}

/// `(path relative to root with `/` separators, absolute source path)` of every file under `dir`.
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("readable directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).expect("UTF-8 file names in the dashboard");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
            continue;
        }
        let relative = path.strip_prefix(root).expect("under the root");
        let parts: Vec<&str> = relative.components().filter_map(|c| c.as_os_str().to_str()).collect();
        out.push((parts.join("/"), path.display().to_string()));
    }
}
