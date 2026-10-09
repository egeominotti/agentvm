//! Real VMs: require `scripts/build.sh` and `scripts/build-golden.sh`.
//! Run with `cargo test --test vm -- --ignored`.

mod helpers;
mod lifecycle;
mod resources;
mod saves;
mod terminal;
mod terminals_at_once;
