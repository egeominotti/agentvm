# agentvm

Esegue più agenti **Claude Code** in parallelo, ognuno dentro una **VM Debian 13 arm64 reale**
su Apple Silicon, e li controlla da una **dashboard web locale**.

## Obiettivo

Dare a ogni agente Claude Code autonomia completa (`--dangerously-skip-permissions`) senza
rischi per il Mac: l'agente lavora in una VM usa-e-getta, su una copia del repo, e restituisce
il risultato come branch git `agent/<id>` nel repo locale. Più task indipendenti girano insieme
e si seguono dal browser.

- **Sicurezza**: l'agente può installare, eseguire, rompere: resta tutto nella VM, che viene buttata.
- **Parallelismo**: N agenti, N VM, nessun conflitto di dipendenze, porte o file.
- **Mac pulito**: nulla installato sull'host; toolchain e dipendenze vivono solo nelle VM.
- **Codice in casa**: il repo resta sul Mac, niente GitHub obbligatorio; escono solo le chiamate API di Claude.
- **Veloce**: VM nativa arm64 su Virtualization.framework, clone del disco istantaneo (APFS),
  ciclo completo di una VM misurato in ~4 s di overhead.

## Come funziona (in breve)

```
Browser ──HTTP/SSE──> agentvm-server (Rust)  ──spawn──> agentvm-vm (Swift, 1 processo per VM)
                         │                                   │ Virtualization.framework
                         └── git bundle ⇄ cartella condivisa ⇄ VM Debian 13 + Claude Code
```

Dettagli, scope dell'MVP e decisioni: [docs/superpowers/specs/2026-10-08-agentvm-mvp-design.md](docs/superpowers/specs/2026-10-08-agentvm-mvp-design.md).

## Stato

Fase di design. Il prototipo di fattibilità è stato completato il 2026-10-08 (vedi la spec).
