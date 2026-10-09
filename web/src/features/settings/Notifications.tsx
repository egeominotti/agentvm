// A desktop notification when Claude waits for you.
import { useState } from "react";
import { notificationsOn, notificationsSupported, setNotifications } from "../../app/attention";
import { Switch } from "../../components/Switch";

export function Notifications() {
  const [on, setOn] = useState(notificationsOn);
  const supported = notificationsSupported();
  const blocked = supported && Notification.permission === "denied";
  return (
    <Switch
      on={on}
      disabled={!supported || blocked}
      onChange={async (next) => {
        await setNotifications(next);
        setOn(notificationsOn());
      }}
      label="Notify me when Claude is waiting"
      hint={
        !supported
          ? "This browser cannot show notifications."
          : blocked
            ? "Blocked for this page in the browser's settings: allow notifications there first."
            : "When an agent finishes a turn while this page is in the background."
      }
    />
  );
}
