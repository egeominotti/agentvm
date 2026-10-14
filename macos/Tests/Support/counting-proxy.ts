// A real HTTP observer: every application request and response passes through unchanged.
const [target, readyFile] = Bun.argv.slice(2);
const upstream = new URL(target);
if (upstream.hostname !== "127.0.0.1" || upstream.port === "7777") {
  throw new Error("The observer requires an isolated local engine.");
}
const counts: Record<string, number> = {};
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === "/__native_counts") return Response.json(counts);
    const key = `${request.method} ${url.pathname}`;
    counts[key] = (counts[key] ?? 0) + 1;
    url.protocol = upstream.protocol;
    url.host = upstream.host;
    const headers = new Headers(request.headers);
    headers.delete("host");
    return fetch(url, {
      method: request.method,
      headers,
      body: request.body,
      redirect: "manual",
    });
  },
});
await Bun.write(readyFile, JSON.stringify({ port: server.port }));
