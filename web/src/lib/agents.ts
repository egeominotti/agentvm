import type { AgentCatalog } from "../api/generated/AgentCatalog";
import type { AccountKind } from "../api/generated/AccountKind";
import { accountLabel, authentication, changeAuthentication, selectedAccount } from "./accounts";
import type { AgentKind } from "../api/generated/AgentKind";
import type { AgentSelection } from "../api/generated/AgentSelection";
import type { ApiProtocol } from "../api/generated/ApiProtocol";
import type { Provider } from "../api/generated/Provider";

const NAMES: Record<AgentKind, string> = {
  claude: "Claude Code",
  codex: "Codex",
  gemini: "Gemini CLI",
  opencode: "OpenCode",
  qwen: "Qwen Code",
  copilot: "GitHub Copilot CLI",
  kilo: "Kilo CLI",
  aider: "Aider",
  goose: "Goose",
  custom: "Custom CLI",
};
export const CLAUDE_AGENT: AgentSelection = {
  kind: "claude",
  provider: null,
  version: null,
  custom: null,
  auth: null,
  account_kind: null,
};
export function selection(kind: AgentKind = "claude", accounts?: AccountKind[]): AgentSelection {
  const agent: AgentSelection = {
    kind,
    provider: defaultProvider(kind),
    version: null,
    custom: null,
    auth: null,
    account_kind: null,
  };
  return accounts?.length
    ? changeAuthentication(agent, "account", accounts)
    : accounts
      ? changeAuthentication(agent, "api", accounts)
      : agent;
}
export function defaultProvider(kind: AgentKind): Provider | null {
  return kind === "claude" || kind === "custom" ? null : kind === "gemini" ? "google" : "openai";
}
export function agentLabel(agent?: AgentSelection | null): string {
  return agent?.kind === "custom" ? agent.custom?.name || NAMES.custom : NAMES[agent?.kind ?? "claude"];
}
export function analyticsAvailable(agent?: AgentSelection | null): boolean {
  return !agent || agent.kind === "claude";
}
function supports(kind: AgentKind, protocol: ApiProtocol): boolean {
  if (kind === "claude") return protocol === "anthropic";
  if (kind === "codex" || kind === "goose") return protocol === "openai";
  if (kind === "gemini") return protocol === "google";
  if (kind === "qwen" || kind === "copilot") return protocol !== "google";
  return true;
}
export function compatibleProviders(kind: AgentKind, catalog: AgentCatalog) {
  const allowed = catalog.agents.find((a) => a.kind === kind)?.providers;
  return catalog.providers.filter((p) => (!allowed || allowed.includes(p.id)) && supports(kind, p.config.protocol));
}
export function launchReadiness(
  agent: AgentSelection,
  catalog: AgentCatalog | undefined,
  token: boolean,
): string | null {
  const name = agentLabel(agent);
  if (
    agent.version &&
    !(
      agent.version === "latest" ||
      (agent.kind === "claude" && agent.version === "stable") ||
      /^\d+\.\d+\.\d+(?:-[A-Za-z0-9.]+)?$/.test(agent.version)
    )
  )
    return "Choose latest or a safe release such as 1.2.3.";
  const custom = agent.kind === "custom" ? agent.custom : null;
  if (agent.kind === "custom" && !custom?.interactive.trim())
    return "Configure a custom interactive command in Settings › General.";
  if (authentication(agent) === "account") {
    const account = selectedAccount(agent);
    const eligible = catalog?.agents.find((d) => d.kind === agent.kind)?.accounts;
    if (!account || (eligible && !eligible.includes(account))) return `${name} does not support the selected account.`;
    if (agent.provider) return "Account authentication cannot select an API provider. Choose account or API access.";
    const status = catalog?.accounts?.find((candidate) => candidate.id === account);
    if (status?.connected || (account === "claude" && !status && (catalog?.claude_token ?? token))) return null;
    if (account === "claude" && !catalog?.accounts) return "Connect your Claude account in Settings › Claude account.";
    return `Connect your ${accountLabel(account)} in Settings › Accounts.${status?.error ? ` ${status.error}` : ""}`;
  }
  if (agent.account_kind) return "API authentication cannot select an account. Choose an API provider.";
  const provider =
    agent.provider ?? (agent.kind === "claude" && agent.auth === "api" ? "anthropic" : defaultProvider(agent.kind));
  if (!provider) {
    if ([custom?.interactive, custom?.headless, custom?.resume].some((t) => t?.includes("{endpoint}")))
      return "Select a provider with an endpoint for the {endpoint} command template.";
    return null;
  }
  if (!catalog) return "Reading provider settings…";
  const p = catalog.providers.find((candidate) => candidate.id === provider);
  if (!p || !p.config.endpoint.trim()) return `Configure the ${p?.name ?? provider} endpoint in Settings › Providers.`;
  try {
    const url = new URL(p.config.endpoint);
    if (!["http:", "https:"].includes(url.protocol) || url.username || url.password || url.search || url.hash)
      return `Fix the ${p.name} endpoint in Settings › Providers: use HTTP(S) without credentials, query or fragment.`;
  } catch {
    return `Configure an absolute HTTP(S) endpoint for ${p.name} in Settings › Providers.`;
  }
  if (!supports(agent.kind, p.config.protocol))
    return `${name} does not support the selected provider protocol. Choose a compatible provider.`;
  if (!compatibleProviders(agent.kind, catalog).some((candidate) => candidate.id === provider))
    return `${name} needs an endpoint that supports its request format. Choose a compatible provider.`;
  if (p.config.requires_key && !p.has_key) return `Add the ${p.name} key in Settings › Providers.`;
  return null;
}

export function launchDefaults(agent: AgentSelection | undefined, custom: AgentSelection["custom"]) {
  const selected = agent ?? CLAUDE_AGENT;
  return {
    agent: selected.kind === "custom" ? { ...selected, custom: selected.custom ?? custom } : selected,
  };
}
