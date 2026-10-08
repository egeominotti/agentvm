//! Regole di dipendenza della spec (§3.2): http → app → domain, adapter indipendenti, domain senza I/O.

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
            assert!(!text.contains(f), "{file} non deve usare `{f}`");
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
        for other in ["git", "jobdir", "keychain", "lock", "tail", "vm"] {
            if other != own {
                assert!(!text.contains(&format!("adapters::{other}")), "{file} usa adapters::{other}");
            }
        }
    }
}
