// Files dropped on a terminal are copied into the VM, then their paths typed, as the Mac's
// Terminal does.
import { shellQuote } from "./paths";

type Say = (text: string, tone?: "ok" | "err") => void;

/** Copies the files one by one; resolves to the paths in the VM of those that made it. */
export async function uploadFiles(taskId: string, files: File[], say: Say): Promise<string[]> {
  const paths: string[] = [];
  for (const f of files) {
    say(`Copying ${f.name} into the VM…`);
    try {
      const res = await fetch(`/api/tasks/${taskId}/upload`, {
        method: "POST",
        body: f,
        headers: { "x-file-name": encodeURIComponent(f.name) },
      });
      const data = (await res.json().catch(() => ({}))) as { path?: string; error?: string };
      if (res.ok && data.path) paths.push(data.path);
      else say(data.error ?? `Could not copy ${f.name}`, "err");
    } catch {
      say(`Could not copy ${f.name}`, "err");
    }
  }
  return paths;
}

/** What to type at the prompt for these paths. */
export const typed = (paths: string[]) => `${paths.map(shellQuote).join(" ")} `;
