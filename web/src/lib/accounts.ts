import type { AccountKind } from "../api/generated/AccountKind";
import type { AgentAuth } from "../api/generated/AgentAuth";
import type { AgentSelection } from "../api/generated/AgentSelection";
import { defaultProvider } from "./agents";

const DEFAULT_ACCOUNTS: Partial<Record<AgentSelection["kind"], AccountKind>> = {
  claude: "claude",
  codex: "chatgpt",
  gemini: "google",
  copilot: "copilot",
  opencode: "chatgpt",
  kilo: "kilo",
  goose: "claude",
};
const NAMES: Record<AccountKind, string> = {
  claude: "Claude account",
  chatgpt: "ChatGPT account",
  google: "Google Code Assist account",
  copilot: "GitHub Copilot account",
  kilo: "Kilo account and credits",
};
export function accountLabel(account: AccountKind): string {
  return NAMES[account];
}
export function authentication(agent: AgentSelection): AgentAuth {
  return agent.auth ?? (agent.account_kind || (agent.kind === "claude" && !agent.provider) ? "account" : "api");
}
export function selectedAccount(agent: AgentSelection): AccountKind | null {
  return authentication(agent) === "api" ? null : (agent.account_kind ?? DEFAULT_ACCOUNTS[agent.kind] ?? null);
}
export function changeAuthentication(agent: AgentSelection, auth: AgentAuth, accounts: AccountKind[]): AgentSelection {
  return auth === "account"
    ? { ...agent, auth, account_kind: accounts[0] ?? null, provider: null }
    : {
        ...agent,
        auth,
        account_kind: null,
        provider: agent.kind === "claude" ? "anthropic" : defaultProvider(agent.kind),
      };
}
