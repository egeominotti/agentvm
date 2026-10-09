// The Tailscale auth key, saved in the Keychain: whether one is there (never the key), saving it,
// removing it.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";

const KEY = ["tailscale-key"] as const;

export function useTailscaleKey() {
  const qc = useQueryClient();
  const saved = useQuery({ queryKey: KEY, queryFn: () => api<{ key_saved: boolean }>("/api/settings/tailscale") });
  const refresh = () => qc.invalidateQueries({ queryKey: KEY });
  const save = useMutation({
    mutationFn: (key: string) => api("/api/settings/tailscale", "PUT", { key: key.trim() }),
    onSuccess: refresh,
  });
  const remove = useMutation({ mutationFn: () => api("/api/settings/tailscale", "DELETE"), onSuccess: refresh });
  return { saved: saved.data?.key_saved ?? false, loaded: saved.isSuccess, save, remove };
}

/** Where an auth key is made: reusable and ephemeral, so VMs that end leave no machine behind. */
export const NEW_KEY_URL = "https://login.tailscale.com/admin/settings/keys";
