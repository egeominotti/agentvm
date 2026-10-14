# Native Live Preview Implementation Plan

> Use executing-plans inline and test-driven development. Root owns commits and the final release review.

**Goal:** Show the live read-only Claude terminal in native machine cards without a GPU renderer per card or conversation-history downloads.

**Architecture:** A single hidden WebKit runtime shares one Ghostty WASM instance across headless parser handles. Native Swift sockets deliver bounded batches; native text screens display parsed state. Actual viewport visibility and active scene state stop sockets and dispose parser handles.

**Tech stack:** SwiftUI, AppKit, URLSessionWebSocketTask, WebKit, Bun, existing restty/headless.

**Spec:** Root's release-review task and the existing `web/src/features/wall/Preview.tsx` protocol.

## Global constraints

- Only new native files, the preview definition removal, and the native asset entry point change.
- Every file stays below 300 lines. UI, code and comments use English.
- No mocks, stubs, fake servers, interactive preview inputs or VM resize messages.
- No GUI actions, release builds, :7777 changes, VM mutations, commits or pushes by this agent.
- Parent owns final integration. Existing main-branch authorization overrides worktree and per-task commit defaults.

## Review focus

- Inactive and offscreen cards must stop streaming and preserve their last actual screen.
- A paused or replaced stream must not deliver stale asynchronous results.
- A burst must stay below 1 MiB before a redraw/reconnect.
- Split UTF-8, wide characters and erased terminal text must render through the actual parser.
- Cold parser readiness and WebContent termination must recover only preview clients.

### Task 1: Native protocol and bounded output

Files: `Machines/Preview/NativePreviewPolicy.swift`, `Tests/AgentVMNativeTests/NativePreviewPolicyTests.swift`.

Interfaces: `NativePreviewPolicy.request(baseURL:id:) -> URLRequest?`, `shouldStream(visible:active:running:interactive:ready:) -> Bool`, and `NativePreviewBuffer.append/take/clear`.

- [x] Write five tests for local read-only URLs, visibility gates, byte ordering and overflow.
- [x] Observe the missing-policy failure and 17 failing behavior assertions before writing production behavior.
- [x] Implement local `session=claude&view=true&cols=200&rows=60` requests and a 1 MiB buffer.
- [x] Run the targeted Swift tests; five policy tests pass.

### Task 2: Shared headless runtime and native preview lifecycle

Files: `macos/preview-screen.ts`, `macos/preview.ts`, `Resources/preview.html`, `Machines/Preview/NativePreviewHub.swift`, `NativePreviewFeed.swift`, `NativePreviewVisibility.swift`, `NativeMachinePreview.swift`.

Interfaces: JS `nativePreview.create/remove/frame/stats`, hub attach/detach/frame, per-card published parsed screen.

- [x] Write real Ghostty tests for erasure, split UTF-8, wide/grapheme text, shared WASM and invisible text.
- [x] Observe four screen-conversion failures, then implement native-readable snapshots.
- [x] Bundle the headless asset beside the existing terminal asset.
- [x] Connect only after parser readiness, batch at least 200 ms apart, and reject stale generations.
- [x] Pause on viewport/scene changes, dispose handles on detach, recover the shared renderer without touching VMs.
- [x] Remove only the old preview definition from the card file after notifying root.
- [x] Run Bun/parser tests and coordinated native debug verification.

### Task 3: Review and runtime evidence

- [x] Request independent review from root with exact changed files and test evidence.
- [ ] Coordinate real read-only PTY verification through root, including a native screen sentinel and unchanged interactive terminal dimensions.
- [x] Record actual results and unresolved limitations, then freeze all preview source.

## Progress and rulings

- Scope and execution were explicitly assigned by root; no approval questions or new branches.
- Root owns shared test/release builds; only the new test files are owned here.
- This document is the local ledger because parent restricts writes to native files and handles final commits.
- Root's source review found a queued visibility callback could run after the view closes. An actual NSView regression first observed `[false, false]`; closed guards now prevent the stale callback.
- Fresh `macos/test.sh --filter NativePreviewPolicyTests` passes all six Swift tests and four actual Ghostty Bun tests. The native debug package compiles. Logs are `/tmp/agentvm-native-preview-visibility-red.log` and `/tmp/agentvm-native-preview-visibility-green.log`.
- Core reported the preceding full gate at 100 Swift tests in 30 suites, zero skips, plus four Bun tests. The final visibility regression adds one Swift test; the final full rerun is coordinated by root/Core.
- `nativePreview.stats()` exposes actual handle count, distinct WASM object count and nonblank parsed screens for root's real PTY/UI checks. Those real VM and filled-grid checks remain pending at this handoff; this agent did not run GUI actions or mutate VMs.
- All preview source is frozen after the targeted green run. Parent owns release packaging, final runtime evidence, commits and push.
