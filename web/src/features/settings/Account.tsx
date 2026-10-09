// The Claude subscription token every VM runs Claude Code with, kept in the macOS Keychain.
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import { keys, useStatus } from "../../api/queries";
import { Credential } from "./Credential";
import { Badge, Page, Panel, Row } from "./kit";

export function Account() {
  const status = useStatus().data;
  const qc = useQueryClient();
  const save = useMutation({
    mutationFn: (token: string) => api("/api/settings/token", "PUT", { token }),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.status }),
  });
  return (
    <Page
      title="Claude account"
      description={
        <>
          A long-lived token of your Claude subscription. Create one in a terminal with <code>claude setup-token</code>;
          it is kept in this Mac's Keychain.
        </>
      }
    >
      <Panel>
        <Row label="Status" description={status?.token ? "New VMs run Claude Code with it." : status?.token_hint}>
          {status ? <Badge tone={status.token ? "ok" : "err"}>{status.token ? "Connected" : "Missing"}</Badge> : null}
        </Row>
        <Row label="Token" description="Running VMs keep the token they started with.">
          <Credential
            saved={!!status?.token}
            label="Claude token"
            placeholder="sk-ant-oat01-…"
            busy={save.isPending}
            onSave={(v) => save.mutate(v)}
          />
        </Row>
      </Panel>
      {save.error ? <p className="msg err">{save.error.message}</p> : null}
    </Page>
  );
}
