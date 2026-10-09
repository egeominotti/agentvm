// The Claude subscription token, kept in the macOS Keychain.
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import { keys, useStatus } from "../../api/queries";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";

export function Account() {
  const connected = useStatus().data?.token;
  const [token, setToken] = useState("");
  const qc = useQueryClient();
  const save = useMutation({
    mutationFn: () => api("/api/settings/token", "PUT", { token: token.trim() }),
    onSuccess: () => {
      setToken("");
      qc.invalidateQueries({ queryKey: keys.status });
    },
  });
  return (
    <>
      <p className="status-line">
        <span className={`pill ${connected ? "ok" : "err"}`}>{connected ? "Connected" : "Missing"}</span>
        <span className="lede">A long-lived token of your Claude subscription, kept in the macOS Keychain.</span>
      </p>
      <form
        className="inline-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (token.trim()) save.mutate();
        }}
      >
        <input
          className="text-input mono"
          type="password"
          aria-label="Claude token"
          placeholder="Paste a token: sk-ant-oat01-…"
          autoComplete="off"
          spellCheck={false}
          value={token}
          onChange={(e) => setToken(e.target.value)}
        />
        <Button type="submit" disabled={!token.trim() || save.isPending}>
          <Icon name="key" />
          {save.isPending ? "Saving…" : "Save token"}
        </Button>
      </form>
      {save.isSuccess ? <p className="msg ok">Saved in the Keychain. New VMs use it.</p> : null}
      {save.error ? <p className="msg err">{save.error.message}</p> : null}
      <p className="hint">
        Create one in a terminal with <code>claude setup-token</code>. Running VMs keep the token they started with.
      </p>
    </>
  );
}
