# agentvm

Run many **Claude Code** agents in parallel on Apple Silicon. Each one gets its own disposable
**Debian 13 arm64 virtual machine** with full root permissions, and nothing it does can touch your Mac.

Every terminal you open in the web dashboard is a VM: Claude Code is already running inside it,
on a fresh clone of your repository, with `--dangerously-skip-permissions`. You type to Claude
directly, open a shell in the same VM, and the work comes back to your repo as a git branch
`agent/<id>`.

```
Browser ──HTTP/WebSocket──> agentvm-server (Rust)  ──spawn──> agentvm-vm (Swift, one process per VM)
                               │                                 │ Virtualization.framework
                               │                                 ▼
                               │                          Debian 13 arm64 VM
                               │                          ├─ Claude Code (root, no prompts)
                               └── git bundle ⇄ virtiofs ⇄├─ your repo on branch agent/<id>
                                   terminal   ⇄  vsock   ⇄└─ tmux sessions: claude, shell
```

## Why

- **Full autonomy, zero risk.** Claude can install packages, run anything, break the system:
  it all happens in a VM that is thrown away. Your Mac and your working tree stay untouched.
- **Real parallelism.** Ten tasks, ten VMs, no fighting over ports, dependencies or files.
- **Nothing installed on the host.** Toolchains and dependencies live only inside the VMs.
- **Your code stays local.** No GitHub round-trip; only Claude API calls leave the machine.
- **Fast.** Native arm64 virtualization, instant copy-on-write disk clones (APFS `clonefile`).

## Performance

Measured on an M5 Max (18 cores, 64 GB):

| What | Time |
|---|---|
| Clone a VM disk from the golden image | ~10 ms |
| VM power-on → guest ready (repo cloned, network up) | ~3 s |
| Keystroke echo through the VM terminal (vsock → tmux → bash) | 0.12 ms median, 1.4 ms p95 |
| Small task end to end ("create a file and commit") | 12–20 s, most of it Claude thinking |

By default agentvm runs as many VMs at once as fit in RAM (`(RAM − 8 GB) / memory per VM`,
14 VMs of 4 GB on a 64 GB Mac); the rest wait in a queue.

## Requirements

- A Mac with Apple Silicon and a recent macOS (developed on macOS 27).
- Xcode Command Line Tools (`swiftc`, `codesign`) and a Rust toolchain (`cargo`).
- A Claude subscription token (`claude setup-token`) or the `CLAUDE_CODE_OAUTH_TOKEN` env var.

No Apple Developer account is needed: the VM helper is ad-hoc signed with the
`com.apple.security.virtualization` entitlement.

## Quick start

```bash
git clone https://github.com/egeominotti/agentvm.git && cd agentvm
scripts/build.sh                     # bin/agentvm-vm (Swift, signed) + bin/agentvm-server (Rust)
scripts/build-golden.sh              # once: Debian 13 image with Claude Code preinstalled (~2 min)
claude setup-token                   # once: create a token for your Claude subscription
security add-generic-password -U -s agentvm -a agentvm -w   # paste the token when asked
bin/agentvm-server                   # dashboard at http://127.0.0.1:7777
```

`build-golden.sh` downloads the official `debian-13-genericcloud-arm64` image, verifies its
SHA-512, and boots it once to install git, build tools, Python, tmux and Claude Code. Everything
lives in `~/AgentVMs`.

## Using it

1. Open **http://127.0.0.1:7777**, pick a repository (recent ones are suggested) and optionally
   write a first task for Claude.
2. Press **New terminal** (⌘ Enter). A VM boots in a few seconds with Claude Code already open.
   Tick **One per line** to open one terminal per line of the task, each with its own agent.
3. Work with Claude as usual. Switch to **Shell** for a root shell in the same VM, on the same
   checkout (`/root/work`). Reloading the page keeps both sessions alive.
4. **Save to repo** turns the current work into commits on `agent/<id>` in your repository
   without stopping anything. **Close VM** saves and destroys the machine. **Force stop**
   powers it off without saving.

Tab colors show each agent's state: blue while Claude works, amber when it is waiting for you.
The page title counts the terminals waiting for you. To select text in a terminal, hold
**⌥ Option** while dragging.

When a terminal is closed, the dashboard shows the branch, the diff per file, and ready-to-copy
`git switch` / `git merge` commands.

## Configuration

Environment variables read by `agentvm-server` at start-up:

| Variable | Default | Meaning |
|---|---|---|
| `AGENTVM_PORT` | `7777` | Dashboard port (always bound to 127.0.0.1) |
| `AGENTVM_CONCURRENCY` | fits in RAM | Maximum VMs running at once |
| `AGENTVM_CPUS` | `4` | vCPUs per VM |
| `AGENTVM_MEMORY_MB` | `4096` | Memory per VM |
| `AGENTVM_TIMEOUT_S` | `1800` | Time limit for non-interactive tasks |
| `AGENTVM_HOME` | `~/AgentVMs` | Images, golden disk and job folders |

## HTTP API

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/tasks` | `{repo_path, prompt?, base_ref?, interactive?}` → `{id}` |
| `GET` | `/api/tasks` | All tasks with state and agent activity |
| `GET` | `/api/tasks/{id}` | One task |
| `GET` | `/api/tasks/{id}/pty?session=claude\|shell` | WebSocket to a terminal in the VM |
| `POST` | `/api/tasks/{id}/save` | Commit and import the work into `agent/<id>` |
| `POST` | `/api/tasks/{id}/close` | Save and shut the VM down |
| `POST` | `/api/tasks/{id}/stop` | Power the VM off without saving |
| `GET` | `/api/tasks/{id}/diff` | Diff of the produced branch |
| `GET` | `/api/tasks/{id}/events` | Server-sent events (state changes, agent events) |
| `GET` | `/api/status` | Golden image and token present, VMs running |

Non-interactive tasks (`interactive: false`) run `claude -p` to completion and return a branch,
which is handy for scripting.

## Security model

- Inside the VM, Claude runs as **root** with every permission (`IS_SANDBOX=1`,
  `defaultMode: bypassPermissions`). The VM is the sandbox.
- A VM only sees its own job folder (virtiofs). The repository goes in as a `git bundle` and
  comes back as one; the VM never sees your real checkout.
- The Claude token is read from the macOS Keychain, handed to Claude on a file descriptor
  (never in the environment of the commands it runs), redacted from logs, and destroyed with the VM.
- The dashboard listens on `127.0.0.1` only and rejects requests whose `Host` or `Origin`
  is not local, so other websites cannot reach the API or the terminals (DNS rebinding,
  cross-site WebSockets).
- VMs have outbound internet through NAT (Claude API, package installs). One server instance
  per `AGENTVM_HOME` is enforced with a lock.

## Development

```
server/      Rust (axum + tokio): domain (pure) → app (use cases) → http; adapters for git, VMs, PTY, Keychain
vm-helper/   Swift: starts one VM, bridges a Unix socket to the guest's vsock port
guest/       Scripts installed in the golden image: job runner, PTY server, Claude wrapper
scripts/     build.sh, build-golden.sh
docs/        Design specs and implementation plan
```

Tests never use mocks: they run against real git repositories, real files, a real Keychain,
real VMs and real Claude.

```bash
cargo test --manifest-path server/Cargo.toml                 # domain, adapters, app, HTTP, architecture rules
cargo test --manifest-path server/Cargo.toml -- --ignored    # real VMs and full system (needs golden + token)
```

An architecture test enforces the dependency rules: `http` never uses adapters directly, the
domain does no I/O, adapters do not know each other.

## Limitations

- Task state lives in memory: restarting the server forgets the task list and stops running VMs
  (branches already saved stay in your repositories).
- Single user, local only. Submodules and Git LFS are not carried into the VM.

xterm.js and its addons are bundled under `server/src/http/web/vendor/` (MIT license).
