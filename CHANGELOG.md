# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.1] - 2026-10-15

### Fixed

- Restored native package builds and tests on the stable Swift toolchain used by GitHub Actions.

## [0.2.0] - 2026-10-15

### Added

- Native macOS workspace, settings, snapshots and terminal previews sharing the engine.
- Nine coding CLI integrations plus reusable custom commands, with pinned ARM64 installers.
- Separate account and API authentication, with host account import and browser sign-in.
- ChatGPT, Google Code Assist, GitHub Copilot and Kilo account connections alongside Claude.
- Alibaba Coding Plan and OpenCode Go subscription-key presets, plus OpenCode Zen usage billing.
- Host-only provider credentials, a private inference relay and automatic model selection.
- Native stdio MCP with 43 tools for VM lifecycle, processes, files, terminals and artifacts.
- Finite, redacted agent output with stable cursors and recovery across engine restarts.
- Packaged native app, MCP binary and CLI releases with checksums.

### Changed

- Removed Model ID entry from launchers and settings; API/MCP overrides remain optional.
- Kept quickstart to one command and enabled direct startup from prebuilt archives.
- Preserved each running VM's provider configuration across settings edits and restarts.
- Improved native resize behavior, terminal updates and narrow dashboard layouts.

### Fixed

- Prevented early terminal attachment from interrupting CLI installation.
- Preserved output sequence metadata when compacted journals exceed the bounded reader.
- Fixed tmux targeting and empty session lists during guest startup.
- Fixed concurrent terminal reconnects that could leave a newly opened terminal blank.

## Initial implementation

### Added
- Every terminal is its own Debian 13 arm64 VM on Virtualization.framework, booted in ~3 s from a
  copy-on-write clone of a golden image, with interactive Claude Code running as root.
- Web dashboard: live wall of machines, focused machine view with Claude and a root shell,
  telemetry (CPU, memory, disk, network, processes) and Claude usage (cost, tokens, lines).
- Boot sequence built from real events with timings.
- Work returns to the repository as `agent/<id>` branches; per-file diffs and git commands.
- Model and Claude Code version per launch; vCPUs and memory per VM.
- Snapshots of running VMs with restore (Claude continues the conversation), download/import as
  `.tar.zst`, and backups to any S3-compatible storage with multipart uploads.
- Ports opened inside a VM are forwarded to `127.0.0.1` on the Mac.
- `.agentvm/setup.sh` in a repository runs before Claude starts.
- VMs survive server restarts; tasks are persisted and re-attached.
- Settings page: resources, model, time limits, Claude token, VM image rebuilds, S3, storage,
  desktop notifications.
- Local JSON API, CI on GitHub Actions, tests against real VMs, Claude and S3 (no mocks).
- A browser for Claude in every VM (headless Chromium + Playwright MCP).
- Every VM serves the same ports under its own name (`<port>.<vm>.localhost`), with a direct
  TCP forward for non-HTTP services.
- Automatic snapshots on a schedule and before closing; per-machine interval.
- Copy/paste in terminals and drag-and-drop of files into VMs.
- Idle VMs give memory back to the Mac; cheap guest disk flushes; shared repository bundles.

### Changed
- New VMs boot straight into the kernel kept beside the golden image, skipping the EFI firmware and
  GRUB: ready in ~1.5 s instead of ~2.5 s. Restored snapshots still boot through EFI.
- A terminal connects as soon as its GPU renderer is up: opening a machine draws its terminal in
  ~25 ms instead of ~75 ms.

### Security
- The host never follows symlinks or blocks on FIFOs planted by a guest in its shared folder.
- Saves never power a VM off on a git error; imports never overwrite commits made by hand.
