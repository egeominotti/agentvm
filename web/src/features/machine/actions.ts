// What can be done to a machine, each with its message: the words say what happened, in the
// same terms as the button that did it.
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { PushedBranch } from "../../api/generated/PushedBranch";
import type { Saved } from "../../api/generated/Saved";
import type { SnapshotMeta } from "../../api/generated/SnapshotMeta";
import { keys } from "../../api/queries";
import { go } from "../../app/router";
import { useToast } from "../../components/Toast";
import { plural } from "../../lib/format";

export function useMachineActions(id: string) {
  const qc = useQueryClient();
  const say = useToast();
  const refresh = () => qc.invalidateQueries({ queryKey: keys.tasks });
  const fail = (e: Error) => say(e.message, "err");
  const base = `/api/tasks/${id}`;

  const save = useMutation({
    mutationFn: () => api<Saved>(`${base}/save`, "POST"),
    onSuccess: (r) =>
      say(r.commits ? `Saved ${plural(r.commits, "commit")} to ${r.branch}` : "Nothing new to save: no commits yet"),
    onError: fail,
  });
  const close = useMutation({
    mutationFn: () => api(`${base}/close`, "POST"),
    onSuccess: refresh,
    // The VM stays up and nothing is lost: the message says why.
    onError: fail,
  });
  const snapshot = useMutation({
    mutationFn: () => api<SnapshotMeta>(`${base}/snapshot`, "POST", {}),
    onSuccess: () => {
      say("Snapshot saved: find it in Snapshots");
      qc.invalidateQueries({ queryKey: ["snapshots"] });
    },
    onError: fail,
  });
  const stop = useMutation({ mutationFn: () => api(`${base}/stop`, "POST"), onSuccess: refresh, onError: fail });
  const remove = useMutation({
    mutationFn: () => api(base, "DELETE"),
    onSuccess: () => {
      say("Machine deleted. Its branch stays in your repository.");
      go("#/wall");
      refresh();
    },
    onError: fail,
  });
  const autoSnapshots = useMutation({
    mutationFn: (every_min: number | null) => api(`${base}/auto-snapshots`, "PUT", { every_min }),
    onSuccess: (_, m) => {
      say(
        m == null ? "Snapshots follow the settings" : m === 0 ? "No automatic snapshots" : `A snapshot every ${m} min`,
      );
      refresh();
    },
    onError: fail,
  });
  const push = useMutation({
    mutationFn: () => api<PushedBranch>(`${base}/push`, "POST"),
    onSuccess: (r) => say(`Pushed ${r.branch} to ${r.remote.replace(/^https:\/\/|^git@|\.git$/g, "")}`),
    onError: fail,
  });
  return { save, close, snapshot, stop, remove, autoSnapshots, push };
}

export type MachineActions = ReturnType<typeof useMachineActions>;
