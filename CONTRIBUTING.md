# Contributing to agentvm

Thanks for helping. This document is short on purpose: it lists what a change needs to be merged.

## Setting up

You need a Mac with Apple Silicon, Xcode Command Line Tools and a current stable Rust toolchain
(CI always builds with the latest stable).

```bash
scripts/build.sh            # Swift helper (signed) + Rust server
scripts/build-golden.sh     # the Debian image used by every VM (~2 min, once)
scripts/dev-s3.sh up        # optional: local S3 (RustFS in Docker) for backup tests
bin/agentvm-server          # http://127.0.0.1:7777
```

Run a second copy for development without touching your real VMs:

```bash
AGENTVM_HOME=/tmp/agentvm-dev AGENTVM_PORT=7788 bin/agentvm-server
```

## Tests

```bash
cargo test --manifest-path server/Cargo.toml                 # fast: domain, adapters, app, HTTP, architecture
cargo test --manifest-path server/Cargo.toml -- --ignored    # real VMs, Claude and S3
```

**No mocks, stubs or fakes.** Tests use real git repositories, files, a temporary Keychain, real
VMs, real Claude and a real S3 server. When something is hard to test without a fake, move the
decision into a pure function in `server/src/domain` and test that with real data instead.

Every behavior change comes with a test that failed before the change.

## Architecture rules

`server/tests/architecture.rs` enforces them; please keep it green rather than relaxing it.

- `http → app → domain`. The HTTP layer never uses adapters directly.
- `domain` is pure: no I/O, no async, no other layers.
- Each adapter wraps one external system and does not know the other adapters.
- A trait exists only when there are two real implementations.

## Style

- `cargo fmt` (120 columns, see `server/rustfmt.toml`) and `cargo clippy --all-targets -- -D warnings`.
- Everything is written in English: code, comments, UI text, docs and commit messages.
- Commit messages: `type(scope): summary` (`feat`, `fix`, `docs`, `ci`, `chore`, `refactor`, `test`),
  with a body that explains why.
- Guest scripts never use `set -x`: secrets pass through them.

## Pull requests

CI must be green. In the description, say what changed for the user, how you tested it (which
`--ignored` tests you ran locally, since CI cannot nest virtualization), and anything you were
unsure about.
