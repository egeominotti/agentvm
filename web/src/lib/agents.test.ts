import { expect, it } from "vitest";
import type { AgentCatalog } from "../api/generated/AgentCatalog";
import {
  agentLabel,
  analyticsAvailable,
  compatibleProviders,
  launchReadiness,
  launchDefaults,
  selection,
} from "./agents";
const catalog: AgentCatalog = {
  claude_token: false,
  accounts: [],
  agents: [
    {
      kind: "claude",
      name: "Claude Code",
      providers: ["anthropic", "custom"],
      accounts: ["claude"],
      headless: true,
      resume: true,
      requires_model: false,
    },
    {
      kind: "codex",
      name: "Codex",
      providers: ["openai", "local", "custom"],
      accounts: ["chatgpt"],
      headless: true,
      resume: true,
      requires_model: false,
    },
    {
      kind: "goose",
      name: "Goose",
      providers: ["openai"],
      accounts: ["claude", "chatgpt"],
      headless: true,
      resume: true,
      requires_model: true,
    },
  ],
  providers: [
    {
      id: "openai",
      name: "OpenAI",
      has_key: true,
      config: { endpoint: "https://api.openai.com/v1", protocol: "openai", requires_key: true },
    },
    {
      id: "local",
      name: "Local",
      has_key: false,
      config: { endpoint: "http://127.0.0.1:11434/v1", protocol: "openai", requires_key: false },
    },
    { id: "custom", name: "Custom", has_key: false, config: { endpoint: "", protocol: "openai", requires_key: true } },
  ],
};
it("launches Codex using its configured provider without a Claude token", () => {
  expect(launchReadiness(selection("codex"), catalog, false)).toBeNull();
});
it("explains which provider credential needs configuring", () => {
  const missing = { ...catalog, providers: catalog.providers.map((p) => ({ ...p, has_key: false })) };
  expect(launchReadiness(selection("codex"), missing, false)).toMatch(/OpenAI.*Settings.*Providers/);
});
it("lets the backend choose a model for CLIs that require one", () => {
  expect(launchReadiness(selection("goose"), catalog, false)).toBeNull();
});
it("uses actual overridden protocols for compatible providers", () => {
  const overridden = {
    ...catalog,
    agents: catalog.agents.map((a) => ({ ...a, providers: catalog.providers.map((p) => p.id) })),
    providers: catalog.providers.map((p) => ({ ...p, config: { ...p.config, protocol: "anthropic" as const } })),
  };
  expect(compatibleProviders("codex", overridden)).toEqual([]);
  expect(compatibleProviders("claude", overridden).map((p) => p.id)).toContain("openai");
});
it("respects server exclusions for endpoints without the CLI's wire format", () => {
  const unsupported: AgentCatalog = {
    ...catalog,
    providers: [
      ...catalog.providers,
      {
        id: "alibabacodingplan",
        name: "Alibaba Coding Plan",
        has_key: true,
        config: { endpoint: "https://coding-intl.dashscope.aliyuncs.com/v1", protocol: "openai", requires_key: true },
      },
    ],
  };
  expect(compatibleProviders("codex", unsupported).map((p) => p.id)).not.toContain("alibabacodingplan");
  expect(launchReadiness({ ...selection("codex"), provider: "alibabacodingplan" }, unsupported, false)).toMatch(
    /format/,
  );
});
it("allows credential-free custom commands and explains missing template inputs", () => {
  const agent = {
    ...selection("custom"),
    custom: { name: "My tool", install: null, interactive: "my-tool", headless: null, resume: null },
  };
  expect(launchReadiness(agent, catalog, false)).toBeNull();
  expect(
    launchReadiness({ ...agent, custom: { ...agent.custom, interactive: "my-tool --model {model}" } }, catalog, false),
  ).toBeNull();
  expect(
    launchReadiness({ ...agent, custom: { ...agent.custom, interactive: "my-tool --url {endpoint}" } }, catalog, false),
  ).toMatch(/provider.*endpoint/i);
});
it("keeps legacy tasks on Claude analytics and labels non-Claude tasks accurately", () => {
  expect(agentLabel()).toBe("Claude Code");
  expect(analyticsAvailable()).toBe(true);
  expect(agentLabel(selection("codex"))).toBe("Codex");
  expect(analyticsAvailable(selection("codex"))).toBe(false);
});

it("resolves reusable commands without carrying a persisted model override into launch choices", () => {
  const custom = { name: "Helper", install: null, interactive: "helper", headless: null, resume: null };
  const defaults = launchDefaults({ ...selection("custom"), custom: null }, custom);
  expect(defaults).toMatchObject({ agent: { kind: "custom", custom } });
  expect(defaults).not.toHaveProperty("model");
  expect(launchDefaults(undefined, custom)).not.toHaveProperty("model");
});
it("blocks unsafe releases and invalid endpoints before a VM is created", () => {
  expect(launchReadiness({ ...selection("codex"), version: "1.2.3; echo bad" }, catalog, false)).toMatch(/release/);
  const invalid = {
    ...catalog,
    providers: catalog.providers.map((p) =>
      Object.assign({}, p, {
        config: Object.assign({}, p.config, { endpoint: "https://api.example.com/?key=secret" }),
      }),
    ),
  };
  expect(launchReadiness(selection("codex"), invalid, false)).toMatch(/endpoint/);
});
