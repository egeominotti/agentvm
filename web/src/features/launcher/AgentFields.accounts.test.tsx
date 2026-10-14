import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { AgentCatalog } from "../../api/generated/AgentCatalog";
import { keys } from "../../api/queries";
import { selection } from "../../lib/agents";
import { AgentFields } from "./AgentFields";

const catalog: AgentCatalog = {
  claude_token: false,
  agents: [
    {
      kind: "codex",
      name: "Codex",
      providers: ["openai"],
      accounts: ["chatgpt"],
      headless: true,
      resume: true,
      requires_model: false,
    },
  ],
  providers: [
    {
      id: "openai",
      name: "OpenAI",
      has_key: false,
      config: { endpoint: "https://api.openai.com/v1", protocol: "openai", requires_key: true },
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
  ],
};
function markup(auth: "account" | "api") {
  const client = new QueryClient();
  client.setQueryData(keys.agents, catalog);
  const agent = {
    ...selection("codex"),
    auth,
    account_kind: auth === "account" ? ("chatgpt" as const) : null,
    provider: auth === "account" ? null : ("openai" as const),
  };
  return renderToStaticMarkup(
    <QueryClientProvider client={client}>
      <AgentFields agent={agent} onChange={() => {}} />
    </QueryClientProvider>,
  );
}
it("offers account access and hides API providers in account mode", () => {
  const html = markup("account");
  expect(html).toContain("Authentication");
  expect(html).toContain("ChatGPT account");
  expect(html).toContain("Connected");
  expect(html).not.toContain("API provider</span>");
  expect(html).not.toContain("Model ID");
});
it("shows API providers separately from account status in API mode", () => {
  const html = markup("api");
  expect(html).toContain("Authentication");
  expect(html).toContain("API provider</span>");
  expect(html).not.toContain("ChatGPT account");
  expect(html).not.toContain("Connected");
});
