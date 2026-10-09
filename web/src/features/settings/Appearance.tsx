// Light or dark for this browser; the terminals follow it (Catppuccin Latte or Mocha).
import { useState } from "react";
import { Segmented } from "../../components/Segmented";
import { setThemePref, storedPref, type ThemePref } from "../../lib/theme";

const CHOICES: [ThemePref, string][] = [
  ["system", "System"],
  ["light", "Light"],
  ["dark", "Dark"],
];

export function Appearance() {
  const [pref, setPref] = useState(storedPref);
  return (
    <div className="set">
      <span className="set-label">Theme</span>
      <Segmented
        label="Theme"
        value={pref}
        options={CHOICES}
        onChange={(next) => {
          setThemePref(next);
          setPref(next);
        }}
        wide
      />
      <p className="hint">System follows macOS. Terminals use Catppuccin: Mocha in dark mode, Latte in light mode.</p>
    </div>
  );
}
