/* Settings › Notifications: a desktop notification when Claude waits. */
import { h } from "../../dom.js";
import { notificationsOn, notificationsSupported, toggleNotifications } from "../../notifications.js";

export function notificationsSection() {
  const toggle = h("button", { class: "switch", type: "button", role: "switch", onclick: async () => {
    if (!notificationsSupported()) return;
    await toggleNotifications();
    paint();
  } }, h("i"));
  const msg = h("span", { class: "hint" });

  function paint() {
    toggle.setAttribute("aria-checked", String(notificationsOn()));
    msg.textContent = !notificationsSupported() ? "Not supported here." : Notification.permission === "denied" ? "Blocked in the browser settings." : "";
  }

  paint();
  return {
    body: [h("div", { class: "status-line" }, toggle, h("div", {}, h("b", {}, "Notify me when Claude is waiting"),
      h("p", { class: "hint" }, "A desktop notification when an agent finishes a turn while this page is in the background.")), msg)],
  };
}
