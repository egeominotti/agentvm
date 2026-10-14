import { useAgentCatalog } from "../../api/queries";
import { Button } from "../../components/Button";
import { AccountCard } from "./AccountCard";
import { Page } from "./kit";

export function Accounts() {
  const q = useAgentCatalog();
  return (
    <Page
      title="Accounts"
      description="Connect the accounts and supported subscription or credit plans your coding CLIs use. API keys and provider billing plans are managed in Providers."
    >
      <div className="tab-actions">
        <Button size="sm" onClick={() => q.refetch()}>
          Refresh accounts
        </Button>
      </div>
      {q.error ? (
        <p className="msg err">Could not read accounts: {q.error.message}</p>
      ) : q.isPending ? (
        <p className="hint">Reading accounts…</p>
      ) : q.data?.accounts ? (
        q.data.accounts.map((a) => <AccountCard key={a.id} account={a} />)
      ) : (
        <p className="hint">
          This engine supports the <a href="#/settings/account">Claude account token</a>. Update agentvm to connect
          additional accounts.
        </p>
      )}
    </Page>
  );
}
