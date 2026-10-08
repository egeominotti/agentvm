//! Full system: real server, VM and Claude. Requires the build, the golden image and the token in the Keychain.
//! Run with `cargo test --test system -- --ignored`.

mod claude;
mod helpers;
mod ports;
mod s3;
mod server;
mod snapshots;
mod tasks;
mod terminals;
