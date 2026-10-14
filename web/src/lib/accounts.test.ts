import { expect, it } from "vitest";
import type { AgentCatalog } from "../api/generated/AgentCatalog";
import { launchReadiness, selection } from "./agents";
import { authentication, changeAuthentication, selectedAccount } from "./accounts";

const catalog: AgentCatalog = {
  claude_token: false,
  agents: [
    {
      kind: "claude",
      name: "Claude Code",
      providers: ["anthropic"],
      accounts: ["claude"],
      headless: true,
      resume: true,
      requires_model: false,
    },
    {
      kind: "codex",
      name: "Codex",
      providers: ["openai"],
      accounts: ["chatgpt"],
      headless: true,
      resume: true,
      requires_model: false,
    },
    {
      kind: "qwen",
      name: "Qwen Code",
      providers: ["openai"],
      accounts: [],
      headless: true,
      resume: true,
      requires_model: true,
    },
  ],
  providers: [
    {
      id: "openai",
      name: "OpenAI",
      has_key: false,
      config: { endpoint: "https://api.openai.com/v1", protocol: "openai", requires_key: true },
    },
    {
      id: "anthropic",
      name: "Anthropic",
      has_key: false,
      config: { endpoint: "https://api.anthropic.com", protocol: "anthropic", requires_key: true },
    },
  ],
  accounts: [
    {
      id: "chatgpt",
      name: "ChatGPT account",
      connected: true,
      source: "Codex CLI",
      error: null,
      can_import: true,
      can_login: true,
    },
    {
      id: "claude",
      name: "Claude account",
      connected: false,
      source: null,
      error: null,
      can_import: true,
      can_login: false,
    },
  ],
};
it("allows connected ChatGPT account access without provider API keys or a Claude token", () => {
  const agent = { ...selection("codex"), auth: "account" as const, account_kind: "chatgpt" as const, provider: null };
  expect(launchReadiness(agent, catalog, false)).toBeNull();
});
it("requires the selected account instead of silently using an API key", () => {
  const agent = { ...selection("codex"), auth: "account" as const, account_kind: "chatgpt" as const, provider: null };
  const missing = {
    ...catalog,
    accounts: catalog.accounts.map((a) => ({ ...a, connected: false })),
    providers: catalog.providers.map((p) => ({ ...p, has_key: true })),
  };
  expect(launchReadiness(agent, missing, false)).toMatch(/ChatGPT.*Settings.*Accounts/);
});
it("keeps API mode separate from consumer subscriptions", () => {
  const agent = { ...selection("claude"), auth: "api" as const, account_kind: null, provider: null };
  expect(launchReadiness(agent, { ...catalog, claude_token: true }, true)).toMatch(/Anthropic.*key/);
});
it("preserves authentication defaults for legacy selections", () => {
  expect(authentication(selection("claude"))).toBe("account");
  expect(selectedAccount(selection("claude"))).toBe("claude");
  expect(authentication(selection("codex"))).toBe("api");
  expect(authentication({ ...selection("claude"), provider: "anthropic" })).toBe("api");
});
it("clears conflicting account and provider choices when changing authentication", () => {
  const account = changeAuthentication(selection("codex"), "account", ["chatgpt"]);
  expect(account).toMatchObject({ auth: "account", account_kind: "chatgpt", provider: null });
  expect(changeAuthentication(account, "api", ["chatgpt"])).toMatchObject({
    auth: "api",
    account_kind: null,
    provider: "openai",
  });
  expect(changeAuthentication(selection("claude"), "api", ["claude"])).toMatchObject({ provider: "anthropic" });
});
it("rejects accounts that are not supported by the selected CLI", () => {
  const agent = { ...selection("qwen"), auth: "account" as const, account_kind: "chatgpt" as const, provider: null };
  expect(launchReadiness(agent, catalog, false)).toMatch(/does not support.*account/i);
});
