# Native Multi-CLI Launcher Implementation Plan

> Execute inline with test-driven development. Root owns integration, native release builds, review, commits and push.

**Goal:** Select the coding CLI, account/subscription or provider API access, version and optional custom commands from the native VM launcher. The backend resolves the model automatically.

**Architecture:** Pure JSON policy consumes the shared agent catalog. Small SwiftUI fields edit a launch draft; the existing launcher retains repository/resource actions. Core owns catalog loading and compatibility detection, and Settings consumes the same default-selection helpers.

**Tech stack:** SwiftUI, Foundation, official Swift Testing, the existing native JSON/API types.

**Spec:** `docs/superpowers/specs/2026-10-14-multi-cli-design.md`, implementation contract in `docs/superpowers/plans/2026-10-14-multi-cli.md` Task 1.

## Constraints

- Edit only Launcher sources and their tests, plus this native implementation record.
- Existing native baseline was captured and committed by root before these changes.
- Do not stage, commit, reset, push or build the production artifact as this agent.
- No GUI actions, VM mutations, provider credential writes or port 7777 operations.
- Every file stays below 300 lines; code and UI use English.
- Tests use pure literal inputs or real isolated API servers; no mocks, stubs or fake services.

## Review focus

- A legacy engine retains Claude compatibility without `agent` or `model` request fields.
- Switching CLI drops the previous provider and version.
- Authentication follows the selected account or API provider; the legacy Claude-token gate applies only when account metadata is unsupported.
- Configured provider protocol overrides must be compatible with the selected CLI.
- Every UI launch omits `model`, even if its input or saved settings contain a wrong provider's ID.
- CLI model requirements and custom `{model}` templates do not require a user-entered ID before launch.
- Custom `{model}` templates require a configured provider for automatic model discovery, including resume templates.
- Custom commands preserve literal arguments and need a headless template for automatic tasks.
- Account support is driven by descriptor eligibility and non-secret catalog statuses; old catalogs without an accounts array retain the existing controls and request shape.
- Authentication mode changes preserve version and reusable command drafts. Explicit eligible accounts survive CLI changes; incompatible fields and versions are cleared when changing CLI.

### Task 1: Shared agent policy

Files: `Launcher/AgentPolicy.swift`, `Launcher/CustomCLIValidation.swift`, `Tests/AgentVMNativeTests/NativeAgentPolicyTests.swift`.

- [x] Write policy tests before the helper exists; observe the absent-helper failure.
- [x] Publish helper signatures to root and Settings owner.
- [x] Observe the additional display-name/repository-gate regressions.
- [x] Implement and verify catalog choices, selection transitions, provider readiness, validation and request construction.
- [x] Run all targeted policy tests using root's warmed debug scratch with four jobs.

### Task 2: Native launch draft and fields

Files: `Launcher/NativeAgentFields.swift`, `Launcher/NativeLauncherView.swift`, `Launcher/LauncherLogic.swift`.

- [x] Add catalog-driven agent/provider fields, native version controls and custom CLI templates.
- [x] Remove all model selection/input and mandatory-model launch blockers following explicit user steering.
- [x] Keep catalog failures inline with Retry and retain old controls on explicit legacy state.
- [x] Initialize settings once after catalog resolution; retain user edits during credential/catalog refresh.
- [x] Send modern selection metadata only to supported engines and keep repository/resources unchanged.
- [x] Run launcher and policy suites and report concrete compile/test evidence to root.

### Task 3: Integration and review

- [ ] Verify catalog consumption against a real new isolated backend once available.
- [x] Request independent root/Settings review of shared policies and payload shape.
- [x] Resolve confirmed review findings with failing regressions, rerun both scoped suites and freeze source at handoff.
- [ ] Coordinate full-suite and native UI/VM evidence with root; freeze source at handoff.

## Progress and rulings

- Ruling: Treat non-Claude `default` as an omitted model, matching backend `Model.cli_name()`; required-model CLIs still block until given an explicit ID. Settings persists `default` because its model field cannot be empty.
- Ruling: Preserve every explicitly saved non-Claude model ID on initial load, including `opus`, `sonnet` and `fable`. Only the reserved `default` marker becomes an empty CLI-default input. Changing CLI clears the old model separately, so initial loading must not erase intentional values. This supersedes the earlier alias-clearing rule for initialization.
- Final user ruling: Remove model selection and Model ID input entirely. Native UI launches omit `model` for all ten CLIs and legacy engines, and readiness delegates model resolution to the backend. This supersedes both model-input rulings above; optional API/MCP overrides remain a backend responsibility. Task/snapshot facts may still display their actual resolved model.
- Ruling: Use current custom commands from the launch draft or reusable `settings.custom_cli`; changing agent does not embed a stale reusable command copy into Settings.
- Root owns API absence detection (404 or genuine legacy HTML), catalog error state, dynamic task labels and analytics.
- Eight initial policy cases were written before implementation. A direct typecheck using the real installed Swift Testing module observed absent `NativeAgentPolicy` before production helper writes. The first cold scratch build was cancelled to preserve root's Golden verification capacity.
- Root's native baseline commits are `3e391ef` and `bb3785f`; the CLI changes remain unstaged and separate.
- Root's warmed scratch is `macos/.build/core-check`. The initial targeted policy checkpoint passed ten policy tests and four actual Ghostty Bun tests. Log: `/tmp/agentvm-native-agent-policy-green.log`. Later scoped results below supersede this count.
- Behavioral RED observed three assertions: custom display name ignored, unknown CLI mislabeled Claude, and the repository gate requiring a Claude subscription even after selected-provider authentication passed. Log: `/tmp/agentvm-native-agent-policy-behavior-red.log`. The subsequent targeted GREEN resolves all three.
- Settings owner received exact shared helper signatures and source-stable acknowledgment; root received `kind(_:)` and `displayName(_:catalog:)` for task labels. Root and Settings reviewed these contracts; their confirmed findings were incorporated and verified below.
- Real new-backend catalog integration and GUI/VM checks remain pending at this checkpoint. The old API remains covered by root's two actual HTTP compatibility tests; this agent has not mutated VMs or credentials.
- An intermediate Settings review added `fable` to initialization alias filtering. Root's later review rejected filtering explicitly saved non-Claude IDs; the final ruling and regression above supersede that intermediate behavior.
- An actual SwiftUI Binding regression observed same-kind reselection clearing the saved version and both custom model/version flags. A guard preserves unchanged drafts; choosing another kind still clears incompatible fields and retains reusable command drafts.
- Further behavioral RED observed seven assertions covering providerless custom endpoint templates, configuration-only defaults validation and unavailable saved provider choices. GREEN preserved invalid selections visibly while keeping credentials and launch-required models separate from saving defaults. Logs: `/tmp/agentvm-native-agent-config-behavior-red.log` and `/tmp/agentvm-native-agent-config-behavior-green.log`.
- Uppercase `HTTPS` provider URLs failed two launch/configuration assertions before case-insensitive scheme validation. Logs: `/tmp/agentvm-native-agent-scheme-red.log` and `/tmp/agentvm-native-agent-scheme-green.log`.
- Final review RED observed nine assertions: seven explicit saved model IDs were erased, a custom resume template's model requirement was ignored, and stale embedded custom commands overrode current reusable commands. The fixes preserve explicit IDs, validate resume placeholders and prefer current `settings.custom_cli`. Logs: `/tmp/agentvm-native-agent-review-red.log` and `/tmp/agentvm-native-agent-review-green.log`.
- Previous scoped checkpoint: `macos/test.sh --scratch-path macos/.build/core-check --jobs 4 --filter 'NativeAgentPolicyTests|LauncherLogicTests'` passed 24 Swift tests across two suites, zero skips, plus four actual Ghostty Bun tests. Root independently inspected that evidence before the final user steering.
- No-model RED observed 70 assertions: 60 saved-model leaks across all ten CLIs and legacy payloads, plus ten old payload/model-readiness assertions. Log: `/tmp/agentvm-native-no-model-red.log`.
- Removed model state and controls from Launcher fields, removed model parameters from readiness/configuration/request helpers, and deleted unused initialization/transition/validation helpers. Request construction removes any preexisting `model` field before either modern or legacy construction. Provider authentication/protocol/endpoint, version and custom-command checks remain.
- Final scoped command: `macos/test.sh --scratch-path macos/.build/core-check --jobs 4 --filter 'NativeAgentPolicyTests|NativeAgentModelResolutionTests|LauncherLogicTests|AgentSettingsPolicyTests|SettingsPolicyTests'`. Observed result: 35 Swift tests across five suites, zero skips, plus four actual Ghostty Bun tests; native debug source compilation clean. Log: `/tmp/agentvm-native-no-model-green.log`.
- Source frozen after this final scoped green run and handed to root. A separate read-only review of root's presentation, guest boot names, Inspector output routing, snapshot/machine labels and MCP configuration/packaging found no native integration blocker. Parent owns backend model auto-resolution, the full integration gate, production build, GUI/VM evidence and delivery.
- A later resource review found the launcher requested Claude versions unconditionally and displayed its failures on Codex/custom launches. A pure resource/cache policy reproduced nine request/cache/error-scope assertions; custom model discovery reproduced six missing-provider assertions in the same RED run. Log: `/tmp/agentvm-native-cli-resource-red.log`.
- Claude versions now load only after defaults initialize with Claude selected. Successful responses, including empty lists, stay cached across CLI changes, and version errors appear only for Claude instead of using the general launch error. Custom model placeholders require a configured provider; credential-free Local providers remain ready without a model input.
- Latest scoped command: `macos/test.sh --scratch-path macos/.build/core-check --jobs 4 --filter 'NativeClaudeVersionCatalogTests|NativeAgentModelResolutionTests|NativeAgentPolicyTests|LauncherLogicTests|AgentSettingsPolicyTests|SettingsPolicyTests'`. Observed GREEN: 39 Swift tests across six suites, zero skips, plus four actual Ghostty Bun tests. Log: `/tmp/agentvm-native-cli-resource-green.log`. Source/compiler handed to root for its final full gate.
- Later user steering added account/subscription access alongside provider APIs. The exact selection contract is `auth?: account|api` and `account_kind?: claude|chatgpt|google|copilot|kilo`; descriptor `accounts` gives eligibility, and catalog `accounts` supplies names, connection state and errors. Account mode excludes API providers, API mode excludes account kind, and all launches still omit models.
- Actual authentication RED observed eight assertions across three tests: a connected account incorrectly required an API key, invalid exclusive/ineligible modes passed configuration, and payloads lost authentication fields while retaining a provider. Log: `/tmp/agentvm-native-auth-policy-red.log`. Shared policy GREEN passed eight tests before UI wiring; log: `/tmp/agentvm-native-auth-policy-green.log`.
- `Launcher/NativeAgentAuthentication.swift` now resolves explicit modes and domain-compatible implicit account selections, validates eligibility/status metadata, separates disconnected default saves from launch readiness, and restores compatible remembered API providers. Launcher adds native account/API controls, account connection navigation and canonical modern payload fields; catalogs without an accounts array omit the new fields and controls.
- A further CLI transition RED showed an explicit eligible ChatGPT account changing to Claude instead. The correction preserves an eligible explicit account kind before choosing a connected default. Log: `/tmp/agentvm-native-auth-kind-red.log`.
- Final combined command filters `NativeAgentAuthenticationTests|NativeAgentAuthenticationBindingTests|NativeClaudeVersionCatalogTests|NativeAgentModelResolutionTests|NativeAgentPolicyTests|LauncherLogicTests|AccountSettingsPolicyTests|AccountLoginPolicyTests|AgentSettingsPolicyTests|SettingsPolicyTests` using the same warmed scratch. Observed GREEN: 61 Swift tests across ten suites, zero skips, plus four actual Ghostty Bun tests. Log: `/tmp/agentvm-native-auth-combined-green.log`.
- Authentication and actual SwiftUI Binding regressions cover implicit account selections, Claude's Anthropic API fallback, all ten CLI payloads without models, mode/version/provider preservation and account status failures. Read-only General/defaults integration review found no blocker. Source and compiler are frozen and handed to root for completed-output verification and the full integration gate; no account credentials, login/import actions, VMs or production builds were touched by this agent.

## Server-owned integration follow-up

- Actual `guest/agent_models.py` now scans custom interactive/headless/resume templates in `needs_model()`, resolving the earlier resume-model gap. Its `discover()` requires provider configuration; the native configuration guard covers providerless `{model}` templates before launch/default save, without requiring a model input.
- The same domain validation permits a custom `{endpoint}` template with no resolved provider/configuration. Reject that combination before launch or restore instead of substituting an empty endpoint.
- Native configuration/readiness still rejects providerless `{endpoint}` commands. Model requirements now intentionally rely on backend automatic resolution; root coordinates these server-owned changes before integration and owns optional API/MCP overrides.
