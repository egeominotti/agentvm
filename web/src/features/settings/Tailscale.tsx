// Tailscale: VMs join the user's tailnet to be reached and managed from it (Tailscale SSH, their
// ports). The auth key is kept in the Keychain and reaches a VM only as it joins.
import type { Settings } from "../../api/generated/Settings";
import { ConfirmButton } from "../../components/ConfirmButton";
import { Icon } from "../../components/Icon";
import { Switch } from "../../components/Switch";
import { KeyForm } from "../tailscale/KeyForm";
import { useTailscaleKey } from "../tailscale/useTailscaleKey";
import type { SetSetting } from "./useDraft";

export function Tailscale({ s, set }: { s: Settings; set: SetSetting }) {
  const key = useTailscaleKey();
  const t = s.tailscale;
  const change = (patch: Partial<typeof t>) => set("tailscale", { ...t, ...patch });
  return (
    <>
      <p className="lede">
        A VM on your tailnet is reachable from your laptop or phone: <code>ssh root@agent-…</code> over Tailscale SSH,
        the services it runs. To set it up, generate an auth key in Tailscale (Settings › Keys, with Reusable on) and
        paste it below, or on any VM's page: it stays in this Mac's Keychain. Then a VM joins with one button, or every
        new VM joins by itself. Each VM is its own machine, <code>agent-&lt;its id&gt;</code>, ephemeral: it leaves the
        tailnet when it ends. Tailscale's connectivity logs are off.
      </p>
      {key.saved ? (
        <ul className="token-list">
          <li>
            <Icon name="key" />
            <b>Auth key</b>
            <span className="hint">saved in the Keychain</span>
            <ConfirmButton confirm="Remove it?" onConfirm={() => key.remove.mutate()}>
              Remove
            </ConfirmButton>
          </li>
        </ul>
      ) : null}
      <KeyForm
        busy={key.save.isPending}
        action={key.saved ? "Replace" : "Save"}
        onSubmit={(k) => key.save.mutate(k)}
        error={key.save.error?.message}
      />
      <Switch
        on={t.enabled}
        onChange={(on) => change({ enabled: on })}
        disabled={!key.saved}
        label="Every new VM joins"
        hint={key.saved ? "Otherwise a VM joins from its page." : "Save an auth key first."}
      />
      <Switch
        on={t.ssh}
        onChange={(on) => change({ ssh: on })}
        label="Tailscale SSH"
        hint="Your tailnet's policy decides who may open a shell in the VM."
      />
      <label className="set">
        <span className="set-label">Tags</span>
        <input
          className="text-input mono"
          placeholder="tag:agentvm"
          value={t.tags.join(", ")}
          onChange={(e) =>
            change({
              tags: e.target.value
                .split(",")
                .map((x) => x.trim())
                .filter(Boolean),
            })
          }
          spellCheck={false}
        />
        <span className="hint">Optional with an auth key; needed with an OAuth client secret.</span>
      </label>
    </>
  );
}
