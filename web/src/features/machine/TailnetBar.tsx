// A running VM on the user's tailnet, from its page: join with one button (pasting a key the first
// time), see its name and addresses, copy how to reach it, leave.
import { useMutation } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { api } from "../../api/client";
import type { TaskDto } from "../../api/generated/TaskDto";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { useToast } from "../../components/Toast";
import { KeyForm } from "../tailscale/KeyForm";
import { useTailscaleKey } from "../tailscale/useTailscaleKey";

export function TailnetBar({ task }: { task: TaskDto }) {
  const say = useToast();
  const key = useTailscaleKey();
  const url = `/api/tasks/${task.id}/tailscale`;
  const join = useMutation({ mutationFn: () => api(url, "POST") });
  const leave = useMutation({ mutationFn: () => api(url, "DELETE") });
  const saveAndJoin = useMutation({
    mutationFn: async (k: string) => {
      await key.save.mutateAsync(k);
      await api(url, "POST");
    },
  });
  const copy = (text: string, what: string) => navigator.clipboard.writeText(text).then(() => say(`Copied ${what}`));
  const net = task.tailnet;
  const busy = join.isPending || leave.isPending || saveAndJoin.isPending;
  const failed = (join.error ?? leave.error ?? saveAndJoin.error)?.message;

  let body: ReactNode;
  if (!task.tailscale) {
    body = key.saved ? (
      <>
        <span className="hint" title="It joins as agent-<its id>; it leaves the tailnet when it ends">
          Reach this VM from your other devices: SSH, the services it runs.
        </span>
        <Button variant="primary" size="sm" disabled={busy} onClick={() => join.mutate()}>
          Join tailnet
        </Button>
      </>
    ) : key.loaded ? (
      <>
        <span className="hint" title="The key is saved in this Mac's Keychain, once, for every VM">
          Reach this VM from your laptop or phone: paste a Tailscale auth key once.
        </span>
        <KeyForm busy={busy} action="Save and join" onSubmit={(k) => saveAndJoin.mutate(k)} />
      </>
    ) : null;
  } else if (net?.name) {
    const name = net.name;
    const host = name.split(".")[0] ?? name;
    const ip = net.ips[0];
    body = (
      <>
        <span className="live" />
        <button
          type="button"
          className="port-link"
          title="Its name on your tailnet (click to copy)"
          onClick={() => copy(name, name)}
        >
          <span className="addr">{name}</span>
        </button>
        {ip ? (
          <button
            type="button"
            className="port-link"
            title="Its tailnet address (click to copy)"
            onClick={() => copy(ip, ip)}
          >
            <span className="addr">{ip}</span>
          </button>
        ) : null}
        <button
          type="button"
          className="port-link"
          title={`ssh root@${host} (click to copy): from any device on your tailnet`}
          onClick={() => copy(`ssh root@${host}`, "the SSH command")}
        >
          <Icon name="terminal" />
          <b>ssh</b>
        </button>
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => leave.mutate()}>
          Leave
        </Button>
      </>
    );
  } else if (net?.error) {
    body = (
      <>
        <span className="err-text" title={net.error}>
          Could not join: {net.error}
        </span>
        <Button size="sm" disabled={busy} onClick={() => join.mutate()}>
          Try again
        </Button>
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => leave.mutate()}>
          Leave
        </Button>
      </>
    );
  } else {
    body = (
      <>
        <span className="hint">Joining your tailnet… (a few seconds)</span>
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => leave.mutate()}>
          Leave
        </Button>
      </>
    );
  }
  return (
    <section className="tailnet-bar" aria-label="Tailscale">
      <span className="ports-label">
        <Icon name="tailnet" />
        Tailscale
      </span>
      {body}
      {failed ? <span className="err-text">{failed}</span> : null}
    </section>
  );
}
