// Tokens for private repositories, one per git host, kept in the macOS Keychain. agentvm uses
// them on this Mac only (clone, fetch, push): they never enter a VM, and never come back out.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import { Button } from "../../components/Button";
import { ConfirmButton } from "../../components/ConfirmButton";
import { Icon } from "../../components/Icon";

const HOSTS = ["github.com", "gitlab.com", "bitbucket.org"];
const HOW: Record<string, { url: string; text: string }> = {
  "github.com": {
    url: "https://github.com/settings/personal-access-tokens/new",
    text: "a fine-grained token: the repositories you want, permission Contents read and write",
  },
  "gitlab.com": {
    url: "https://gitlab.com/-/user_settings/personal_access_tokens",
    text: "a personal access token with read_repository and write_repository",
  },
};

export function GitAccess() {
  const qc = useQueryClient();
  const saved = useQuery({ queryKey: ["git-access"], queryFn: () => api<{ hosts: string[] }>("/api/settings/git") });
  const [host, setHost] = useState("github.com");
  const [token, setToken] = useState("");
  const refresh = () => qc.invalidateQueries({ queryKey: ["git-access"] });
  const save = useMutation({
    mutationFn: () => api(`/api/settings/git/${encodeURIComponent(host.trim())}`, "PUT", { token: token.trim() }),
    onSuccess: () => {
      setToken("");
      refresh();
    },
  });
  const remove = useMutation({
    mutationFn: (h: string) => api(`/api/settings/git/${encodeURIComponent(h)}`, "DELETE"),
    onSuccess: refresh,
  });
  const how = HOW[host.trim()];
  const hosts = saved.data?.hosts ?? [];
  return (
    <>
      <p className="lede">
        Public repositories need nothing. For private ones agentvm uses this Mac's own access (
        <code>gh auth login</code>, an ssh key) or a token saved here, in the Keychain: used on this Mac only, it never
        enters a VM.
      </p>
      {hosts.length ? (
        <ul className="token-list">
          {hosts.map((h) => (
            <li key={h}>
              <Icon name="key" />
              <b>{h}</b>
              <span className="hint">token saved</span>
              <ConfirmButton confirm="Remove it?" onConfirm={() => remove.mutate(h)}>
                Remove
              </ConfirmButton>
            </li>
          ))}
        </ul>
      ) : null}
      <form
        className="inline-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (host.trim() && token.trim()) save.mutate();
        }}
      >
        <input
          className="text-input mono host-input"
          aria-label="Git host"
          list="git-hosts"
          value={host}
          onChange={(e) => setHost(e.target.value.toLowerCase())}
          spellCheck={false}
        />
        <datalist id="git-hosts">
          {HOSTS.map((h) => (
            <option key={h} value={h} />
          ))}
        </datalist>
        <input
          className="text-input mono"
          type="password"
          aria-label="Token"
          placeholder="Paste a token: github_pat_…"
          autoComplete="off"
          spellCheck={false}
          value={token}
          onChange={(e) => setToken(e.target.value)}
        />
        <Button type="submit" disabled={!host.trim() || !token.trim() || save.isPending}>
          <Icon name="key" />
          {save.isPending ? "Saving…" : "Save token"}
        </Button>
      </form>
      {save.isSuccess ? (
        <p className="msg ok">Saved in the Keychain. Private repositories on {host} open now.</p>
      ) : null}
      {save.error ? <p className="msg err">{save.error.message}</p> : null}
      {remove.error ? <p className="msg err">{remove.error.message}</p> : null}
      {how ? (
        <p className="hint">
          Create{" "}
          <a href={how.url} target="_blank" rel="noopener">
            {how.text}
          </a>
          .
        </p>
      ) : null}
    </>
  );
}
