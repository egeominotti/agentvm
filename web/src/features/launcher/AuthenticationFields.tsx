import type { AccountKind } from "../../api/generated/AccountKind";
import type { AgentAuth } from "../../api/generated/AgentAuth";
import type { AgentCatalog } from "../../api/generated/AgentCatalog";
import type { AgentSelection } from "../../api/generated/AgentSelection";
import type { Provider } from "../../api/generated/Provider";
import { accountLabel, authentication, changeAuthentication, selectedAccount } from "../../lib/accounts";
import { compatibleProviders, defaultProvider } from "../../lib/agents";

type Props = { agent: AgentSelection; catalog: AgentCatalog; onChange: (a: AgentSelection) => void };
export function AuthenticationFields({ agent: a, catalog, onChange }: Props) {
  const eligible = catalog.agents.find((d) => d.kind === a.kind)?.accounts ?? [];
  const modern = !!catalog.accounts;
  const auth = authentication(a);
  const account = selectedAccount(a);
  const status = catalog.accounts?.find((s) => s.id === account);
  const providers = compatibleProviders(a.kind, catalog);
  const provider = a.provider ?? (modern && a.kind === "claude" ? "anthropic" : defaultProvider(a.kind));
  return (
    <>
      {modern ? (
        <label>
          <span>Authentication</span>
          <select
            className="text-input"
            value={auth}
            onChange={(e) => onChange(changeAuthentication(a, e.target.value as AgentAuth, eligible))}
          >
            {eligible.length || auth === "account" ? <option value="account">Account / subscription</option> : null}
            <option value="api">API key / provider plan</option>
          </select>
        </label>
      ) : null}
      {modern && auth === "account" ? (
        <>
          <label>
            <span>Account / plan</span>
            <select
              className="text-input"
              value={account ?? ""}
              onChange={(e) =>
                onChange({ ...a, auth: "account", account_kind: e.target.value as AccountKind, provider: null })
              }
            >
              {account && !eligible.includes(account) ? (
                <option value={account}>Unsupported: {accountLabel(account)}</option>
              ) : null}
              {eligible.map((id) => (
                <option key={id} value={id}>
                  {catalog.accounts.find((s) => s.id === id)?.name ?? accountLabel(id)}
                </option>
              ))}
            </select>
          </label>
          <p className="res-hint">
            {status?.connected ? (
              `Connected${status.source ? ` · ${status.source}` : ""}`
            ) : (
              <a href="#/settings/accounts">
                Connect {account ? accountLabel(account) : "an account"} in Settings › Accounts
              </a>
            )}
          </p>
        </>
      ) : (
        <label>
          <span>{modern ? "API provider" : "Provider"}</span>
          <select
            className="text-input"
            value={provider ?? ""}
            onChange={(e) =>
              onChange({
                ...a,
                ...(modern ? { auth: "api" as const, account_kind: null } : {}),
                provider: e.target.value ? (e.target.value as Provider) : null,
              })
            }
          >
            {!modern && a.kind === "claude" ? (
              <option value="">Claude subscription</option>
            ) : a.kind === "custom" ? (
              <option value="">None (command only)</option>
            ) : null}
            {provider && !providers.some((p) => p.id === provider) ? (
              <option value={provider}>Incompatible: {provider}</option>
            ) : null}
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
      )}
    </>
  );
}
