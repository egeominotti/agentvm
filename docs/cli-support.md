# Coding CLIs

Choose a CLI in New VM or save a default in Settings. Each VM and snapshot keeps
its selection. Older tasks and snapshots retain the Claude Code default.

| CLI | Pinned recipe | Account access | API protocols |
| --- | --- | --- | --- |
| Claude Code | Golden image release | Claude subscription | Anthropic |
| Codex | 0.162.1 | ChatGPT | OpenAI Responses |
| Gemini CLI | 0.64.0 | Google Code Assist | Google Gemini |
| OpenCode V1 | 1.19.0 | ChatGPT | OpenAI, Anthropic, Google |
| Qwen Code | 0.25.1 | Plan keys use the provider mode | OpenAI, Anthropic |
| GitHub Copilot CLI | 1.0.98 | GitHub Copilot | OpenAI, Anthropic BYOK |
| Kilo CLI | 7.8.8 | Kilo account/credits, ChatGPT | OpenAI, Anthropic, Google |
| Aider | 0.86.2, Python 3.12 | Plan keys use the provider mode | OpenAI, Anthropic, Google |
| Goose | 1.54.0 | Claude or ChatGPT | OpenAI Chat Completions |
| Custom CLI | Your commands | Selected provider or no credential | Selected protocol |

Recipes are pinned to tested releases. Enter a semantic version or `latest` to
choose another; Claude also accepts `stable`. Each disposable disk installs the
selected CLI. Goose account mode also uses its selected Claude/Codex CLI backend.
The shared golden image remains unchanged. Restored disks reuse an installation
with the same selection and version.

## Account and subscription access

Select **Account** to use a connected account's entitlement. In Settings, connect
ChatGPT, Google Code Assist, GitHub Copilot or Kilo through browser sign-in or
**Import local login**, where offered. Import reads the supported host CLI login;
it never copies that credential into a VM. Claude uses `claude setup-token` and
its existing token field. Account status advertises the available connection
methods; an unsupported import or login action is not offered.

Codex and OpenCode host credentials that rotate are referenced read-only and
reloaded for requests. Sign in again with the host CLI when that session expires.
AgentVM-owned sign-ins and refreshable Google sessions live in the macOS Keychain.
The first Google sign-in or refresh after engine startup reads public client
metadata from the pinned official Gemini CLI source over HTTPS. This requires
access to `raw.githubusercontent.com`; metadata stays in host memory. A failed
download can be retried without changing the saved account.
Disconnecting an imported account removes AgentVM's connection and preserves the
host CLI login. GitHub.com Copilot accounts are supported; enterprise-server
login hosts are not currently imported.

OpenCode and Kilo use a Responses provider for ChatGPT account access. Goose uses
its Claude Code or Codex backend. The guest receives a dummy credential and a
loopback endpoint; the host performs authentication and refresh. Models come from
the account catalog or native CLI defaults. Plan and organization restrictions
still apply, including Copilot model access and Google Code Assist eligibility.

## Provider keys and billing plans

Select **API key** for provider access. Presets include OpenAI, Anthropic, Google,
OpenRouter, a local server, a custom endpoint, Alibaba Coding Plan, OpenCode Go and
OpenCode Zen. Keys can represent either a subscription or usage billing:

- **Alibaba Coding Plan:** use the plan's key and dedicated endpoint. The default
  is `https://coding-intl.dashscope.aliyuncs.com/v1`; the China endpoint is
  `https://coding.dashscope.aliyuncs.com/v1`. Claude and other Messages clients
  use Anthropic protocol and `/apps/anthropic` on the same regional host. The
  current Codex recipe requires Responses, which this plan does not provide.
  The service has no models endpoint,
  so its pinned preset supplies a documented coding model automatically.
- **OpenCode Go / Go Plus:** monthly-plan key and
  `https://opencode.ai/inference/go/v1`.
- **OpenCode Zen:** usage billing and `https://opencode.ai/inference/v1`.
- **Local/custom:** choose the actual protocol, endpoint and whether a key is
  required. Host-loopback model servers are reachable through the private relay.

Qwen's former qwen.ai OAuth sign-in was discontinued. Its current Coding Plan
uses the provider-key mode. This does not turn a monthly plan into usage billing:
the credential and chosen service determine billing.

No model ID is required in the launcher or settings. Native provider endpoints
use suitable CLI defaults; other integrations select a compatible chat/tool
model from the catalog. OpenCode Go/Zen catalogs are filtered for the CLI's wire
format. Local providers need an installed chat model; custom endpoints need a
model catalog when their CLI requires discovery. API/MCP clients can optionally
send an explicit model override. Protocol compatibility alone cannot guarantee
that every model implements every tool feature.

Provider keys are saved in the Mac's Keychain and never returned by the API. A
private per-VM vsock relay adds the host credential to inference requests. The VM
receives only dummy keys. Automatic runs preserve normalized text and tool events;
Claude usage analytics are shown only for Claude, without attributing other CLI
usage to Claude billing.

## Custom commands

Custom settings contain a name, optional installation script, interactive
command, automatic command and optional resume command. Installation runs as a
shell script inside the VM. Launch commands are an executable plus arguments:
quotes group arguments, and no shell expansion occurs. Placeholders `{prompt}`,
`{model}` and `{endpoint}` are replaced once inside each argument.

For a credential-free shell:

```text
Name: Root shell
Interactive: /bin/bash --noprofile --norc
Automatic: /bin/echo '{prompt}'
```

For another API client, use `{endpoint}` or the standard OpenAI, Anthropic and
Gemini environment variables supplied by the runtime. Keep real credentials out
of commands and installation scripts.

Official contracts: [Codex](https://github.com/openai/codex),
[Gemini](https://github.com/google-gemini/gemini-cli),
[OpenCode V1](https://github.com/anomalyco/opencode/releases/tag/v1.19.0),
[Qwen authentication](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/auth/),
[Copilot authentication](https://docs.github.com/en/copilot/how-tos/copilot-sdk/auth/authenticate),
[Kilo](https://kilo.ai/docs/code-with-ai/platforms/cli),
[Aider](https://aider.chat/docs/install/uv.html),
[Goose](https://github.com/aaif-goose/goose/releases/tag/v1.54.0),
[OpenCode Go](https://opencode.ai/v2/docs/console/go/),
[Alibaba model discovery](https://www.alibabacloud.com/help/en/model-studio/deepseek-harness).
