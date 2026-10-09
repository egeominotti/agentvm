// No machine yet: what agentvm is, and the one thing to do next.
import { Button } from "../../components/Button";
import { openLauncher } from "../launcher/open";

export function EmptyState() {
  return (
    <div className="empty">
      <img className="empty-mark" src={`${import.meta.env.BASE_URL}logo.svg`} alt="" />
      <h1>Every terminal is a sealed machine.</h1>
      <p>
        Launch a VM and Claude Code opens inside it with root and every permission, on a fresh clone of your repository.
        Nothing it does can touch your Mac.
      </p>
      <Button variant="primary" onClick={openLauncher}>
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
