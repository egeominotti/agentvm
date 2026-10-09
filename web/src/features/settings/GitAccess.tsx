// Tokens for private repositories, one per git host, kept in the macOS Keychain. agentvm uses
// them on this Mac only (clone, fetch, push): they never enter a VM, and never come back out.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import { Button } from "../../components/Button";
import { ConfirmButton } from "../../components/ConfirmButton";
import { Icon } from "../../components/Icon";
import { Badge, Page, Panel } from "./kit";

const HOSTS = ["github.com", "gitlab.com", "bitbucket.org"];
const HOW: Record<string, { url: string; text: string }> = {
  "github.com": {
    url: "https://github.com/settings/personal-access-tokens/new",
    text: "A fine-grained token: the repositories you want, Contents read and write.",
  },
  "gitlab.com": {
    url: "https://gitlab.com/-/user_settings/personal_access_tokens",
    text: "A personal access token with read_repository and write_repository.",
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
    <Page
      title="Git access"
      description={
        <>
          Public repositories need nothing. Private ones open with this Mac's own access (<code>gh auth login</code>, an
          ssh key) or with a token saved here. Tokens are used on this Mac only: they never enter a VM.
        </>
      }
    >
      <Panel title="Saved tokens">
        <ul className="set-list">
          {hosts.length ? (
            hosts.map((h) => (
              <li key={h}>
                <Icon name="key" />
                <b>{h}</b>
                <Badge tone="ok">Token saved</Badge>
                <span className="set-value" />
                <ConfirmButton confirm="Remove it?" onConfirm={() => remove.mutate(h)}>
                  Remove
                </ConfirmButton>
              </li>
            ))
          ) : (
            <li className="msg">No tokens yet.</li>
          )}
        </ul>
      </Panel>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (host.trim() && token.trim()) save.mutate();
        }}
      >
        <Panel
          title="Add a token"
          description={
            how ? (
              <>
                {how.text}{" "}
                <a href={how.url} target="_blank" rel="noopener">
                  Create one
                </a>
              </>
            ) : (
              "A token with read and write access to the repositories you want."
            )
          }
          footer={
            <>
              {save.error ? <span className="msg err">{save.error.message}</span> : null}
              {save.isSuccess ? <span className="msg ok">Saved in the Keychain.</span> : null}
              <Button type="submit" variant="primary" disabled={save.isPending || !host.trim() || !token.trim()}>
                Save token
              </Button>
            </>
          }
        >
          <div className="set-fields">
            <label>
              <span>Host</span>
              <input
                className="text-input mono"
                list="git-hosts"
                value={host}
                onChange={(e) => setHost(e.target.value.toLowerCase())}
                spellCheck={false}
              />
              <datalist id="git-hosts">
                {HOSTS.map((h) => (
                  // oxlint-disable-next-line jsx-a11y/control-has-associated-label -- a suggestion of a datalist: its value is its text
                  <option key={h} value={h} />
                ))}
              </datalist>
            </label>
            <label>
              <span>Token</span>
              <input
                className="text-input mono"
                type="password"
                placeholder="github_pat_…"
                autoComplete="off"
                spellCheck={false}
                value={token}
                onChange={(e) => setToken(e.target.value)}
              />
            </label>
          </div>
        </Panel>
      </form>
    </Page>
  );
}
