/* The agentvm HTTP API: JSON in, JSON out, never throws. */

/** Resolves to `{ ok, status, data }`; a server that cannot be reached is status 0 with an error message. */
export async function api(path, opts = {}) {
  const init = { ...opts, headers: { "content-type": "application/json", ...(opts.headers || {}) } };
  if (opts.body && typeof opts.body !== "string") init.body = JSON.stringify(opts.body);
  try {
    const res = await fetch(path, init);
    const text = await res.text();
    let data = null;
    try { data = text ? JSON.parse(text) : null; } catch { data = text; }
    return { ok: res.ok, status: res.status, data };
  } catch (e) {
    return { ok: false, status: 0, data: { error: "The agentvm server is not reachable." } };
  }
}
