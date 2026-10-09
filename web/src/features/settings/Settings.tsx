// Settings: a grouped index with search on the left, one page at a time on the right, every
// change saved on the spot. `#/settings/<section>` opens that page.
import { type ReactNode, useState } from "react";
import { useSettings } from "../../api/queries";
import { go } from "../../app/router";
import { Icon } from "../../components/Icon";
import { Account } from "./Account";
import { Backups } from "./Backups";
import { General } from "./General";
import { GitAccess } from "./GitAccess";
import { Image } from "./Image";
import { Preferences } from "./Preferences";
import { Resources } from "./Resources";
import { GROUPS, matching, SECTIONS, type SectionId, sectionOf } from "./sections";
import { Snapshots } from "./Snapshots";
import { Storage } from "./Storage";
import { Tailscale } from "./Tailscale";
import { type SaveState, useDraft } from "./useDraft";

export function SettingsView({ section }: { section?: string }) {
  const view = useSettings();
  const { draft: s, set, save } = useDraft(view.data?.settings);
  const [query, setQuery] = useState("");
  const current = sectionOf(section);
  const found = matching(query);
  const title = SECTIONS.find((x) => x.id === current)?.title ?? "";

  let page: ReactNode;
  if (view.error) page = <p className="msg err set-page">{view.error.message}</p>;
  else if (!s || !view.data) page = <p className="msg set-page">Loading the settings…</p>;
  else {
    const limits = view.data.limits;
    const pages: Record<SectionId, ReactNode> = {
      general: <General s={s} set={set} />,
      resources: <Resources s={s} set={set} limits={limits} />,
      preferences: <Preferences />,
      account: <Account />,
      git: <GitAccess />,
      tailscale: <Tailscale s={s} set={set} />,
      snapshots: <Snapshots s={s} set={set} />,
      backups: <Backups />,
      storage: <Storage />,
      image: <Image s={s} set={set} />,
    };
    page = pages[current];
  }

  return (
    <div className="settings">
      <nav className="set-nav" aria-label="Settings">
        <span className="set-nav-title">Settings</span>
        <label className="set-search">
          <Icon name="search" />
          <input
            className="text-input"
            type="search"
            placeholder="Search settings"
            aria-label="Search settings"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              const first = SECTIONS.find((x) => found.has(x.id));
              if (e.key === "Enter" && first) go(`#/settings/${first.id}`);
              if (e.key === "Escape") setQuery("");
            }}
          />
        </label>
        {GROUPS.map((g) => {
          const items = g.items.filter((x) => found.has(x.id));
          if (!items.length) return null;
          return (
            <div key={g.title} className="set-nav-group">
              <span>{g.title}</span>
              {items.map((x) => (
                <a key={x.id} href={`#/settings/${x.id}`} aria-current={x.id === current ? "page" : undefined}>
                  <Icon name={x.icon} />
                  {x.title}
                </a>
              ))}
            </div>
          );
        })}
        {found.size ? null : <p className="set-nav-empty">No setting matches “{query}”.</p>}
        <select
          className="text-input set-nav-select"
          aria-label="Settings page"
          value={current}
          onChange={(e) => go(`#/settings/${e.target.value}`)}
        >
          {GROUPS.map((g) => (
            <optgroup key={g.title} label={g.title}>
              {g.items.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.title}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </nav>
      <main className="set-main">
        <div className="set-bar">
          <span className="set-crumb">Settings</span>
          <span className="set-crumb" aria-hidden="true">
            /
          </span>
          <b>{title}</b>
          <SaveLine save={save} autosaved={AUTOSAVED.has(current)} />
        </div>
        {page}
      </main>
    </div>
  );
}

/** Pages whose fields save themselves (the others have their own Save buttons). */
const AUTOSAVED = new Set<SectionId>(["general", "resources", "tailscale", "snapshots", "image"]);

function SaveLine({ save, autosaved }: { save: SaveState; autosaved: boolean }) {
  if (save.kind === "error") {
    return (
      <span className="save-state err" role="alert">
        Not saved: {save.message}
      </span>
    );
  }
  if (save.kind === "idle")
    return autosaved ? <span className="save-state idle">Changes save automatically</span> : null;
  return (
    <span className={`save-state ${save.kind}`} role="status">
      {save.kind === "saved" ? <Icon name="check" /> : null}
      {save.kind === "saving" ? "Saving…" : "Saved"}
    </span>
  );
}
