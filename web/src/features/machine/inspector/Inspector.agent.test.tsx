import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { task } from "../../../test/task";
import { Inspector } from "./Inspector";

it("shows readable output rather than Claude analytics for a Codex automatic task", () => {
  const t = task({
    interactive: false,
    agent: { kind: "codex", provider: "openai", version: null, custom: null, auth: null, account_kind: null },
  });
  const html = renderToStaticMarkup(
    <QueryClientProvider client={new QueryClient()}>
      <Inspector task={t} tab="claude" onTab={() => {}} onClose={() => {}} />
    </QueryClientProvider>,
  );
  expect(html).toContain(">Output</button>");
  expect(html).toContain("Reading agent output");
  expect(html).not.toContain(">Claude</button>");
});
it("keeps the Claude history panel for a task from an older engine", () => {
  const t = task();
  delete (t as Partial<typeof t>).agent;
  const html = renderToStaticMarkup(
    <QueryClientProvider client={new QueryClient()}>
      <Inspector task={t} tab="telemetry" onTab={() => {}} onClose={() => {}} />
    </QueryClientProvider>,
  );
  expect(html).toContain(">Claude</button>");
  expect(html).not.toContain(">Output</button>");
});
