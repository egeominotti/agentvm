// Settings: an index on the left, every section on one page, each change saved on the spot.
// `#/settings/<section>` opens the page at that section.
import { type ReactNode, useEffect, useRef, useState } from "react";
import { useSettings } from "../../api/queries";
import { Account } from "./Account";
import { Agent } from "./Agent";
import { Appearance } from "./Appearance";
import { AutoSnapshots } from "./AutoSnapshots";
import { GitAccess } from "./GitAccess";
import { Image } from "./Image";
import { Notifications } from "./Notifications";
import { Resources } from "./Resources";
import { S3 } from "./S3";
import { Storage } from "./Storage";
import { type SaveState, useDraft } from "./useDraft";

const SECTIONS = [
  ["resources", "Resources"],
  ["agent", "Agent"],
  ["account", "Claude account"],
  ["browser", "This browser"],
  ["git", "Git access"],
  ["snapshots", "Automatic snapshots"],
  ["image", "VM image"],
  ["storage", "Storage"],
  ["s3", "Backups to S3"],
] as const;

export function SettingsView({ section }: { section?: string }) {
  const view = useSettings();
  const { draft: s, set, save } = useDraft(view.data?.settings);
  const content = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState<string>(section ?? "resources");

  // Opened at a section: scroll to it once it is on the page.
  const loaded = !!s;
  useEffect(() => {
    if (section && loaded) document.getElementById(`set-${section}`)?.scrollIntoView({ block: "start" });
  }, [section, loaded]);

  const spy = () => {
    const box = content.current;
    if (!box) return;
    const top = box.getBoundingClientRect().top + 80;
    let current: string = SECTIONS[0][0];
    for (const [id] of SECTIONS) {
      const el = document.getElementById(`set-${id}`);
      if (el && el.getBoundingClientRect().top <= top) current = id;
    }
    setActive(current);
  };

  if (view.error) return <p className="hint pad">{view.error.message}</p>;
  if (!s || !view.data) return <p className="hint pad">Loading the settings…</p>;
  const limits = view.data.limits;
  const body: Record<string, ReactNode> = {
    resources: <Resources s={s} set={set} limits={limits} />,
    agent: <Agent s={s} set={set} />,
    account: <Account />,
    browser: (
      <>
        <Appearance />
        <Notifications />
      </>
    ),
    git: <GitAccess />,
    snapshots: <AutoSnapshots s={s} set={set} />,
    image: <Image s={s} set={set} />,
    storage: <Storage />,
    s3: <S3 />,
  };

  return (
    <div className="settings">
      <nav className="settings-nav" aria-label="Settings sections">
        {SECTIONS.map(([id, label]) => (
          <a
            key={id}
            href={`#/settings/${id}`}
            aria-current={active === id ? "true" : undefined}
            onClick={(e) => {
              e.preventDefault();
              document.getElementById(`set-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
              history.replaceState(null, "", `#/settings/${id}`);
            }}
          >
            {label}
          </a>
        ))}
      </nav>
      <div className="settings-content" ref={content} onScroll={spy}>
        <header className="settings-head">
          <h1>Settings</h1>
          <SaveLine save={save} />
        </header>
        <div className="settings-grid">
          {SECTIONS.map(([id, label]) => (
            <section key={id} id={`set-${id}`} className={`set-card span-${id}`} aria-labelledby={`h-${id}`}>
              <h2 id={`h-${id}`}>{label}</h2>
              {body[id]}
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}

function SaveLine({ save }: { save: SaveState }) {
  if (save.kind === "error") {
    return (
      <span className="save-state err" role="alert">
        Not saved: {save.message}
      </span>
    );
  }
  const text = { idle: "Changes are saved automatically", saving: "Saving…", saved: "All changes saved" }[save.kind];
  return (
    <span className={`save-state ${save.kind}`} role="status">
      {text}
    </span>
  );
}
