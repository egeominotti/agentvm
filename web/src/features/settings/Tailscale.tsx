// Tailscale: VMs join the user's tailnet to be reached and managed from it (Tailscale SSH, their
// ports). The auth key is kept in the Keychain and reaches a VM only as it joins.
import type { Settings } from "../../api/generated/Settings";
import { NEW_KEY_URL, useTailscaleKey } from "../tailscale/useTailscaleKey";
import { Credential } from "./Credential";
import { Badge, Page, Panel, Row, Toggle } from "./kit";
import type { SetSetting } from "./useDraft";

export function Tailscale({ s, set }: { s: Settings; set: SetSetting }) {
  const key = useTailscaleKey();
  const t = s.tailscale;
  const change = (patch: Partial<typeof t>) => set("tailscale", { ...t, ...patch });
  return (
    <Page
      title="Tailscale"
      description={
        <>
          Reach your VMs from your laptop or phone: <code>ssh root@agent-…</code> and the services they run. Each VM
          joins as <code>agent-&lt;its id&gt;</code>, an ephemeral machine that leaves the tailnet when the VM ends.
          Tailscale's connectivity logs are off.
        </>
      }
    >
      <Panel
        title="Auth key"
        description={
          <>
            In Tailscale,{" "}
            <a href={NEW_KEY_URL} target="_blank" rel="noopener">
              Settings › Keys › Generate auth key
            </a>{" "}
            with Reusable on (and Pre-approved if your tailnet approves new devices). One key for every VM.
          </>
        }
      >
        <Row
          label="Status"
          description={key.saved ? "VMs can join with one click on their page." : "Paste a key to start."}
        >
          {key.loaded ? <Badge tone={key.saved ? "ok" : "off"}>{key.saved ? "Ready" : "Not set up"}</Badge> : null}
        </Row>
        <Row label="Key" description="Kept in this Mac's Keychain.">
          <Credential
            saved={key.saved}
            label="Tailscale auth key"
            placeholder="tskey-auth-…"
            busy={key.save.isPending}
            onSave={(v) => key.save.mutate(v)}
            onRemove={() => key.remove.mutate()}
          />
        </Row>
      </Panel>
      {key.save.error ? <p className="msg err">{key.save.error.message}</p> : null}
      <Panel title="New VMs">
        <Row
          label="Join the tailnet"
          description={
            key.saved ? "Every new VM joins by itself. Otherwise, join from a VM's page." : "Save an auth key first."
          }
        >
          <Toggle
            label="Every new VM joins"
            on={t.enabled}
            disabled={!key.saved}
            onChange={(on) => change({ enabled: on })}
          />
        </Row>
        <Row label="Tailscale SSH" description="Your tailnet's policy decides who may open a shell in a VM.">
          <Toggle label="Tailscale SSH" on={t.ssh} onChange={(on) => change({ ssh: on })} />
        </Row>
        <Row
          label="ACL tags"
          htmlFor="set-tags"
          description="Optional with an auth key; needed with an OAuth client secret."
        >
          <input
            id="set-tags"
            className="text-input mono"
            placeholder="tag:agentvm"
            spellCheck={false}
            value={t.tags.join(", ")}
            onChange={(e) =>
              change({
                tags: e.target.value
                  .split(",")
                  .map((x) => x.trim())
                  .filter(Boolean),
              })
            }
          />
        </Row>
      </Panel>
    </Page>
  );
}
