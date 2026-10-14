import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { AccountStatus } from "../../api/generated/AccountStatus";
import { keys } from "../../api/queries";
import { Button } from "../../components/Button";
import { ConfirmButton } from "../../components/ConfirmButton";
import { Badge, Panel, Row, Value } from "./kit";
import { useAccountLogin } from "./useAccountLogin";

type Props = { account: AccountStatus };
export function AccountCard({ account: a }: Props) {
  const qc = useQueryClient();
  const refresh = async () => {
    await qc.invalidateQueries({ queryKey: keys.agents });
    await qc.invalidateQueries({ queryKey: keys.status });
  };
  const login = useAccountLogin(a.id, refresh);
  const imported = useMutation({
    mutationFn: () => api<AccountStatus>(`/api/settings/accounts/${a.id}/import`, "POST"),
    onSuccess: async () => {
      login.reset();
      await refresh();
    },
  });
  const remove = useMutation({
    mutationFn: () => api(`/api/settings/accounts/${a.id}`, "DELETE"),
    onSuccess: async () => {
      login.reset();
      await refresh();
    },
  });
  const busy = imported.isPending || remove.isPending || login.start.isPending || login.pending;
  const error = imported.error ?? remove.error ?? login.start.error;
  return (
    <Panel
      title={a.name}
      description="Use a supported account or plan by signing in on this Mac or importing an existing CLI login."
    >
      <Row label="Status" description={a.error ?? undefined}>
        <Badge tone={a.connected ? "ok" : "warn"}>{a.connected ? "Connected" : "Not connected"}</Badge>
      </Row>
      {a.source ? (
        <Row label="Connected through">
          <Value>{a.source}</Value>
        </Row>
      ) : null}
      <Row label="Account access" description="Account credentials remain on this Mac.">
        <div className="inline-form">
          {a.can_import ? (
            <Button size="sm" disabled={busy} onClick={() => imported.mutate()}>
              {imported.isPending ? "Importing…" : "Import local login"}
            </Button>
          ) : null}
          {a.can_login ? (
            <Button size="sm" variant="primary" disabled={busy} onClick={() => login.start.mutate()}>
              {login.start.isPending ? "Starting…" : "Sign in"}
            </Button>
          ) : null}
          {a.connected ? (
            <ConfirmButton confirm="Remove account access?" disabled={busy} onConfirm={() => remove.mutate()}>
              Remove
            </ConfirmButton>
          ) : null}
        </div>
      </Row>
      {a.id === "claude" ? (
        <Row label="Claude subscription token">
          <a href="#/settings/account">Manage Claude token</a>
        </Row>
      ) : !a.can_login ? (
        <Row
          label="Local sign-in"
          description="Sign in with this account's CLI on the Mac, then choose Import local login."
        />
      ) : null}
      {login.login && login.pending ? (
        <Row label="Complete sign-in" wide>
          <p className="hint">
            <a href={login.login.verification_url} target="_blank" rel="noopener noreferrer">
              Open sign-in page
            </a>
            {login.login.user_code ? (
              <>
                {" "}
                and enter <code>{login.login.user_code}</code>
              </>
            ) : null}
            .
          </p>
          <p className="hint" role="status">
            Waiting for sign-in… This page checks automatically until the sign-in expires.
          </p>
        </Row>
      ) : null}
      {login.result?.status === "connected" ? (
        <Row label="Sign-in complete">
          <Badge tone="ok">Connected</Badge>
        </Row>
      ) : null}
      {login.expired ? (
        <p className="msg err" role="alert">
          Sign-in expired. Choose Sign in to start again.
        </p>
      ) : null}
      {login.result?.status === "failed" ? (
        <p className="msg err" role="alert">
          {login.result.error ?? "Sign-in failed. Try again."}
        </p>
      ) : null}
      {login.pollError ? <p className="msg err">Could not check sign-in: {login.pollError}</p> : null}
      {error ? (
        <p className="msg err" role="alert">
          {error.message}
        </p>
      ) : null}
      {imported.isSuccess ? (
        <p className="hint" role="status">
          Local login imported
        </p>
      ) : null}
    </Panel>
  );
}
