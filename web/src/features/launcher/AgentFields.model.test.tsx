import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { AgentCatalog } from "../../api/generated/AgentCatalog";
import { keys } from "../../api/queries";
import { selection } from "../../lib/agents";
import { AgentFields } from "./AgentFields";

it("offers CLI and provider choices without a model input even when the CLI needs a model", () => {
  const catalog: AgentCatalog = {
    claude_token: false,
    accounts: [],
    agents: [
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
    ],
  };
  const client = new QueryClient();
  client.setQueryData(keys.agents, catalog);
  const legacyChoice = { agent: selection("goose"), model: "sonnet" };
  const html = renderToStaticMarkup(
    <QueryClientProvider client={client}>
      <AgentFields {...legacyChoice} onChange={() => {}} />
    </QueryClientProvider>,
  );
  expect(html).toContain("Coding CLI");
  expect(html).toContain("API provider");
  expect(html).not.toContain("Model ID");
  expect(html).not.toContain("sonnet");
});
