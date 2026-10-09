// Pasting a Tailscale auth key: one field and one button, here or on a VM's page.
import { useState } from "react";
import { Button } from "../../components/Button";
import { NEW_KEY_URL } from "./useTailscaleKey";

type Props = { busy: boolean; action: string; onSubmit: (key: string) => void; error?: string | null };

export function KeyForm({ busy, action, onSubmit, error }: Props) {
  const [key, setKey] = useState("");
  return (
    <form
      className="inline-form ts-key"
      onSubmit={(e) => {
        e.preventDefault();
        if (key.trim()) onSubmit(key);
      }}
    >
      <input
        className="text-input mono"
        type="password"
        aria-label="Tailscale auth key"
        placeholder="Paste an auth key: tskey-auth-…"
        value={key}
        onChange={(e) => setKey(e.target.value)}
        autoComplete="off"
        spellCheck={false}
      />
      <Button type="submit" variant="primary" disabled={busy || !key.trim()}>
        {action}
      </Button>
      <a
        className="hint"
        href={NEW_KEY_URL}
        target="_blank"
        rel="noopener"
        title="In Tailscale: Settings › Keys › Generate auth key, with Reusable on (and Pre-approved if your tailnet approves devices)"
      >
        Get a key: Generate auth key, Reusable on
      </a>
      {error ? <span className="err-text">{error}</span> : null}
    </form>
  );
}
