// New VM (⌘K): a repository, an optional first task, and the machine's options. It says why it
// cannot launch instead of failing after the click.
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
import { notLaunched, rememberRepo, splitPrompts, storedRecent } from "./prompts";
import { RepoField, useSettledCheck } from "./RepoField";

/** A task that is only a path was most likely meant as the repository. */
const looksLikePath = (text: string) => /^(~\/|\/)\S+$/.test(text.trim());

export function Launcher() {
  const [open, setOpen] = useState(false);
  const [repo, setRepo] = useState("");
  const [task, setTask] = useState("");
  const [perLine, setPerLine] = useState(false);
  const [choice, setChoice] = useState<Choice | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const settings = useSettings().data;
  const status = useStatus().data;
  const tasks = useTasks().data ?? [];
  const qc = useQueryClient();
  const check = useSettledCheck(repo);
  const repoInput = useRef<HTMLInputElement>(null);
  const taskInput = useRef<HTMLTextAreaElement>(null);

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
  useEffect(() => {
    if (!open) return;
    setError(null);
    setChoice(null);
    setRepo((r) => r || recent[0] || "");
  }, [open, recent[0]]);
  useEffect(() => {
    if (!open || !settings) return;
    const s = settings.settings;
    setChoice((c) => c ?? { model: s.model, version: "", cpus: s.cpus, memoryMb: s.memory_mb });
  }, [open, settings]);

  const prompts = splitPrompts(task, perLine);
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
    const ids: string[] = [];
    let failure: string | null = null;
    for (const prompt of prompts) {
      try {
        const r = await api<{ id: string }>("/api/tasks", "POST", {
          repo_path: repo.trim(),
          prompt,
          interactive: true,
          model: choice.model,
          claude_version: choice.version || null,
          cpus: choice.cpus,
          memory_mb: choice.memoryMb,
        });
        ids.push(r.id);
      } catch (err) {
        failure = (err as Error).message;
        setError(failure);
        break;
      }
    }
    setBusy(false);
    if (ids.length) {
      rememberRepo(repo.trim());
      await qc.invalidateQueries({ queryKey: keys.tasks });
    }
    if (ids.length < prompts.length) {
      // Some did not launch: the dialog stays, with the reason and only the tasks still to launch.
      if (ids.length) {
        setError(`${ids.length} of ${prompts.length} launched, then: ${failure ?? "a launch failed"}`);
        setTask(notLaunched(prompts, ids.length));
      }
      return;
    }
    setTask("");
    setOpen(false);
    go(ids.length === 1 ? `#/vm/${ids[0]}` : "#/wall");
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
            (repo.trim() || recent[0] ? taskInput : repoInput).current?.focus();
          }}
        >
          <form
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
              onChange={setRepo}
              onPicked={() => taskInput.current?.focus()}
              recent={recent}
              inputRef={repoInput}
              check={check}
            />
            <label className="field-label" htmlFor="launcher-task">
              First task for Claude <span className="optional">optional</span>
            </label>
            <textarea
              id="launcher-task"
              ref={taskInput}
              className="task-input"
              value={task}
              rows={4}
              placeholder="Leave it empty to start in the terminal"
              onChange={(e) => setTask(e.target.value)}
            />
            {looksLikePath(task) ? (
              <p className="path-hint">
                This looks like a folder.{" "}
                <button
                  type="button"
                  className="link"
                  onClick={() => {
                    setRepo(task.trim());
                    setTask("");
                    taskInput.current?.focus();
                  }}
                >
                  Use it as the repository
                </button>
              </p>
            ) : null}
            <label className="check-row">
              <input type="checkbox" checked={perLine} onChange={(e) => setPerLine(e.target.checked)} />
              One VM per line of the task
            </label>
            {choice ? <Options value={choice} onChange={setChoice} count={prompts.length} /> : null}
            {error ? <p className="form-error">{error}</p> : null}
            <footer className="dialog-foot">
              <span className="blocker">{blocker}</span>
              <Button type="submit" variant="primary" disabled={!!blocker || busy}>
                {busy
                  ? check?.to_clone
                    ? "Cloning…"
                    : check?.remote
                      ? "Fetching…"
                      : "Launching…"
                  : prompts.length > 1
                    ? `Launch ${prompts.length} VMs`
                    : "Launch VM"}{" "}
                <kbd>⌘↵</kbd>
              </Button>
            </footer>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
