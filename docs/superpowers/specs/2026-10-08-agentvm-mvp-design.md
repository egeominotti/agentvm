# agentvm — Design dell'MVP

Data: 2026-10-08 · Stato: bozza da approvare

## 1. Obiettivo

Lanciare da una dashboard web locale più task per **Claude Code**, ognuno eseguito in autonomia
completa dentro una **VM Debian 13 arm64 reale** su Apple Silicon. Il risultato di ogni task
torna nel repo locale come branch `agent/<id>`, pronto da rivedere.

**Utente**: un solo sviluppatore, sul proprio Mac (M5 Max, 18 core, 64 GB).

**Criteri di successo dell'MVP**
1. Dalla dashboard inserisco repo locale + prompt e premo "Avvia": parte una VM e l'agente lavora.
2. Vedo i log dell'agente dal vivo (messaggi, comandi eseguiti) nel browser.
3. A fine task trovo `agent/<id>` nel mio repo, con il diff visibile nella dashboard.
4. Posso lanciare almeno 4 task in parallelo senza interferenze.
5. Posso fermare un task in corso.
6. Il token Claude non compare mai in log, file di job o output.
7. Nulla viene installato sull'host macOS (solo i binari del progetto e i file in `~/AgentVMs`).

## 2. Cosa ha dimostrato il prototipo (2026-10-08)

| Verifica | Esito |
|---|---|
| Debian 13.7 arm64 (`genericcloud`, `.raw`) avviata con Virtualization.framework, EFI | ✅ |
| Binario firmato ad-hoc con entitlement `com.apple.security.virtualization` (nessun account Apple) | ✅ |
| virtiofs nel kernel `6.12-cloud-arm64`, lettura/scrittura dal Mac | ✅ |
| Claude Code 2.1.294 (installer nativo) + `claude -p` con `CLAUDE_CODE_OAUTH_TOKEN` | ✅ |
| Immagine golden + clone APFS: clone 0,01 s, VM pronta col job 2,9 s, ciclo completo 12 s (di cui 8 s Claude) | ✅ |

**Lezioni incorporate nel design**
- cloud-init scrive una config di rete legata al MAC della golden → i cloni restano senza rete.
  La golden usa una config systemd-networkd generica (`Name=en*`, DHCP) e niente cloud-init a runtime.
- `/etc/machine-id` va svuotato nella golden, altrimenti i cloni chiedono lo stesso lease DHCP.
- `set -x` ha stampato il token in un log: gli script guest non usano mai tracing e leggono il token
  da file, cancellandolo subito.
- Il tempo era dominato da apt e installer: tutto va nella golden, mai a runtime.

## 3. Architettura

```
Browser ──HTTP + SSE──> agentvm-server (Rust: axum + tokio)          127.0.0.1:7777
                          ├── API REST + dashboard statica (HTML/JS incorporati, nessuna build frontend)
                          ├── coda task + limite di concorrenza (default 4)
                          ├── git: bundle in ingresso, fetch del bundle in uscita, diff
                          ├── tail di stream.jsonl → eventi SSE verso il browser
                          └── avvia 1 processo per VM ─┐
                                                       ▼
                          agentvm-vm (Swift, ~200 righe, derivato dal prototipo)
                             input : --config job.json
                             output: eventi JSON su stdout, una riga per evento
                             stop  : SIGTERM → stop forzato della VM
                                                       │ Virtualization.framework
                                                       ▼
                          VM Debian 13 (clone APFS della golden)
                             agentvm.service → /usr/local/bin/agentvm-job
                             /mnt/job (virtiofs) = ~/AgentVMs/jobs/<id>/share
```

**Perché due linguaggi**: Virtualization.framework è un'API Swift/Objective-C. Il codice delle VM resta
nativo e già verificato; Rust gestisce server, concorrenza e git. Ogni VM è un processo separato,
quindi il crash di una VM non ferma il server né le altre VM. Solo `agentvm-vm` richiede l'entitlement.

### 3.1 Componenti

| Unità | Linguaggio | Responsabilità | Dipende da |
|---|---|---|---|
| `vm-helper/` → `agentvm-vm` | Swift | Avvia e ferma una VM dato un config JSON; emette eventi | Virtualization.framework |
| `server/` → `agentvm-server` | Rust | API, dashboard, coda, ciclo di vita del task, git | `agentvm-vm`, `git` CLI, `security` CLI |
| `guest/` | Bash + systemd | Script e unit installati nella golden: esegue il job nella VM | Claude Code, git |
| `scripts/build.sh` | Bash | Compila Rust + Swift, firma `agentvm-vm` | cargo, swiftc, codesign |
| `scripts/build-golden.sh` | Bash | Scarica Debian, verifica SHA512, crea la golden con cloud-init (una volta) | `agentvm-vm`, `hdiutil`, curl |

### 3.2 Principi architetturali

1. **Functional core, imperative shell.** Le decisioni (transizioni di stato, interpretazione di
   `result.json`, parsing degli eventi di Claude, costruzione dei comandi git) sono funzioni pure su
   dati. L'I/O (processi, file, git, rete) sta in un guscio sottile che esegue le decisioni prese
   dal core. Il core si testa con dati reali, senza sostituti.
2. **Dipendenze in una sola direzione**: `http → app → domain`. Gli adapter (`vm`, `git`,
   `keychain`, `jobdir`) sono usati da `app` e non conoscono né HTTP né gli altri adapter.
   `domain` non dipende da nulla del progetto e non fa I/O.
3. **Nessuna astrazione per il solo testing.** Un trait esiste solo se ci sono davvero due
   implementazioni in produzione. Gli adapter sono `struct` concrete. I test usano le implementazioni
   vere (vedi §10).
4. **Stati invalidi non rappresentabili.** Newtype per `TaskId`, `CommitSha`, `RepoPath` (validato
   alla costruzione: è un repo git) e `Prompt` (non vuoto). Lo stato del task è un `enum` con i dati
   propri di ogni stato (es. `Running { started_at }`, `Failed { reason }`). Le transizioni passano da
   un'unica funzione `transition(state, event) -> Result<State, InvalidTransition>`.
5. **Segreti tipizzati.** `Secret<String>` non implementa `Display`; il suo `Debug` stampa `[REDACTED]`.
   Il valore si legge solo con `expose()`, usato in un unico punto: la scrittura di `.token`.
6. **Una responsabilità per modulo, file piccoli.** Se un file supera ~300 righe o mescola livelli
   diversi, va diviso.
7. **Errori tipizzati ai confini.** Ogni modulo ha il suo `enum Error` (`thiserror`). `anyhow` solo
   in `main`. Ogni errore che arriva all'utente ha un messaggio azionabile.
8. **Pulizia garantita da RAII.** `JobWorkspace` possiede la cartella del job e il disco clonato:
   `Drop` cancella il disco anche in caso di panic o errore. `VmProcess` termina il processo figlio in
   `Drop`.
9. **Nessuno stato globale.** La configurazione (`Config`: percorsi, porta, concorrenza, CPU/RAM,
   timeout) si legge una volta in `main` e passa per costruttore.

### 3.3 Pattern adottati (e dove)

| Pattern | Dove | Perché |
|---|---|---|
| **State machine** | `domain::task` | Ciclo di vita esplicito, transizioni verificate in un unico punto |
| **Supervisor (actor)** | `app::supervisor` | Una task tokio per ogni task possiede tutto il suo ciclo di vita; nessun lock condiviso sul flusso |
| **Publish/subscribe** | `app::events` (`tokio::sync::broadcast`) | Più client SSE seguono lo stesso task senza accoppiarsi al supervisor |
| **Bounded scheduler** | `app::scheduler` (`Semaphore`) | Limite di concorrenza in un solo posto |
| **Repository** | `app::store` | Unico proprietario dello stato dei task (in memoria per l'MVP; sostituibile con SQLite senza toccare il resto) |
| **Adapter** | `vm`, `git`, `keychain`, `jobdir` | Isolano un sistema esterno dietro un'API piccola e tipizzata |
| **RAII guard** | `JobWorkspace`, `VmProcess` | Pulizia certa delle risorse |
| **Newtype** | `domain::ids` | Validazione al confine, tipi espressivi |

### 3.4 Moduli del server Rust

```
server/src/
├── main.rs              legge Config, costruisce i componenti, avvia axum (solo wiring)
├── config.rs            Config + caricamento da env/flag
├── domain/              puro, senza I/O
│   ├── ids.rs           TaskId, CommitSha, RepoPath, Prompt
│   ├── task.rs          Task, TaskState, Event, transition()
│   ├── agent_event.rs   parsing di una riga di stream.jsonl → AgentEvent (testo, tool_use, result…)
│   └── outcome.rs       interpretazione di result.json + esito del processo → stato finale
├── app/                 casi d'uso, orchestrazione
│   ├── store.rs         repository dei task
│   ├── scheduler.rs     coda + semaforo
│   ├── supervisor.rs    ciclo di vita di un task (prepare → boot → follow → collect → cleanup)
│   └── events.rs        broadcast per task
├── adapters/            I/O verso sistemi esterni
│   ├── vm.rs            VmProcess: spawn di agentvm-vm, eventi da stdout, SIGTERM
│   ├── git.rs           rev-parse, bundle create, fetch da bundle, diff (CLI git)
│   ├── jobdir.rs        JobWorkspace: cartella del job, clonefile, file del contratto §3.6
│   ├── keychain.rs      lettura del token → Secret
│   └── tail.rs          tail di un file che cresce → Stream di righe
└── http/                axum: route, DTO, SSE, asset statici
    ├── routes.rs
    ├── dto.rs
    └── web/index.html
```

Il codice Swift (`vm-helper/`) segue la stessa divisione: `Config.swift` (Codable + validazione),
`MachineFactory.swift` (configurazione VZ), `Runner.swift` (ciclo di vita ed eventi), `main.swift`
(solo wiring). Gli script guest usano `set -euo pipefail` e non usano mai `set -x`.

### 3.5 Protocollo `agentvm-vm`

Config (`job.json`, scritto dal server):
```json
{ "disk": ".../disk.raw", "efivars": ".../efivars", "share": ".../share",
  "console": ".../console.log", "cpus": 4, "memory_mb": 4096, "seed_iso": null }
```
Eventi su stdout (JSON Lines): `{"event":"started"}`, `{"event":"stopped","seconds":12.1}`,
`{"event":"error","message":"..."}`. Codice di uscita: 0 se la VM si è spenta da sola, 1 se c'è stato un errore, 130 se è stata fermata.
`seed_iso` serve solo a `build-golden.sh`.

### 3.6 Contratto della cartella condivisa (`/mnt/job`)

| File | Chi scrive | Contenuto |
|---|---|---|
| `task.json` | server | `{id, prompt, branch, base_sha, timeout_s}` |
| `repo.bundle` | server | `git bundle create --all` del repo locale |
| `.token` | server | token OAuth; l'agente lo legge e lo cancella subito |
| `stream.jsonl` | guest | output `claude -p --output-format stream-json --verbose` |
| `out.bundle` | guest | `git bundle create out.bundle <base_sha>..<branch>` (assente se non ci sono commit) |
| `result.json` | guest | `{status: "ok"\|"no_changes"\|"failed", claude_exit, commits, error?}` |
| `job.log` | guest | log dello script guest (senza tracing, senza token) |

## 4. Ciclo di vita di un task

```
queued → preparing → booting → running → collecting → done | no_changes | failed | stopped
```

1. **queued**: `POST /api/tasks {repo_path, prompt, base_ref?}`. Il server valida che `repo_path`
   sia un repo git e risolve `base_ref` (default `HEAD`) in `base_sha`. Le modifiche non committate
   del working tree non vengono incluse: l'agente parte sempre da un commit.
2. **preparing** (quando c'è uno slot libero): crea `~/AgentVMs/jobs/<id>/`, scrive `repo.bundle` e
   `task.json`, legge il token dal Portachiavi (`security find-generic-password -s agentvm -w`) o dalla
   variabile `CLAUDE_CODE_OAUTH_TOKEN` e scrive `.token`, poi `clonefile(golden/disk.raw → disk.raw)`.
3. **booting**: avvia `agentvm-vm`, `started` → **running**.
4. **running**: nella VM `agentvm-job` clona `repo.bundle` in `/home/agent/work`, fa checkout di
   `base_sha` su un nuovo branch `agent/<id>`, attende il DNS (max 10 s) e lancia
   `claude -p "$prompt" --dangerously-skip-permissions --output-format stream-json --verbose`
   come utente `agent`. Il server segue `stream.jsonl` e inoltra ogni riga via SSE.
5. **collecting**: l'agente fa `git add -A && git commit` se restano modifiche non committate, crea
   `out.bundle` e `result.json`, poi `sync; poweroff -f`. Quando il server riceve `stopped`, esegue
   `git fetch <out.bundle> agent/<id>:agent/<id>` nel repo locale.
6. **Fine**: `done` (branch creato), `no_changes`, `failed` o `stopped`. Il disco della VM viene
   cancellato; `stream.jsonl`, `result.json` e `job.log` restano per la consultazione.

### 4.1 Gestione degli errori

| Caso | Comportamento |
|---|---|
| Timeout del task (default 30 min) | SIGTERM a `agentvm-vm` → `failed` con motivo `timeout` |
| Stop dall'utente | SIGTERM → `stopped`; nessun fetch |
| `agentvm-vm` esce con errore o crash | `failed`, ultime righe di `console.log` nell'errore |
| Nessun `result.json` dopo lo spegnimento | `failed` con motivo `guest_no_result` |
| `claude` esce ≠ 0 | `failed` con l'evento `result` di Claude, ma si fa comunque il fetch se ci sono commit |
| Branch `agent/<id>` già esistente | impossibile: `<id>` è univoco (timestamp + suffisso casuale) |
| Token assente | il task non parte: errore chiaro nella dashboard con le istruzioni per il Portachiavi |
| Riavvio del server | lo stato è in memoria: i task in corso vengono persi. Ogni job scrive `vm.pid`; all'avvio il server termina i PID ancora vivi e cancella i dischi rimasti in `~/AgentVMs/jobs/` |

## 5. API

| Metodo | Percorso | Descrizione |
|---|---|---|
| `GET` | `/` | Dashboard |
| `POST` | `/api/tasks` | Crea un task `{repo_path, prompt, base_ref?}` → `{id}` |
| `GET` | `/api/tasks` | Elenco task con stato e durata |
| `GET` | `/api/tasks/:id` | Dettaglio, `result.json`, nome del branch |
| `GET` | `/api/tasks/:id/events` | SSE: righe di `stream.jsonl` + cambi di stato (replay dall'inizio) |
| `GET` | `/api/tasks/:id/diff` | `git diff base_sha..agent/<id>` |
| `POST` | `/api/tasks/:id/stop` | Ferma il task |

## 6. Dashboard (MVP)

Una sola pagina HTML con JS vanilla, incorporata nel binario:
- form "Nuovo task": percorso del repo, prompt, ref base opzionale;
- elenco dei task: stato, repo, inizio del prompt, durata, pulsante Stop;
- dettaglio: log dal vivo resi leggibili (testo dell'assistente, nome e input dei tool, esito finale),
  diff del branch prodotto.

## 7. Immagine golden

`scripts/build-golden.sh` (una tantum, ~2 min):
1. scarica `debian-13-genericcloud-arm64.tar.xz` e verifica lo SHA512 da `SHA512SUMS`;
2. clona il disco, lo porta a 20 GB e lo avvia con un seed cloud-init (`hdiutil makehybrid`);
3. il seed installa `git curl ca-certificates ripgrep jq build-essential`, Claude Code come utente
   `agent`, `agentvm-job` e `agentvm.service`, la config di rete generica; disattiva cloud-init,
   ssh, timer apt e attesa di GRUB; pulisce la cache apt e svuota `machine-id`;
4. risultato: `~/AgentVMs/golden/disk.raw`.

## 8. Sicurezza

- Il server ascolta solo su `127.0.0.1`.
- Ogni VM vede solo la propria cartella `share/`, mai il repo vero: il codice entra e esce tramite bundle.
- Il token è letto dal Portachiavi, scritto in `.token` con permessi `600`, letto e cancellato dal guest
  prima di avviare Claude, mai passato su riga di comando visibile nei log e mai tracciato.
- Il disco della VM, che contiene l'ambiente dell'agente, viene cancellato a fine task.
- Le VM hanno accesso a internet tramite NAT: serve per l'API di Claude e per installare dipendenze.

## 9. Fuori scope per l'MVP

Pool di VM già accese, avvio diretto del kernel, pulsante merge, persistenza su SQLite,
autenticazione e accesso remoto, più immagini golden, grafici CPU/RAM, submodule e Git LFS,
CLI separata, task che proseguono una sessione esistente.

## 10. Test — nessun mock

**Regola**: nessun mock, stub, fake o test double. Ogni test usa i componenti veri: git vero, file veri,
processi veri, VM vere, server HTTP vero, e Claude vero dove serve. Quello che è difficile da testare
senza sostituti si rende puro (§3.2, punto 1) invece di simularlo.

| Livello | Cosa | Con cosa | Quando gira |
|---|---|---|---|
| **1. Core puro** | `transition()`, `outcome`, parsing di `agent_event`, newtype | Dati reali: `stream.jsonl`, `result.json` registrati da esecuzioni vere (`server/tests/fixtures/`, verificati senza token) | sempre (`cargo test`) |
| **2. Adapter** | `git`: bundle, fetch, diff su repo temporanei veri · `jobdir`: clonefile vero su APFS, pulizia in `Drop` · `tail`: file che cresce scritto da un processo vero · `keychain`: portachiavi temporaneo vero (`security create-keychain` in una tempdir) | Sistema reale, tempdir isolate | sempre |
| **3. VM** | `agentvm-vm` + `VmProcess`: avvio della golden vera senza `task.json` → il guest scrive `result.json {status:"failed", error:"no_task"}` e si spegne; SIGTERM su una VM in esecuzione → uscita 130 e disco cancellato | VM Debian vera (~4 s per test) | `cargo test -- --ignored` (richiede la golden) |
| **4. Sistema** | Server vero su porta effimera → `POST /api/tasks` su un repo temporaneo con il prompt "crea hello.txt con scritto ciao e fai commit" → SSE fino a `done` → `agent/<id>` contiene `hello.txt`; 4 task in parallelo; stop di un task in corso; grep del token su tutto `~/AgentVMs` → 0 occorrenze | Server + VM + Claude veri (~$0,03 per task) | `cargo test -- --ignored` con token nel Portachiavi |

I test di livello 3 e 4 sono `#[ignore]` solo perché richiedono la golden e il token, non perché
usino qualcosa di finto. Il livello 4 sostituisce l'end-to-end manuale.

## 11. Struttura del repo

```
agentvm/
├── README.md
├── docs/superpowers/specs/2026-10-08-agentvm-mvp-design.md
├── vm-helper/      main.swift, Config.swift, MachineFactory.swift, Runner.swift, vz.entitlements
├── server/         Cargo.toml, src/ (vedi §3.4), tests/{adapters,vm,system}.rs, tests/fixtures/
├── guest/          agentvm-job, agentvm.service, 10-agentvm.network, golden-user-data.yaml
└── scripts/        build.sh, build-golden.sh
```
Dati a runtime: `~/AgentVMs/{images,golden,jobs}/`.
