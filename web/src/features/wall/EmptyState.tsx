// No machine yet: what agentvm is, and the one thing to do next.

import { Button } from "../../components/Button";
import { Icon } from "../../components/Icon";
import { Logo } from "../../components/Logo";
import { openLauncher } from "../launcher/open";

export function EmptyState() {
  return (
    <div className="empty">
      <Logo className="empty-mark" size={56} />
      <h1>Every terminal is a sealed machine.</h1>
      <p>
        Launch a VM and Claude Code opens inside it with root and every permission, on a fresh clone of your repository.
        Nothing it does can touch your Mac.
      </p>
      <Button variant="primary" onClick={openLauncher}>
        <Icon name="plus" />
        Launch your first VM <kbd>⌘K</kbd>
      </Button>
      <ol>
        <li>
          <b>Pick a repository</b> and, if you like, a first task for Claude.
        </li>
        <li>
          <b>Work with Claude</b> in the VM's terminal, or in a root shell next to it.
        </li>
        <li>
          <b>Save</b> copies the work to a branch agent/… in your repository.
        </li>
      </ol>
    </div>
  );
}
