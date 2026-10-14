# Native CLI and MCP integration

Root owns macOS sources, packaging, real native UI checks and the scoped commit.
The concurrent engine thread owns Rust, guest, helper, web and repository docs.
Keep commits scoped and preserve port 7777 and all user-created machines.

Coordinator handoff on October 15: root and its native subagents have no further
non-macOS edits pending. The only earlier native-owned non-macOS change is the
already-green `server/tests/app/golden.rs` preparation admission diagnostic;
the engine coordinator may include that file in its scoped integration commit.
All other current non-macOS changes belong to the engine coordinator. Native
source is frozen, all important review findings are fixed, and the latest full
gate is 196 collected tests / 50 suites / no failures / one intentional skip,
plus four Ghostty tests. The final native commit waits for release-bin freeze,
the native UI gate and artifact checks. No production restart has been performed.
Direct replies to coordinator thread `01a13bb0-15ba-71b0-aba7-22cf6074625a` remain
unavailable through the Codex transport on port 49445; this is the handoff record.

Terminal-stress handoff: native previously verified four commands exactly once
and in order through a real delayed WebSocket and forced reconnect; it did not
diagnose an equivalent 39/40 terminal failure. The current
`server/tests/vm/terminals_at_once.rs` initially collapsed Unix open errors, read errors,
EOF and timeout into `None` at lines 48-49, and discards connection identity at
line 52. Thus 39/40 does not yet prove the eight-second deadline expired. This
test uses PtyConnection → helper Unix socket → vsock → guest tmux, bypassing HTTP,
WebSocket and the native client. Preserve round/index/view, open/read durations
and distinct error/EOF/timeout/byte-count outcomes before changing its deadline.
The engine coordinator subsequently reproduced the missing client with precise
diagnostics, fixed the guest PTY reader lifetime before descriptor reuse, and
reported the unchanged-deadline run plus five repeats green. This is an engine
fix; native and its children made no guest/helper/transport edits.

## Account protocol review for the engine owner

During review, Goose advertised Claude account support
in `server/src/domain/agents.rs`, but submission resolves that account to the
Anthropic protocol in `server/src/app/accounts/access.rs` and passes its config
to `AgentSelection.validate` in `server/src/app/submission.rs`. The generic
`AgentKind::Goose` protocol check accepts only OpenAI, so this eligible account
selection is rejected before launch. Validate the account's supported transport
separately from the CLI's API-provider protocol policy. Add a real domain RED
regression for Goose + `auth: account` + `account_kind: claude` with its resolved
Anthropic configuration. Keep invalid API-provider combinations rejected.

The engine owner concurrently corrected this path: the current validation
applies the CLI's API-provider protocol restriction only when
`resolved_account().is_none()`. Eligible accounts therefore use their account
transport. The final backend regression gate still owns runtime verification.

The engine coordinator subsequently announced the provider IDs
`alibabacodingplan`, `opencodego` and `opencodezen`, all with default OpenAI
protocols. Native provider choices and editors consume catalog IDs and labels
as strings; they impose no provider enum or fixed six-provider list. The new
IDs therefore follow the same protocol, endpoint, credential and draft rules.
Their real catalog and UI verification waits for the final engine build.

The latest attempt to deliver this finding to the engine coordinator also failed
because its Codex transport endpoint on port 49445 was unavailable. The native
source review and final gate must not imply runtime verification until that
engine regression passes.

## Integration gates

- [x] Preserve the committed native baseline (`3e391ef`) and its original release
  archive; the archive SHA-256 is `7dff4e359ec4fb27b00950fef687f78c7a0eebc3fe2ce2805c3aecab79270537`.
- [x] Detect catalog support separately from background overview requests and
  quietly retain Claude compatibility on a genuine legacy 404.
- [x] Route non-Claude inspector output to bounded normalized events and retain
  actual Claude analytics for Claude tasks.
- [x] Validate selected CLI labels, model defaults and guest installation evidence.
- [x] Drain finite completed output, including legacy journal import, and retain
  streaming cancellation and bounded active output.
- [x] Verify configuration-only defaults and provider edits against a real owned
  engine without writing shared credentials.
- [x] Verify current Settings and launcher controls in the distinct native app,
  including compact/wide and light/dark layouts.
- [ ] Verify bundled MCP protocol, arm64 executables, minimum macOS version,
  virtualization entitlement and signatures.
- [x] Run the complete native source gate and review before the scoped commit and push.
- [ ] Complete account/API authentication, account Settings and their real UI
  checks against the final engine contract before publishing version 0.2.0.

## Current verification evidence

Final native source gate: 196 collected tests in 50 suites, no failures, 23.123
seconds, plus four actual Ghostty checks. Only the unchanged delayed socket test
was skipped to preserve an active user fixture. Both new read-only account API
tests passed against the current account-capable debug engine. Evidence is
`/tmp/agentvm-native-auth-final-source-full-green.log`.
Its owned metadata-only profile is `/tmp/agentvm-native-auth-contract-mxop2b2k`,
port 53866, PID 60923; it has no VMs and exposes all five accounts and nine provider
IDs. No Import, Login, Delete or credential writes were exercised. Final release
bins, package signatures/checksum and the account-aware native UI gate remain
pending the engine coordinator's release-binary freeze; port 7777 is preserved.

Manual native checks against that debug fixture confirmed both authentication
selectors, mutually exclusive account/provider fields, all nine provider editor
labels, no model input, remembered provider drafts and the launcher account CTA.
Twenty actual compact/wide resizes completed in 1.996 seconds of automation and
retained an invalid Alibaba endpoint draft with Save disabled; Discard restored
the saved endpoint. This measures automation and draft stability, not frame rate.
A real native General Save persisted Codex/account/ChatGPT with a null provider
and default model while disconnected, as intended for preferences. The original
public settings were restored, and all provider/account credential flags remained
identical. No authentication, import, deletion or paid inference was performed.
The debug catalog preceded the coordinator's final wire-format exclusions, so
these checks do not assert that every displayed provider works with every CLI.

The account/API extension passed 61 focused native tests in ten suites and four
Ghostty checks. Independent review found that pane navigation discarded a pending
login and that delayed responses could overwrite a replacement login. Both were
corrected with observed behavioral RED runs. Per-account public login state is
retained in memory without publishing through the workspace; captured-session
responses, errors, expiry and limit writes now require the current session ID.
The final focused gate passed 37 tests in seven suites with no skips.

The preceding full native gate collected 192 tests in 49 suites without failures
in 26.581 seconds. Three tests were skipped: two new read-only account-route
checks awaiting a fresh engine, plus the unchanged delayed socket check protecting
an active user fixture. The four subsequent login-race regressions passed in the
focused gate; the fresh full gate and account-aware UI captures remain pending.
The final QA-only native source build succeeded in 17.61 seconds without rebuilding
the concurrent engine or dashboard assets. Its subsequent isolated layout run
completed all 68 captures across fourteen Settings sections and three root routes
in both appearances and widths. The captures are retained in
`/tmp/agentvm-native-verify.dZvvhp/captures`; visual review precedes the source commit.
All 68 actual images were independently reviewed with no Critical or Important
visual findings. Lower Copilot/Kilo cards require scrolling beyond the captured
top account viewport; the full AX tree exposes their controls. The production
artifact's real-VM/preview and package gate remains separate after binary freeze.

Before the account extension, the native suite passed 165 tests in 43 suites in
22.531 seconds, plus four checks through the actual Ghostty WebAssembly core.
One delayed socket check was intentionally skipped to preserve an active VM;
that unchanged transport check passed in the committed baseline. Completed Bash
output loaded seven real events in one finite request in 0.110 seconds, without
opening SSE. The bundled MCP server initialized and listed 43 tools, then read
the owned idle profile's status through its real stdio protocol.

The CLI UI run produced 73 reviewed captures, including a real custom Bash VM.
Review found a blank wide-list Open button and duplicate example labels; both
were corrected. A later run confirmed the visible light-mode Open button and
model-free launcher, but a person opened New VM during the run and its sheet
obscured the workspace. Its remaining preview timeout is not clean UI evidence.
The QA harness now reports foreground state and renderer statistics; the final
account-aware light/dark and compact/wide run remains required.

An eight-second process sample during 20 actual Settings resizes showed 107.1 MB
physical footprint and 274.3 MB peak. The automation completed those changes in
3.290 seconds and preserved the unsaved endpoint. This is responsiveness and
draft-preservation evidence, not a frame-rate measurement or a zero-lag guarantee.

## Engine contract issues found during native review

The user subsequently required model input removal. Launcher and General no
longer expose model controls; every UI request omits `model`. The backend owns
automatic resolution and API/MCP optional model overrides. The original review
findings below remain relevant to restored custom templates and auto resolution,
rather than a request to restore a native input field.

These belong to the engine owner; native validation cannot protect direct API or
MCP clients. At the time of investigation:

1. Custom template model detection must include `resume`, so its `{model}`
   placeholder receives the automatically resolved model when restoring.
2. A custom command containing `{endpoint}` with no provider selected receives an
   empty endpoint in the guest. Validate all interactive/headless/resume templates
   against resolved provider presence before launch or restoration.
3. Explicitly saved non-Claude model IDs are arbitrary safe identifiers. Preserve
   valid IDs such as `opus` at initial load; reset Claude aliases when changing the
   agent kind, rather than discarding intentionally configured IDs.

The native launcher owner is implementing regressions for these cases. A message
to the engine owner's thread failed because its Codex transport endpoint on
127.0.0.1:49445 was unavailable; this record preserves the findings for handoff.

Native no-model regression evidence: 70 failing assertions before the new UI
policy, then 35 Swift tests across five suites with no skips and four real Ghostty
checks. All ten selected CLIs and legacy requests remove stale model payloads.
Public defaults preserve an API-set model when its CLI kind stays unchanged and
reset it to `default` when the CLI changes. Read-only actual model facts remain.

Golden fixture handoff: `server/tests/app/golden.rs` now admits cold preparation
for 30 seconds and records its PID/phase/duration on timeout. The repeated failure
was in its former 10-second preparation phase, before GoldenService started;
the actual rebuild watchdog remains 500 ms. Targeted real test passed in 0.60 s.
The parent coordinator owns the fresh complete parallel gate, with MCP `--help`
preparation before parallel subprocess-heavy test binaries.

## Owned new-engine fixture

`/tmp/agentvm-native-cli-280608yy`, port 7801, PID 20536, started from the new
runtime on 2026-10-14. The initial profile has no VMs. Its image links to the
existing golden disk read-only through normal clone operations. Configuration
checks may change only this fixture's public settings. Separate profiles still
share Keychain services; do not exercise credential Save or Remove.

Two owned headless custom tasks subsequently finished with `no_changes`: the
short output fixture `01a13c30-ff92-7ec6-97d2-ae257edb9597` and the paged output
fixture `01a13c61-90e1-7238-bea2-1556a609f204`. The latter ran Python inside its
disposable VM and emitted 601 literal lines, without inference. Its real journal
retains 500 records across five 100-record pages, including 498 text entries and
the final two states; 107 earlier records are omitted by the engine. The profile
returned to zero running machines. Its native feed regression remains a separate
gate from those read-only API observations. That regression subsequently failed
five behavioral assertions with a temporary first-page-only mutation, then passed
after restoring the production feed. Three real completed-output checks passed
with no skips in 0.583 seconds, plus four Ghostty checks. The five-page native
load completed in 0.133 seconds, retained the literal final row and final state,
and issued exactly five finite requests with no SSE connection.
