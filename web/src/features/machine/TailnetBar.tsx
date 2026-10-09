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
        <span className="hint">Reach and manage this VM from your tailnet (SSH, its ports).</span>
        <Button variant="primary" size="sm" disabled={busy} onClick={() => join.mutate()}>
          Join tailnet
        </Button>
      </>
    ) : key.loaded ? (
      <KeyForm busy={busy} action="Save and join" onSubmit={(k) => saveAndJoin.mutate(k)} />
    ) : null;
  } else if (net?.name) {
    const host = net.name.split(".")[0] ?? net.name;
    const ip = net.ips[0];
    body = (
      <>
        <span className="live" />
        <button
          type="button"
          className="port-link"
          title="Copy its tailnet name"
          onClick={() => copy(net.name ?? "", net.name ?? "")}
        >
          <b>{host}</b>
          <span className="addr">{net.name}</span>
        </button>
        {ip ? (
          <button type="button" className="port-link" title="Copy its address" onClick={() => copy(ip, ip)}>
            <span className="addr">{ip}</span>
          </button>
        ) : null}
        <button
          type="button"
          className="port-link"
          title="Copy the SSH command"
          onClick={() => copy(`ssh root@${host}`, "the SSH command")}
        >
          <Icon name="terminal" />
          <span className="addr">ssh root@{host}</span>
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
        <span className="hint">Joining your tailnet…</span>
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
