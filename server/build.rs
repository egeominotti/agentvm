//! Embeds the dashboard: every file under `src/http/web/` (but `index.html`, served at `/`, and
//! hidden files like `.DS_Store`) goes into `$OUT_DIR/web_assets.rs` as a `(path, bytes)` table
//! sorted by path, which `http::assets` serves. A new dashboard file needs no Rust change.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const WEB: &str = "src/http/web";

fn main() {
    // A directory: Cargo reruns this script when any file under it is added, removed or changed.
    println!("cargo:rerun-if-changed={WEB}");
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo")).join(WEB);
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();

    let mut table = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (path, source) in &files {
        writeln!(table, "    ({path:?}, include_bytes!({source:?})),").expect("writing to a String");
    }
    table.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("set by Cargo")).join("web_assets.rs");
    std::fs::write(&out, table).unwrap_or_else(|e| panic!("cannot write {}: {e}", out.display()));
}

/// `(path relative to web/ with `/` separators, absolute source path)` of every file under `dir`.
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("readable directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).expect("UTF-8 file names in web/");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
            continue;
        }
        let relative = path.strip_prefix(root).expect("under web/");
        let parts: Vec<&str> = relative.components().filter_map(|c| c.as_os_str().to_str()).collect();
        let web_path = parts.join("/");
        if web_path != "index.html" {
            out.push((web_path, path.display().to_string()));
        }
    }
}
