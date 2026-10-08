/* Settings › Backups to S3: any S3-compatible bucket, tested before it is saved. */
import { api } from "../../api.js";
import { h } from "../../dom.js";

const PRESETS = {
  aws: { label: "AWS S3", endpoint: "https://s3.eu-central-1.amazonaws.com", region: "eu-central-1", path_style: false, hint: "Endpoint s3.<region>.amazonaws.com." },
  r2: { label: "Cloudflare R2", endpoint: "https://<account-id>.r2.cloudflarestorage.com", region: "auto", path_style: true, hint: "Region is always auto. Use an R2 API token's access key and secret." },
  hetzner: { label: "Hetzner", endpoint: "https://fsn1.your-objectstorage.com", region: "fsn1", path_style: false, hint: "Locations: fsn1, nbg1, hel1. Region matches the location." },
  b2: { label: "Backblaze B2", endpoint: "https://s3.eu-central-003.backblazeb2.com", region: "eu-central-003", path_style: false, hint: "Use an application key; the region is in the endpoint." },
  local: { label: "Local (RustFS / MinIO)", endpoint: "http://127.0.0.1:9100", region: "us-east-1", path_style: true, hint: "Start it with scripts/dev-s3.sh up. Access key agentvm, secret agentvm-local-secret." },
};
const SECRET_KEPT = "saved in the Keychain; leave empty to keep it";

export function s3Section() {
  const f = {};
  const input = (name, attrs = {}) => (f[name] = h("input", { name, spellcheck: "false", autocomplete: "off", ...attrs }));
  const hint = h("p", { class: "hint" });
  const msg = h("span", { class: "msg" });
  const pill = h("span", { class: "pill" });
  const presetSeg = h("div", { class: "seg wide presets", role: "radiogroup", "aria-label": "Provider" },
    Object.entries(PRESETS).map(([key, p]) => h("button", { type: "button", role: "radio", "data-value": key, onclick: () => applyPreset(key) }, p.label)));

  function applyPreset(key) {
    const p = PRESETS[key];
    for (const b of presetSeg.children) b.setAttribute("aria-checked", String(b.dataset.value === key));
    f.endpoint.value = p.endpoint;
    f.region.value = p.region;
    f.path_style.checked = p.path_style;
    hint.textContent = p.hint;
    if (key === "local") { f.bucket.value ||= "agentvm-backups"; f.access_key.value ||= "agentvm"; }
  }

  async function save(e) {
    e.preventDefault();
    const config = { endpoint: f.endpoint.value.trim(), region: f.region.value.trim(), bucket: f.bucket.value.trim(), prefix: f.prefix.value.trim(), access_key: f.access_key.value.trim(), path_style: f.path_style.checked };
    msg.className = "msg";
    msg.textContent = "Testing the connection…";
    const r = await api("/api/settings/s3", { method: "PUT", body: { config, secret: f.secret.value || null } });
    msg.className = `msg ${r.ok ? "ok" : "err"}`;
    msg.textContent = r.ok ? "Connected: the bucket accepts uploads. Saved." : r.data?.error || "Could not connect.";
    if (r.ok) { f.secret.value = ""; f.secret.placeholder = SECRET_KEPT; pill.className = "pill ok"; pill.textContent = "Connected"; }
  }

  const form = h("form", { class: "s3-form", onsubmit: save },
    h("div", { class: "set" }, h("label", {}, "Provider"), presetSeg, hint),
    h("div", { class: "grid3" },
      h("div", { class: "set" }, h("label", {}, "Endpoint"), input("endpoint", { placeholder: "https://…" })),
      h("div", { class: "set" }, h("label", {}, "Region"), input("region", { placeholder: "auto" })),
      h("div", { class: "set" }, h("label", {}, "Bucket"), input("bucket", { placeholder: "agentvm-backups" })),
      h("div", { class: "set" }, h("label", {}, "Folder in the bucket"), input("prefix", { placeholder: "agentvm", value: "agentvm" })),
      h("div", { class: "set" }, h("label", {}, "Access key"), input("access_key")),
      h("div", { class: "set" }, h("label", {}, "Secret key"), input("secret", { type: "password", placeholder: "kept in the Keychain" }))),
    h("label", { class: "check" }, (f.path_style = h("input", { type: "checkbox", name: "path_style" })), "Path-style addresses (endpoint/bucket/key)"),
    h("div", { class: "row-actions" }, h("button", { class: "btn primary", type: "submit" }, "Test & save"), msg));

  api("/api/settings/s3").then(r => {
    const c = r.ok ? r.data.config : null;
    pill.className = `pill ${c ? "ok" : "err"}`;
    pill.textContent = c ? "Connected" : "Not set up";
    if (c) { for (const k of ["endpoint", "region", "bucket", "prefix", "access_key"]) f[k].value = c[k]; f.path_style.checked = c.path_style; }
    if (r.ok && r.data.secret_saved) f.secret.placeholder = SECRET_KEPT;
    if (!c) applyPreset("local");
  });

  return { body: [h("div", { class: "status-line" }, pill, h("span", { class: "lede" }, "Back up snapshots to any S3-compatible storage and restore them on this or another Mac.")), form] };
}
