//! Task lifecycle entry points, at the path callers have always used.
//!
//! The lifecycle itself lives in focused modules: [`context`](super::context) (the services),
//! [`submission`](super::submission) (validate and queue), `launch` (slot, job folder, boot),
//! `supervise` (follow the running VM), `collect` (decide the outcome) and
//! [`recover`](super::recover) (re-attach after a restart).

pub use super::context::{AppCtx, ClaudeReleases};
pub use super::random::random_bytes;
pub use super::recover::recover;
pub use super::submission::{NewTask, SubmitError, submit};
