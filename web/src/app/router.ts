// Hash routing: #/wall, #/vm/<id>, #/snapshots, #/settings. The hash keeps every screen
// bookmarkable without the server knowing about routes.
import { useSyncExternalStore } from "react";

export type Route =
  | { name: "wall" }
  | { name: "machine"; id: string }
  | { name: "snapshots" }
  | { name: "settings"; section?: string };

export function parse(hash: string): Route {
  const vm = hash.match(/^#\/vm\/([\w-]+)$/);
  if (vm?.[1]) return { name: "machine", id: vm[1] };
  if (hash.startsWith("#/snapshots")) return { name: "snapshots" };
  const settings = hash.match(/^#\/settings(?:\/([\w-]+))?$/);
  if (settings) return { name: "settings", section: settings[1] };
  return { name: "wall" };
}

const subscribe = (onChange: () => void) => {
  window.addEventListener("hashchange", onChange);
  return () => window.removeEventListener("hashchange", onChange);
};

export function useRoute(): Route {
  const hash = useSyncExternalStore(subscribe, () => location.hash);
  return parse(hash);
}

export const go = (hash: string) => {
  location.hash = hash;
};
