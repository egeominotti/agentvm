// A machine as the server describes it, for tests: a running interactive VM unless told otherwise.
import type { TaskDto } from "../api/generated/TaskDto";

export function task(over: Partial<TaskDto> = {}): TaskDto {
  return {
    id: "01a11df3-e983-7c77-a556-74bbf38e794b",
    repo: "/Users/me/code/shop",
    prompt: "",
    base_sha: "abc123",
    branch: "main",
    interactive: true,
    activity: null,
    model: "sonnet",
    claude_version: null,
    cpus: 4,
    memory_mb: 8192,
    label: null,
    auto_snapshot_min: null,
    ports: [],
    tailscale: false,
    tailnet: null,
    boot_log: [],
    ready: true,
    usage: null,
    metrics: null,
    cpu_history: [],
    mem_history: [],
    metrics_age_s: null,
    memory_limit_mb: null,
    status: { state: "running" },
    created_at: 1_791_500_000,
    finished_at: null,
    ...over,
  };
}
