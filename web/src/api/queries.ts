// The server's data, kept fresh: TanStack Query polls, caches and shares it between screens, and
// never runs two identical requests at once.
import { useQuery } from "@tanstack/react-query";
import { api } from "./client";
import type { Diagnostics } from "./generated/Diagnostics";
import type { GoldenStatus } from "./generated/GoldenStatus";
import type { Releases } from "./generated/Releases";
import type { RepoCheck } from "./generated/RepoCheck";
import type { Series } from "./generated/Series";
import type { SettingsView } from "./generated/SettingsView";
import type { Status } from "./generated/Status";
import type { TaskDto } from "./generated/TaskDto";
import type { Usage } from "./generated/Usage";

export const keys = {
  tasks: ["tasks"] as const,
  status: ["status"] as const,
  settings: ["settings"] as const,
  telemetry: (id: string, range: string) => ["telemetry", id, range] as const,
  diagnostics: (id: string) => ["diagnostics", id] as const,
  usage: (id: string) => ["usage", id] as const,
};

/** Every machine, running ones first, then oldest first. Refreshed every second, in a background
 *  tab too: that is how a machine waiting for you is noticed while you work elsewhere. */
export function useTasks() {
  return useQuery({
    queryKey: keys.tasks,
    queryFn: () => api<TaskDto[]>("/api/tasks"),
    refetchInterval: 1000,
    refetchIntervalInBackground: true,
    select: (tasks) => tasks.toSorted(byLiveThenAge),
  });
}

/** One machine, from the same list (no request of its own). */
export function useTask(id: string): TaskDto | undefined {
  return useTasks().data?.find((t) => t.id === id);
}

export function useStatus() {
  return useQuery({
    queryKey: keys.status,
    queryFn: () => api<Status>("/api/status"),
    refetchInterval: 3000,
    refetchIntervalInBackground: true,
  });
}

export function useSettings() {
  return useQuery({ queryKey: keys.settings, queryFn: () => api<SettingsView>("/api/settings") });
}

export function useTelemetry(id: string, range: string, live: boolean) {
  return useQuery({
    queryKey: keys.telemetry(id, range),
    queryFn: () => api<Series>(`/api/tasks/${id}/telemetry?range=${range}`),
    refetchInterval: live ? (range === "5m" ? 2000 : 15000) : false,
  });
}

export function useDiagnostics(id: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.diagnostics(id),
    queryFn: () => api<Diagnostics>(`/api/tasks/${id}/diagnostics`),
    enabled,
  });
}

export function useClaudeUsage(id: string, live: boolean) {
  return useQuery({
    queryKey: keys.usage(id),
    queryFn: () => api<Usage>(`/api/tasks/${id}/claude/usage`),
    refetchInterval: live ? 5000 : false,
  });
}

export function useGolden() {
  return useQuery({ queryKey: ["golden"], queryFn: () => api<GoldenStatus>("/api/golden") });
}

/** Published Claude Code versions: fetched once a session (the server caches them too). */
export function useReleases() {
  return useQuery({
    queryKey: ["releases"],
    queryFn: () => api<Releases>("/api/claude/versions"),
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  });
}

/** Whether a VM can be launched on `path` (nothing asked for an empty path). */
export function useRepoCheck(path: string) {
  return useQuery({
    queryKey: ["repo-check", path],
    queryFn: () => api<RepoCheck>(`/api/repos/check?path=${encodeURIComponent(path)}`),
    enabled: path.trim() !== "",
    staleTime: 5000,
    // An answer, good or bad, is the answer: asking again would only delay a clear "no".
    retry: false,
  });
}

const ENDED = new Set(["done", "no_changes", "failed", "stopped"]);
function byLiveThenAge(a: TaskDto, b: TaskDto): number {
  const ended = (t: TaskDto) => (ENDED.has(t.status.state) ? 1 : 0);
  return ended(a) - ended(b) || a.created_at - b.created_at;
}
