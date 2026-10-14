# Multiple coding CLIs in AgentVM

## Intent

The user explicitly requested support for most coding-agent CLIs inside AgentVM,
instead of a product restricted to Claude Code. Their repeated instruction to
proceed authorizes implementation using sensible defaults. Existing native macOS
work and running user VMs must be preserved.

## User-visible behavior

- Select Claude Code, Codex, Gemini CLI, OpenCode, Qwen Code, GitHub Copilot CLI,
  Kilo CLI, Aider, Goose, or a custom CLI when launching a VM.
- Select an appropriate inference provider and enter a model ID. Claude retains
  its current defaults; other agents never inherit an incompatible Claude alias.
- Configure provider API endpoints and credentials in Settings in both clients.
  Credentials are saved in macOS Keychain and never returned by the API.
- Use the same terminals, repository cloning, save/close, telemetry, port
  forwarding, time limits, and snapshots with any selected CLI.
- Snapshot restore preserves the agent, provider, model, version and custom
  command. It retrieves current host credentials rather than snapshotting them.
- Install only the selected CLI at launch, and reuse its installation on restore.
  Existing golden images work without a rebuild.
- A custom CLI accepts an installation command, an interactive command, an
  optional headless command and an optional resume command. Launch command lines
  are parsed into arguments, never evaluated as shell with a prompt or model.
  `{prompt}`, `{model}` and `{endpoint}` are substituted within arguments after
  parsing. Installation commands run explicitly as user-configured shell code.
- Show the selected agent's name in terminals, machine lists, and diagnostics.
  Conversation/cost analytics remain available where native data is supported;
  other agents expose their real output without invented usage numbers.

## Authentication

New VMs have a loopback HTTP relay. A reverse vsock bridge in the Swift VM helper
connects requests to a per-VM Unix socket on the host. The host attaches the real
provider credential and streams the response. Guest configuration contains only a
dummy credential. The relay is fixed to the configured provider endpoint, strips
guest credentials, never follows redirects, and cannot become an arbitrary proxy.

Default providers are OpenAI (Responses and Chat Completions), Anthropic,
Google Gemini, OpenRouter, and local OpenAI-compatible inference. A custom
endpoint can use OpenAI, Anthropic or Google protocol. Credential-free endpoints
are supported explicitly. Claude's existing subscription token stays supported
through the host relay; other agents initially use provider API credentials.
Copilot runs in its documented offline BYOK mode. Consumer sign-in transports
such as Cursor/Kiro require their own integration and are not advertised as
working built-ins until that transport exists and is verified.

## API and persistence

- `AgentSelection` is an object: `kind`, optional `provider`, optional `version`,
  optional `custom`. An omitted object defaults to Claude for old tasks,
  snapshots and API requests.
- Task creation accepts `agent`; task and snapshot DTOs include it.
- Settings gain `agent` (default selection), `providers` (non-secret overrides),
  and `custom_cli` (optional reusable custom command).
- `GET /api/agents` returns descriptors and provider credential presence only.
- `PUT /api/settings/providers/{provider}` updates non-secret configuration and
  an optional credential; `DELETE` removes that credential.
- The primary terminal's existing wire name `claude` stays compatible with old
  helpers and saved client tabs; its visible name follows the selected agent.

## Architecture

Pure domain types validate selections, supported protocol combinations, model
IDs, versions, custom arguments and relay targets. The app resolves launch
defaults and credentials, owns relay lifetime, and preserves snapshot metadata.
Independent adapters own Keychain access and HTTP forwarding. Rust embeds the
new guest installer/runner/relay scripts. The guest runner constructs actual
argv/configuration for each CLI and writes normalized task output.

The web dashboard and Swift native client consume the same API catalog. The
other active native-app task retains ownership of its current resize/release
changes; CLI integration is coordinated before changing those files.

## Constraints

- Work directly on `main`; do not create a branch or worktree.
- Never restart or alter the user's engine on port 7777 during development.
- Never write into the shared golden image; clone and test using isolated homes.
- No source file over 300 lines; no mocks/stubs/fakes in tests.
- Write a failing behavior test before implementation. Use real CLIs and a real
  VM to verify installation, actual flags, terminal startup and host relay.
- Use Bun for dashboard work. Generate API types from Rust.
- Avoid spending inference quota merely to validate CLI installation and flags.
- Run the fast suite, native checks, relevant ignored VM tests and visual checks
  before committing/pushing. Report any inference paths that lack credentials
  and consequently cannot be exercised against a live model.

## Builder decisions

The CLI set, host relay design, Keychain-backed API credential approach, and
backward-compatible terminal wire name are implementation defaults selected by
Codex to satisfy the broad request. Exact model defaults come from each CLI's
documented behavior or are requested explicitly when a custom endpoint needs
one; no Claude-only default is silently imposed on another CLI.

## Accepted implementation steering

- The user corrected the API-only restriction: built-ins must offer account or
  subscription authentication as well as API credentials wherever their shipped
  CLI supports it. Verify the exact provider transport and current entitlement
  rules; a discontinued login must not be presented as working.
- Account access uses credentials on the Mac and refreshes them on the host.
  Guests and snapshots retain only dummy authentication and the selected mode.
  Both clients expose the same account/API choice, connection status and host
  account connection actions; external API/MCP control covers these actions.

The user subsequently requested that Model ID entry disappear and quickstart stay
simple. Launchers and General settings now expose no model input; the backend
accepts default/automatic models. CLIs needing an internal model discover it from
the configured provider through the existing dummy-credential relay. Explicit
API/MCP overrides remain optional. Changing default CLI/provider clears inherited
model aliases. Quickstart is one command, including prebuilt CLI archives.

The user also requested a rewritten README and updated GitHub code, tags and
release. Delivery uses version0.2.0 across engine/native package, documented
CLI/MCP limits, native and CLI archives, and checksums after final verification.
