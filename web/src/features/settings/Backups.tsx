// Backups of snapshots to any S3-compatible bucket, tested before they are saved.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { S3Config } from "../../api/generated/S3Config";
import { Button } from "../../components/Button";
import { Badge, Page, Panel, Row, Toggle } from "./kit";
import { PRESETS } from "./presets";

type Stored = { config: S3Config | null; secret_saved: boolean };
const EMPTY: S3Config = { endpoint: "", region: "", bucket: "", prefix: "agentvm", access_key: "", path_style: false };

/** `c` addressed the way the provider `key` wants it. */
function withPreset(key: string, c: S3Config): S3Config {
  const p = PRESETS[key];
  if (!p) return c;
  const local = key === "local" ? { bucket: c.bucket || "agentvm-backups", access_key: c.access_key || "agentvm" } : {};
  return { ...c, endpoint: p.endpoint, region: p.region, path_style: p.pathStyle, ...local };
}

export function Backups() {
  const qc = useQueryClient();
  const stored = useQuery({ queryKey: ["s3"], queryFn: () => api<Stored>("/api/settings/s3") });
  const [c, setC] = useState<S3Config>(EMPTY);
  const [preset, setPreset] = useState("");
  const [secret, setSecret] = useState("");
  // What is saved, or a local bucket to start from.
  useEffect(() => {
    const config = stored.data?.config;
    if (config) {
      setC(config);
      setPreset(Object.entries(PRESETS).find(([, p]) => p.endpoint === config.endpoint)?.[0] ?? "");
    } else if (stored.data) {
      setPreset("local");
      setC((now) => withPreset("local", now));
    }
  }, [stored.data]);
  const save = useMutation({
    mutationFn: () => api("/api/settings/s3", "PUT", { config: c, secret: secret || null }),
    onSuccess: () => {
      setSecret("");
      qc.invalidateQueries({ queryKey: ["s3"] });
      qc.invalidateQueries({ queryKey: ["backups"] });
    },
  });
  const field = (k: keyof Omit<S3Config, "path_style">, label: string, placeholder = "") => (
    <label>
      <span>{label}</span>
      <input
        className="text-input mono"
        spellCheck={false}
        autoComplete="off"
        placeholder={placeholder}
        value={c[k]}
        onChange={(e) => setC({ ...c, [k]: e.target.value })}
      />
    </label>
  );
  const connected = !!stored.data?.config;
  return (
    <Page
      title="Backups"
      description="Back up snapshots to any S3-compatible storage, and bring them back here or on another Mac. They travel as the compressed chunks they are stored as."
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <Panel
          title="S3 storage"
          description="Saved only once the bucket accepts an upload."
          footer={
            <>
              {save.isSuccess ? <span className="msg ok">Connected: the bucket accepts uploads.</span> : null}
              {save.error ? <span className="msg err">{save.error.message}</span> : null}
              <Button type="submit" variant="primary" disabled={save.isPending}>
                {save.isPending ? "Testing the connection…" : "Test and save"}
              </Button>
            </>
          }
        >
          <Row label="Status">
            {stored.isSuccess ? (
              <Badge tone={connected ? "ok" : "off"}>{connected ? "Connected" : "Not set up"}</Badge>
            ) : null}
          </Row>
          <Row
            label="Provider"
            htmlFor="set-provider"
            description={preset ? PRESETS[preset]?.hint : "Fills in the endpoint and region."}
          >
            <select
              id="set-provider"
              className="text-input"
              value={preset}
              onChange={(e) => {
                setPreset(e.target.value);
                setC((now) => withPreset(e.target.value, now));
              }}
            >
              <option value="">Custom</option>
              {Object.entries(PRESETS).map(([k, p]) => (
                <option key={k} value={k}>
                  {p.label}
                </option>
              ))}
            </select>
          </Row>
          <div className="set-fields">
            {field("endpoint", "Endpoint", "https://…")}
            {field("region", "Region", "auto")}
            {field("bucket", "Bucket", "agentvm-backups")}
            {field("prefix", "Folder in the bucket", "agentvm")}
            {field("access_key", "Access key")}
            <label>
              <span>Secret key</span>
              <input
                className="text-input mono"
                type="password"
                autoComplete="off"
                placeholder={stored.data?.secret_saved ? "Saved (empty keeps it)" : "Kept in the Keychain"}
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
              />
            </label>
          </div>
          <Row
            label="Path-style addresses"
            description="endpoint/bucket/key instead of bucket.endpoint/key: MinIO-like servers need it."
          >
            <Toggle label="Path-style addresses" on={c.path_style} onChange={(on) => setC({ ...c, path_style: on })} />
          </Row>
        </Panel>
      </form>
    </Page>
  );
}
