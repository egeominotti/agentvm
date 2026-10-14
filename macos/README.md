# agentvm Native

An additional Apple Silicon macOS app. The existing dashboard, engine and
command-line workflow remain available.

The workspace, launcher, settings, snapshots, file dialogs, menus, telemetry and
conversation views use SwiftUI and AppKit. A small WKWebView renders each terminal
using the dashboard's existing Ghostty/restty engine and xterm fallback. Terminal
assets and fonts are bundled locally. The app uses the same Rust engine, Swift VM
helper, API, Keychain credentials and `~/AgentVMs` data as the dashboard.

## Build and open

Requires Apple Silicon, Xcode Command Line Tools, Rust and Bun.
The native UI targets macOS 14 and Swift 5.9+. The packaged minimum macOS version
is derived from every included executable; the shared VM helper built on this
Mac requires macOS 27. The verified environment is macOS 27 with Swift 6.4 and
the selected stable macOS SDK. Native tests require Swift 6+ and use Swift Testing.

```bash
macos/build.sh
open 'macos/dist/agentvm Native.app'
```

The build produces a signed arm64 app and
`macos/dist/agentvm-Native-Apple-Silicon.zip`. Move the app to Applications if
desired. The build checks the packaged deployment requirement. Its VM helper
retains the virtualization entitlement. Signing is ad hoc;
a downloaded release is not notarized with an Apple Developer certificate.

The app attaches to an existing engine on `127.0.0.1:7777`. If none is running it
starts the bundled engine. Closing a window or quitting the app leaves the engine
and VMs running. A running engine is never restarted by the native app.

Set `AGENTVM_HOME` and `AGENTVM_PORT` to use a separate profile. Build output can be
selected with `AGENTVM_NATIVE_OUTPUT`; `AGENTVM_NATIVE_SDK` selects an installed SDK.

## Features

- Grid/list machines, search, state filters, sorting and finished-machine cleanup.
- Folder or git-link launcher, repository checks, branches, models, Claude Code
  versions, CPU/memory selection, interactive and automatic tasks.
- Claude and up to nine shell sessions, clipboard, file upload, ports and Tailscale.
- Save, close, force stop, push, snapshots, schedules, outcomes and branch diffs.
- CPU, memory, disk, network and process telemetry, usage, conversations and logs.
- Snapshot grouping/search, restore/resume, export/import, batch deletion and S3.
- All engine settings, Keychain accounts, S3 providers, storage and image management.
- Native light/dark appearance, notifications, menu bar and launch at login.

Keyboard shortcuts: Command-K opens New VM, Command-comma opens Settings,
Command-1 shows Machines, Command-2 shows Snapshots and Command-R refreshes.

## Verification

```bash
macos/test.sh --jobs 4
AGENTVM_NATIVE_TEST_URL=http://127.0.0.1:7793 macos/test.sh --jobs 4
scripts/test.sh
macos/verify.sh --port 7797
```

Swift Testing exercises pure decisions and, when supplied, a real isolated engine.
Write integration checks reject port 7777 and restore settings after testing.
Without fixture variables, integration checks are skipped. For the archive,
batch and telemetry checks, supply `AGENTVM_NATIVE_SNAPSHOT_TEST_URL`,
`AGENTVM_NATIVE_SNAPSHOT_ID` and `AGENTVM_NATIVE_TELEMETRY_TASK_ID` from a disposable
profile you own. `AGENTVM_NATIVE_ERROR_TEST_URL` selects another owned profile
configured with an unreachable S3 endpoint for the outage checks. The normal
`AGENTVM_NATIVE_TEST_URL` profile must have a working S3 listing when configured.

The UI verification runner is compiled only with
`AGENTVM_NATIVE_QA_BUILD=1`; ordinary production builds exclude it.
`macos/verify.sh` builds a distinct verification app in a new temporary profile,
refuses port 7777 and occupied ports, and preserves live or unknown machines.
It captures only the app's own window, checks light/dark and compact layouts,
and verifies the terminal component against an actual VM created through the API.
This harness does not verify clicks, keyboard focus or file dialogs. Test those
with an external UI test tool or the manual release checklist in OPERATIONS.md.
For a custom QA build, set `AGENTVM_NATIVE_QA_DIRECTORY` when launching it with a
temporary `/tmp/agentvm-native…` home and an isolated port.
To reuse an already built QA app, point to it explicitly:

```bash
AGENTVM_NATIVE_OUTPUT='/path/to/agentvm Native Verified.app' macos/verify.sh --port 7797 --no-build
```

`--no-build` rejects a production app without the QA harness marker.
For a focused layout check without creating a VM, add `--layouts-only`; set
`AGENTVM_NATIVE_QA_ROUTES=snapshots` to capture only that route in light/dark at
both widths. This mode does not perform the terminal or preview checks.
Screen captures use ScreenCaptureKit on macOS 14.4+.
QA builds also expose a Verification menu: Command-Option-7 selects 780×900 and
Command-Option-8 selects 1400×900. These commands resize and center the app's own
window for external UI checks; production builds exclude them.

S3 tests require an accessible configured service; `scripts/dev-s3.sh up` starts
the local RustFS endpoint on port 9100. A saved configuration does not mean that
the external service is currently reachable.

Separate data profiles share the user's default Keychain credential services.
Verification must not replace or remove account, git, Tailscale or S3 secrets.
Operational recovery steps, measured verification results and release limits are
documented in [OPERATIONS.md](OPERATIONS.md).
