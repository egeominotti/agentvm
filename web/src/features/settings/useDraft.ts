// The settings being edited: every change is kept at once and saved a moment after the last one,
// so a slider dragged across ten values makes one request, not ten.
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import type { Settings } from "../../api/generated/Settings";
import type { SettingsView } from "../../api/generated/SettingsView";
import { keys } from "../../api/queries";

export type SaveState = { kind: "idle" | "saving" | "saved" } | { kind: "error"; message: string };
export type SetSetting = <K extends keyof Settings>(key: K, value: Settings[K]) => void;

const DELAY_MS = 450;

export function useDraft(initial: Settings | undefined) {
  const [draft, setDraft] = useState<Settings | undefined>(initial);
  const [save, setSave] = useState<SaveState>({ kind: "idle" });
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const latest = useRef(draft);
  const qc = useQueryClient();

  // The first answer from the server becomes the draft; later ones never overwrite your edits.
  useEffect(() => {
    if (initial && !latest.current) {
      latest.current = initial;
      setDraft(initial);
    }
  }, [initial]);
  const send = async () => {
    timer.current = undefined;
    try {
      const saved = await api<SettingsView>("/api/settings", "PUT", latest.current);
      qc.setQueryData(keys.settings, saved);
      qc.invalidateQueries({ queryKey: keys.status });
      setSave({ kind: "saved" });
    } catch (e) {
      setSave({ kind: "error", message: (e as Error).message });
    }
  };
  // Leaving the page with a change still waiting sends it now: it is never dropped.
  const sendRef = useRef(send);
  sendRef.current = send;
  useEffect(
    () => () => {
      if (timer.current === undefined) return;
      clearTimeout(timer.current);
      sendRef.current();
    },
    [],
  );

  const set: SetSetting = (key, value) => {
    if (!latest.current) return;
    const next = { ...latest.current, [key]: value };
    latest.current = next;
    setDraft(next);
    setSave({ kind: "saving" });
    clearTimeout(timer.current);
    timer.current = setTimeout(send, DELAY_MS);
  };
  return { draft, set, save };
}
