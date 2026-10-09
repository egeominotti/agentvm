// The settings, grouped as people look for them. Old addresses (#/settings/agent, …/browser,
// …/s3) still open the page that took them over.

export type SectionId =
  | "general"
  | "resources"
  | "preferences"
  | "account"
  | "git"
  | "tailscale"
  | "snapshots"
  | "backups"
  | "storage"
  | "image";

export type Section = { id: SectionId; title: string; icon: string; keywords: string };

export const GROUPS: { title: string; items: Section[] }[] = [
  {
    title: "Workspace",
    items: [
      {
        id: "general",
        title: "General",
        icon: "settings",
        keywords: "model default repository time limit timeout agent",
      },
      { id: "resources", title: "Resources", icon: "chip", keywords: "vms cpu cores memory ram concurrency" },
      {
        id: "preferences",
        title: "Preferences",
        icon: "sun",
        keywords: "theme light dark appearance notifications browser",
      },
    ],
  },
  {
    title: "Accounts & access",
    items: [
      { id: "account", title: "Claude account", icon: "user", keywords: "token subscription keychain claude" },
      { id: "git", title: "Git access", icon: "branch", keywords: "github gitlab token private repositories" },
      { id: "tailscale", title: "Tailscale", icon: "tailnet", keywords: "tailnet vpn ssh auth key network" },
    ],
  },
  {
    title: "Data",
    items: [
      { id: "snapshots", title: "Snapshots", icon: "snapshots", keywords: "automatic schedule interval keep close" },
      { id: "backups", title: "Backups", icon: "cloud-up", keywords: "s3 r2 hetzner backblaze minio rustfs bucket" },
      { id: "storage", title: "Storage", icon: "disk", keywords: "disk space logs cleanup delete" },
    ],
  },
  {
    title: "System",
    items: [{ id: "image", title: "VM image", icon: "refresh", keywords: "golden debian rebuild claude code version" }],
  },
];

export const SECTIONS: Section[] = GROUPS.flatMap((g) => g.items);

const ALIASES: Record<string, SectionId> = { agent: "general", browser: "preferences", s3: "backups" };

/** The page an address opens (the first one when it names none, or one that does not exist). */
export function sectionOf(id: string | undefined): SectionId {
  const wanted = (id && ALIASES[id]) || id;
  return SECTIONS.find((s) => s.id === wanted)?.id ?? "general";
}

/** The sections whose title or keywords hold every word of `query`. */
export function matching(query: string): Set<SectionId> {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return new Set(
    SECTIONS.filter((s) => {
      const text = `${s.title} ${s.keywords}`.toLowerCase();
      return words.every((w) => text.includes(w));
    }).map((s) => s.id),
  );
}
