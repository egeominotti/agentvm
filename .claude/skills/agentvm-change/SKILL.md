---
name: agentvm-change
description: Ship a change to agentvm from failing test to pushed commit, in the right layer and with the project's rules (architecture, 300 lines, no mocks, generated types). Use when adding a feature or fixing a bug in agentvm.
---

# A change to agentvm, end to end

## 1. Find the layer

| The change is about… | It goes in | Its test goes in |
|---|---|---|
| A rule, a parse, a decision (no I/O) | `server/src/domain/` | `server/tests/domain/` |
| Talking to one external thing (git, VM, Keychain, S3, files) | `server/src/adapters/<one>.rs` | `server/tests/adapters/` |
| A use case combining them | `server/src/app/` | `server/tests/app/` |
| An endpoint | `server/src/http/` + a route in `router.rs` | `server/tests/http/` |
| Inside the VM | `guest/` | `server/tests/vm/` (real VM, `--ignored`) |
| The dashboard | `web/src/features/<area>/`, shared bits in `components/` and `lib/` | next to it, `*.test.ts` |

`http` never calls adapters; adapters never call each other; the domain never does I/O. A new
adapter file goes in the list of `server/tests/architecture.rs`. Every file stays under 300 lines.

## 2. Red

Write the test with real things (a temporary git repo, a temporary Keychain, a real HTTP server
in the test, a real VM). Run it and see it fail for the right reason. A test that passes before
the change proves nothing: change it until it fails.

## 3. Green

Write the smallest code that makes it pass, in the style around it (comments say why, plain
sentences, English). Then:

```bash
cd server && cargo fmt && cargo clippy --quiet --all-targets -- -D warnings
TS_RS_LARGE_INT=number cargo test --quiet --lib        # only if an API type changed
cd ../web && bun run format && bun run typecheck && bun run lint && bun run test
cd .. && scripts/test.sh
```

Touching VMs, terminals, guest scripts, disks or snapshots: also
`cd server && cargo test --release --quiet --test vm -- --ignored`.

## 4. Look at it

UI: `scripts/build.sh && scripts/sandbox.sh 7791`, open `http://127.0.0.1:7791`, use the feature
for real (light and dark mode, a 390 px wide window too). Stop the sandbox when done.

## 5. Ship

```bash
git add -A <paths> && git commit -F - <<'EOF'
type(scope): what changed for the user

Why, and anything a reviewer must know (what was tested for real).

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
git push origin main
./quickstart --no-open      # the user's server on the new build (their VMs re-attach)
```

Update README.md when the user-visible behavior or the API changes.
