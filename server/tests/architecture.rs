//! Dependency rules from the spec (§3.2): http → app → domain, independent adapters, domain without I/O.

use std::path::Path;

fn sources(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(dir);
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push((p.display().to_string(), std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out
}

fn assert_absent(dir: &str, forbidden: &[&str]) {
    for (file, text) in sources(dir) {
        for f in forbidden {
            assert!(!text.contains(f), "{file} must not use `{f}`");
        }
    }
}

#[test]
fn http_depends_only_on_app_and_domain() {
    assert_absent("http", &["crate::adapters"]);
}

#[test]
fn domain_has_no_io_and_no_outer_layers() {
    assert_absent("domain", &["crate::adapters", "crate::app", "crate::http", "std::fs", "std::process", "tokio"]);
}

#[test]
fn adapters_do_not_know_each_other_or_outer_layers() {
    for (file, text) in sources("adapters") {
        assert!(!text.contains("crate::app") && !text.contains("crate::http"), "{file}");
        let own = Path::new(&file).file_stem().unwrap().to_str().unwrap().to_owned();
        for other in [
            "archive",
            "forward",
            "git",
            "host",
            "jobdir",
            "keychain",
            "lock",
            "orphans",
            "pty",
            "records",
            "releases",
            "s3",
            "server_log",
            "settings_file",
            "snapshots",
            "tail",
            "transcripts",
            "vm",
        ] {
            if other != own {
                assert!(!text.contains(&format!("adapters::{other}")), "{file} uses adapters::{other}");
            }
        }
    }
}

/// No file of the project grows past 300 lines: split it by responsibility instead.
#[test]
fn no_file_is_longer_than_300_lines() {
    const MAX: usize = 300;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let skip = ["target", "vendor", "fixtures", "node_modules", "dist", "docs", ".git", ".claude", ".build"];
    let kinds = ["rs", "js", "ts", "tsx", "css", "html", "swift", "sh", "py", "toml", "yml"];
    let mut long = Vec::new();
    let mut stack = vec![repo.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let (p, name) = (e.path(), e.file_name().to_string_lossy().into_owned());
            if p.is_dir() {
                if !skip.contains(&name.as_str()) {
                    stack.push(p);
                }
                continue;
            }
            // Guest scripts have no extension; anything else must be source of a known kind.
            let source = p.extension().is_some_and(|x| kinds.contains(&x.to_str().unwrap_or("")))
                || p.parent().is_some_and(|d| d.ends_with("guest"));
            if !source {
                continue;
            }
            let lines = std::fs::read_to_string(&p).map_or(0, |t| t.lines().count());
            if lines > MAX {
                long.push(format!("{} ({lines})", p.strip_prefix(&repo).unwrap().display()));
            }
        }
    }
    long.sort();
    assert!(long.is_empty(), "files over {MAX} lines, split them: {long:?}");
}
