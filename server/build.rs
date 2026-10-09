//! Embeds the dashboard built by `bun run build` in `web/` (`web/dist`) into
//! `$OUT_DIR/web_assets.rs` as a `FILES: &[(path, bytes)]` table sorted by path, which
//! `http::assets` serves. Empty when the dashboard was not built, so the server still compiles
//! and its tests can say what is missing. Hidden files like `.DS_Store` are left out.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const DIST: &str = "../web/dist";

fn main() {
    // A directory: Cargo reruns this script when any file under it is added, removed or changed.
    println!("cargo:rerun-if-changed={DIST}");
    let dist = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo")).join(DIST);
    let mut files = Vec::new();
    if dist.join("index.html").is_file() {
        collect(&dist, &dist, &mut files);
    } else if std::env::var("PROFILE").as_deref() == Ok("release") {
        // A release without its dashboard would answer / with "file not found".
        panic!("web/dist is missing: build the dashboard first (scripts/build.sh does it)");
    } else {
        println!("cargo:warning=web/dist is missing: the dashboard is empty (cd web && bun install && bun run build)");
    }
    files.sort();
    let mut table = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (path, source) in &files {
        writeln!(table, "    ({path:?}, include_bytes!({source:?})),").expect("writing to a String");
    }
    table.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("set by Cargo")).join("web_assets.rs");
    // Rewritten only when it changes: the same table must not recompile the crate.
    if std::fs::read_to_string(&out).ok().as_deref() != Some(table.as_str()) {
        std::fs::write(&out, table).unwrap_or_else(|e| panic!("cannot write {}: {e}", out.display()));
    }
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
