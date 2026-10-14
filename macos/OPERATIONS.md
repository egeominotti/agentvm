# Native app operations

The application is a native SwiftUI/AppKit interface to the existing Rust engine.
The Swift helper owns each VM. Terminal rendering uses locally bundled
Ghostty/restty or xterm assets in WKWebView. Packaging does not replace the engine
with Swift or change the dashboard's API.

## Profiles and credentials

`AGENTVM_HOME` selects engine data and `AGENTVM_PORT` selects a local endpoint.
The default profile is `~/AgentVMs` on `127.0.0.1:7777`. Opening the native app
attaches to an existing engine; quitting leaves the engine and machines running.
Use separate temporary homes and ports for verification.

Separate homes share the user's default macOS Keychain services. They do not
isolate Claude, git, S3 or Tailscale credentials. Saving or removing credentials
from a verification profile can affect the regular profile. Automated UI checks
read credential presence and never exercise credential replacement or removal.
Secrets belong in SecureField and the Keychain; do not put them in command
arguments, diagnostic files, URLs or release evidence.

The engine accepts local connections and rejects foreign Host/Origin values.
Its permissions are those of the signed-in Mac user. Central account management,
role-based access and a multi-user server are not provided by this local app.

## Snapshots and recovery

Snapshots preserve the VM disk, installed packages and the selected agent's history.
Local snapshots share compressed chunks. The size shown for an individual
snapshot estimates its exclusive chunk cost, rather than its full portable size.
An export or S3 backup includes every chunk needed to restore, including shared
system chunks. Each S3 archive is self-contained; remote archives do not share
chunks with other backup objects.

For a recovery check:

1. Save a known marker file inside a disposable VM and take a snapshot.
2. Export it with the native save dialog and import it into a separate profile.
3. Restore it and read the marker in the new VM.
4. Back up the snapshot, read its S3 listing, remove only the disposable local
   copy, bring it back and repeat the marker check.

Only restore archives from a trusted source. Chunk hashes detect damaged content;
they are not an archive signature or encryption. Use protected storage and the
provider's access controls and encryption settings for backup retention.
Restore currently requires the snapshot's repository path and starting commit to
exist on the destination Mac. Preserve a clone containing that commit when moving
archives between machines.

Keep sufficient free space for the export on the engine, the download's temporary
file, and the selected destination. The native client streams archive files through
URLSession rather than loading an entire archive into memory. The engine still
needs disk and CPU time to prepare a portable archive.

## S3 service availability

A saved configuration is shown as saved until a successful connection check.
The local RustFS service is separate from the app and must remain running when
its endpoint is configured. Start the existing development service with:

```bash
scripts/dev-s3.sh up
```

It serves S3 on `http://127.0.0.1:9100` and its console on port 9101. Its Docker
volume survives `scripts/dev-s3.sh down`. The `reset` command deletes that volume.
For remote storage, verify the endpoint, region, bucket permissions, access key
and saved secret with **Test saved connection**. A failed connection does not
mean the Mac's engine or local snapshots are unavailable.

Background S3 failures appear inline in Snapshots with retry and settings actions.
They must not repeatedly interrupt the workspace with modal alerts. Explicit
backup actions still report real failures. A successful saved-connection check
reloads the backup listing so a previous outage banner is cleared with fresh data.
Keep provider retention rules and
backups of the repository itself separate from VM snapshot retention.

## Verified example and performance limits

An isolated Apple Silicon verification used a real 1-vCPU, 2-GB VM, wrote a marker,
took a snapshot, uploaded a 792-MB archive to local RustFS, exported 830,933,851
bytes, imported it, brought the backup back from S3, and restored another VM.
The restored guest contained the identical marker and uncommitted file.
This is recovery evidence for that fixture, not a maximum disk-size benchmark.
The native client's streamed snapshot roundtrip also passed in 13.985 seconds
against the real isolated engine. The complete native suite later passed 101 tests
in 30 suites with no skips in 21.894 seconds, plus four checks through the actual
Ghostty WebAssembly core. A delayed real WebSocket handshake and forced disconnect
also preserved four shell commands exactly once and in order.

Native layout review inspected 61 captures: 52 empty-workspace and Settings views,
eight filled grid/list views, and one real VM terminal view. The captures cover
light/dark appearance at 780 and 1400 pixels. After correcting the Snapshots
viewport, a focused run verified four additional layouts with the complete header,
sidebar, statistics, search and filters visible. The real VM check also confirmed
a nonblank live preview with one shared Ghostty WebAssembly instance, shell input
reaching a guest marker file, and OSC 52 clipboard delivery with restoration of
the previous clipboard. The harness stopped its own VM before closing its engine.

Memory pressure reported 90% free and no swap before that run. Every engine and
test profile on the Mac shares physical memory. The scheduler waits for free
memory before launching; increasing concurrency cannot create additional RAM.
Archive compression and chunk compaction use CPU and disk bandwidth; run large
recovery drills sequentially when foreground workloads need those resources.

UI polling should follow the visible work. Local compaction needs local updates;
it does not need repeated S3 listings. Remote listing requests may fetch metadata
for each archive, so keep cloud polling slower than active local progress checks.
The snapshot view updates local compaction every 2 seconds, idle local data every
15 seconds, and an open S3 tab every 30 seconds; its polling pauses while inactive.
A real unavailable-S3 regression improved local-only refresh from 3.073 seconds
to 0.035 seconds after remote work was removed from that refresh.
A real two-snapshot deletion check reduced follow-up API reads from 15 to 2:
one snapshot listing and one storage summary, with no S3 or overview requests.
The check deleted only imported fixture copies and preserved the original snapshot.
Telemetry and usage histories prepare their chart models once per response away
from the UI thread. Native Canvas plots draw at most 300 samples per metric,
retaining bucket minima/maxima and separating missing-data intervals. Latest
values, peaks and selected readings use the complete API response. This bounds rendering
work without replacing recorded measurements with averages.
Measure request counts, response time, CPU and memory on the actual workload
before setting a production concurrency or retention policy.

The CLI and account integration subsequently collected 196 native tests in 50
suites without failures in 23.123 seconds, with four actual Ghostty checks. One existing delayed terminal-input
check was intentionally skipped to preserve an active verification VM; the
committed baseline had already passed that check against a real delayed socket.
The new real public-provider/default/custom-command roundtrip restored its owned
profile and left all credential availability flags unchanged. A completed real
custom Bash task loaded its literal output through one finite request in 0.110
seconds, without opening SSE. Active output batches updates at most five times
per second, keeps 500 entries within approximately 1 MB, and pauses when inactive.
Completed output drains cursor pages and supports the engine's legacy journal
import. Interactive CLI output stays in the terminal; its runner does not create
a normalized event stream. Claude's recorded billing view applies to Claude tasks.

A larger real custom task emitted 601 literal lines. The native feed loaded its
retained 500 records across five requests in 0.133 seconds, showing 498 ordered
text entries and the final state, with 107 earlier records reported as omitted.
The read-only account-route checks passed against the current debug engine and
inspected five account statuses without beginning authentication or changing
credentials. The release artifact has a separate runtime and UI verification gate.
The account-aware source build also produced 68 independently reviewed captures:
all fourteen Settings sections and three root routes at both widths and themes.
Manual controls verified account/API transitions, new provider drafts, twenty
resizes and a public defaults Save/restore with unchanged credential flags.

Account login metadata stays in memory across pane navigation. Polling follows
the visible active pane, stops at expiry or its finite bound, and rejects late
responses and errors belonging to cleared or replaced sessions. Pending checks
publish no workspace changes. Service capabilities control Import and Sign in;
subscription API keys and provider usage billing remain separate from account
authentication. Automated verification never imports, replaces or removes shared
host account connections.

## DMG installation and packaging

Download the Apple Silicon DMG from the GitHub release, open it, and drag
**agentvm Native.app** onto **Applications**. Eject the disk image before opening
the installed app. The ZIP contains the same app as an alternative download.
The release's `SHA256SUMS` covers the DMG, native ZIP and CLI archive.

Homebrew installs the same DMG through the project's tap:

```bash
brew install --cask egeominotti/tap/agentvm
```

The current release is ad-hoc signed and is not notarized. If macOS blocks the
first launch, use **System Settings → Privacy & Security → Open Anyway** after
trying to open the app. See [Apple's approval instructions](https://support.apple.com/en-us/102445).
The tap preserves quarantine and does not change system security settings.

After building and verifying the native app, create the standard installer image:

```bash
scripts/package-dmg.sh "macos/dist/agentvm Native.app" \
  "macos/dist/agentvm-Native-Apple-Silicon.dmg"
bun test scripts/tests/package-dmg.test.ts
```

The packager copies the signed bundle into temporary staging with an
`Applications` shortcut, creates a compressed read-only image, verifies it and
removes staging. It preserves the source app and its signature. The real-artifact
test mounts its own image read-only, checks the shortcut and signatures, runs
`--check-resources` from the mounted app without starting the engine, and detaches
the image. Set `AGENTVM_DMG_TEST_APP` to check a different verified bundle.
Set `AGENTVM_DMG_TEST_IMAGE` to verify an existing DMG before uploading it.
DMG packaging does not change the app's ad-hoc signing or notarization status.

## Release verification

Run the native decisions and real integration checks against a separate engine:

```bash
macos/test.sh --jobs 4
AGENTVM_NATIVE_TEST_URL=http://127.0.0.1:7793 macos/test.sh --jobs 4
macos/verify.sh --port 7797
scripts/test.sh
```

Default native test runs skip checks requiring external fixtures. A complete run
also supplies `AGENTVM_NATIVE_SNAPSHOT_TEST_URL` with an owned disposable snapshot
and its `AGENTVM_NATIVE_SNAPSHOT_ID` and `AGENTVM_NATIVE_TELEMETRY_TASK_ID`.
`AGENTVM_NATIVE_ERROR_TEST_URL` points to a separate owned profile with an
unreachable S3 endpoint. Keep its fixture configuration separate from the normal
integration profile, and never change shared Keychain credentials to induce an outage.
The real terminal transport check also needs `AGENTVM_NATIVE_TERMINAL_TASK_ID` and
`AGENTVM_NATIVE_TERMINAL_SHARE_PATH` for an owned running fixture. Check that its
Shell 8 is unused first; the test closes only that session and removes its unique
shared marker file. `AGENTVM_NATIVE_S3_RECOVERY_TEST_URL` selects an owned profile
whose configured S3 endpoint is a closed private loopback port. Its check briefly
forwards that port to the existing S3 service on 9100 without changing signed
requests, credentials, engine configuration or the S3 service.

`verify.sh` builds a distinct verification app, creates a new temporary profile,
uses an existing golden image, captures light/dark and compact layouts, and checks
the terminal component against a real VM created through the API. It refuses port 7777 and
occupied ports. It retains evidence and preserves the engine if live or unknown
machines remain. Review screenshots, logs and result/failure files before release;
captures can contain repository paths and conversation content.
Use `--layouts-only` for a layout correction that does not need another VM;
`AGENTVM_NATIVE_QA_ROUTES` accepts comma-separated `machines,snapshots,settings`.
This focused mode is not a terminal or preview verification.

The in-app harness cannot establish whether real native controls accept clicks or
keyboard focus. Use an external UI test tool, or complete these manual checks in
the distinct verification app before release:

- Open New VM, reject an invalid repository, choose a real disposable repository
  and its resources, then create and inspect only that known machine.
- Navigate Settings rapidly without cancellation alerts. Save non-secret drafts,
  check narrow light/dark layouts, and exercise native folder/file dialogs.
- Search and filter known snapshots, select several, confirm bulk deletion, and
  export, import and restore a disposable fixture with a guest marker.
- Check S3 outage, retry, listing, backup, bring-back and deletion against known
  fixture objects. Skip credential Save/Remove in profiles sharing the Keychain.
- Type into Claude and shell tabs; check clipboard, file upload, port links,
  inspector panels and keyboard shortcuts with the app in the foreground.
- Resize repeatedly while telemetry and conversation histories are visible,
  including Whole life. Check responsiveness and exact selected readings.

Record the app artifact, profile, owned machine IDs and evidence. Preserve every
other machine, including machines opened by another person during verification.

Before distributing a build, confirm arm64 binaries, helper virtualization
entitlement, deep signature verification, successful UI and recovery evidence,
and the artifact checksum and source commit. Current packaging uses ad-hoc
signatures. Managed external distribution needs a signing/notarization and release
process appropriate to the organization; that infrastructure is not supplied by
the app.

Keep the previous app artifact and the engine data available for rollback. A
window-only rollback can reconnect to an existing compatible engine; replacing
the on-disk app does not restart it. Plan engine maintenance separately from a UI
release and preserve running-machine ownership throughout it.
