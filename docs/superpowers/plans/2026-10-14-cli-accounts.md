# CLI Accounts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Follow each task's red/green checks.

**Goal:** Offer real account/subscription and API authentication for the supported coding CLIs.

**Architecture:** Pure account choices and transport decisions, independent host account and Keychain adapters, app-owned orchestration, typed HTTP/MCP access. The existing private VM relay receives fresh host authentication through a callback; guests retain dummy credentials.

**Tech Stack:** Rust/reqwest/Keychain, Python guest runtime, React/Bun and native Swift.

**Spec:** `docs/superpowers/specs/2026-10-14-cli-accounts-design.md`

## Global constraints

- Work on main; preserve all running user VMs and the shared golden image.
- No real credentials in guest state, logs, API output, argv or new credential files.
- No source over 300 lines; real integration tests and pure fixtures, no fakes.
- No model-ID controls; authentication and billing labels must describe actual access.

## Review focus

- A referenced rotating CLI session is reloaded rather than independently refreshed.
- API mode never silently falls back to a consumer account.
- Account mode never sends dummy tokens directly to a provider through a bypassed CLI plugin.
- Concurrent refresh and expired/revoked sessions retain secrets and give actionable errors.
- Account model catalogs and overridden API endpoints choose usable models automatically.

## Task 1: Account types and host connection

**Files:** `domain/accounts.rs`, `domain/agents.rs`, `adapters/accounts/`, `adapters/keychain/accounts.rs`, `app/accounts/`, `http/accounts.rs`, catalog/router/module declarations and generated DTOs.

**Interfaces:** `AgentAuth::{Account,Api}`; `AccountKind::{Claude,Chatgpt,Google,Copilot,Kilo}`; optional `AgentSelection.auth` and `account_kind`. `AccountAccess` contains a non-secret `ProviderConfig`, real bearer `Secret`, and additional host headers. `app::accounts::access(ctx, account, force_refresh)` asynchronously obtains current access. `app::accounts::config(account)` returns the fixed public relay config; a validated Copilot backend may be supplied by access.

- [ ] Write failing validation/default tests for missing legacy fields, eligible account kinds, explicit API mode, and account/API ambiguity.
- [ ] Add host-only connection/import/login APIs and Keychain session storage; account catalog contains status only.
- [ ] Reference native rotating sessions read-only. Own device/browser login sessions refresh on the host under a shared account lock.
- [ ] Verify real temporary credential files/Keychains, malformed/expired inputs, deletion and HTTP responses without credential echoes.

## Task 2: Account relay and lifecycle

**Files:** `adapters/agent_proxy.rs` and focused submodules, pure `domain/account_proxy.rs`, `app/agent_gateway.rs`, launch/submission/recovery.

**Interfaces:** `ProviderRelay::start_account(path, config, callback)`, callback `Fn(bool) -> BoxFuture<Result<AccountAccess,String>>`. Current and refreshed access may update a validated account backend while guest paths stay local.

- [ ] Write failing pure routing/header/request tests for ChatGPT, Google Code Assist, Copilot regional backends and Kilo gateway.
- [ ] Inject host-only account headers and redaction; retry one authenticated 401 after the host refresh callback.
- [ ] Bind account mode/config when submitting, preserve it in recovery/snapshots and avoid API key checks for account mode.
- [ ] Verify live account/public catalogs without inference and actual VM relay paths.

## Task 3: Guest account modes

**Files:** `guest/agent_auth.py`, existing command/config/model helpers, installer/runtime embedding and Python tests.

- [ ] Write failing guest behavior tests for dummy account env/config, no BYOK in Copilot account mode, Gemini Code Assist override and ChatGPT model catalog.
- [ ] Configure each account mode using pinned CLI source contracts. Avoid OpenCode/Kilo builtin ChatGPT OAuth plugins because their fixed upstream bypasses the relay.
- [ ] Enable Goose's supported CLI account providers with pinned dependencies and automatic native models.
- [ ] Run guest tests and relevant real-VM installer/help/start checks without inference quota.

## Task 4: Clients, documentation and delivery

**Files:** web launcher/settings/helpers and tests; native owner handles macOS. README, CLI docs, MCP reference and release notes.

- [ ] Write failing client tests for account readiness, switching back to API and backward-compatible defaults.
- [ ] Add account/API selectors, account settings and host login status actions; keep all model-ID inputs absent.
- [ ] Check web/native light/dark and compact layouts against isolated engines.
- [ ] Review combined account changes, run complete verification, build final archives and update GitHub main/tag/release.
- [ ] Restart the user's engine only at the end and verify existing VMs reattach.
