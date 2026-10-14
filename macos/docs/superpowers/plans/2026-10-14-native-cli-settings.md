# Native multi-CLI Settings implementation plan

> **For agentic workers:** Use Superpowers execution and TDD skills. The native coordinator owns commits and integration; this task must not stage files or replace the frozen base artifact.

**Goal:** Configure default coding agents, inference providers and reusable custom commands in the native app.

**Architecture:** Read the server's catalog through NativeStore. Keep provider drafts local until an explicit host-only update, and send ordinary non-secret defaults through the existing serialized Settings queue. Preserve the Claude account section and legacy engine support.

**Tech Stack:** SwiftUI, AppKit, Foundation, Swift Testing, the existing local Rust API.

**Spec:** `docs/superpowers/specs/2026-10-14-multi-cli-design.md`; API shapes are in `docs/superpowers/plans/2026-10-14-multi-cli.md`, Task 1.

## Constraints and ownership

- Work on `main`, without another worktree. The coordinator handles commits and pushes.
- Edit native Settings files and their policy/navigation tests only.
- Never mutate the shared Keychain, operate the GUI, launch VMs or alter port 7777 in this phase.
- Do not run a production build until the native base commit gate is released.
- Keep every file under 300 lines, English throughout, and use real dependencies or pure decisions in tests.
- Existing README/OPERATIONS and the frozen Git index are outside this task.

## Interfaces

NativeStore is owned by the coordinator and supplies `agentCatalog: JSON`,
`NativeAgentCatalogState` (`loading`, `supported`, `legacy`, `failed(String)`),
and `refreshAgents()`. Successful dedicated provider actions refresh the catalog.
HTTP 404 and an actual old-engine HTML 200 response select legacy mode;
transport failures remain visible and retryable.

The catalog contains `agents` with `kind`, `name`, `providers`, `headless`,
`resume`, and `requires_model`; `providers` with `id`, `name`, `config`,
and `has_key`; and `claude_token`. Provider config contains `endpoint`,
`protocol` (`openai`, `anthropic`, `google`), and `requires_key`.

The launcher owner supplies NativeAgentPolicy's catalog projections, transitions,
configuration validation and launch readiness. General keeps selection and
version in a local draft until Save. It uses `providerChoices` to show an existing
incompatible selection explicitly and `configurationBlocker` before queueing one
complete update. It never silently chooses another provider. Credential gates
remain in the launcher so incomplete public defaults can still be saved. Models
are resolved by the backend for native launches, without a manual model control.

Provider updates use `PUT /api/settings/providers/{id}` with `{config, token?}`.
Omit the token to preserve the saved credential. `DELETE` removes only the
credential, retaining endpoint configuration. Secrets are ephemeral SecureField
values, never Settings fields, UserDefaults values or returned API data.

Reusable commands are `settings.custom_cli` with `name`, optional `install`,
required `interactive`, optional `headless`, and optional `resume`.
Launch templates remain literal text; this UI does not evaluate commands.
The backend resolves a missing default selection's `custom` from `custom_cli`.
Saving defaults removes their embedded command copy so future reusable edits take
effect. Existing inline-only custom defaults migrate into `custom_cli` first.

MCP is a third added navigation page, supplied by the coordinator. Its pane and
configuration-copy policy are outside this Settings task's implementation scope.

## Backend coordination concern

Full-object Settings saves can overlap dedicated provider configuration saves.
`SettingsService.update_then` already preserves S3 instead of trusting a stale
draft. It must likewise preserve the current provider overrides, so an ordinary
native Settings save cannot undo a completed provider update. The backend owner
has implemented that preservation in `server/src/app/settings.rs`; backend
regression verification stays with that owner.

## Tasks

- [x] Write navigation tests that discover inference credentials and custom installation commands, retain the Claude account route, and enumerate Providers, Custom CLI and MCP. Observe failures against the existing catalog before editing it.
- [x] Implement Providers and Custom CLI catalog entries and pane routing without changing the common content identity used during resizing.
- [x] Write pure draft tests for credential omission, endpoint restrictions, blank optional commands, and literal quoted placeholders. Observe red before implementing draft normalization and validation.
- [x] Implement explicit-save provider forms, confirmed credential removal and retryable catalog states. Never label saved credential presence as a successful connection.
- [x] Implement the reusable custom command editor, including name/interactive validation, optional commands and an explanation of shell installation versus literal launch arguments.
- [x] Integrate catalog-driven General defaults through the shared launcher policy. Preserve legacy Claude engine compatibility without manual model controls.
- [x] Run native checks in a separate scratch build directory and report all failures. Visual checks and live inference/credential mutations remain with the coordinator.

## Review focus

- Legacy engines do not accept new settings fields or trigger repeated unavailable-catalog alerts.
- Provider drafts survive resizing and background refreshes until saved or discarded explicitly.
- Credential-free endpoints remain possible without deleting an existing saved credential.
- Credential removal depends on provider identity and availability, even when the
  unsaved public endpoint draft is invalid; saving that draft remains blocked.
- Switching agents never retains a stale Claude model default or submits a partial Settings transition.
- Changing reusable custom commands does not leave a stale embedded default selection.

## Progress

- Specification, current native controls and exact backend types inspected.
- Shared helper ownership agreed with the launcher owner.
- Navigation red run uses `macos/.build/settings-cli-check/nav-red.log`.
- MCP navigation red run uses `macos/.build/settings-cli-check/mcp-nav-red.log`.
- `reusable-custom-red.log` proves four stale inline-command failures and the
  uppercase HTTPS endpoint failure against the extracted previous behavior.
- The generic notification and storage wording now refers to agents; the Claude
  subscription account and actual golden-image Claude controls remain specific.
- `AgentSettingsIntegrationTests` uses only a caller-supplied owned idle engine
  through `AGENTVM_NATIVE_AGENT_TEST_URL`. It updates public local-provider config
  without a token, then round-trips defaults and literal reusable commands through
  the real NativeStore queue. It restores effective config/defaults and checks
  credential availability is unchanged. Restoring an implicit provider default
  leaves the equivalent explicit override because this API cannot erase overrides.
- `cli-search-red.log` proves General was absent for both "default CLI" and
  "coding agent" before adding the feature's discovery keywords.
- `settings-green.log`: 11 Settings tests in 3 suites, zero skips; real public
  configuration/default/custom-command roundtrip passed in 0.559 seconds.
- Before model-control removal, `native-cli-full-green.log`: 160 tests in 41 suites passed in 23.912 seconds,
  with only the preexisting real terminal input test intentionally skipped to
  preserve the user's existing VM. Real snapshot, S3, catalog and completed-output
  fixtures were enabled. Four actual Ghostty preview tests also passed.
- All Settings files remain under 300 lines and the macOS whitespace check passed.

## Final user direction and verification

The user requested removal of native model ID and model choice controls. General
now edits only coding CLI, provider and version. Legacy engines show their Claude
Code default without a model editor. Native launches omit model overrides so the
backend can resolve them; the HTTP API's optional explicit model remains intact.

`defaultsUpdate(settings:selection:version:)` preserves the existing public
`Settings.model` when editing the same CLI kind, including API-configured literal
overrides. Switching kinds resets that field to `default` to avoid carrying a
Claude alias into another runtime. The pure regression first failed because
`sonnet` survived a switch to Codex, then passed after the reset policy changed.

- `no-model-defaults-red.log` / `no-model-defaults-green.log`: meaningful reset
  RED then 11 Settings policy/navigation tests passed.
- `native-no-model-full-green.log`: fresh 161 tests in 42 suites passed in
  23.075 seconds; one existing terminal-input test intentionally skipped. Real
  public Settings roundtrip passed in 0.799 seconds; four actual Ghostty tests
  passed separately.
- Actual QA captures exposed duplicate persistent endpoint/example labels. Those
  fields now hide their built-in labels while retaining descriptive headings and
  accessibility labels. New captures must verify these changes and the removed
  General model controls after the coordinator releases the production build.
- Offline review inspected all 73 actual PNGs in
  `/tmp/agentvm-native-verify.RDgRW6/captures`, across light/dark appearances and
  780/1400 widths. All Settings sections and MCP JSON remain readable; valid
  provider forms, empty credentials and empty custom-command drafts show the
  expected enabled or disabled actions. The old General images precede model
  removal and cannot validate the final form.
- The same image review found the wide Machines list's Open button reduced to a
  blank square in both appearances. Compact list actions remain readable. This
  regression belongs to the coordinator's machine-row component and needs a
  fresh visual check after its fix.
- Ruling: preserve the frozen index and use a separate test scratch directory because the coordinator is validating the native base artifact concurrently.
