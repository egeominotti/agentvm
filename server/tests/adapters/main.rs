//! Adapter tests against real systems: git, APFS, processes, Keychain. No mocks.

mod git;
mod git_concurrency;
mod guest_files;
mod helpers;
mod lock;
mod process;
mod pty;
mod s3;
mod secrets;
mod server_log;
mod settings;
mod snapshots;
mod tail;
mod telemetry_file;
mod workspace;
