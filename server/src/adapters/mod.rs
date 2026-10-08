//! Adapters to external systems. They do not depend on each other.
pub mod git;
pub mod host;
pub mod jobdir;
pub mod keychain;
pub mod lock;
pub mod pty;
pub mod settings_file;
pub mod tail;
pub mod vm;
