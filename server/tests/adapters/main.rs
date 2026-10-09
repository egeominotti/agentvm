//! Adapter tests against real systems: git, APFS, processes, Keychain. No mocks.

mod forward;
mod git;
mod git_concurrency;
mod guest_files;
mod guest_save;
mod helpers;
mod jsonl;
mod lock;
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
