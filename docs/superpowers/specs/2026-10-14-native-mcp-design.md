# Native MCP control of AgentVM

The user additionally requested native MCP so AgentVM can be fully controlled
from external clients. This extends the ongoing multi-CLI work.

## Deliverable

A native Rust `agentvm-mcp` binary ships with the app and the command-line build.
It uses the official Rust MCP SDK and newline-delimited stdio. It talks to the
existing loopback daemon and never creates another supervisor or owns VMs.
Closing the client or MCP process leaves the daemon and VMs running. Remote
clients can use an authenticated SSH stdio connection to the Mac; this does not
require exposing an unauthenticated HTTP control port.

Expose typed tools for host status, agent catalog, VM list/detail/launch/save/
close/stop/delete, repository checks, diagnostics/diff/telemetry, snapshots and
backup operations, settings/provider configuration, execution, guest files and
terminal I/O. A validated JSON API tool covers every existing JSON REST route;
archive import/export and upload tools stream files without loading large disks
into tool results. An API reference resource documents routes and semantics.

Structured guest control uses the existing port-5000 vsock transport, with a
`{"control":true}` header and length-framed JSON requests. It is implemented in
small Python modules shipped on new VM launches. Execution takes literal argv,
with an explicit shell tool for scripts; starts return execution IDs. Reads and
cancel use those IDs. Timeout/cancel kills the execution's process group and
collects bounded stdout/stderr. File APIs support listing/stat, chunked binary
read/write, mkdir/remove/rename. Terminal APIs support list/capture, literal text
and keys, creation of supported shell sessions and resizing; detached clients
leave tmux alive. Identifiers are explicit arguments, not MCP connection state.

Use MCP 2026-07-28 and legacy-client compatibility provided by the SDK. Return
structured JSON and readable text; route failures are tool errors. Advertise
mutation/read-only/destructive annotations accurately. Requests and secret
arguments are not logged. Provider/Claude/git/S3/Tailscale keys can be saved but
never read back from host APIs. New VMs use host-side inference credentials.
Legacy guest runtimes cannot serve structured control and return a clear upgrade
message; their existing lifecycle controls remain available and their VMs remain
untouched during implementation.

## Verification

- Exercise real JSON-RPC stdio messages and official SDK handling, including
  discovery/tool listing/tool calls and disconnect behavior.
- Execute against the real AgentVM router/daemon in an isolated temporary home.
- Pure input/route/schema tests reject absolute URLs, non-API paths and unknown
  methods; secrets never appear in tool results or schema examples.
- Real guest process/file tests verify literal argv, timeout/cancel cleanup,
  bounded output, binary transfers and directory operations without fake CLIs.
- Run real VM control and relay/recovery tests on an isolated engine, without
  touching port 7777 or spending inference quota unnecessarily.
- Bundle the binary in the final app and document local/SSH client configuration.
