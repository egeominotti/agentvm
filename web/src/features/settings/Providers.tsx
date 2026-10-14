import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import type { ApiProtocol } from "../../api/generated/ApiProtocol";
import type { ProviderDescriptor } from "../../api/generated/ProviderDescriptor";
import { keys, useAgentCatalog } from "../../api/queries";
import { Button } from "../../components/Button";
import { Credential } from "./Credential";
import { Badge, Page, Panel, Row, Toggle } from "./kit";

export function Providers() {
  const q = useAgentCatalog();
  return (
    <Page
      title="Providers"
      description="Inference endpoints and API keys for coding CLIs, including provider billing plans such as Alibaba Coding Plan. Keys stay in the macOS Keychain; running VMs keep the settings they started with."
    >
      {q.error ? (
        <p className="msg err">{q.error.message}</p>
      ) : q.isPending ? (
        <p className="hint">Reading providers…</p>
      ) : q.data ? (
        q.data.providers.map((p) => <ProviderForm key={`${p.id}-${JSON.stringify(p.config)}`} provider={p} />)
      ) : (
        <p className="hint">
          This engine supports the Claude subscription account. Update agentvm to configure other coding CLIs.
        </p>
      )}
    </Page>
  );
}
function ProviderForm({ provider: p }: { provider: ProviderDescriptor }) {
  const [config, setConfig] = useState(p.config);
  const qc = useQueryClient();
  const refresh = async () => {
    await qc.invalidateQueries({ queryKey: keys.agents });
    await qc.invalidateQueries({ queryKey: keys.settings });
  };
  const save = useMutation({
    mutationFn: (token?: string) => api(`/api/settings/providers/${p.id}`, "PUT", { config, token: token ?? null }),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: () => api(`/api/settings/providers/${p.id}`, "DELETE"),
    onSuccess: refresh,
  });
  const busy = save.isPending || remove.isPending;
  return (
    <Panel
      title={p.name}
      footer={
        <Button size="sm" disabled={busy} onClick={() => save.mutate(undefined)}>
          Save endpoint
        </Button>
      }
    >
      <Row
        label="Endpoint"
        htmlFor={`provider-${p.id}`}
        description="Absolute HTTP(S) URL, without credentials or query parameters. Local endpoints are reached through this Mac."
      >
        <input
          id={`provider-${p.id}`}
          className="text-input mono"
          spellCheck={false}
          value={config.endpoint}
          onChange={(e) => setConfig({ ...config, endpoint: e.target.value })}
        />
      </Row>
      <Row label="API protocol" htmlFor={`protocol-${p.id}`}>
        <select
          id={`protocol-${p.id}`}
          className="text-input"
          value={config.protocol}
          onChange={(e) => setConfig({ ...config, protocol: e.target.value as ApiProtocol })}
        >
          {["openai", "anthropic", "google"].map((protocol) => (
            <option key={protocol} value={protocol}>
              {protocol}
            </option>
          ))}
        </select>
      </Row>
      <Row label="Requires API key">
        <Toggle
          label={`${p.name} requires API key`}
          on={config.requires_key}
          onChange={(requires_key) => setConfig({ ...config, requires_key })}
        />
      </Row>
      <Row
        label="Credential"
        description={
          config.requires_key
            ? "Save a key before launching with this provider."
            : "This endpoint can run without a key."
        }
      >
        <Badge tone={p.has_key ? "ok" : config.requires_key ? "warn" : "off"}>
          {p.has_key ? "Key saved" : config.requires_key ? "Key missing" : "No key required"}
        </Badge>
        <Credential
          saved={p.has_key}
          label={`${p.name} API key`}
          placeholder="API key"
          busy={busy}
          onSave={(token) => save.mutate(token)}
          onRemove={() => remove.mutate()}
        />
      </Row>
      {save.error || remove.error ? <p className="msg err">{(save.error ?? remove.error)?.message}</p> : null}
      {save.isSuccess ? (
        <p className="hint" role="status">
          Saved
        </p>
      ) : null}
    </Panel>
  );
}
