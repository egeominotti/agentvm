// The dashboard's line icons (16 px, current color).
import type { ReactNode } from "react";

const paths: Record<string, ReactNode> = {
  plus: <path d="M8 3.5v9M3.5 8h9" />,
  machines: (
    <>
      <rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1.2" />
      <rect x="9" y="2.5" width="4.5" height="4.5" rx="1.2" />
      <rect x="2.5" y="9" width="4.5" height="4.5" rx="1.2" />
      <rect x="9" y="9" width="4.5" height="4.5" rx="1.2" />
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
