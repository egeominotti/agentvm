// What a VM serves, reachable from the Mac: web pages open under the VM's own name, other
// services through a TCP port on the Mac (click to copy its address).
import type { ForwardedPort } from "../../api/generated/ForwardedPort";
import { useToast } from "../../components/Toast";

export function PortsBar({ ports }: { ports: ForwardedPort[] }) {
  const say = useToast();
  if (!ports.length) return null;
  return (
    <section className="ports-bar" aria-label="Open on this Mac">
      <span className="ports-label">Open on this Mac</span>
      {ports.map((p) => {
        const name = p.name || "service";
        if (p.kind === "http" && p.url) {
          return (
            <a key={p.port} className="port-link" href={p.url} target="_blank" rel="noopener" title={`Opens ${p.url}`}>
              <span className="live" />
              <b>{name}</b>
              <span className="addr">{p.url.replace(/^http:\/\//, "")}</span>
            </a>
          );
        }
        const addr = `localhost:${p.host_port}`;
        return (
          <button
            key={p.port}
            type="button"
            className="port-link tcp"
            title={`Not a web page: connect to ${addr} (click to copy)`}
            onClick={() => navigator.clipboard.writeText(addr).then(() => say(`Copied ${addr}`))}
          >
            <span className="live" />
            <b>{name}</b>
            <span className="addr">{addr}</span>
            {p.host_port !== p.port ? <span className="moved">{p.port} in the VM</span> : null}
          </button>
        );
      })}
    </section>
  );
}
