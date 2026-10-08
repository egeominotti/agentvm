/* What a VM serves, reachable from the Mac. */
import { h, svg } from "../../dom.js";
import { toast } from "../../ui/toast.js";

/** A service of a VM: HTTP ones under the VM's own name, others through a TCP port on the Mac. */
function portLink(p) {
  const name = p.name || "service";
  if (p.kind === "http") {
    const host = p.url.replace(/^http:\/\//, "");
    return h("a", { class: "port-link", href: p.url, target: "_blank", rel: "noopener",
      title: `${name} on port ${p.port} of this VM.\nEvery VM has its own name, so they can all use port ${p.port}.\nOpens ${p.url}` },
      h("span", { class: "live" }), h("b", {}, name), h("span", { class: "addr" }, host),
      svg("svg", { class: "i ext", viewBox: "0 0 16 16", "aria-hidden": "true" }, svg("path", { d: "M6 3.5H3.5v9h9V10M9 3.5h3.5V7M12.5 3.5 7 9" })));
  }
  const addr = `localhost:${p.host_port}`;
  const copy = h("button", { class: "port-link tcp", type: "button",
    title: `${name} on port ${p.port} of this VM is not a web page: connect to ${addr} (click to copy).` },
    h("span", { class: "live" }), h("b", {}, name), h("span", { class: "addr" }, addr),
    p.host_port !== p.port ? h("span", { class: "moved" }, `${p.port} in the VM`) : null, h("span", { class: "moved" }, "TCP"));
  copy.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(addr); toast(`Copied ${addr}`); } catch { toast(addr); }
  });
  return copy;
}

/** Always in view, right under the toolbar; rebuilt only when the ports change. */
export class PortsBar {
  constructor() {
    this.el = h("div", { class: "ports-bar", "aria-label": "Ports open on this Mac" });
  }

  update(ports) {
    this.el.hidden = !ports.length;
    const key = ports.map(p => `${p.port}:${p.url}:${p.host_port}:${p.name}`).join(",");
    if (key === this.key) return;
    this.key = key;
    this.el.replaceChildren(h("span", { class: "ports-label" }, "Open on this Mac"), ...ports.map(portLink));
  }
}
