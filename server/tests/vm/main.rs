//! Real VMs: require `scripts/build.sh` and `scripts/build-golden.sh`.
//! Run with `cargo test --test vm -- --ignored`.

mod account_catalog;
mod agent_relay;
mod automatic_model;
mod cli_agents;
mod cli_runtime;
mod control;
mod direct_boot;
mod helpers;
mod lifecycle;
mod resources;
mod saves;
mod shells;
mod tailscale;
mod terminal;
mod terminals_at_once;
