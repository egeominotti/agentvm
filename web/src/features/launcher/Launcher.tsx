// New VM (⌘K): a repository (a folder or a link) and the machine's options; Claude starts in its
// terminal. It says why it cannot launch instead of failing after the click.
import * as Dialog from "@radix-ui/react-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { keys, useSettings, useStatus, useTasks } from "../../api/queries";
import { go } from "../../app/router";
import { Button } from "../../components/Button";
import { shortPath } from "../../lib/format";
import { type Choice, Options } from "./Options";
import { NEW_VM_EVENT } from "./open";
import { RepoField, useSettledCheck } from "./RepoField";
import { rememberRepo, storedRecent } from "./recent";

export function Launcher() {
  const [open, setOpen] = useState(false);
  const [repo, setRepo] = useState("");
  const [choice, setChoice] = useState<Choice | null>(null);
  // The branch to start from; empty means the repository's default.
  const [branch, setBranch] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const settings = useSettings().data;
  const status = useStatus().data;
  const tasks = useTasks().data ?? [];
  const qc = useQueryClient();
  const check = useSettledCheck(repo);
  const repoInput = useRef<HTMLInputElement>(null);
  const form = useRef<HTMLFormElement>(null);
  const focusLaunch = () => form.current?.querySelector<HTMLButtonElement>("button[type=submit]")?.focus();

  const recent = [
    ...new Set([
      ...(settings?.settings.default_repo?.trim() ? [settings.settings.default_repo.trim()] : []),
      ...storedRecent(),
      ...tasks.map((t) => shortPath(t.repo)),
    ]),
  ].slice(0, 8);

  useEffect(() => {
    const show = () => setOpen(true);
    const keyboard = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener(NEW_VM_EVENT, show);
    document.addEventListener("keydown", keyboard);
    return () => {
      window.removeEventListener(NEW_VM_EVENT, show);
      document.removeEventListener("keydown", keyboard);
    };
  }, []);

  // On every opening: the last repository, and the choices as Settings has them now (changes
  // made for one launch do not stick to the next).
  const lastRepo = recent[0];
  useEffect(() => {
    if (!open) return;
    setError(null);
    setChoice(null);
    setRepo((r) => r || lastRepo || "");
  }, [open, lastRepo]);
  useEffect(() => {
    if (!open || !settings) return;
    const s = settings.settings;
    setChoice((c) => c ?? { model: s.model, version: "", cpus: s.cpus, memoryMb: s.memory_mb });
  }, [open, settings]);

  const blocker = !status
    ? "Connecting to agentvm…"
    : !status.golden
      ? "The VM image is missing: build it in Settings › VM image."
      : !status.token
        ? "Connect your Claude account in Settings › Claude account."
        : !repo.trim()
          ? "Choose a repository."
          : !check
            ? "Checking the repository…"
            : !check.ok
              ? "Choose a git repository with at least one commit."
              : null;

  const launch = async (e?: FormEvent) => {
    e?.preventDefault();
    if (blocker || busy || !choice) return;
    setBusy(true);
    setError(null);
    try {
      const r = await api<{ id: string }>("/api/tasks", "POST", {
        repo_path: repo.trim(),
        prompt: "",
        branch: branch || null,
        interactive: true,
        model: choice.model,
        claude_version: choice.version || null,
        cpus: choice.cpus,
        memory_mb: choice.memoryMb,
      });
      rememberRepo(repo.trim());
      await qc.invalidateQueries({ queryKey: keys.tasks });
      setOpen(false);
      go(`#/vm/${r.id}`);
    } catch (err) {
      // The dialog stays, with the reason.
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog launcher"
          aria-describedby={undefined}
          // Escape first closes the repository suggestions, and only then the dialog.
          onEscapeKeyDown={(e) => {
            if (document.activeElement?.getAttribute("aria-expanded") === "true") e.preventDefault();
          }}
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            // With a repository already there, Enter launches at once.
            if (repo.trim() || recent[0]) requestAnimationFrame(focusLaunch);
            else repoInput.current?.focus();
          }}
        >
          {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- shortcuts for the whole form */}
          <form
            ref={form}
            onSubmit={launch}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) launch();
            }}
          >
            <header className="dialog-head">
              <Dialog.Title>New VM</Dialog.Title>
              <Dialog.Close className="icon-btn" aria-label="Close">
                ✕
              </Dialog.Close>
            </header>
            <RepoField
              value={repo}
              onChange={(v) => {
                // Another repository: back to its default branch.
                setRepo(v);
                setBranch("");
              }}
              onPicked={focusLaunch}
              recent={recent}
              inputRef={repoInput}
              check={check}
              onGitAccess={() => {
                setOpen(false);
                go("#/settings/git");
              }}
            />
            {check?.ok && check.branches.length > 1 ? (
              <label className="branch-field">
                <span className="field-label">Branch</span>
                <select value={branch} onChange={(e) => setBranch(e.target.value)}>
                  {check.branches.map((b) => (
                    <option key={b} value={b === check.default_branch ? "" : b}>
                      {b === check.default_branch ? `${b} (default)` : b}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
            {choice ? <Options value={choice} onChange={setChoice} /> : null}
            {error ? <p className="form-error">{error}</p> : null}
            <footer className="dialog-foot">
              <span className="blocker">{blocker}</span>
              <Button type="submit" variant="primary" disabled={!!blocker || busy}>
                {busy ? (check?.to_clone ? "Cloning…" : check?.remote ? "Fetching…" : "Launching…") : "Launch VM"}{" "}
                <kbd>⌘↵</kbd>
              </Button>
            </footer>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
