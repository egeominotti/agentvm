# Account and subscription access for coding CLIs

## Intent

The user corrected AgentVM's API-only treatment of non-Claude CLIs. A supported
CLI must offer its supported account or subscription access alongside provider
API keys. Keep the simple quickstart and remove all model-ID fields.

## Authentication choices

`AgentSelection` gains optional `auth` (`account` or `api`) and `account_kind`.
Omitted fields preserve existing selections, including Claude's subscription
default. `AccountKind` has `claude`, `chatgpt`, `google`, `copilot`, and `kilo`.
The catalog lists eligible account kinds for each CLI and connection status.

- Claude uses its existing Claude account token or Anthropic provider API.
- Codex uses ChatGPT account access or OpenAI-compatible Responses APIs.
- Gemini uses Google account/Code Assist access, including eligible AI plans,
  or the Gemini API.
- Copilot uses its GitHub account/plan or its documented offline BYOK mode.
- OpenCode supports ChatGPT through a custom Responses provider and API keys.
- Kilo supports its Kilo account/credit plan, ChatGPT, or provider API keys.
- Goose can use Claude/Codex CLI providers with the same host account relays,
  or its existing API provider. Prefer the pinned provider's supported adapter.
- Qwen uses provider keys, including Alibaba Coding Plan keys and its dedicated
  regional endpoint. The discontinued qwen.ai OAuth flow is not offered.
- Aider uses provider APIs. Custom commands keep their existing API/no-provider
  behavior.

API-key billing plans must be labelled accurately; an account is not synonymous
with a consumer subscription, and a monthly credit plan is not an API PAYG key.

## Host credentials

Guests receive dummy credentials and a private loopback relay. Host account
access supplies the real bearer/account headers and may refresh credentials on
the Mac. Credentials never enter task metadata, snapshots, logs, API responses,
command arguments, or newly written credential files.

Existing host CLI sign-ins can be referenced read-only. Rotating refresh tokens
must not be copied into a second independent owner. A referenced session is
reloaded before requests; expired sessions require the native host sign-in when
AgentVM does not own its refresh token. AgentVM-owned device/browser login stores
its session only in Keychain and serializes refresh per account.

ChatGPT CLI sessions use the fixed ChatGPT Codex backend, account-ID headers and
the account's visible model catalog. A separately registered SIWC direct-API
session has a different endpoint and must never be confused with CLI sessions.
Google account mode uses Code Assist with host Google OAuth refresh. Copilot
mints its short-lived backend token on the host and validates the returned
regional backend. Kilo performs device sign-in on the host; expired or revoked
opaque account tokens need a new sign-in.

## API and UI

- `GET /api/agents` adds `accounts` and per-CLI eligible account kinds.
- `/api/settings/accounts/{account}` exposes connection status and deletion.
- `POST /api/settings/accounts/{account}/import` references a local CLI login.
- `POST /api/settings/accounts/{account}/login` starts a host sign-in and returns
  a login ID, verification URL and optional user code, never a credential.
- `GET /api/settings/accounts/{account}/login/{id}` polls finite status.
- Both clients show Authentication and then the account or API provider choice.
  Account settings show connected state, Import local login, Sign in and Remove.
- The native MCP API tool and reference cover every new account route. Add typed
  tools where that helps account connection without duplicating API behavior.

The account's model is selected automatically. ChatGPT's `models[].slug` catalog
uses visibility and priority rather than the ordinary OpenAI `data[].id` shape.
Google account mode keeps Gemini's native Code Assist model choice. Subscription
transports preserve their native request formats and streaming responses.

## Responsibilities and checks

The backend implementer owns account domain types, persistence/import/login,
app/HTTP routes, catalog changes and generated types. Root owns relay adaptation,
launch/recovery integration and final verification. Guest implementation owns
dummy auth, account-mode CLI configs/arguments/model discovery and its tests.
The web implementer owns account/API selectors and account settings. The active
native task retains ownership of all macOS sources and applies the shared API.

Use red/green pure decision tests and actual Keychain/files/subprocesses. Exercise
available account model discovery without inference, real pinned CLI launch
paths, sandbox clients and MCP schemas. Do not spend inference quota for login
or installer validation. Complete the repository fast suite, VM checks and
native/web visual checks before committing, pushing and publishing v0.2.0.
