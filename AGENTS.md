# AGENTS.md

How to work on agentvm quickly without breaking it. Read this first; the README explains the
product, CONTRIBUTING.md the review rules.

## What it is

A Mac app that runs Claude Code in disposable Debian VMs (Apple Virtualization.framework), one VM
per agent, managed from a web dashboard at `http://127.0.0.1:7777`.

```
server/     Rust server: domain (pure) → app (use cases) → http, plus adapters (git, VMs, PTY,
            Keychain, S3…). Embeds the dashboard's build. Tests in server/tests/<layer>/.
web/        Dashboard: React + TypeScript + Vite, built with Bun. API types are generated from Rust
            into web/src/api/generated/ (never edit them by hand).
vm-helper/  Swift: one process per VM, plus the vsock bridge for terminals.
guest/      Scripts that run inside the VMs. Shipped at every launch (embedded with include_str!),
            except what scripts/build-golden.sh bakes into the image (guest/setup-golden.sh).
scripts/    build.sh (incremental), test.sh, sandbox.sh, vm-run.ts, build-golden.sh, dev-s3.sh.
./quickstart  Builds what changed, starts or restarts the server, opens the dashboard.
```

## Fast commands

| Do | Command |
|---|---|
| Build everything that changed (~1 s when nothing did) | `scripts/build.sh` |
| Run the real app with the new build | `./quickstart --no-open` (running VMs re-attach) |
| Fast tests: web, domain, adapters, app, http, architecture | `scripts/test.sh` (~30 s) |
| Real VMs, Claude, S3 | `scripts/test.sh --ignored` (~3 min, spends Claude usage) |
| One Rust test | `cd server && cargo test --quiet --test <layer> <name>` |
| One real-VM test | `cd server && cargo test --release --quiet --test vm <name> -- --ignored` |
| Dashboard checks | `cd web && bun run typecheck && bun run lint && bun run test` |
| Format and lint Rust | `cd server && cargo fmt && cargo clippy --all-targets -- -D warnings` |
| Regenerate API types after changing a Rust DTO | `cd server && TS_RS_LARGE_INT=number cargo test --quiet --lib` |
| A server of your own, with real VMs | `scripts/sandbox.sh 7791` … `scripts/sandbox.sh stop` |
| Run a command inside a VM | `cd web && bun ../scripts/vm-run.ts <port> <vm-id> '<command>'` |
| Dashboard with hot reload | `./quickstart --dev` (or `AGENTVM_API=http://127.0.0.1:7791 bunx vite`) |

## Rules that are enforced

- **Layers** (`server/tests/architecture.rs`): `http` uses only `app` and `domain`; `domain` does
  no I/O; an adapter never uses another adapter. New adapter → add it to the list in that test.
- **No file over 300 lines**, any language (same test). Split by responsibility.
- **A failing test first**, then the code. No mocks, stubs or fakes: real git repositories, files,
  Keychain, VMs, servers. Hard to test without a fake → move the decision into a pure function in
  `server/src/domain` and test that.
- **Bun, never npm/node** for `web/`.
- **English** everywhere: code, comments, UI, commits. Comments say why, in plain sentences.

## Rules that are not (and cost a lot when broken)

- **Never touch the user's agentvm on :7777** or its VMs: no restarts mid-task, no stop, no delete.
  Try things on `scripts/sandbox.sh` (its own home and port). Restart :7777 only at the end, with
  `./quickstart --no-open`.
- **The VM image is shared.** Sandbox and test homes symlink `~/AgentVMs/golden/disk.raw`. Never
  change how disks are cloned or opened without a real-VM run. If launches fail with
  "/root/work already exists", the image was written to: rebuild it with `scripts/build-golden.sh`.
- **Guest scripts are embedded at build time**: after editing `guest/*`, run `scripts/build.sh`
  and launch a *new* VM to see the change. Running VMs keep the old ones.
- **Secrets** (Claude token, git tokens, S3 key) live in the macOS Keychain, reach processes on
  stdin or in their own environment, never on a command line, in a URL, a file or a VM.
- **Real Claude costs the user's quota**: run `--ignored` system tests only when they matter.
- Commit and push straight to `main` (no branches), `type(scope): summary` + a body saying why,
  ending with the co-author line.

## Verifying a change

1. The test you wrote fails, then passes.
2. `scripts/test.sh` is green; for anything touching VMs, terminals, guest scripts or disks, also
   the relevant `--ignored` tests.
3. UI: build, run the sandbox, open it in a browser and look (light and dark, 390 px wide too).
   A background tab throttles timers and drops synthetic keys: judge behavior, not those artifacts.
