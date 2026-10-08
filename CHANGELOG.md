# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
- Private API token (cookie for the browser, Bearer for scripts).
- Idle VMs give memory back to the Mac; cheap guest disk flushes; shared repository bundles.

### Security
- The host never follows symlinks or blocks on FIFOs planted by a guest in its shared folder.
- Saves never power a VM off on a git error; imports never overwrite commits made by hand.
