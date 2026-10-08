# Security

## Model

agentvm runs AI agents with full root access **inside** disposable virtual machines. The design
goal is that nothing an agent does can reach the Mac or other agents:

- Each VM sees only its own job folder (virtiofs); the repository enters and leaves as a git bundle.
- Terminals and port forwarding travel over vsock, not the network.
- The Claude token is handed to Claude on a file descriptor, never in an environment variable,
  redacted from logs, and destroyed with the VM. The S3 secret stays in the macOS Keychain and
  reaches `curl` on stdin.
- The dashboard and every forwarded port listen on `127.0.0.1` only. Requests whose `Host` or
  `Origin` is not local are rejected, which blocks DNS rebinding and cross-site WebSockets.
- VMs have outbound internet access through NAT. Treat what an agent fetches from the internet
  as untrusted, like any other code you did not write.

## Reporting a vulnerability

Please do **not** open a public issue. Use GitHub's
[private vulnerability reporting](https://github.com/egeominotti/agentvm/security/advisories/new)
with steps to reproduce and the impact you see. You will get an answer within a few days.
