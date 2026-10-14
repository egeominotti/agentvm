# Native CLI Accounts Implementation Plan

> **For agentic workers:** Use Superpowers execution and TDD skills task by task. The native coordinator owns integration, commits and production builds.

**Goal:** Configure coding agents with their supported account access or provider API credentials, and connect accounts through explicit native host actions.

**Architecture:** Share authentication decisions with the launcher owner. General saves one exclusive account/API selection through the existing Settings queue. Account cards display only public catalog status; finite host login sessions poll only while their view and application are active.

**Tech Stack:** SwiftUI, AppKit, Foundation, Swift Testing and the existing local Rust API.

**Spec:** `docs/superpowers/specs/2026-10-14-cli-accounts-design.md`; exact API types in the parent account implementation contract.

## Global constraints

- Work on main without staging, committing or resetting another worker's changes.
- Preserve port 7777, all user VMs, provider credentials and the existing Claude token pane.
- No model-ID controls. Account and API billing labels describe the actual access.
- Secrets never enter native Settings, UserDefaults, guest state, logs or files.
- Tests may read public account routes but must not import, start login or remove shared credentials.
- Every file stays under 300 lines; use real dependencies or pure decisions, never fake API responses.
- Coordinate the shared Swift compiler slot; production builds belong to the coordinator.

## Interfaces and files

The catalog adds optional `accounts` status entries (`id`, `name`, `connected`,
`source`, `error`, optional `can_import`/`can_login`). Agent descriptors list their
eligible account IDs. Presence of the accounts array distinguishes modern account
support from the older API-only catalog.

The launcher owner supplies `NativeAgentAuthentication.supported`, `auth`,
`accountChoices`, `accountKind`, `provider`, `changingAuth`, `changingAccount` and
`changingKind`. General consumes those decisions and leaves launch credentials
and launch validation with the shared policy.

- Modify `Settings/AgentDefaultsSettings.swift`: account/API selector and the
  matching account or provider picker; preserve a remembered provider locally.
- Modify `Settings/AgentSettingsPolicy.swift`: preserve account identity and
  version, excluding provider fields in account mode and account fields in API mode.
- Modify `Settings/SettingsPolicy.swift` and `NativeSettingsView.swift`: discover
  and route the added `accounts` page while retaining `account` for legacy Claude.
- Create `Settings/AccountLoginPolicy.swift`: public login projection, correlated
  finite status and a pure poll decision for inactivity, expiry and request bound.
- Create `Settings/AccountLoginState.swift`: per-account observable public login
  state retained by a private, nonpublished NativeStore dictionary. Returning to
  the page resumes the same session; unchanged pending polls publish nothing.
- Create `Settings/AccountsSettings.swift`: account status cards, explicit import,
  user-triggered browser login, visible verification code and confirmed removal.
- Create `Tests/AgentVMNativeTests/AccountSettingsPolicyTests.swift` and
  `AccountLoginPolicyTests.swift`, plus ownership regressions and read-only real
  API coverage gated by `AGENTVM_NATIVE_ACCOUNT_TEST_URL`.

Host routes: GET/DELETE `/api/settings/accounts/{kind}`, POST `/import`, POST
`/login` without a body, and GET `/login/{id}`. Login start returns `id`,
`verification_url`, optional `user_code` and `expires_in`; finite polling returns
the same ID, `pending`/`connected`/`failed`, and an optional public error.

## Review focus

- Switching authentication removes incompatible fields, preserving version and
  reusable custom commands. Legacy omitted fields retain their prior meaning.
- A disconnected eligible account may be saved as a public default, while launch
  readiness still requires a connection; General never silently switches modes.
- Expired, failed, mismatched or malformed login responses cannot show Connected
  or open an unvalidated URL.
- Navigation or app inactivity pauses requests; polling resumes only for a still
  pending session and ends at its expiry or finite request bound.
- Account cards respect unsupported actions, retain actionable backend errors
  and never expose tokens. Existing Claude token controls remain available.

## Tasks

### Task 1: Exclusive authentication defaults and navigation

- [x] Write tests for account identity, stale API-provider removal, explicit API
  clearing an old account kind, implicit account kind, and legacy Claude defaults.
- [x] Run `AccountSettingsPolicyTests` and observe the previous save behavior
  losing account fields; observe absent account discovery before adding navigation.
- [x] Preserve the exclusive public authentication fields in `defaultsUpdate`.
- [x] Wire General to the shared authentication transitions and add the account page.
- [x] Run Settings/navigation/shared policy checks after the launcher source is stable.

### Task 2: Finite public login flow

- [x] Write pure tests for valid public login start, rejected malformed/unsafe
  browser links, correlated pending/connected/failed status, expiry, inactive pause
  and finite polling exhaustion. Observe failure before implementing the helper.
- [x] Implement `NativeAccountLoginPolicy` and the account cards. Browser opening
  occurs only after the user's Sign in action and validated host response.
- [x] Poll GET at two-second intervals while the card is visible and the scene is
  active, stopping after server expiry or 300 requests. Cancellation is quiet;
  genuine request errors remain visible and retryable.
- [x] Prove the previous card-owned login factory loses its session/code and poll
  progress after reconstruction, then retain it in NativeStore without publishing
  global changes on every pending status poll.
- [ ] Run pure checks and read-only real catalog/account status checks when the
  coordinator supplies the account-capable fixture.

### Task 3: Combined verification

- [x] Review authentication payloads and state transitions with the launcher owner.
- [ ] Run the fresh native suite with the coordinator's real fixture map and report
  the intentional existing-terminal skip separately.
- [ ] Have the coordinator verify light/dark and compact native forms, preserving
  all shared credentials and leaving sign-in actions to the user.

## Evidence

Record each failing-first log, resulting test count and real visual/API evidence
here after verification. Parent permission to implement this exact contract is
already given; no additional approval or account connection is implied by tests.

- `native-account-defaults-red.log`: five tests, eight failing assertions against
  the existing default-save and navigation decisions. Account auth/identity was
  dropped and a stale provider remained. Legacy Claude defaults stayed valid.
- `native-account-login-red.log`: pure login tests first failed because the
  requested production login policy did not exist; all other consumers compiled.
- `/tmp/agentvm-native-auth-combined-green.log`: 61 tests in ten suites passed,
  including exclusive defaults, login decisions, Settings navigation, launcher
  transitions and no-model behavior. Four actual Ghostty tests also passed.
- The launcher owner independently reviewed General/defaults and found no
  blocker. The coordinator independently reviewed account actions and polling.
- Router verification confirms GET
  `/api/settings/accounts/{account}/login/{id}` in `server/src/http/router.rs`,
  matching the native polling path. Real read-only account-route verification
  awaits the coordinator's updated engine fixture.
- Remove remains available when an account reports a source or error even if it
  is disconnected; an expired stored connection still requires explicit removal
  confirmation. Import/Sign in respect the backend's capability flags.
- `native-account-ownership-red.log` / `native-account-ownership-green.log`:
  navigation reconstruction first lost object identity, session and poll count
  (three failed assertions). After caching, 33 tests in six suites passed. The
  publisher test proves unchanged pending polls neither redraw the card nor
  publish through the workspace store.
- Two compiled read-only integration tests await the updated owned engine:
  all five public account status routes/capabilities and the typed failure from
  polling a login that was never started. No authentication mutation is tested.
- A second review found a late response from login A could overwrite login B or
  revive cleared state before SwiftUI cancellation arrived. The real shared
  state boundary first failed 13 assertions across four pure race tests in
  `native-account-race-red.log`.
- Captured response, error, expiry and polling-limit writes now require the
  current session ID; loop entry also guards the counter and request. Old results
  publish nothing and preserve the replacement's session/phase/poll count.
  `native-account-race-green.log`: 37 tests in seven suites passed, including
  current response acceptance, navigation retention and unchanged-pending
  publication checks. The coordinator owns the subsequent full/visual gate.
