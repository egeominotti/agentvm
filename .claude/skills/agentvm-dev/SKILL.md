---
name: agentvm-dev
description: Build, run and test agentvm fast, try a change on real VMs in an isolated sandbox server, and check what is inside a VM. Use when working on agentvm's server, dashboard, guest scripts or VM helper and you need to build, test, run or verify something.
---

# Working on agentvm, fast

All commands run from the repository root unless noted.

## Build and run

- `scripts/build.sh` builds only what changed (helper and dashboard side by side, then the
  server). About 1 s when nothing changed, ~6 s after a dashboard change. `--force` rebuilds all.
- `./quickstart --no-open` restarts the user's server on :7777 with the new build. Running VMs
  re-attach. Do it once, at the end of a task: never while testing.

## Try it on real VMs without touching the user's

```bash
scripts/sandbox.sh 7791            # its own home (~/AgentVMs-sandbox) and port, the shared image
curl -s -X POST -H 'Origin: http://127.0.0.1:7791' -H 'content-type: application/json' \
  http://127.0.0.1:7791/api/tasks -d '{"repo_path":"'$PWD'","prompt":"","interactive":true}'
# poll GET /api/tasks/<id> until status.state == "running" and ready == true (~5 s)
cd web && bun ../scripts/vm-run.ts 7791 <id> 'cd /root/work && git status'   # prints, exits with its code
scripts/sandbox.sh stop            # stops its VMs, then the server
```

`repo_path` takes a folder or a link (`github.com/owner/repo`). `vm-run` uses its own shell
(Shell 9); for VMs started before extra shells existed set `SESSION=shell`; slow commands
`WAIT=120000`.

## Tests

| What changed | Run |
|---|---|
| Anything | `scripts/test.sh` (all fast groups in parallel, ~30 s) |
| One Rust test | `cd server && cargo test --quiet --test <domain\|adapters\|app\|http\|architecture> <name>` |
| VMs, terminals, guest scripts, disks, snapshots | `cd server && cargo test --release --quiet --test vm -- --ignored` (~1 min) |
| End to end with real Claude (spends the user's quota) | `scripts/test.sh --ignored` (~3 min) |
| Dashboard only | `cd web && bun run typecheck && bun run lint && bun run test` |

Failures: `scripts/test.sh` prints the logs folder; read `<group>.log` there. A system test that
times out under heavy load (many VMs running) can pass alone: rerun it alone before debugging.

## Dashboard

- Hot reload against the sandbox: `cd web && AGENTVM_API=http://127.0.0.1:7791 bunx vite --port 5179`.
- After changing a Rust type used by the API: `cd server && TS_RS_LARGE_INT=number cargo test --quiet --lib`
  regenerates `web/src/api/generated/`; commit them with the change (CI checks they match).
- Browser checks: a tab in the background throttles timers, pauses telemetry polling and may drop
  synthetic key presses. Drive the page from a headless Chrome instead (it draws every frame and
  takes real keys): `playwright-core` with `channel: "chrome"`. On the Vite dev build
  `window.__terms.<session>.socket.send()` types into a terminal. `data-engine` on a `.term`
  says what draws it (`restty:webgpu`, `restty:webgl2`, `xterm`).

## Guest (inside the VMs)

- `guest/*` is embedded in the server at build time: `scripts/build.sh`, then a *new* VM.
- Packages or config baked into the image: `guest/setup-golden.sh`, then `scripts/build-golden.sh`
  (~2-5 min). Running VMs keep their disks; new ones get the new image.
