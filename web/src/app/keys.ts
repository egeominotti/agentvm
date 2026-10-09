// Keyboard shortcuts: which exist, and when a key is a shortcut rather than typing.

export const TOGGLE_PANEL_EVENT = "agentvm:toggle-panel";

/** [keys, what they do], as the help lists them. */
export const SHORTCUTS: [string, string][] = [
  ["⌘ K", "New VM"],
  ["⌘ ↵", "Launch, in New VM"],
  ["⌘ J", "Show or hide a machine's details"],
  ["G then M", "Machines"],
  ["G then S", "Snapshots"],
  ["G then ,", "Settings"],
  ["?", "This list"],
];

/** Keys typed into a field, an editable text or a terminal belong to it, not to the page. */
export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable || target.contentEditable === "true") return true;
  if (target.closest(".xterm")) return true;
  return (
    target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement
  );
}
