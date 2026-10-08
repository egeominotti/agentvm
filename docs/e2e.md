# End-to-end verification

The `#[ignore]` tests use real components and require `scripts/build.sh`, `scripts/build-golden.sh`
and the token in the Keychain.

| Test | Checks |
|---|---|
| `tests/vm.rs::helper_reports_invalid_config` | `agentvm-vm` rejects a config without a disk with an `error` event |
| `tests/vm.rs::boots_golden_without_task_and_powers_off` | the golden boots, the guest writes `no_task` and shuts down in < 20 s |
| `tests/vm.rs::terminate_stops_a_running_vm` | SIGTERM stops a running VM (exit 130) |
| `tests/system.rs::task_produces_a_branch_in_the_local_repo` | real prompt → `agent/<id>` with `hello.txt`, diff and SSE with `state`/`agent` events |
| `tests/system.rs::running_task_can_be_stopped` | stopping a working task → `stopped`, no branch |
| `tests/system.rs::invalid_requests_are_rejected_with_a_message` | non-git repo and empty prompt → readable errors; unknown id → 404 |
| `tests/system.rs::token_never_lands_in_job_files` | no `sk-ant-` in the files under `~/AgentVMs/jobs` (run after the others) |

Manual verification on 2026-10-08: 4 tasks in parallel on the demo repo `~/AgentVMs/demo`, all `done`
in 14–24 s each, 4 `agent/*` branches created.
