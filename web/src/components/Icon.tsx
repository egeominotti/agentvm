// agentvm's own line icons, drawn for it on one 16 px grid: 1.35 px strokes, round ends, the
// same corner radius (1.5) and a 2.5 px margin, so every icon sits at the same weight.
import type { ReactNode } from "react";

// A tray: what saves and transfers land in or leave from.
const tray = <path d="M2.5 10.5V12A1.5 1.5 0 0 0 4 13.5h8a1.5 1.5 0 0 0 1.5-1.5v-1.5" />;
const cloud = <path d="M5 12.5H4.5a2.5 2.5 0 0 1-.3-5A4 4 0 0 1 12 6.6a2.9 2.9 0 0 1-.5 5.9H11" />;

const paths: Record<string, ReactNode> = {
  plus: <path d="M8 3.5v9M3.5 8h9" />,
  machines: (
    <>
      <rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1.5" />
      <rect x="9" y="2.5" width="4.5" height="4.5" rx="1.5" />
      <rect x="2.5" y="9" width="4.5" height="4.5" rx="1.5" />
      <rect x="9" y="9" width="4.5" height="4.5" rx="1.5" />
    </>
  ),
  snapshots: (
    <>
      <path d="M8 2.5 13.5 5 8 7.5 2.5 5Z" />
      <path d="m2.5 8 5.5 2.5L13.5 8" />
      <path d="m2.5 11 5.5 2.5 5.5-2.5" />
    </>
  ),
  settings: (
    <>
      <path d="M2.5 4.5h6M12 4.5h1.5M2.5 8h1.5M7.5 8h6M2.5 11.5h7M13 11.5h.5" />
      <circle cx="10.25" cy="4.5" r="1.5" />
      <circle cx="5.75" cy="8" r="1.5" />
      <circle cx="11.25" cy="11.5" r="1.5" />
    </>
  ),
  more: (
    <>
      <circle cx="3.5" cy="8" r=".9" />
      <circle cx="8" cy="8" r=".9" />
      <circle cx="12.5" cy="8" r=".9" />
    </>
  ),
  panel: (
    <>
      <rect x="2.5" y="3" width="11" height="10" rx="1.5" />
      <path d="M10 3v10" />
    </>
  ),
  close: <path d="m4 4 8 8M12 4l-8 8" />,
  // Claude: a four-point spark, and a small one beside it.
  spark: (
    <>
      <path d="M7 2.5c.35 2.6 1.4 3.65 4 4-2.6.35-3.65 1.4-4 4-.35-2.6-1.4-3.65-4-4 2.6-.35 3.65-1.4 4-4Z" />
      <path d="M12 10.5v3M10.5 12h3" />
    </>
  ),
  terminal: (
    <>
      <rect x="2" y="3" width="12" height="10" rx="1.5" />
      <path d="m5 6.5 2 1.5-2 1.5M8.5 10h2.5" />
    </>
  ),
  // Save: the VM's commits go to the branch, drawn as a commit on its line.
  save: (
    <>
      <circle cx="8" cy="8" r="2.5" />
      <path d="M2.5 8h3M10.5 8h3" />
    </>
  ),
  upload: (
    <>
      <path d="M8 9.5V2.5M5 5.5l3-3 3 3" />
      {tray}
    </>
  ),
  download: (
    <>
      <path d="M8 2.5V9.5M5 6.5l3 3 3-3" />
      {tray}
    </>
  ),
  power: (
    <>
      <path d="M8 2.5v5" />
      <path d="M4.8 4.6a4.75 4.75 0 1 0 6.4 0" />
    </>
  ),
  stop: <rect x="4" y="4" width="8" height="8" rx="1.5" />,
  snapshot: (
    <>
      <path d="M2.5 6A1.5 1.5 0 0 1 4 4.5h1.5l1-1.5h3l1 1.5H12A1.5 1.5 0 0 1 13.5 6v5.5A1.5 1.5 0 0 1 12 13H4a1.5 1.5 0 0 1-1.5-1.5Z" />
      <circle cx="8" cy="8.75" r="2.25" />
    </>
  ),
  restore: (
    <>
      <path d="M3 8a5 5 0 1 0 1.5-3.55" />
      <path d="M3 2.75v2.5h2.5" />
      <path d="M8 5.5V8l1.75 1.25" />
    </>
  ),
  "cloud-up": (
    <>
      {cloud}
      <path d="M8 13.5v-5M6 10.5l2-2 2 2" />
    </>
  ),
  "cloud-down": (
    <>
      {cloud}
      <path d="M8 8.5v5M6 11.5l2 2 2-2" />
    </>
  ),
  trash: (
    <>
      <path d="M3 4.5h10M6.5 4.5V3h3v1.5" />
      <path d="m4.5 4.5.6 8.1a1 1 0 0 0 1 .9h3.8a1 1 0 0 0 1-.9l.6-8.1" />
    </>
  ),
  branch: (
    <>
      <circle cx="5" cy="4" r="1.5" />
      <circle cx="5" cy="12" r="1.5" />
      <circle cx="11" cy="5.5" r="1.5" />
      <path d="M5 5.5v5M11 7c0 2.5-6 1.5-6 3.5" />
    </>
  ),
  copy: (
    <>
      <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" />
      <path d="M10.5 5.5V4A1.5 1.5 0 0 0 9 2.5H4A1.5 1.5 0 0 0 2.5 4v5A1.5 1.5 0 0 0 4 10.5h1.5" />
    </>
  ),
  external: (
    <>
      <path d="M9.5 2.5h4v4M13.5 2.5 8 8" />
      <path d="M12 9.5V12a1.5 1.5 0 0 1-1.5 1.5h-6A1.5 1.5 0 0 1 3 12V5.5A1.5 1.5 0 0 1 4.5 4H7" />
    </>
  ),
  refresh: (
    <>
      <path d="M13 8a5 5 0 0 1-8.9 3.1M3 8a5 5 0 0 1 8.9-3.1" />
      <path d="M12 2.5v2.6H9.4M4 13.5v-2.6h2.6" />
    </>
  ),
  check: <path d="m3.5 8.5 3 3 6-7" />,
  // Private: a padlock. Public: a globe.
  lock: (
    <>
      <rect x="3.5" y="7" width="9" height="6.5" rx="1.5" />
      <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
    </>
  ),
  globe: (
    <>
      <circle cx="8" cy="8" r="5.5" />
      <path d="M2.5 8h11M8 2.5c1.5 1.6 2.3 3.4 2.3 5.5S9.5 11.9 8 13.5C6.5 11.9 5.7 10.1 5.7 8S6.5 4.1 8 2.5Z" />
    </>
  ),
  key: (
    <>
      <circle cx="5.5" cy="10.5" r="2.75" />
      <path d="M7.5 8.5 13 3M11 5l1.75 1.75" />
    </>
  ),
};

export type IconName = keyof typeof paths;

export function Icon({ name, label }: { name: IconName; label?: string }) {
  if (!label) {
    return (
      <svg className="i" viewBox="0 0 16 16" aria-hidden="true">
        {paths[name]}
      </svg>
    );
  }
  return (
    <svg className="i" viewBox="0 0 16 16" role="img">
      <title>{label}</title>
      {paths[name]}
    </svg>
  );
}
