// What this browser shows: light or dark (the terminals follow), and notifications.
import { useState } from "react";
import { notificationsOn, notificationsSupported, setNotifications } from "../../app/attention";
import { Segmented } from "../../components/Segmented";
import { setThemePref, storedPref, type ThemePref } from "../../lib/theme";
import { Page, Panel, Row, Toggle } from "./kit";

const THEMES: [ThemePref, string][] = [
  ["system", "System"],
  ["light", "Light"],
  ["dark", "Dark"],
];

export function Preferences() {
  const [theme, setTheme] = useState(storedPref);
  const [notify, setNotify] = useState(notificationsOn);
  const supported = notificationsSupported();
  const blocked = supported && Notification.permission === "denied";
  return (
    <Page title="Preferences" description="Kept in this browser only.">
      <Panel title="Appearance">
        <Row label="Theme" description="Terminals use Catppuccin: Mocha in dark mode, Latte in light mode.">
          <Segmented
            label="Theme"
            value={theme}
            options={THEMES}
            onChange={(next) => {
              setThemePref(next);
              setTheme(next);
            }}
            wide
          />
        </Row>
      </Panel>
      <Panel title="Notifications">
        <Row
          label="When Claude is waiting"
          description={
            !supported
              ? "This browser cannot show notifications."
              : blocked
                ? "Blocked for this page in the browser's settings: allow notifications there first."
                : "A notification when an agent finishes a turn while this page is in the background."
          }
        >
          <Toggle
            label="Notify me when Claude is waiting"
            on={notify}
            disabled={!supported || blocked}
            onChange={async (next) => {
              await setNotifications(next);
              setNotify(notificationsOn());
            }}
          />
        </Row>
      </Panel>
    </Page>
  );
}
