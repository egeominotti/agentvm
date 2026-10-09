// Backups of snapshots to any S3-compatible bucket, tested before they are saved.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { S3Config } from "../../api/generated/S3Config";
import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Segmented } from "../../components/Segmented";
import { PRESETS } from "./presets";

type Stored = { config: S3Config | null; secret_saved: boolean };
const EMPTY: S3Config = { endpoint: "", region: "", bucket: "", prefix: "agentvm", access_key: "", path_style: false };
const KEPT = "kept in the Keychain; leave empty to keep it";

/** `c` addressed the way the provider `key` wants it. */
function withPreset(key: string, c: S3Config): S3Config {
  const p = PRESETS[key];
  if (!p) return c;
  const local = key === "local" ? { bucket: c.bucket || "agentvm-backups", access_key: c.access_key || "agentvm" } : {};
  return { ...c, endpoint: p.endpoint, region: p.region, path_style: p.pathStyle, ...local };
}

export function S3() {
  const qc = useQueryClient();
  const stored = useQuery({ queryKey: ["s3"], queryFn: () => api<Stored>("/api/settings/s3") });
  const [c, setC] = useState<S3Config>(EMPTY);
  const [preset, setPreset] = useState<string>("");
  const [secret, setSecret] = useState("");
  // What is saved, or a local bucket to start from.
  useEffect(() => {
    const config = stored.data?.config;
    if (config) setC(config);
    else if (stored.data) {
      setPreset("local");
      setC((now) => withPreset("local", now));
    }
  }, [stored.data]);
  const pick = (key: string) => {
    setPreset(key);
    setC((now) => withPreset(key, now));
  };
  const save = useMutation({
    mutationFn: () => api("/api/settings/s3", "PUT", { config: c, secret: secret || null }),
    onSuccess: () => {
      setSecret("");
      qc.invalidateQueries({ queryKey: ["s3"] });
      qc.invalidateQueries({ queryKey: ["backups"] });
    },
  });
  const field = (k: keyof Omit<S3Config, "path_style">, label: string, placeholder = "") => (
    <label className="set">
      <span className="set-label">{label}</span>
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
    <form
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate();
      }}
    >
      <p className="status-line">
        <span className={`pill ${connected ? "ok" : "err"}`}>{connected ? "Connected" : "Not set up"}</span>
        <span className="lede">
          Back up snapshots to any S3-compatible storage, and bring them back here or on another Mac.
        </span>
      </p>
      <div className="set">
        <span className="set-label">Provider</span>
        <Segmented
          label="Provider"
          value={preset}
          options={Object.entries(PRESETS).map(([k, p]) => [k, p.label])}
          onChange={pick}
          wide
        />
        {preset ? <p className="hint">{PRESETS[preset]?.hint}</p> : null}
      </div>
      <div className="grid3">
        {field("endpoint", "Endpoint", "https://…")}
        {field("region", "Region", "auto")}
        {field("bucket", "Bucket", "agentvm-backups")}
        {field("prefix", "Folder in the bucket", "agentvm")}
        {field("access_key", "Access key")}
        <label className="set">
          <span className="set-label">Secret key</span>
          <input
            className="text-input mono"
            type="password"
            autoComplete="off"
            placeholder={stored.data?.secret_saved ? KEPT : "kept in the Keychain"}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
          />
        </label>
      </div>
      <label className="check-row">
        <input type="checkbox" checked={c.path_style} onChange={(e) => setC({ ...c, path_style: e.target.checked })} />
        Path-style addresses (endpoint/bucket/key)
      </label>
      <div className="row-actions">
        <Button type="submit" variant="primary" disabled={save.isPending}>
          <Icon name="cloud-up" />
          {save.isPending ? "Testing the connection…" : "Test & save"}
        </Button>
        {save.isSuccess ? <span className="msg ok">Connected: the bucket accepts uploads. Saved.</span> : null}
        {save.error ? <span className="msg err">{save.error.message}</span> : null}
      </div>
    </form>
  );
}
