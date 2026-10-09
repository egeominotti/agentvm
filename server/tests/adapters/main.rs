//! Adapter tests against real systems: git, APFS, processes, Keychain. No mocks.

mod chunks;
mod forward;
mod git;
mod git_concurrency;
mod guest_files;
mod guest_save;
mod helpers;
mod jsonl;
mod kernel;
mod lock;
mod private_git;
mod process;
mod pty;
mod remote;
mod s3;
mod secrets;
mod server_log;
mod settings;
mod snapshots;
mod tail;
mod transcripts;
mod workspace;
