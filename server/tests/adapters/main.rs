//! Adapter tests against real systems: git, APFS, processes, Keychain. No mocks.

mod git;
mod git_concurrency;
mod guest_files;
mod helpers;
mod lock;
mod pty;
mod s3;
mod secrets;
mod settings;
mod snapshots;
mod tail;
mod workspace;
