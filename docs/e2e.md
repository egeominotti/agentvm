# Verifica end-to-end

I test `#[ignore]` usano componenti reali e richiedono `scripts/build.sh`, `scripts/build-golden.sh`
e il token nel Portachiavi.

| Test | Verifica |
|---|---|
| `tests/vm.rs::helper_reports_invalid_config` | `agentvm-vm` rifiuta un config senza disco con un evento `error` |
| `tests/vm.rs::boots_golden_without_task_and_powers_off` | la golden si avvia, il guest scrive `no_task` e si spegne in < 20 s |
| `tests/vm.rs::terminate_stops_a_running_vm` | SIGTERM ferma una VM in esecuzione (uscita 130) |
| `tests/system.rs::task_produces_a_branch_in_the_local_repo` | prompt reale → `agent/<id>` con `hello.txt`, diff e SSE con eventi `state`/`agent` |
| `tests/system.rs::running_task_can_be_stopped` | stop di un task al lavoro → `stopped`, nessun branch |
| `tests/system.rs::invalid_requests_are_rejected_with_a_message` | repo non git e prompt vuoto → errori leggibili; id sconosciuto → 404 |
| `tests/system.rs::token_never_lands_in_job_files` | nessun `sk-ant-` nei file di `~/AgentVMs/jobs` (eseguire dopo gli altri) |

Verifica manuale del 2026-10-08: 4 task in parallelo sul repo demo `~/AgentVMs/demo`, tutti `done`
in 14–24 s ciascuno, 4 branch `agent/*` creati.
