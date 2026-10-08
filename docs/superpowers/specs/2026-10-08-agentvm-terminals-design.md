# agentvm — Terminali (ogni terminale è una VM)

Data: 2026-10-08 · Stato: approvato in chat · Estende `2026-10-08-agentvm-mvp-design.md`

## Obiettivo

Ogni terminale aperto nella dashboard è una VM Debian dedicata con dentro il repo scelto e
**Claude Code interattivo** (la TUI vera, con `--dangerously-skip-permissions`). L'utente scrive
direttamente all'agente, apre shell nella stessa VM, salva il lavoro come branch `agent/<id>` e
chiude la VM. "Un agente per riga" apre N terminali, ognuno con Claude avviato su quel compito.

## Decisioni

- **Canale terminale: vsock**, non rete. L'helper Swift aggiunge un `VZVirtioSocketDevice` e
  ascolta su un socket Unix (`jobs/<id>/pty.sock`); ogni connessione viene inoltrata alla porta
  vsock 5000 del guest. Il server Rust fa da ponte WebSocket ↔ socket Unix.
- **Server PTY nel guest** (`/usr/local/bin/agentvm-pty`, Python 3 già presente): per ogni
  connessione legge un'intestazione JSON `{"cmd":"claude"|"shell","cols","rows"}` e avvia il
  comando in un PTY come utente `agent` in `/home/agent/work`. Frame dal client:
  `[tipo:1 byte][lunghezza:4 byte BE][payload]`, tipo 0 = input, 1 = resize `{"cols","rows"}`.
  Dal guest al client: byte grezzi del PTY.
- **Token**: resta in `/run/agentvm/token` (root, 0600). Il server PTY (root) lo passa a Claude
  solo su fd 3 (`CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR`); le shell non lo ricevono.
- **Stato dell'agente** via hook di Claude Code: `UserPromptSubmit` → `working`,
  `Stop`/`Notification` → `waiting`, scritto in `share/activity`. Il supervisor lo legge e lo
  espone come `activity` nel DTO.
- **Salva / chiudi** tramite file di richiesta nella cartella condivisa:
  `save.request` → il guest fa commit + `out.bundle` + `save.done`, il server fa `git fetch`
  forzato su `agent/<id>`; `close.request` → salvataggio finale, `result.json`, spegnimento.
- **Nessun timeout** per i terminali: li chiude l'utente. "Ferma" resta lo stop forzato.
- **Limite di VM** calcolato dalla RAM: `(RAM − 8 GB) / memory_mb`, sovrascrivibile con
  `AGENTVM_CONCURRENCY`.
- **xterm.js** incluso nel binario (vendor), nessuna risorsa esterna.
- La modalità non interattiva (prompt → branch) resta disponibile via API, e i test esistenti
  continuano a coprirla.

## API nuove

| Metodo | Percorso | Descrizione |
|---|---|---|
| `POST` | `/api/tasks` | `interactive: true` apre un terminale; `prompt` facoltativo |
| `GET` | `/api/tasks/{id}/pty?cmd=claude\|shell&cols&rows` | WebSocket verso un PTY nella VM |
| `POST` | `/api/tasks/{id}/save` | salva il lavoro nel branch senza chiudere |
| `POST` | `/api/tasks/{id}/close` | salva e spegne la VM |

## Test (senza mock)

- VM reale: shell via vsock (`echo ciao` → `ciao`), `close.request` → spegnimento pulito.
- Sistema: terminale interattivo con prompt iniziale → stato `waiting` → `save` → il branch
  contiene il file → `close` → `done`.
