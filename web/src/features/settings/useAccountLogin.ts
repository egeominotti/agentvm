import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import type { AccountKind } from "../../api/generated/AccountKind";
import type { LoginStart } from "../../api/generated/LoginStart";
import type { LoginStatus } from "../../api/generated/LoginStatus";
import { loginPhase } from "./account-login";

export function useAccountLogin(account: AccountKind, onConnected: () => Promise<void>) {
  const [login, setLogin] = useState<{ start: LoginStart; expiresAt: number } | null>(null);
  const [result, setResult] = useState<LoginStatus | null>(null);
  const [expired, setExpired] = useState(false);
  const [pollError, setPollError] = useState<string | null>(null);
  const connected = useRef(onConnected);
  connected.current = onConnected;
  const path = `/api/settings/accounts/${account}`;
  const start = useMutation({
    mutationFn: () => api<LoginStart>(`${path}/login`, "POST"),
    onSuccess: (value) => {
      setResult(null);
      setExpired(false);
      setPollError(null);
      setLogin({ start: value, expiresAt: Date.now() + value.expires_in * 1000 });
    },
  });
  useEffect(() => {
    if (!login) return;
    let gone = false;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const expire = () => {
      if (!gone && !stopped) {
        stopped = true;
        setExpired(true);
      }
    };
    const deadline = setTimeout(expire, Math.max(0, login.expiresAt - Date.now()));
    const schedule = () => {
      if (!gone && !stopped) timer = setTimeout(poll, Math.min(2000, Math.max(0, login.expiresAt - Date.now())));
    };
    const poll = async () => {
      if (gone || stopped) return;
      if (loginPhase("pending", login.expiresAt, Date.now()) === "expired") {
        expire();
        return;
      }
      try {
        const value = await api<LoginStatus>(`${path}/login/${encodeURIComponent(login.start.id)}`);
        if (gone || stopped) return;
        setResult(value);
        setPollError(null);
        if (value.status === "pending") schedule();
        else {
          stopped = true;
          clearTimeout(deadline);
          if (value.status === "connected") await connected.current();
        }
      } catch (error) {
        if (!gone && !stopped) {
          setPollError((error as Error).message);
          schedule();
        }
      }
    };
    poll();
    return () => {
      gone = true;
      clearTimeout(timer);
      clearTimeout(deadline);
    };
  }, [account, path, login]);
  return {
    start,
    reset: () => {
      setLogin(null);
      setResult(null);
      setExpired(false);
      setPollError(null);
      start.reset();
    },
    login: login?.start,
    result,
    expired,
    pollError,
    pending: !!login && !expired && (!result || result.status === "pending"),
  };
}
