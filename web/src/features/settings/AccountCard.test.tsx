import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { AccountStatus } from "../../api/generated/AccountStatus";
import { AccountCard } from "./AccountCard";

function markup(account: AccountStatus) {
  return renderToStaticMarkup(
    <QueryClientProvider client={new QueryClient()}>
      <AccountCard account={account} />
    </QueryClientProvider>,
  );
}
it("offers local import and host sign-in according to account capabilities", () => {
  const html = markup({
    id: "chatgpt",
    name: "ChatGPT account",
    connected: false,
    source: null,
    error: "Sign in on this Mac",
    can_import: true,
    can_login: true,
  });
  expect(html).toContain("Import local login");
  expect(html).toContain("Sign in");
  expect(html).toContain("Sign in on this Mac");
  expect(html).not.toContain("API key");
});
it("keeps Claude token management available when browser login is unsupported", () => {
  const html = markup({
    id: "claude",
    name: "Claude account",
    connected: true,
    source: "Keychain",
    error: null,
    can_import: true,
    can_login: false,
  });
  expect(html).toContain("Connected");
  expect(html).toContain("Remove");
  expect(html).toContain('href="#/settings/account"');
  expect(html).not.toContain(">Sign in</button>");
});
